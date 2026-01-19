use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::Config as TermConfig;
use alacritty_terminal::term::Term;
use alacritty_terminal::term::test::TermSize;
use alacritty_terminal::vte::ansi::Processor;
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::terminal::PtyProcess;

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
    pty: PtyProcess,
    term: Term<EventProxy>,
    grid_size: (u16, u16),
    worktree_path: PathBuf,
    #[allow(dead_code)]
    event_proxy: EventProxy,
    parser: Processor,
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
        // Spawn PTY process
        let mut pty =
            PtyProcess::spawn(&worktree_path, shell).context("Failed to spawn PTY process")?;

        // Resize PTY to specified dimensions
        pty.resize(rows, cols)?;

        // Create event proxy
        let event_proxy = EventProxy::new();

        // Create Alacritty terminal with specified dimensions
        let term_size = TermSize::new(cols as usize, rows as usize);

        let term_config = TermConfig::default();
        let term = Term::new(term_config, &term_size, event_proxy.clone());

        // Create ANSI parser
        let parser = Processor::new();

        Ok(TerminalSession {
            pty,
            term,
            grid_size: (rows, cols),
            worktree_path,
            event_proxy,
            parser,
        })
    }

    /// Writes input data to the terminal.
    pub fn write_input(&mut self, data: &[u8]) -> Result<()> {
        let mut total_written = 0;
        while total_written < data.len() {
            match self.pty.write(&data[total_written..]) {
                Ok(0) => {
                    // Would block, try again later
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Ok(n) => total_written += n,
                Err(e) => return Err(e).context("Failed to write input to PTY"),
            }
        }
        Ok(())
    }

    /// Reads and processes output from the terminal.
    /// Returns true if new content was processed.
    pub fn read_and_process_output(&mut self) -> Result<bool> {
        let mut buf = [0u8; 4096];
        let mut has_new_content = false;

        loop {
            match self.pty.read(&mut buf) {
                Ok(0) => break, // No more data available
                Ok(n) => {
                    // Feed data to parser and terminal
                    self.parser.advance(&mut self.term, &buf[..n]);
                    has_new_content = true;
                }
                Err(e) => {
                    return Err(e).context("Failed to read from PTY");
                }
            }
        }

        Ok(has_new_content)
    }

    /// Resizes the terminal.
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        self.grid_size = (rows, cols);

        // Resize PTY
        self.pty.resize(rows, cols)?;

        // Resize Alacritty terminal
        let term_size = TermSize::new(cols as usize, rows as usize);
        self.term.resize(term_size);

        Ok(())
    }

    /// Gets the visible content of the terminal.
    pub fn get_visible_content(&self) -> Vec<String> {
        let grid = self.term.grid();
        let mut lines = Vec::new();

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
            lines.push(line);
        }

        lines
    }

    /// Gets the current cursor position.
    pub fn get_cursor_position(&self) -> (u16, u16) {
        let point = self.term.renderable_content().cursor.point;
        (point.line.0 as u16, point.column.0 as u16)
    }

    /// Checks if the terminal session is still alive.
    pub fn is_alive(&self) -> bool {
        self.pty.is_alive()
    }

    /// Kills the terminal session.
    pub fn kill(&mut self) -> Result<()> {
        // PTY will be dropped and cleaned up automatically
        Ok(())
    }

    /// Gets the worktree path for this session.
    #[allow(dead_code)]
    pub fn worktree_path(&self) -> &PathBuf {
        &self.worktree_path
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

        // Read and process output
        let has_output = session
            .read_and_process_output()
            .expect("Failed to read output");
        assert!(has_output);
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
}
