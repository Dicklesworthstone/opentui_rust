//! Mock terminal for capturing ANSI output without a real PTY.
//!
//! This module provides [`MockTerminal`] which implements `Write` and captures
//! all ANSI sequences for later inspection. Useful for testing renderer output
//! without needing actual terminal access.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

/// A mock terminal that captures all output for inspection.
///
/// Implements `Write` so it can be used wherever a terminal writer is expected.
/// All written bytes are captured and can be inspected via [`output()`] and
/// [`output_str()`].
///
/// # Thread Safety
///
/// `MockTerminal` is `Send + Sync` via internal mutex, allowing use in
/// multi-threaded test scenarios.
///
/// # Example
///
/// ```ignore
/// let mut term = MockTerminal::new(80, 24);
/// write!(term, "\x1b[2J").unwrap(); // Clear screen
/// assert!(term.output_str().contains("\x1b[2J"));
/// ```
#[derive(Clone)]
pub struct MockTerminal {
    /// Captured output bytes.
    output: Arc<Mutex<Vec<u8>>>,
    /// Terminal width.
    pub width: u32,
    /// Terminal height.
    pub height: u32,
    /// Whether writes should succeed (for error simulation).
    write_enabled: Arc<Mutex<bool>>,
    /// Simulated write latency in microseconds (0 = no delay).
    write_latency_us: u64,
}

impl MockTerminal {
    /// Create a new mock terminal with given dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            output: Arc::new(Mutex::new(Vec::with_capacity(4096))),
            width,
            height,
            write_enabled: Arc::new(Mutex::new(true)),
            write_latency_us: 0,
        }
    }

    /// Create a mock terminal with simulated write latency.
    ///
    /// Useful for testing buffering behavior under slow I/O conditions.
    pub fn with_latency(width: u32, height: u32, latency_us: u64) -> Self {
        Self {
            output: Arc::new(Mutex::new(Vec::with_capacity(4096))),
            width,
            height,
            write_enabled: Arc::new(Mutex::new(true)),
            write_latency_us: latency_us,
        }
    }

    /// Get the captured output as a byte slice.
    pub fn output(&self) -> Vec<u8> {
        self.output.lock().unwrap().clone()
    }

    /// Get the captured output as a string (lossy UTF-8 conversion).
    pub fn output_str(&self) -> String {
        String::from_utf8_lossy(&self.output()).into_owned()
    }

    /// Get the captured output length in bytes.
    pub fn output_len(&self) -> usize {
        self.output.lock().unwrap().len()
    }

    /// Clear the captured output buffer.
    pub fn clear_output(&self) {
        self.output.lock().unwrap().clear();
    }

    /// Disable writes (simulates I/O error).
    pub fn disable_writes(&self) {
        *self.write_enabled.lock().unwrap() = false;
    }

    /// Enable writes.
    pub fn enable_writes(&self) {
        *self.write_enabled.lock().unwrap() = true;
    }

    /// Check if a specific ANSI sequence is present in output.
    pub fn contains_sequence(&self, seq: &[u8]) -> bool {
        let output = self.output();
        output.windows(seq.len()).any(|window| window == seq)
    }

    /// Count occurrences of a specific ANSI sequence.
    pub fn count_sequence(&self, seq: &[u8]) -> usize {
        let output = self.output();
        output
            .windows(seq.len())
            .filter(|window| *window == seq)
            .count()
    }

    /// Find all positions of a specific sequence in the output.
    pub fn find_sequences(&self, seq: &[u8]) -> Vec<usize> {
        let output = self.output();
        output
            .windows(seq.len())
            .enumerate()
            .filter(|(_, window)| *window == seq)
            .map(|(i, _)| i)
            .collect()
    }

    /// Extract all CSI sequences (ESC [ ... final_byte) from output.
    pub fn extract_csi_sequences(&self) -> Vec<Vec<u8>> {
        let output = self.output();
        let mut sequences = Vec::new();
        let mut i = 0;

        while i < output.len() {
            // Look for ESC [
            if i + 1 < output.len() && output[i] == 0x1b && output[i + 1] == b'[' {
                let start = i;
                i += 2; // Skip ESC [

                // Find the final byte (0x40-0x7E)
                while i < output.len() {
                    let b = output[i];
                    i += 1;
                    if (0x40..=0x7E).contains(&b) {
                        sequences.push(output[start..i].to_vec());
                        break;
                    }
                }
            } else {
                i += 1;
            }
        }

        sequences
    }

    /// Extract all SGR sequences (ESC [ ... m) from output.
    pub fn extract_sgr_sequences(&self) -> Vec<Vec<u8>> {
        self.extract_csi_sequences()
            .into_iter()
            .filter(|seq| seq.last() == Some(&b'm'))
            .collect()
    }

    /// Check if output contains the alt screen enter sequence.
    pub fn entered_alt_screen(&self) -> bool {
        self.contains_sequence(b"\x1b[?1049h")
    }

    /// Check if output contains the alt screen leave sequence.
    pub fn left_alt_screen(&self) -> bool {
        self.contains_sequence(b"\x1b[?1049l")
    }

    /// Check if output contains the cursor hide sequence.
    pub fn cursor_hidden(&self) -> bool {
        self.contains_sequence(b"\x1b[?25l")
    }

    /// Check if output contains the cursor show sequence.
    pub fn cursor_shown(&self) -> bool {
        self.contains_sequence(b"\x1b[?25h")
    }

    /// Check if output contains the clear screen sequence.
    pub fn screen_cleared(&self) -> bool {
        self.contains_sequence(b"\x1b[2J")
    }
}

impl Write for MockTerminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !*self.write_enabled.lock().unwrap() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Mock terminal writes disabled",
            ));
        }

        // Simulate latency if configured
        if self.write_latency_us > 0 {
            std::thread::sleep(std::time::Duration::from_micros(self.write_latency_us));
        }

        self.output.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if !*self.write_enabled.lock().unwrap() {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Mock terminal writes disabled",
            ));
        }
        Ok(())
    }
}

impl std::fmt::Debug for MockTerminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTerminal")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("output_len", &self.output_len())
            .finish()
    }
}

/// A write-only handle to a `MockTerminal` for use as a writer.
///
/// This is useful when you need to pass ownership of a writer to something
/// that expects `impl Write`, while retaining ability to inspect the output.
pub struct MockWriter {
    terminal: MockTerminal,
}

impl MockWriter {
    /// Create a new mock writer wrapping the given terminal.
    pub fn new(terminal: MockTerminal) -> Self {
        Self { terminal }
    }

    /// Get a reference to the underlying terminal.
    pub fn terminal(&self) -> &MockTerminal {
        &self.terminal
    }
}

impl Write for MockWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.terminal.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.terminal.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_terminal_captures_output() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "Hello").unwrap();
        assert_eq!(term.output_str(), "Hello");
    }

    #[test]
    fn test_mock_terminal_captures_ansi() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "\x1b[2J\x1b[H").unwrap();
        assert!(term.screen_cleared());
        assert!(term.contains_sequence(b"\x1b[H"));
    }

    #[test]
    fn test_mock_terminal_alt_screen_detection() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "\x1b[?1049h").unwrap();
        assert!(term.entered_alt_screen());
        assert!(!term.left_alt_screen());

        write!(term, "\x1b[?1049l").unwrap();
        assert!(term.left_alt_screen());
    }

    #[test]
    fn test_mock_terminal_cursor_detection() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "\x1b[?25l").unwrap();
        assert!(term.cursor_hidden());
        assert!(!term.cursor_shown());

        write!(term, "\x1b[?25h").unwrap();
        assert!(term.cursor_shown());
    }

    #[test]
    fn test_mock_terminal_sequence_counting() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "\x1b[0m\x1b[0m\x1b[0m").unwrap();
        assert_eq!(term.count_sequence(b"\x1b[0m"), 3);
    }

    #[test]
    fn test_mock_terminal_csi_extraction() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "\x1b[2J\x1b[1;1H\x1b[31m").unwrap();
        let sequences = term.extract_csi_sequences();
        assert_eq!(sequences.len(), 3);
        assert_eq!(sequences[0], b"\x1b[2J");
        assert_eq!(sequences[1], b"\x1b[1;1H");
        assert_eq!(sequences[2], b"\x1b[31m");
    }

    #[test]
    fn test_mock_terminal_sgr_extraction() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "\x1b[2J\x1b[31m\x1b[1mHello\x1b[0m").unwrap();
        let sgr_sequences = term.extract_sgr_sequences();
        assert_eq!(sgr_sequences.len(), 3);
        assert_eq!(sgr_sequences[0], b"\x1b[31m");
        assert_eq!(sgr_sequences[1], b"\x1b[1m");
        assert_eq!(sgr_sequences[2], b"\x1b[0m");
    }

    #[test]
    fn test_mock_terminal_clear() {
        let mut term = MockTerminal::new(80, 24);
        write!(term, "test").unwrap();
        assert!(!term.output().is_empty());
        term.clear_output();
        assert!(term.output().is_empty());
    }

    #[test]
    fn test_mock_terminal_write_disable() {
        let mut term = MockTerminal::new(80, 24);
        term.disable_writes();
        assert!(term.write_all(b"test").is_err());
        term.enable_writes();
        assert!(term.write_all(b"test").is_ok());
    }

    #[test]
    fn test_mock_terminal_clone_shares_output() {
        let mut term1 = MockTerminal::new(80, 24);
        let term2 = term1.clone();

        write!(term1, "hello").unwrap();
        assert_eq!(term2.output_str(), "hello");
    }
}
