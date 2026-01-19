use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::grid::Dimensions;
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::vte::ansi::Processor;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use crossbeam_channel::Receiver;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::terminal::event_loop::{start_event_loop, EventLoopHandle};
use crate::terminal::events::TerminalEvent;
use crate::terminal::handle::TerminalHandle;
use crate::terminal::PtyProcess;

/// Session state for persistence across app restarts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    /// Associated worktree path
    pub worktree_path: PathBuf,
    /// Current working directory in the shell
    pub working_directory: String,
    /// Terminal scrollback buffer content
    pub scrollback_lines: Vec<String>,
    /// Timestamp of last save
    pub last_updated: DateTime<Utc>,
}

impl SessionState {
    /// Creates a SessionState from a terminal session
    pub fn from_terminal(session: &TerminalSession) -> Self {
        let scrollback_lines = session.get_scrollback_content();
        let working_directory = session.worktree_path.display().to_string();

        SessionState {
            worktree_path: session.worktree_path.clone(),
            working_directory,
            scrollback_lines,
            last_updated: Utc::now(),
        }
    }

    /// Returns the path to the sessions directory
    fn sessions_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("maestro").join("sessions"))
    }

    /// Generates a hash-based filename for a worktree path
    fn session_filename(worktree_path: &Path) -> String {
        let mut hasher = DefaultHasher::new();
        worktree_path.hash(&mut hasher);
        format!("{:x}.json", hasher.finish())
    }

    /// Saves the session state to disk
    pub fn save(&self) -> Result<()> {
        let sessions_dir = Self::sessions_dir()?;

        // Create sessions directory if it doesn't exist
        fs::create_dir_all(&sessions_dir)
            .context("Failed to create sessions directory")?;

        let filename = Self::session_filename(&self.worktree_path);
        let file_path = sessions_dir.join(&filename);
        let temp_path = sessions_dir.join(format!("{}.tmp", filename));

        // Serialize to JSON
        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize session state")?;

        // Write to temp file first (atomic write)
        let mut temp_file = fs::File::create(&temp_path)
            .context("Failed to create temp file")?;
        temp_file.write_all(json.as_bytes())
            .context("Failed to write to temp file")?;
        temp_file.sync_all()
            .context("Failed to sync temp file")?;

        // Rename temp file to final location
        fs::rename(&temp_path, &file_path)
            .context("Failed to rename temp file to final location")?;

        Ok(())
    }

    /// Loads session state from disk for a given worktree path
    pub fn load(worktree_path: &Path) -> Result<Option<Self>> {
        let sessions_dir = match Self::sessions_dir() {
            Ok(dir) => dir,
            Err(_) => return Ok(None),
        };

        let filename = Self::session_filename(worktree_path);
        let file_path = sessions_dir.join(filename);

        if !file_path.exists() {
            return Ok(None);
        }

        let contents = fs::read_to_string(&file_path)
            .context("Failed to read session file")?;

        let state: SessionState = serde_json::from_str(&contents)
            .context("Failed to deserialize session state")?;

        // Validate that the worktree path matches
        if state.worktree_path != worktree_path {
            return Ok(None);
        }

        Ok(Some(state))
    }

    /// Removes old session files that haven't been updated in the specified duration
    #[allow(dead_code)]
    pub fn cleanup_old_sessions(max_age_days: i64) -> Result<usize> {
        let sessions_dir = Self::sessions_dir()?;

        if !sessions_dir.exists() {
            return Ok(0);
        }

        let cutoff = Utc::now() - chrono::Duration::days(max_age_days);
        let mut removed = 0;

        for entry in fs::read_dir(&sessions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "json") {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(state) = serde_json::from_str::<SessionState>(&contents) {
                        if state.last_updated < cutoff {
                            if fs::remove_file(&path).is_ok() {
                                removed += 1;
                            }
                        }
                    }
                }
            }
        }

        Ok(removed)
    }
}

/// Event proxy for handling terminal events from Alacritty
pub struct EventProxy {
    events: Arc<Mutex<Vec<Event>>>,
}

impl EventProxy {
    pub fn new() -> Self {
        EventProxy {
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn drain_events(&self) -> Vec<Event> {
        let mut events = self.events.lock().unwrap();
        events.drain(..).collect()
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

/// Represents a terminal session for a specific worktree.
/// Wraps a PTY process and integrates with Alacritty's terminal emulator.
pub struct TerminalSession {
    /// Handle for sending commands to the PTY background thread
    terminal_handle: TerminalHandle,
    /// Handle for the event loop (manages thread lifecycle)
    _event_loop_handle: EventLoopHandle,
    /// Receiver for events from the PTY background thread
    event_rx: Receiver<TerminalEvent>,
    /// Alacritty terminal emulator
    term: Term<EventProxy>,
    /// Current grid size (rows, cols)
    grid_size: (u16, u16),
    /// Path to the worktree
    worktree_path: PathBuf,
    #[allow(dead_code)]
    event_proxy: EventProxy,
    /// ANSI parser for processing PTY output
    parser: Processor,
    /// Flag to track if session has activity since last save
    has_activity: bool,
    /// Flag to track if the PTY process is still alive
    is_process_alive: bool,
}

impl TerminalSession {
    /// Creates a new terminal session.
    ///
    /// # Arguments
    /// * `worktree_path` - Path to the worktree for this terminal
    /// * `shell` - Optional shell path (defaults to $SHELL or /bin/sh)
    /// * `rows` - Initial terminal height in rows
    /// * `cols` - Initial terminal width in columns
    pub fn new(
        worktree_path: PathBuf,
        shell: Option<String>,
        rows: u16,
        cols: u16,
    ) -> Result<Self> {
        // Try to load saved session state
        let saved_state = SessionState::load(&worktree_path).ok().flatten();

        // Spawn PTY process
        let mut pty =
            PtyProcess::spawn(&worktree_path, shell).context("Failed to spawn PTY process")?;

        // Resize PTY to specified dimensions
        pty.resize(rows, cols)?;

        // Start the async event loop for this PTY
        let event_loop_handle = start_event_loop(pty);
        let terminal_handle = event_loop_handle.terminal_handle.clone();
        let event_rx = event_loop_handle.event_rx.clone();

        // Create event proxy
        let event_proxy = EventProxy::new();

        // Create Alacritty terminal with specified dimensions
        let term_size = TermSize::new(cols as usize, rows as usize);

        let term_config = TermConfig::default();
        let term = Term::new(term_config, &term_size, event_proxy.clone());

        // Create ANSI parser
        let parser = Processor::new();

        let mut session = TerminalSession {
            terminal_handle,
            _event_loop_handle: event_loop_handle,
            event_rx,
            term,
            grid_size: (rows, cols),
            worktree_path,
            event_proxy,
            parser,
            has_activity: false,
            is_process_alive: true,
        };

        // Restore saved state if available
        if let Some(state) = saved_state {
            session.restore_from_state(&state);
        }

        Ok(session)
    }

    /// Writes input data to the terminal.
    /// This is now non-blocking - input is sent to the background thread.
    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        self.has_activity = true;
        self.terminal_handle
            .send_input(data.to_vec())
            .map_err(|e| anyhow::anyhow!("Failed to send input to PTY: {}", e))
    }

    /// Processes pending events from the PTY background thread.
    /// Returns true if new content was processed, or None if process exited.
    pub fn process_events(&mut self) -> Option<bool> {
        let mut has_new_content = false;

        // Process all available events without blocking
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                TerminalEvent::Output(data) => {
                    // Feed data to parser and terminal
                    self.parser.advance(&mut self.term, &data);
                    has_new_content = true;
                }
                TerminalEvent::ProcessExited(_exit_code) => {
                    // Process has exited
                    self.is_process_alive = false;
                    return None;
                }
                TerminalEvent::Resized { rows, cols } => {
                    // Update our stored grid size
                    self.grid_size = (rows, cols);
                }
            }
        }

        Some(has_new_content)
    }

    /// Returns the event receiver for async event handling.
    /// Used by TerminalView to subscribe to events.
    pub fn event_receiver(&self) -> &Receiver<TerminalEvent> {
        &self.event_rx
    }

    /// Resizes the terminal.
    /// This sends a resize command to the PTY background thread.
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.grid_size = (rows, cols);

        // Send resize command to PTY via handle
        self.terminal_handle
            .resize(rows, cols)
            .map_err(|e| anyhow::anyhow!("Failed to send resize to PTY: {}", e))?;

        // Resize Alacritty terminal
        let term_size = TermSize::new(cols as usize, rows as usize);
        self.term.resize(term_size);

        Ok(())
    }

    /// Gets the visible content of the terminal.
    pub fn get_visible_content(&self) -> Vec<String> {
        let grid = self.term.grid();
        let rows = self.grid_size.0 as usize;
        let cols = self.grid_size.1 as usize;

        // Pre-allocate the lines vector
        let mut lines = Vec::with_capacity(rows);

        for row in 0..rows {
            // Pre-allocate string with exact capacity
            let mut line = String::with_capacity(cols);
            let line_idx = alacritty_terminal::index::Line(row as i32);

            for col in 0..cols {
                let point = alacritty_terminal::index::Point::new(
                    line_idx,
                    alacritty_terminal::index::Column(col),
                );
                let cell = &grid[point];
                line.push(cell.c);
            }
            lines.push(line);
        }

        lines
    }

    /// Gets the current cursor position.
    pub fn get_cursor_position(&self) -> (u16, u16) {
        let point = self.term.renderable_content().cursor.point;
        (point.line.0 as u16, point.column.0 as u16)
    }

    /// Gets batched renderable content for efficient rendering.
    /// This batches adjacent cells with the same style into text runs.
    pub fn get_renderable_content(&self) -> crate::terminal::render::RenderableContent {
        crate::terminal::render::batch_cells(&self.term, self.grid_size.0, self.grid_size.1)
    }

    /// Checks if the terminal session is still alive.
    pub fn is_alive(&self) -> bool {
        self.is_process_alive
    }

    /// Kills the terminal session.
    /// Sends shutdown command to the background thread.
    pub fn kill(&mut self) -> Result<()> {
        self.terminal_handle
            .shutdown()
            .map_err(|e| anyhow::anyhow!("Failed to send shutdown: {}", e))
    }

    /// Gets the worktree path for this session.
    #[allow(dead_code)]
    pub fn worktree_path(&self) -> &PathBuf {
        &self.worktree_path
    }

    /// Gets the scrollback buffer content (visible content plus history)
    pub fn get_scrollback_content(&self) -> Vec<String> {
        let grid = self.term.grid();
        let mut lines = Vec::new();
        let cols = self.grid_size.1 as usize;

        // Get history lines (scrollback) - access via negative line indices
        let history_size = grid.history_size();
        for i in (0..history_size).rev() {
            let mut line = String::new();
            let line_idx = alacritty_terminal::index::Line(-(i as i32) - 1);

            for col in 0..cols {
                let point = alacritty_terminal::index::Point::new(
                    line_idx,
                    alacritty_terminal::index::Column(col),
                );
                // Use direct indexing which is safe for valid history lines
                let cell = &grid[point];
                line.push(cell.c);
            }
            // Trim trailing whitespace
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                lines.push(trimmed.to_string());
            }
        }

        // Add visible lines
        for row in 0..self.grid_size.0 {
            let mut line = String::new();
            for col in 0..self.grid_size.1 {
                let point = alacritty_terminal::index::Point::new(
                    alacritty_terminal::index::Line(row as i32),
                    alacritty_terminal::index::Column(col as usize),
                );
                let cell = &grid[point];
                line.push(cell.c);
            }
            // Trim trailing whitespace
            let trimmed = line.trim_end();
            lines.push(trimmed.to_string());
        }

        lines
    }

    /// Restores session from saved state
    fn restore_from_state(&mut self, state: &SessionState) {
        // If the saved working directory is different from the worktree path,
        // send a cd command to change to the saved directory
        if state.working_directory != self.worktree_path.display().to_string() {
            let cd_cmd = format!("cd '{}'\n", state.working_directory);
            let _ = self.terminal_handle.send_input(cd_cmd.into_bytes());
        }

        // Note: Restoring scrollback content to Alacritty terminal is complex
        // and would require writing the content to the terminal.
        // For now, we just store the state but don't actively restore scrollback.
        // The scrollback will be rebuilt as the user interacts with the terminal.
    }

    /// Saves the current session state
    pub fn save_state(&mut self) -> Result<()> {
        let state = SessionState::from_terminal(self);
        state.save()?;
        self.has_activity = false;
        Ok(())
    }

    /// Checks if the session has activity since last save
    pub fn has_activity(&self) -> bool {
        self.has_activity
    }

    /// Clears the activity flag
    pub fn clear_activity_flag(&mut self) {
        self.has_activity = false;
    }
}

impl Clone for EventProxy {
    fn clone(&self) -> Self {
        EventProxy {
            events: Arc::clone(&self.events),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_session_creation() {
        let temp_dir = std::env::temp_dir();
        let result = TerminalSession::new(temp_dir, Some("/bin/sh".to_string()), 24, 80);
        assert!(result.is_ok());

        let session = result.unwrap();
        assert!(session.is_alive());
    }

    #[test]
    fn test_terminal_session_write_read() {
        let temp_dir = std::env::temp_dir();
        let mut session = TerminalSession::new(temp_dir, Some("/bin/sh".to_string()), 24, 80)
            .expect("Failed to create terminal session");

        // Write a command
        let cmd = b"echo hello\n";
        session.write_input(cmd).expect("Failed to write input");

        // Give shell time to process
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Process events from the async channel
        let result = session.process_events();
        assert!(result.is_some(), "Session should still be alive");
        assert!(result.unwrap(), "Should have received output");
    }

    #[test]
    fn test_terminal_session_resize() {
        let temp_dir = std::env::temp_dir();
        let mut session = TerminalSession::new(temp_dir, Some("/bin/sh".to_string()), 24, 80)
            .expect("Failed to create terminal session");

        let result = session.resize(50, 100);
        assert!(result.is_ok());
        assert_eq!(session.grid_size, (50, 100));
    }

    #[test]
    fn test_terminal_session_get_content() {
        let temp_dir = std::env::temp_dir();
        let session = TerminalSession::new(temp_dir, Some("/bin/sh".to_string()), 24, 80)
            .expect("Failed to create terminal session");

        let content = session.get_visible_content();
        assert_eq!(content.len(), 24);
        assert_eq!(content[0].len(), 80);
    }

    #[test]
    fn test_session_state_save_load() {
        // Create a unique temp directory for this test
        let test_dir = std::env::temp_dir().join(format!("maestro_test_{}", std::process::id()));
        fs::create_dir_all(&test_dir).expect("Failed to create test directory");

        // Create a session state
        let state = SessionState {
            worktree_path: test_dir.clone(),
            working_directory: test_dir.display().to_string(),
            scrollback_lines: vec!["line1".to_string(), "line2".to_string()],
            last_updated: Utc::now(),
        };

        // Save the state
        state.save().expect("Failed to save session state");

        // Load the state
        let loaded = SessionState::load(&test_dir)
            .expect("Failed to load session state")
            .expect("Session state not found");

        // Verify the loaded state
        assert_eq!(loaded.worktree_path, state.worktree_path);
        assert_eq!(loaded.working_directory, state.working_directory);
        assert_eq!(loaded.scrollback_lines, state.scrollback_lines);

        // Cleanup
        let _ = fs::remove_dir_all(&test_dir);
    }

    #[test]
    fn test_session_state_load_nonexistent() {
        let nonexistent_path = PathBuf::from("/nonexistent/path/that/should/not/exist");
        let result = SessionState::load(&nonexistent_path)
            .expect("Load should not error for nonexistent file");
        assert!(result.is_none());
    }

    #[test]
    fn test_session_filename_generation() {
        let path1 = PathBuf::from("/some/path");
        let path2 = PathBuf::from("/different/path");

        let filename1 = SessionState::session_filename(&path1);
        let filename2 = SessionState::session_filename(&path2);

        // Different paths should produce different filenames
        assert_ne!(filename1, filename2);

        // Same path should produce same filename
        let filename1_again = SessionState::session_filename(&path1);
        assert_eq!(filename1, filename1_again);

        // Filename should end with .json
        assert!(filename1.ends_with(".json"));
    }

    #[test]
    fn test_terminal_session_activity_tracking() {
        let temp_dir = std::env::temp_dir();
        let mut session = TerminalSession::new(temp_dir, Some("/bin/sh".to_string()), 24, 80)
            .expect("Failed to create terminal session");

        // Initially no activity
        assert!(!session.has_activity());

        // Write triggers activity
        session.write_input(b"test").expect("Failed to write");
        assert!(session.has_activity());

        // Clear activity flag
        session.clear_activity_flag();
        assert!(!session.has_activity());
    }
}
