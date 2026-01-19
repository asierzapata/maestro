//! Terminal handle for sending commands to the PTY background thread.
//!
//! This module provides a thread-safe handle for communicating with
//! the PTY event loop running in a background thread.

use crossbeam_channel::Sender;

use crate::terminal::events::TerminalCommand;

/// A handle for sending commands to the PTY background thread.
///
/// This struct provides a safe, non-blocking interface for sending
/// commands to the PTY process running in a background thread.
#[derive(Clone)]
pub struct TerminalHandle {
    command_tx: Sender<TerminalCommand>,
}

impl TerminalHandle {
    /// Creates a new terminal handle with the given command sender.
    pub fn new(command_tx: Sender<TerminalCommand>) -> Self {
        TerminalHandle { command_tx }
    }

    /// Sends user input to the PTY.
    ///
    /// This method is non-blocking and will return immediately.
    /// The input will be queued and processed by the background thread.
    pub fn send_input(&self, data: Vec<u8>) -> Result<(), crossbeam_channel::SendError<TerminalCommand>> {
        self.command_tx.send(TerminalCommand::Input(data))
    }

    /// Requests a resize of the PTY.
    ///
    /// This method is non-blocking and will return immediately.
    /// The resize will be processed by the background thread.
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), crossbeam_channel::SendError<TerminalCommand>> {
        self.command_tx.send(TerminalCommand::Resize { rows, cols })
    }

    /// Requests a clean shutdown of the PTY and background thread.
    ///
    /// This method is non-blocking and will return immediately.
    /// The shutdown will be processed by the background thread.
    pub fn shutdown(&self) -> Result<(), crossbeam_channel::SendError<TerminalCommand>> {
        self.command_tx.send(TerminalCommand::Shutdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;

    #[test]
    fn test_terminal_handle_send_input() {
        let (tx, rx) = unbounded();
        let handle = TerminalHandle::new(tx);

        let input = b"echo hello\n".to_vec();
        handle.send_input(input.clone()).expect("Failed to send input");

        let received = rx.recv().expect("Failed to receive command");
        match received {
            TerminalCommand::Input(data) => assert_eq!(data, input),
            _ => panic!("Expected Input command"),
        }
    }

    #[test]
    fn test_terminal_handle_resize() {
        let (tx, rx) = unbounded();
        let handle = TerminalHandle::new(tx);

        handle.resize(50, 100).expect("Failed to send resize");

        let received = rx.recv().expect("Failed to receive command");
        match received {
            TerminalCommand::Resize { rows, cols } => {
                assert_eq!(rows, 50);
                assert_eq!(cols, 100);
            }
            _ => panic!("Expected Resize command"),
        }
    }

    #[test]
    fn test_terminal_handle_shutdown() {
        let (tx, rx) = unbounded();
        let handle = TerminalHandle::new(tx);

        handle.shutdown().expect("Failed to send shutdown");

        let received = rx.recv().expect("Failed to receive command");
        assert!(matches!(received, TerminalCommand::Shutdown));
    }

    #[test]
    fn test_terminal_handle_clone() {
        let (tx, rx) = unbounded();
        let handle1 = TerminalHandle::new(tx);
        let handle2 = handle1.clone();

        handle1.send_input(b"from handle1".to_vec()).expect("Failed to send");
        handle2.send_input(b"from handle2".to_vec()).expect("Failed to send");

        let cmd1 = rx.recv().expect("Failed to receive");
        let cmd2 = rx.recv().expect("Failed to receive");

        assert!(matches!(cmd1, TerminalCommand::Input(_)));
        assert!(matches!(cmd2, TerminalCommand::Input(_)));
    }

    #[test]
    fn test_terminal_handle_non_blocking() {
        let (tx, _rx) = unbounded();
        let handle = TerminalHandle::new(tx);

        // These should all return immediately without blocking
        let start = std::time::Instant::now();
        for _ in 0..100 {
            handle.send_input(b"test".to_vec()).expect("Failed to send");
        }
        let elapsed = start.elapsed();

        // Should complete in well under 100ms for 100 non-blocking sends
        assert!(elapsed.as_millis() < 100, "send_input should be non-blocking");
    }
}
