use anyhow::{Context, Result};
use git2::Repository;
use std::path::Path;

/// Check if a directory contains a valid git repository
pub fn is_git_repository(path: &Path) -> bool {
    Repository::open(path).is_ok()
}

/// Get the repository name from a path
/// Returns the folder name as the repository name
pub fn get_repository_name(path: &Path) -> Result<String> {
    let name = path
        .file_name()
        .context("Invalid path: no file name")?
        .to_str()
        .context("Path contains invalid UTF-8")?
        .to_string();

    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    #[test]
    fn test_is_git_repository_with_valid_repo() {
        // Test with the current repository (maestro itself)
        let current_dir = std::env::current_dir().unwrap();
        assert!(is_git_repository(&current_dir));
    }

    #[test]
    fn test_is_git_repository_with_invalid_repo() {
        // Create a temporary directory that's not a git repo
        let temp_dir = std::env::temp_dir().join("not_a_git_repo");
        fs::create_dir_all(&temp_dir).unwrap();

        assert!(!is_git_repository(&temp_dir));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_is_git_repository_with_nonexistent_path() {
        let nonexistent = Path::new("/nonexistent/path/that/does/not/exist");
        assert!(!is_git_repository(nonexistent));
    }

    #[test]
    fn test_get_repository_name() {
        let path = Path::new("/some/path/my-repo");
        let name = get_repository_name(path).unwrap();
        assert_eq!(name, "my-repo");
    }

    #[test]
    fn test_get_repository_name_current_dir() {
        let current_dir = std::env::current_dir().unwrap();
        let name = get_repository_name(&current_dir).unwrap();
        assert_eq!(name, "maestro");
    }

    #[test]
    #[ignore] // This test creates a temporary git repository
    fn test_is_git_repository_with_temp_repo() {
        // Create a temporary directory and initialize it as a git repo
        let temp_dir = std::env::temp_dir().join("temp_git_test_repo");
        let _ = fs::remove_dir_all(&temp_dir); // Clean up if exists
        fs::create_dir_all(&temp_dir).unwrap();

        // Initialize git repository
        Command::new("git")
            .args(&["init"])
            .current_dir(&temp_dir)
            .output()
            .expect("Failed to init git repo");

        // Test that it's recognized as a git repository
        assert!(is_git_repository(&temp_dir));

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
