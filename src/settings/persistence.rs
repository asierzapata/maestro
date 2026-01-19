use super::config::Settings;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

/// Get the OS-specific path for storing application settings
pub fn get_settings_path() -> Result<PathBuf> {
    let config_dir = if cfg!(target_os = "macos") {
        // macOS: ~/Library/Application Support/Maestro
        dirs::home_dir()
            .context("Could not determine home directory")?
            .join("Library")
            .join("Application Support")
            .join("Maestro")
    } else if cfg!(target_os = "linux") {
        // Linux: ~/.config/maestro
        dirs::config_dir()
            .context("Could not determine config directory")?
            .join("maestro")
    } else if cfg!(target_os = "windows") {
        // Windows: %APPDATA%\Maestro
        dirs::config_dir()
            .context("Could not determine config directory")?
            .join("Maestro")
    } else {
        anyhow::bail!("Unsupported operating system");
    };

    // Ensure the directory exists
    fs::create_dir_all(&config_dir).context("Failed to create settings directory")?;

    Ok(config_dir.join("settings.json"))
}

/// Load settings from the settings file
/// Returns default settings if the file doesn't exist
pub fn load_settings() -> Result<Settings> {
    let settings_path = get_settings_path()?;

    if !settings_path.exists() {
        // Return default settings if file doesn't exist
        return Ok(Settings::default());
    }

    let contents = fs::read_to_string(&settings_path).context("Failed to read settings file")?;

    let settings: Settings =
        serde_json::from_str(&contents).context("Failed to parse settings JSON")?;

    Ok(settings)
}

/// Save settings to the settings file
pub fn save_settings(settings: &Settings) -> Result<()> {
    let settings_path = get_settings_path()?;

    let json = serde_json::to_string_pretty(settings).context("Failed to serialize settings")?;

    fs::write(&settings_path, json).context("Failed to write settings file")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_settings_path() {
        let path = get_settings_path().unwrap();
        assert!(path.ends_with("settings.json"));

        // Verify the directory was created
        assert!(path.parent().unwrap().exists());
    }

    #[test]
    fn test_load_settings_with_nonexistent_file() {
        // This should return default settings without error
        let settings = load_settings().unwrap();
        assert_eq!(settings.max_recent_workspaces, 10);
    }

    #[test]
    #[ignore] // Ignore by default to avoid interfering with other tests
    fn test_save_and_load_settings() {
        // Clean slate
        let _ = fs::remove_file(get_settings_path().unwrap());

        let mut settings = Settings::new();
        settings.add_workspace(PathBuf::from("/test/path1"), "test_workspace".to_string());
        settings.max_recent_workspaces = 5;

        // Save settings
        save_settings(&settings).unwrap();

        // Load settings back
        let loaded_settings = load_settings().unwrap();

        assert_eq!(loaded_settings.recent_workspaces.len(), 1);
        assert_eq!(loaded_settings.recent_workspaces[0].name, "test_workspace");
        assert_eq!(loaded_settings.max_recent_workspaces, 5);

        // Clean up
        let _ = fs::remove_file(get_settings_path().unwrap());
    }

    #[test]
    #[ignore] // Ignore by default to avoid interfering with other tests
    fn test_invalid_json_returns_error() {
        let settings_path = get_settings_path().unwrap();

        // Write invalid JSON (missing closing brace and malformed)
        fs::write(&settings_path, "{ \"recent_workspaces\": ").unwrap();

        // Should return error when loading
        let result = load_settings();
        assert!(result.is_err());

        // Clean up - restore to valid state
        let settings = Settings::default();
        save_settings(&settings).unwrap();
    }
}
