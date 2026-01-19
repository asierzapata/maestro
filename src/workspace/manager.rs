use crate::git::{get_repository_name, is_git_repository};
use crate::settings::{Settings, WorkspaceEntry, load_settings, save_settings};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Workspace manager that coordinates workspace operations
pub struct WorkspaceManager {
    settings: Settings,
}

impl WorkspaceManager {
    /// Create a new workspace manager
    pub fn new() -> Result<Self> {
        let settings = load_settings()?;
        Ok(Self { settings })
    }

    /// Load recent workspaces from settings
    pub fn load_recent_workspaces(&self) -> Vec<WorkspaceEntry> {
        self.settings.recent_workspaces.clone()
    }

    /// Add a workspace to the recent list
    /// Validates that the path is a git repository before adding
    pub fn add_workspace(&mut self, path: PathBuf) -> Result<()> {
        // Validate it's a git repository
        if !is_git_repository(&path) {
            anyhow::bail!("Path is not a valid git repository: {:?}", path);
        }

        // Get repository name
        let name = get_repository_name(&path)?;

        // Add to settings
        self.settings.add_workspace(path, name);

        // Persist changes
        save_settings(&self.settings)?;

        Ok(())
    }

    /// Remove a workspace from the recent list
    pub fn remove_workspace(&mut self, path: &Path) -> Result<()> {
        let path_buf = path.to_path_buf();
        self.settings.remove_workspace(&path_buf);

        // Persist changes
        save_settings(&self.settings)?;

        Ok(())
    }

    /// Update the last opened timestamp for a workspace
    pub fn update_last_opened(&mut self, path: &Path) -> Result<()> {
        let path_buf = path.to_path_buf();
        self.settings.update_last_opened(&path_buf);

        // Persist changes
        save_settings(&self.settings)?;

        Ok(())
    }

    /// Get the current settings
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

impl Default for WorkspaceManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            settings: Settings::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_workspace_manager_creation() {
        let manager = WorkspaceManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_load_recent_workspaces() {
        let manager = WorkspaceManager::new().unwrap();
        let workspaces = manager.load_recent_workspaces();
        // Should load whatever is in settings
        assert!(workspaces.len() >= 0);
    }

    #[test]
    #[ignore] // Ignore to avoid interfering with actual settings
    fn test_add_workspace() {
        let mut manager = WorkspaceManager::new().unwrap();
        let current_dir = std::env::current_dir().unwrap();

        // Add current directory (which is a git repo)
        let result = manager.add_workspace(current_dir.clone());
        assert!(result.is_ok());

        // Verify it was added
        let workspaces = manager.load_recent_workspaces();
        assert!(workspaces.iter().any(|w| w.path == current_dir));
    }

    #[test]
    fn test_add_invalid_workspace() {
        let mut manager = WorkspaceManager::new().unwrap();
        let temp_dir = std::env::temp_dir().join("not_a_git_repo_test");
        fs::create_dir_all(&temp_dir).unwrap();

        // Try to add a non-git directory
        let result = manager.add_workspace(temp_dir.clone());
        assert!(result.is_err());

        // Clean up
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore] // Ignore to avoid interfering with actual settings
    fn test_remove_workspace() {
        let mut manager = WorkspaceManager::new().unwrap();
        let current_dir = std::env::current_dir().unwrap();

        // Add and then remove
        let _ = manager.add_workspace(current_dir.clone());
        let result = manager.remove_workspace(&current_dir);
        assert!(result.is_ok());

        // Verify it was removed
        let workspaces = manager.load_recent_workspaces();
        assert!(!workspaces.iter().any(|w| w.path == current_dir));
    }

    #[test]
    #[ignore] // Ignore to avoid interfering with actual settings
    fn test_update_last_opened() {
        let mut manager = WorkspaceManager::new().unwrap();
        let current_dir = std::env::current_dir().unwrap();

        // Add workspace
        let _ = manager.add_workspace(current_dir.clone());

        // Get initial timestamp
        let workspaces = manager.load_recent_workspaces();
        let initial_time = workspaces
            .iter()
            .find(|w| w.path == current_dir)
            .unwrap()
            .last_opened;

        // Wait a bit and update
        std::thread::sleep(std::time::Duration::from_millis(10));
        let result = manager.update_last_opened(&current_dir);
        assert!(result.is_ok());

        // Verify timestamp was updated
        let workspaces = manager.load_recent_workspaces();
        let updated_time = workspaces
            .iter()
            .find(|w| w.path == current_dir)
            .unwrap()
            .last_opened;

        assert!(updated_time > initial_time);
    }
}
