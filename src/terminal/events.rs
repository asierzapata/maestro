//! Event types for async PTY communication.
//!
//! This module defines the event and command types used for asynchronous
//! communication between the PTY background thread and the UI.

/// Events sent from the PTY background thread to the UI.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Raw bytes received from the PTY output.
    Output(Vec<u8>),
    /// The PTY process has exited with the given exit code.
    ProcessExited(i32),
    /// Confirmation that the PTY was resized to the given dimensions.
    Resized { rows: u16, cols: u16 },
}

/// Commands sent from the UI to the PTY background thread.
#[derive(Debug, Clone)]
pub enum TerminalCommand {
    /// User input to send to the PTY.
    Input(Vec<u8>),
    /// Request to resize the PTY to the given dimensions.
    Resize { rows: u16, cols: u16 },
    /// Request to shut down the PTY and background thread cleanly.
    Shutdown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_event_output() {
        let event = TerminalEvent::Output(vec![72, 101, 108, 108, 111]); // "Hello"
        match event {
            TerminalEvent::Output(data) => assert_eq!(data, vec![72, 101, 108, 108, 111]),
            _ => panic!("Expected Output event"),
        }
    }

    #[test]
    fn test_terminal_event_process_exited() {
        let event = TerminalEvent::ProcessExited(0);
        match event {
            TerminalEvent::ProcessExited(code) => assert_eq!(code, 0),
            _ => panic!("Expected ProcessExited event"),
        }
    }

    #[test]
    fn test_terminal_event_resized() {
        let event = TerminalEvent::Resized { rows: 24, cols: 80 };
        match event {
            TerminalEvent::Resized { rows, cols } => {
                assert_eq!(rows, 24);
                assert_eq!(cols, 80);
            }
            _ => panic!("Expected Resized event"),
        }
    }

    #[test]
    fn test_terminal_command_input() {
        let cmd = TerminalCommand::Input(b"ls -la\n".to_vec());
        match cmd {
            TerminalCommand::Input(data) => assert_eq!(data, b"ls -la\n".to_vec()),
            _ => panic!("Expected Input command"),
        }
    }

    #[test]
    fn test_terminal_command_resize() {
        let cmd = TerminalCommand::Resize { rows: 50, cols: 100 };
        match cmd {
            TerminalCommand::Resize { rows, cols } => {
                assert_eq!(rows, 50);
                assert_eq!(cols, 100);
            }
            _ => panic!("Expected Resize command"),
        }
    }

    #[test]
    fn test_terminal_command_shutdown() {
        let cmd = TerminalCommand::Shutdown;
        matches!(cmd, TerminalCommand::Shutdown);
    }
}
