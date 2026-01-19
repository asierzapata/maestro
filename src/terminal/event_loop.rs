//! PTY event loop running in a background thread.
//!
//! This module implements the async event loop that:
//! - Polls the PTY for output in a background thread
//! - Batches events to reduce UI updates
//! - Handles commands from the UI thread

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, TryRecvError, unbounded};
use mio::unix::SourceFd;
use mio::{Events, Interest, Poll, Token};
use nix::errno::Errno;
use nix::unistd::read;

use crate::terminal::events::{TerminalCommand, TerminalEvent};
use crate::terminal::handle::TerminalHandle;
use crate::terminal::pty::PtyProcess;

/// Token for PTY read events in mio
const PTY_TOKEN: Token = Token(0);

/// Maximum duration to batch events before sending to UI (4ms)
const BATCH_DURATION: Duration = Duration::from_millis(4);

/// Maximum number of events to batch before sending to UI
const MAX_BATCH_SIZE: usize = 100;

/// Result of starting the event loop
pub struct EventLoopHandle {
    /// Handle for sending commands to the PTY
    pub terminal_handle: TerminalHandle,
    /// Receiver for events from the PTY
    pub event_rx: Receiver<TerminalEvent>,
    /// Join handle for the background thread
    thread_handle: Option<JoinHandle<()>>,
}

impl EventLoopHandle {
    /// Waits for the background thread to finish.
    pub fn join(mut self) {
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for EventLoopHandle {
    fn drop(&mut self) {
        // Send shutdown command if possible
        let _ = self.terminal_handle.shutdown();

        // Wait for thread to finish
        if let Some(handle) = self.thread_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Starts the PTY event loop in a background thread.
///
/// # Arguments
/// * `pty` - The PTY process to manage
///
/// # Returns
/// An `EventLoopHandle` containing the terminal handle and event receiver.
pub fn start_event_loop(pty: PtyProcess) -> EventLoopHandle {
    let (command_tx, command_rx) = unbounded();
    let (event_tx, event_rx) = unbounded();

    let terminal_handle = TerminalHandle::new(command_tx);

    let thread_handle = thread::spawn(move || {
        run_event_loop(pty, command_rx, event_tx);
    });

    EventLoopHandle {
        terminal_handle,
        event_rx,
        thread_handle: Some(thread_handle),
    }
}

/// The main event loop running in the background thread.
fn run_event_loop(
    mut pty: PtyProcess,
    command_rx: Receiver<TerminalCommand>,
    event_tx: Sender<TerminalEvent>,
) {
    // Set up mio polling
    let mut poll = match Poll::new() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to create poll: {}", e);
            return;
        }
    };

    let fd = pty.master_fd();
    let mut source_fd = SourceFd(&fd);

    if let Err(e) = poll.registry().register(&mut source_fd, PTY_TOKEN, Interest::READABLE) {
        eprintln!("Failed to register PTY fd: {}", e);
        return;
    }

    let mut events = Events::with_capacity(128);
    let mut output_buffer: Vec<u8> = Vec::with_capacity(4096);
    let mut batch_start: Option<Instant> = None;
    let mut batch_count = 0;

    loop {
        // Check for commands from UI thread (non-blocking)
        match command_rx.try_recv() {
            Ok(TerminalCommand::Input(data)) => {
                if let Err(e) = pty.write(&data) {
                    eprintln!("Failed to write to PTY: {}", e);
                }
            }
            Ok(TerminalCommand::Resize { rows, cols }) => {
                if let Err(e) = pty.resize(rows, cols) {
                    eprintln!("Failed to resize PTY: {}", e);
                } else {
                    let _ = event_tx.send(TerminalEvent::Resized { rows, cols });
                }
            }
            Ok(TerminalCommand::Shutdown) => {
                // Flush any remaining output
                if !output_buffer.is_empty() {
                    let _ = event_tx.send(TerminalEvent::Output(output_buffer.clone()));
                }
                break;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                // UI thread has disconnected, shut down
                break;
            }
        }

        // Check if process is still alive
        if !pty.is_alive() {
            // Flush any remaining output
            if !output_buffer.is_empty() {
                let _ = event_tx.send(TerminalEvent::Output(output_buffer.clone()));
            }
            // Send process exited event (exit code 0 as default, could be improved)
            let _ = event_tx.send(TerminalEvent::ProcessExited(0));
            break;
        }

        // Poll for PTY events with a short timeout
        let poll_timeout = if batch_start.is_some() {
            // If we're batching, use remaining batch time
            Some(Duration::from_millis(1))
        } else {
            // Otherwise, poll with a longer timeout
            Some(Duration::from_millis(10))
        };

        if let Err(e) = poll.poll(&mut events, poll_timeout) {
            if e.kind() != std::io::ErrorKind::Interrupted {
                eprintln!("Poll error: {}", e);
                break;
            }
            continue;
        }

        // Process PTY events
        for event in events.iter() {
            if event.token() == PTY_TOKEN && event.is_readable() {
                // Read from PTY
                let mut buf = [0u8; 4096];
                loop {
                    match read(fd, &mut buf) {
                        Ok(0) => break, // EOF
                        Ok(n) => {
                            output_buffer.extend_from_slice(&buf[..n]);

                            // Start batch timing if not already started
                            if batch_start.is_none() {
                                batch_start = Some(Instant::now());
                            }
                            batch_count += 1;
                        }
                        Err(Errno::EAGAIN) | Err(Errno::EWOULDBLOCK) => break,
                        Err(Errno::EIO) => {
                            // PTY closed
                            break;
                        }
                        Err(e) => {
                            eprintln!("PTY read error: {}", e);
                            break;
                        }
                    }
                }
            }
        }

        // Check if we should flush the batch
        let should_flush = if let Some(start) = batch_start {
            start.elapsed() >= BATCH_DURATION || batch_count >= MAX_BATCH_SIZE
        } else {
            false
        };

        if should_flush && !output_buffer.is_empty() {
            let _ = event_tx.send(TerminalEvent::Output(std::mem::take(&mut output_buffer)));
            output_buffer = Vec::with_capacity(4096);
            batch_start = None;
            batch_count = 0;
        }
    }

    // Deregister the fd before dropping
    let _ = poll.registry().deregister(&mut source_fd);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_event_loop_starts_and_responds_to_shutdown() {
        let temp_dir = std::env::temp_dir();
        let pty = PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string()))
            .expect("Failed to spawn PTY");

        let handle = start_event_loop(pty);

        // Send shutdown command
        handle.terminal_handle.shutdown().expect("Failed to send shutdown");

        // Wait for thread to finish
        handle.join();
    }

    #[test]
    fn test_event_loop_sends_input_and_receives_output() {
        let temp_dir = std::env::temp_dir();
        let pty = PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string()))
            .expect("Failed to spawn PTY");

        let handle = start_event_loop(pty);

        // Send a simple command
        handle
            .terminal_handle
            .send_input(b"echo test_output_12345\n".to_vec())
            .expect("Failed to send input");

        // Wait for output (with timeout)
        let start = Instant::now();
        let mut received_output = false;

        while start.elapsed() < Duration::from_millis(500) {
            match handle.event_rx.try_recv() {
                Ok(TerminalEvent::Output(data)) => {
                    let output = String::from_utf8_lossy(&data);
                    if output.contains("test_output_12345") {
                        received_output = true;
                        break;
                    }
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryRecvError::Disconnected) => break,
            }
        }

        assert!(received_output, "Should have received output containing 'test_output_12345'");

        handle.terminal_handle.shutdown().expect("Failed to shutdown");
    }

    #[test]
    fn test_event_loop_resize() {
        let temp_dir = std::env::temp_dir();
        let pty = PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string()))
            .expect("Failed to spawn PTY");

        let handle = start_event_loop(pty);

        // Send resize command
        handle
            .terminal_handle
            .resize(50, 100)
            .expect("Failed to send resize");

        // Wait for resize confirmation
        let start = Instant::now();
        let mut received_resize = false;

        while start.elapsed() < Duration::from_millis(200) {
            match handle.event_rx.try_recv() {
                Ok(TerminalEvent::Resized { rows, cols }) => {
                    assert_eq!(rows, 50);
                    assert_eq!(cols, 100);
                    received_resize = true;
                    break;
                }
                Ok(_) => {}
                Err(TryRecvError::Empty) => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(TryRecvError::Disconnected) => break,
            }
        }

        assert!(received_resize, "Should have received resize confirmation");

        handle.terminal_handle.shutdown().expect("Failed to shutdown");
    }

    #[test]
    fn test_terminal_handle_non_blocking_send() {
        let temp_dir = std::env::temp_dir();
        let pty = PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string()))
            .expect("Failed to spawn PTY");

        let handle = start_event_loop(pty);

        // Send many commands quickly - should not block
        let start = Instant::now();
        for i in 0..100 {
            handle
                .terminal_handle
                .send_input(format!("echo {}\n", i).into_bytes())
                .expect("Failed to send");
        }
        let elapsed = start.elapsed();

        // All sends should complete in under 50ms
        assert!(
            elapsed.as_millis() < 50,
            "Sending should be non-blocking, took {:?}",
            elapsed
        );

        handle.terminal_handle.shutdown().expect("Failed to shutdown");
    }
}
