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

    #[test]
    fn test_is_tty() {
        // In CI/tests, stdin might not be a TTY
        let _ = is_tty(&io::stdin());
    }

    #[test]
    fn test_terminal_size() {
        // This might fail in CI without a TTY, so just ensure it doesn't panic
        let _ = terminal_size();
    }
}
