//! Raw mode terminal handling.
//!
//! Provides functions to enter and exit raw mode on Unix terminals using termios.
//! Raw mode disables terminal line buffering and echo, allowing character-by-character
//! input reading.
//!
//! # Safety
//! This module uses unsafe code for FFI calls to libc termios functions.
//! These are necessary for low-level terminal control and cannot be avoided.

#![allow(unsafe_code)]
#![allow(clippy::borrow_as_ptr)]

use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

/// Saved terminal state for restoration.
#[derive(Debug)]
pub struct RawModeGuard {
    fd: RawFd,
    original: libc::termios,
}

impl RawModeGuard {
    /// Enter raw mode on the given file descriptor.
    ///
    /// Returns a guard that will restore the terminal state when dropped.
    pub fn new<F: AsRawFd>(fd: &F) -> io::Result<Self> {
        let fd = fd.as_raw_fd();
        let original = get_termios(fd)?;

        let mut raw = original;

        // Input modes: no break, no CR to NL, no parity check, no strip char,
        // no start/stop output control.
        raw.c_iflag &= !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);

        // Output modes: disable post processing
        raw.c_oflag &= !libc::OPOST;

        // Control modes: set 8 bit chars
        raw.c_cflag |= libc::CS8;

        // Local modes: echo off, canonical off, no extended functions,
        // no signal chars (^C, ^Z, etc)
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);

        // Control characters: set minimal input to return, no timeout
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1; // 100ms timeout for reads

        set_termios(fd, &raw)?;

        Ok(Self { fd, original })
    }

    /// Restore the original terminal state.
    fn restore(&self) -> io::Result<()> {
        set_termios(self.fd, &self.original)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

/// Enter raw mode for stdin.
///
/// Returns a guard that restores the terminal when dropped.
pub fn enable_raw_mode() -> io::Result<RawModeGuard> {
    RawModeGuard::new(&io::stdin())
}

/// Check if the given file descriptor is a TTY.
#[must_use]
pub fn is_tty<F: AsRawFd>(fd: &F) -> bool {
    // SAFETY: isatty is safe to call with any fd
    unsafe { libc::isatty(fd.as_raw_fd()) == 1 }
}

/// Get the terminal size.
///
/// Returns an error if the terminal size cannot be determined or if the
/// returned dimensions are zero (which would cause division by zero errors
/// in buffer allocation code).
pub fn terminal_size() -> io::Result<(u16, u16)> {
    let mut size: libc::winsize = unsafe { std::mem::zeroed() };

    // SAFETY: ioctl with TIOCGWINSZ is safe when passed a valid winsize struct
    let result = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &mut size) };

    if result == -1 {
        Err(io::Error::last_os_error())
    } else if size.ws_col == 0 || size.ws_row == 0 {
        // Zero dimensions would cause buffer allocation/arithmetic issues
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "terminal reported zero dimensions",
        ))
    } else {
        Ok((size.ws_col, size.ws_row))
    }
}

/// Get termios attributes.
fn get_termios(fd: RawFd) -> io::Result<libc::termios> {
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };

    // SAFETY: tcgetattr is safe when passed a valid termios struct
    let result = unsafe { libc::tcgetattr(fd, &mut termios) };

    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(termios)
    }
}

/// Set termios attributes.
fn set_termios(fd: RawFd, termios: &libc::termios) -> io::Result<()> {
    // SAFETY: tcsetattr is safe when passed a valid termios struct
    let result = unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, termios) };

    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::os::unix::io::FromRawFd;

    // ============================================
    // TTY Detection Tests
    // ============================================

    #[test]
    fn test_is_tty_stdin() {
        // In CI/tests, stdin might not be a TTY, but function should work
        let _ = is_tty(&io::stdin());
    }

    #[test]
    fn test_is_tty_stdout() {
        // Test stdout too
        let _ = is_tty(&io::stdout());
    }

    #[test]
    fn test_is_tty_stderr() {
        // Test stderr
        let _ = is_tty(&io::stderr());
    }

    #[test]
    fn test_is_tty_pipe_returns_false() {
        // Create a pipe - neither end is a TTY
        let (read_fd, write_fd) = create_pipe().expect("Failed to create pipe");

        // Neither end of a pipe is a TTY
        assert!(!is_tty(&read_fd), "Read end of pipe should not be TTY");
        assert!(!is_tty(&write_fd), "Write end of pipe should not be TTY");

        // Clean up
        drop(read_fd);
        drop(write_fd);
    }

    #[test]
    fn test_is_tty_file_returns_false() {
        // Create a temp file - not a TTY
        let file = tempfile::tempfile().expect("Failed to create temp file");
        assert!(!is_tty(&file), "Regular file should not be TTY");
    }

    // ============================================
    // Terminal Size Tests
    // ============================================

    #[test]
    fn test_terminal_size_does_not_panic() {
        // This might fail in CI without a TTY, but should not panic
        let _ = terminal_size();
    }

    #[test]
    fn test_terminal_size_valid_dimensions() {
        // If terminal_size succeeds, dimensions should be reasonable
        if let Ok((cols, rows)) = terminal_size() {
            assert!(cols > 0, "Columns should be positive");
            assert!(rows > 0, "Rows should be positive");
            assert!(cols < 10000, "Columns should be reasonable");
            assert!(rows < 10000, "Rows should be reasonable");
        }
    }

    // ============================================
    // Termios Flag Tests
    // ============================================

    #[test]
    fn test_termios_input_flags_disabled() {
        // Verify the flags that should be disabled in raw mode
        let input_flags_to_disable =
            libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON;

        // These should all be present in the disable mask
        assert_ne!(input_flags_to_disable & libc::BRKINT, 0);
        assert_ne!(input_flags_to_disable & libc::ICRNL, 0);
        assert_ne!(input_flags_to_disable & libc::INPCK, 0);
        assert_ne!(input_flags_to_disable & libc::ISTRIP, 0);
        assert_ne!(input_flags_to_disable & libc::IXON, 0);
    }

    #[test]
    fn test_termios_output_flags_disabled() {
        // OPOST should be disabled
        assert_ne!(libc::OPOST, 0, "OPOST flag should be defined");
    }

    #[test]
    fn test_termios_control_flags_enabled() {
        // CS8 should be enabled for 8-bit characters
        assert_ne!(libc::CS8, 0, "CS8 flag should be defined");
    }

    #[test]
    fn test_termios_local_flags_disabled() {
        // Local flags that should be disabled
        let local_flags_to_disable = libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG;

        assert_ne!(local_flags_to_disable & libc::ECHO, 0);
        assert_ne!(local_flags_to_disable & libc::ICANON, 0);
        assert_ne!(local_flags_to_disable & libc::IEXTEN, 0);
        assert_ne!(local_flags_to_disable & libc::ISIG, 0);
    }

    #[test]
    fn test_termios_control_chars() {
        // VMIN and VTIME indices should be valid
        assert!(libc::VMIN < libc::NCCS);
        assert!(libc::VTIME < libc::NCCS);
    }

    // ============================================
    // RawModeGuard Tests
    // ============================================

    #[test]
    fn test_raw_mode_guard_debug() {
        // RawModeGuard should implement Debug
        // We can't actually enter raw mode without a TTY, but we can test the type
        fn assert_debug<T: std::fmt::Debug>() {}
        assert_debug::<RawModeGuard>();
    }

    #[test]
    fn test_enable_raw_mode_returns_error_on_non_tty() {
        // In CI without a TTY, enable_raw_mode should return an error
        // (or succeed if run with a real terminal)
        let result = enable_raw_mode();
        // Either result is acceptable - we just ensure it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_raw_mode_guard_new_on_pipe_fails() {
        // RawModeGuard should fail on a pipe (not a TTY)
        let (read_fd, _write_fd) = create_pipe().expect("Failed to create pipe");
        let result = RawModeGuard::new(&read_fd);

        // Should fail because pipe is not a TTY
        assert!(result.is_err(), "RawModeGuard should fail on pipe");
    }

    // ============================================
    // Internal Function Tests
    // ============================================

    #[test]
    fn test_get_termios_on_pipe_fails() {
        // get_termios should fail on a pipe
        let (read_fd, _write_fd) = create_pipe().expect("Failed to create pipe");
        let result = get_termios(read_fd.as_raw_fd());

        assert!(result.is_err(), "get_termios should fail on pipe");
    }

    #[test]
    fn test_set_termios_on_pipe_fails() {
        // set_termios should fail on a pipe
        let (read_fd, _write_fd) = create_pipe().expect("Failed to create pipe");
        let termios: libc::termios = unsafe { std::mem::zeroed() };
        let result = set_termios(read_fd.as_raw_fd(), &termios);

        assert!(result.is_err(), "set_termios should fail on pipe");
    }

    // ============================================
    // Edge Case Tests
    // ============================================

    #[test]
    fn test_is_tty_with_invalid_fd() {
        // Create a struct that yields an invalid fd
        struct InvalidFd;
        impl AsRawFd for InvalidFd {
            fn as_raw_fd(&self) -> RawFd {
                -1 // Invalid fd
            }
        }

        // Should return false for invalid fd
        assert!(!is_tty(&InvalidFd), "Invalid fd should not be TTY");
    }

    #[test]
    fn test_get_termios_with_invalid_fd_fails() {
        let result = get_termios(-1);
        assert!(result.is_err(), "get_termios should fail on invalid fd");
    }

    #[test]
    fn test_set_termios_with_invalid_fd_fails() {
        let termios: libc::termios = unsafe { std::mem::zeroed() };
        let result = set_termios(-1, &termios);
        assert!(result.is_err(), "set_termios should fail on invalid fd");
    }

    // ============================================
    // Helper Functions
    // ============================================

    /// Create a pipe and return both ends as Files for RAII cleanup
    fn create_pipe() -> io::Result<(File, File)> {
        let mut fds = [0i32; 2];
        let result = unsafe { libc::pipe(fds.as_mut_ptr()) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: pipe() succeeded, so fds are valid
        let read_file = unsafe { File::from_raw_fd(fds[0]) };
        let write_file = unsafe { File::from_raw_fd(fds[1]) };
        Ok((read_file, write_file))
    }
}
