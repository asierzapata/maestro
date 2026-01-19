pub mod config;
pub mod persistence;

pub use config::{Settings, WorkspaceEntry};
pub use persistence::{load_settings, save_settings};
