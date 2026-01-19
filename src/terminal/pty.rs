use anyhow::{Context, Result};
use nix::errno::Errno;
use nix::libc;
use nix::pty::{Winsize, openpty};
use nix::sys::signal::{Signal, kill};
use nix::sys::wait::{WaitPidFlag, WaitStatus, waitpid};
use nix::unistd::{ForkResult, Pid, close, dup2, execvp, fork, read, setsid, write};
use std::ffi::CString;
use std::os::unix::io::{AsRawFd, BorrowedFd, RawFd};
use std::path::Path;

/// Represents a PTY (pseudo-terminal) process.
/// Manages the master file descriptor and the shell process lifecycle.
pub struct PtyProcess {
    master_fd: RawFd,
    child_pid: Pid,
}

impl PtyProcess {
    /// Spawns a new PTY process with the shell.
    ///
    /// # Arguments
    /// * `working_dir` - The working directory for the shell
    /// * `shell` - Optional shell path (defaults to $SHELL or /bin/sh)
    pub fn spawn(working_dir: &Path, shell: Option<String>) -> Result<Self> {
        // Determine shell to use
        let shell_path = shell
            .or_else(|| std::env::var("SHELL").ok())
            .unwrap_or_else(|| "/bin/sh".to_string());

        // Open PTY master and slave
        let winsize = Winsize {
            ws_row: 24,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        let pty_result = openpty(Some(&winsize), None).context("Failed to open PTY")?;

        let master = pty_result.master;
        let slave = pty_result.slave;

        let master_fd = master.as_raw_fd();
        let slave_fd = slave.as_raw_fd();

        // Set master FD to non-blocking mode
        unsafe {
            let flags = libc::fcntl(master_fd, libc::F_GETFL);
            if flags == -1 {
                anyhow::bail!("Failed to get master FD flags");
            }
            if libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
                anyhow::bail!("Failed to set master FD to non-blocking");
            }
        }

        // Fork the process
        match unsafe { fork() }.context("Failed to fork process")? {
            ForkResult::Parent { child } => {
                // Parent process
                // master and slave will be dropped here, closing the slave in the parent
                std::mem::forget(master); // Keep master alive by forgetting it

                Ok(PtyProcess {
                    master_fd,
                    child_pid: child,
                })
            }
            ForkResult::Child => {
                // Child process

                // Create new session
                if let Err(e) = setsid() {
                    eprintln!("Failed to create new session: {}", e);
                    std::process::exit(1);
                }

                // Set slave as controlling terminal
                unsafe {
                    if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) == -1 {
                        eprintln!("Failed to set controlling terminal");
                        std::process::exit(1);
                    }
                }

                // Redirect stdin, stdout, stderr to slave
                if dup2(slave_fd, libc::STDIN_FILENO).is_err() {
                    eprintln!("Failed to dup2 stdin");
                    std::process::exit(1);
                }
                if dup2(slave_fd, libc::STDOUT_FILENO).is_err() {
                    eprintln!("Failed to dup2 stdout");
                    std::process::exit(1);
                }
                if dup2(slave_fd, libc::STDERR_FILENO).is_err() {
                    eprintln!("Failed to dup2 stderr");
                    std::process::exit(1);
                }

                // Close slave FD if it's not one of the standard FDs
                if slave_fd > libc::STDERR_FILENO {
                    let _ = close(slave_fd);
                }

                // Change working directory
                if let Err(e) = std::env::set_current_dir(working_dir) {
                    eprintln!("Failed to change directory: {}", e);
                    std::process::exit(1);
                }

                // Execute shell
                let shell_cstring = CString::new(shell_path.as_str()).unwrap();
                let args = vec![shell_cstring.clone()];

                if let Err(e) = execvp(&shell_cstring, &args) {
                    eprintln!("Failed to exec shell: {}", e);
                    std::process::exit(1);
                }

                // This should never be reached
                unreachable!("execvp returned");
            }
        }
    }

    /// Writes data to the PTY.
    pub fn write(&mut self, data: &[u8]) -> Result<usize> {
        let fd = unsafe { BorrowedFd::borrow_raw(self.master_fd) };
        match write(&fd, data) {
            Ok(n) => Ok(n),
            Err(Errno::EAGAIN) => Ok(0),
            Err(e) => Err(e).context("Failed to write to PTY"),
        }
    }

    /// Reads data from the PTY.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match read(self.master_fd, buf) {
            Ok(n) => Ok(n),
            Err(Errno::EAGAIN) => Ok(0),
            Err(Errno::EIO) => Ok(0), // End of input
            Err(e) => Err(e).context("Failed to read from PTY"),
        }
    }

    /// Resizes the PTY window.
    pub fn resize(&mut self, rows: u16, cols: u16) -> Result<()> {
        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };

        unsafe {
            if libc::ioctl(self.master_fd, libc::TIOCSWINSZ, &winsize as *const _) == -1 {
                anyhow::bail!("Failed to resize PTY window");
            }
        }

        Ok(())
    }

    /// Checks if the child process is still alive.
    pub fn is_alive(&self) -> bool {
        match waitpid(self.child_pid, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => true,
            Ok(_) => false,  // Process has exited
            Err(_) => false, // Error checking status
        }
    }

    /// Returns the raw file descriptor for the PTY master.
    ///
    /// This is used by the event loop for polling.
    pub fn master_fd(&self) -> RawFd {
        self.master_fd
    }
}

impl Drop for PtyProcess {
    fn drop(&mut self) {
        // Send SIGHUP to child process
        let _ = kill(self.child_pid, Signal::SIGHUP);

        // Wait for child process to terminate (with timeout)
        for _ in 0..10 {
            match waitpid(self.child_pid, Some(WaitPidFlag::WNOHANG)) {
                Ok(WaitStatus::StillAlive) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                _ => break,
            }
        }

        // Force kill if still alive
        if self.is_alive() {
            let _ = kill(self.child_pid, Signal::SIGKILL);
            let _ = waitpid(self.child_pid, None);
        }

        // Close master FD
        let _ = close(self.master_fd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_pty_spawn() {
        let temp_dir = std::env::temp_dir();
        let result = PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string()));
        assert!(result.is_ok());

        let pty = result.unwrap();
        assert!(pty.is_alive());
    }

    #[test]
    fn test_pty_write_read() {
        let temp_dir = std::env::temp_dir();
        let mut pty =
            PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string())).expect("Failed to spawn PTY");

        // Write a command
        let cmd = b"echo hello\n";
        let written = pty.write(cmd).expect("Failed to write to PTY");
        assert!(written > 0);

        // Give shell time to process
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Read output
        let mut buf = [0u8; 1024];
        let read = pty.read(&mut buf).expect("Failed to read from PTY");
        assert!(read > 0);
    }

    #[test]
    fn test_pty_resize() {
        let temp_dir = std::env::temp_dir();
        let mut pty =
            PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string())).expect("Failed to spawn PTY");

        let result = pty.resize(50, 100);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pty_is_alive() {
        let temp_dir = std::env::temp_dir();
        let pty =
            PtyProcess::spawn(&temp_dir, Some("/bin/sh".to_string())).expect("Failed to spawn PTY");

        assert!(pty.is_alive());
    }
}
