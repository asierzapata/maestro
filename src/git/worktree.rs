use anyhow::{Context, Result};
use git2::Repository;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a git worktree
#[derive(Debug, Clone, PartialEq)]
pub struct Worktree {
    /// Path to the worktree directory
    pub path: PathBuf,
    /// Current branch name (or "HEAD" if detached)
    pub branch: String,
    /// Whether the worktree is in detached HEAD state
    pub is_detached: bool,
    /// Whether the worktree is locked
    pub is_locked: bool,
}

impl Worktree {
    /// Create a new Worktree instance
    pub fn new(path: PathBuf, branch: String, is_detached: bool, is_locked: bool) -> Self {
        Self {
            path,
            branch,
            is_detached,
            is_locked,
        }
    }
}

/// List all worktrees for a given repository, including the root worktree
///
/// This function uses git2 to get the root worktree information and
/// falls back to git command execution to list additional worktrees.
///
/// # Arguments
///
/// * `repo_path` - Path to the git repository (can be any worktree)
///
/// # Returns
///
/// A Vec of Worktree structs, with the root worktree typically first in the list
pub fn list_worktrees(repo_path: &Path) -> Result<Vec<Worktree>> {
    let repo = Repository::discover(repo_path).context("Failed to discover git repository")?;

    let mut worktrees = Vec::new();

    // Get root worktree information using git2
    let root_worktree = get_root_worktree(&repo)?;
    worktrees.push(root_worktree);

    // Get additional worktrees using git command
    let additional_worktrees = parse_worktree_list(&repo)?;
    worktrees.extend(additional_worktrees);

    Ok(worktrees)
}

/// Get the root worktree information using git2
fn get_root_worktree(repo: &Repository) -> Result<Worktree> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory (bare repository?)")?;

    let head = repo.head().context("Failed to get HEAD reference")?;

    let (branch, is_detached) = if head.is_branch() {
        // Get the branch name
        let branch_name = head.shorthand().unwrap_or("unknown").to_string();
        (branch_name, false)
    } else {
        // Detached HEAD state
        ("HEAD".to_string(), true)
    };

    Ok(Worktree {
        path: workdir.to_path_buf(),
        branch,
        is_detached,
        is_locked: false, // Root worktree is never locked
    })
}

/// Parse the output of `git worktree list --porcelain` to get additional worktrees
fn parse_worktree_list(repo: &Repository) -> Result<Vec<Worktree>> {
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;

    let output = Command::new("git")
        .args(&["worktree", "list", "--porcelain"])
        .current_dir(workdir)
        .output()
        .context("Failed to execute git worktree list")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree list failed: {}", stderr);
    }

    let stdout =
        String::from_utf8(output.stdout).context("git worktree list output is not valid UTF-8")?;

    let mut worktrees = Vec::new();
    let mut current_worktree: Option<ParsedWorktree> = None;
    let root_path = workdir
        .canonicalize()
        .unwrap_or_else(|_| workdir.to_path_buf());

    for line in stdout.lines() {
        if line.is_empty() {
            // Empty line indicates end of a worktree entry
            if let Some(wt) = current_worktree.take() {
                // Skip the root worktree as we already added it
                if wt.path != root_path {
                    worktrees.push(wt.into_worktree());
                }
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            current_worktree = Some(ParsedWorktree {
                path: PathBuf::from(path),
                branch: String::new(),
                is_detached: false,
                is_locked: false,
            });
        } else if let Some(branch) = line.strip_prefix("branch ") {
            if let Some(ref mut wt) = current_worktree {
                // Strip refs/heads/ prefix if present
                wt.branch = branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_string();
            }
        } else if line == "detached" {
            if let Some(ref mut wt) = current_worktree {
                wt.is_detached = true;
                if wt.branch.is_empty() {
                    wt.branch = "HEAD".to_string();
                }
            }
        } else if line.starts_with("locked") {
            if let Some(ref mut wt) = current_worktree {
                wt.is_locked = true;
            }
        }
    }

    // Handle last entry if file doesn't end with empty line
    if let Some(wt) = current_worktree {
        if wt.path != root_path {
            worktrees.push(wt.into_worktree());
        }
    }

    Ok(worktrees)
}

/// Create a new worktree with a new branch
///
/// Creates a new worktree in a sibling directory to the root repository.
/// The worktree will be created with a new branch based on the current HEAD.
///
/// # Arguments
///
/// * `repo_path` - Path to the git repository (can be any worktree)
/// * `branch_name` - Name for the new branch (will be validated)
/// * `worktree_name` - Optional custom name for the worktree directory.
///                     If None, uses the branch name (replacing slashes with dashes)
///
/// # Returns
///
/// The newly created Worktree struct
///
/// # Errors
///
/// Returns an error if:
/// - The branch name is invalid
/// - The branch already exists
/// - The worktree directory already exists
/// - Git command fails
pub fn create_worktree(
    repo_path: &Path,
    branch_name: &str,
    worktree_name: Option<&str>,
) -> Result<Worktree> {
    // Validate branch name
    validate_branch_name(branch_name)?;

    // Discover the repository
    let repo = Repository::discover(repo_path).context("Failed to discover git repository")?;

    // Get the parent directory where worktrees should be created
    let parent_dir = get_worktree_parent_dir(repo_path)?;

    // Determine the worktree directory name
    let dir_name = if let Some(name) = worktree_name {
        name.to_string()
    } else {
        // Replace slashes with dashes for directory name
        branch_name.replace('/', "-")
    };

    let worktree_path = parent_dir.join(&dir_name);

    // Check if the worktree directory already exists
    if worktree_path.exists() {
        anyhow::bail!(
            "Worktree directory already exists: {}",
            worktree_path.display()
        );
    }

    // Get the root workdir for running git commands
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;

    // Execute git worktree add command
    let output = Command::new("git")
        .args(&[
            "worktree",
            "add",
            "-b",
            branch_name,
            worktree_path.to_str().context("Invalid worktree path")?,
        ])
        .current_dir(workdir)
        .output()
        .context("Failed to execute git worktree add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git worktree add failed: {}", stderr);
    }

    // Return the created worktree
    Ok(Worktree {
        path: worktree_path,
        branch: branch_name.to_string(),
        is_detached: false,
        is_locked: false,
    })
}

/// Validate a branch name according to git ref naming rules
///
/// # Arguments
///
/// * `name` - The branch name to validate
///
/// # Returns
///
/// Ok(()) if valid, error with descriptive message otherwise
///
/// # Validation Rules
///
/// - Cannot be empty
/// - Cannot contain spaces
/// - Cannot start or end with a slash
/// - Cannot contain consecutive slashes
/// - Cannot contain special characters like: ~, ^, :, ?, *, [, \, @{
/// - Cannot be a reserved name like "HEAD"
pub fn validate_branch_name(name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("Branch name cannot be empty");
    }

    if name == "HEAD" {
        anyhow::bail!("'HEAD' is a reserved name");
    }

    if name.contains(' ') {
        anyhow::bail!("Branch name cannot contain spaces");
    }

    if name.starts_with('/') || name.ends_with('/') {
        anyhow::bail!("Branch name cannot start or end with a slash");
    }

    if name.contains("//") {
        anyhow::bail!("Branch name cannot contain consecutive slashes");
    }

    if name.starts_with('.') || name.ends_with('.') {
        anyhow::bail!("Branch name cannot start or end with a period");
    }

    if name.contains("..") {
        anyhow::bail!("Branch name cannot contain '..'");
    }

    // Check for invalid characters
    let invalid_chars = ['~', '^', ':', '?', '*', '[', '\\', '@'];
    for ch in invalid_chars.iter() {
        if name.contains(*ch) {
            anyhow::bail!("Branch name cannot contain '{}'", ch);
        }
    }

    // Check for @{ sequence
    if name.contains("@{") {
        anyhow::bail!("Branch name cannot contain '@{{'");
    }

    Ok(())
}

/// Get the parent directory where worktrees should be created
///
/// Returns the parent directory of the root repository, where sibling
/// worktrees will be created.
///
/// # Arguments
///
/// * `repo_path` - Path to the git repository
///
/// # Returns
///
/// PathBuf to the parent directory
fn get_worktree_parent_dir(repo_path: &Path) -> Result<PathBuf> {
    let repo = Repository::discover(repo_path).context("Failed to discover git repository")?;
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;

    let parent = workdir
        .parent()
        .context("Repository is at filesystem root")?;

    Ok(parent.to_path_buf())
}

/// Temporary struct for parsing worktree list output
struct ParsedWorktree {
    path: PathBuf,
    branch: String,
    is_detached: bool,
    is_locked: bool,
}

impl ParsedWorktree {
    fn into_worktree(self) -> Worktree {
        Worktree {
            path: self.path,
            branch: self.branch,
            is_detached: self.is_detached,
            is_locked: self.is_locked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command as StdCommand;

    #[test]
    fn test_list_worktrees_current_repo() {
        // Test with the current repository (maestro itself)
        let current_dir = std::env::current_dir().unwrap();
        let worktrees = list_worktrees(&current_dir).unwrap();

        // Should have at least the root worktree
        assert!(!worktrees.is_empty());

        // First worktree should be the root
        let root = &worktrees[0];
        assert_eq!(root.path.file_name().unwrap(), "maestro");
        assert!(!root.is_locked);
    }

    #[test]
    fn test_worktree_struct() {
        let wt = Worktree::new(
            PathBuf::from("/path/to/worktree"),
            "main".to_string(),
            false,
            false,
        );

        assert_eq!(wt.path, PathBuf::from("/path/to/worktree"));
        assert_eq!(wt.branch, "main");
        assert!(!wt.is_detached);
        assert!(!wt.is_locked);
    }

    #[test]
    #[ignore] // This test creates temporary git repositories and worktrees
    fn test_list_worktrees_with_multiple() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("maestro_worktree_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Initialize a git repository
        StdCommand::new("git")
            .args(&["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to init git repo");

        // Create an initial commit
        fs::write(temp_dir.join("README.md"), "# Test").unwrap();
        StdCommand::new("git")
            .args(&["add", "."])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to add files");
        StdCommand::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to commit");

        // Create a worktree
        let worktree_dir = temp_dir.join("feature-test");
        StdCommand::new("git")
            .args(&[
                "worktree",
                "add",
                "-b",
                "feature",
                worktree_dir.to_str().unwrap(),
            ])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to create worktree");

        // List worktrees
        let worktrees = list_worktrees(&temp_dir).unwrap();

        // Should have root + the new worktree
        assert_eq!(worktrees.len(), 2);

        // Verify root worktree
        assert!(worktrees[0].path.ends_with("maestro_worktree_test"));

        // Verify additional worktree
        assert!(worktrees[1].path.ends_with("feature-test"));
        assert_eq!(worktrees[1].branch, "feature");

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_validate_branch_name_valid() {
        assert!(validate_branch_name("feature").is_ok());
        assert!(validate_branch_name("feature-1").is_ok());
        assert!(validate_branch_name("feature/test").is_ok());
        assert!(validate_branch_name("feature_test").is_ok());
        assert!(validate_branch_name("feature-1.0").is_ok());
    }

    #[test]
    fn test_validate_branch_name_empty() {
        let result = validate_branch_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_branch_name_reserved() {
        let result = validate_branch_name("HEAD");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("reserved"));
    }

    #[test]
    fn test_validate_branch_name_with_spaces() {
        let result = validate_branch_name("feature test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("spaces"));
    }

    #[test]
    fn test_validate_branch_name_with_slashes() {
        let result = validate_branch_name("/feature");
        assert!(result.is_err());
        let result = validate_branch_name("feature/");
        assert!(result.is_err());
        let result = validate_branch_name("feature//test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_branch_name_with_dots() {
        let result = validate_branch_name(".feature");
        assert!(result.is_err());
        let result = validate_branch_name("feature.");
        assert!(result.is_err());
        let result = validate_branch_name("feature..test");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_branch_name_with_invalid_chars() {
        assert!(validate_branch_name("feature~test").is_err());
        assert!(validate_branch_name("feature^test").is_err());
        assert!(validate_branch_name("feature:test").is_err());
        assert!(validate_branch_name("feature?test").is_err());
        assert!(validate_branch_name("feature*test").is_err());
        assert!(validate_branch_name("feature[test").is_err());
        assert!(validate_branch_name("feature\\test").is_err());
        assert!(validate_branch_name("feature@{test").is_err());
    }

    #[test]
    #[ignore] // This test creates temporary git repositories and worktrees
    fn test_create_worktree() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("maestro_create_worktree_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Initialize a git repository
        StdCommand::new("git")
            .args(&["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to init git repo");

        // Create an initial commit
        fs::write(temp_dir.join("README.md"), "# Test").unwrap();
        StdCommand::new("git")
            .args(&["add", "."])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to add files");
        StdCommand::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to commit");

        // Create a worktree using our function
        let result = create_worktree(&temp_dir, "feature/new-feature", None);
        assert!(result.is_ok());

        let worktree = result.unwrap();
        assert_eq!(worktree.branch, "feature/new-feature");
        assert!(!worktree.is_detached);
        assert!(!worktree.is_locked);
        assert!(worktree.path.exists());
        assert!(worktree.path.ends_with("feature-new-feature"));

        // Verify the worktree was created
        let worktrees = list_worktrees(&temp_dir).unwrap();
        assert_eq!(worktrees.len(), 2);

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore] // This test creates temporary git repositories and worktrees
    fn test_create_worktree_with_custom_name() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("maestro_create_worktree_custom_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Initialize a git repository
        StdCommand::new("git")
            .args(&["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to init git repo");

        // Create an initial commit
        fs::write(temp_dir.join("README.md"), "# Test").unwrap();
        StdCommand::new("git")
            .args(&["add", "."])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to add files");
        StdCommand::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to commit");

        // Create a worktree with custom name
        let result = create_worktree(&temp_dir, "feature-x", Some("custom-name"));
        assert!(result.is_ok());

        let worktree = result.unwrap();
        assert_eq!(worktree.branch, "feature-x");
        assert!(worktree.path.ends_with("custom-name"));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore] // This test creates temporary git repositories
    fn test_create_worktree_invalid_branch_name() {
        // Create a temporary directory for testing
        let temp_dir = std::env::temp_dir().join("maestro_create_invalid_test");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // Initialize a git repository
        StdCommand::new("git")
            .args(&["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to init git repo");

        // Create an initial commit
        fs::write(temp_dir.join("README.md"), "# Test").unwrap();
        StdCommand::new("git")
            .args(&["add", "."])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to add files");
        StdCommand::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to commit");

        // Try to create worktree with invalid name
        let result = create_worktree(&temp_dir, "invalid name", None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("spaces"));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
