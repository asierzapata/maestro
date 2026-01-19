use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Represents a workspace entry with its metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceEntry {
    /// Full path to the workspace directory
    pub path: PathBuf,
    /// Display name of the workspace (typically the folder name)
    pub name: String,
    /// Timestamp of when this workspace was last opened
    pub last_opened: DateTime<Utc>,
}

impl WorkspaceEntry {
    /// Create a new workspace entry
    pub fn new(path: PathBuf, name: String) -> Self {
        Self {
            path,
            name,
            last_opened: Utc::now(),
        }
    }

    /// Update the last_opened timestamp to now
    pub fn touch(&mut self) {
        self.last_opened = Utc::now();
    }
}

/// Application settings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Settings {
    /// List of recently opened workspaces
    pub recent_workspaces: Vec<WorkspaceEntry>,
    /// Maximum number of recent workspaces to keep
    pub max_recent_workspaces: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            recent_workspaces: Vec::new(),
            max_recent_workspaces: 10,
        }
    }
}

impl Settings {
    /// Create new settings with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a workspace to the recent list, updating if it already exists
    pub fn add_workspace(&mut self, path: PathBuf, name: String) {
        // Check if workspace already exists
        if let Some(existing) = self.recent_workspaces.iter_mut().find(|w| w.path == path) {
            existing.touch();
        } else {
            // Add new workspace
            self.recent_workspaces.push(WorkspaceEntry::new(path, name));
        }

        // Sort by last_opened (most recent first)
        self.recent_workspaces
            .sort_by(|a, b| b.last_opened.cmp(&a.last_opened));

        // Trim to max size
        if self.recent_workspaces.len() > self.max_recent_workspaces {
            self.recent_workspaces.truncate(self.max_recent_workspaces);
        }
    }

    /// Remove a workspace from the recent list
    pub fn remove_workspace(&mut self, path: &PathBuf) {
        self.recent_workspaces.retain(|w| &w.path != path);
    }

    /// Update the last_opened timestamp for a workspace
    pub fn update_last_opened(&mut self, path: &PathBuf) {
        if let Some(workspace) = self.recent_workspaces.iter_mut().find(|w| &w.path == path) {
            workspace.touch();
            // Re-sort after updating timestamp
            self.recent_workspaces
                .sort_by(|a, b| b.last_opened.cmp(&a.last_opened));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workspace_entry_creation() {
        let path = PathBuf::from("/test/path");
        let name = "test".to_string();
        let entry = WorkspaceEntry::new(path.clone(), name.clone());

        assert_eq!(entry.path, path);
        assert_eq!(entry.name, name);
    }

    #[test]
    fn test_workspace_entry_touch() {
        let mut entry = WorkspaceEntry::new(PathBuf::from("/test"), "test".to_string());
        let original_time = entry.last_opened;

        std::thread::sleep(std::time::Duration::from_millis(10));
        entry.touch();

        assert!(entry.last_opened > original_time);
    }

    #[test]
    fn test_settings_default() {
        let settings = Settings::default();
        assert_eq!(settings.recent_workspaces.len(), 0);
        assert_eq!(settings.max_recent_workspaces, 10);
    }

    #[test]
    fn test_add_workspace() {
        let mut settings = Settings::new();
        settings.add_workspace(PathBuf::from("/test/path1"), "workspace1".to_string());

        assert_eq!(settings.recent_workspaces.len(), 1);
        assert_eq!(settings.recent_workspaces[0].name, "workspace1");
    }

    #[test]
    fn test_add_duplicate_workspace_updates_timestamp() {
        let mut settings = Settings::new();
        let path = PathBuf::from("/test/path1");

        settings.add_workspace(path.clone(), "workspace1".to_string());
        let first_timestamp = settings.recent_workspaces[0].last_opened;

        std::thread::sleep(std::time::Duration::from_millis(10));
        settings.add_workspace(path, "workspace1".to_string());

        assert_eq!(settings.recent_workspaces.len(), 1);
        assert!(settings.recent_workspaces[0].last_opened > first_timestamp);
    }

    #[test]
    fn test_remove_workspace() {
        let mut settings = Settings::new();
        let path = PathBuf::from("/test/path1");

        settings.add_workspace(path.clone(), "workspace1".to_string());
        assert_eq!(settings.recent_workspaces.len(), 1);

        settings.remove_workspace(&path);
        assert_eq!(settings.recent_workspaces.len(), 0);
    }

    #[test]
    fn test_max_workspaces_limit() {
        let mut settings = Settings::new();
        settings.max_recent_workspaces = 3;

        // Add 5 workspaces
        for i in 0..5 {
            settings.add_workspace(
                PathBuf::from(format!("/test/path{}", i)),
                format!("workspace{}", i),
            );
        }

        // Should only keep 3 most recent
        assert_eq!(settings.recent_workspaces.len(), 3);
    }

    #[test]
    fn test_workspaces_sorted_by_last_opened() {
        let mut settings = Settings::new();

        settings.add_workspace(PathBuf::from("/test/path1"), "workspace1".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        settings.add_workspace(PathBuf::from("/test/path2"), "workspace2".to_string());
        std::thread::sleep(std::time::Duration::from_millis(10));
        settings.add_workspace(PathBuf::from("/test/path3"), "workspace3".to_string());

        // Most recent should be first
        assert_eq!(settings.recent_workspaces[0].name, "workspace3");
        assert_eq!(settings.recent_workspaces[1].name, "workspace2");
        assert_eq!(settings.recent_workspaces[2].name, "workspace1");
    }

    #[test]
    fn test_serialization() {
        let mut settings = Settings::new();
        settings.add_workspace(PathBuf::from("/test/path1"), "workspace1".to_string());

        let json = serde_json::to_string(&settings).unwrap();
        let deserialized: Settings = serde_json::from_str(&json).unwrap();

        assert_eq!(settings, deserialized);
    }
}
