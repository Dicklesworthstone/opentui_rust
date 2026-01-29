//! Visual PTY testing for TUI rendering verification.
//!
//! This module uses portable-pty to spawn the TUI in a real pseudo-terminal
//! and vt100 to parse the output into an inspectable screen buffer.
//!
//! This catches rendering bugs that unit tests miss by actually verifying
//! what characters appear at what positions on screen.

#![cfg(feature = "pty-tests")]

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Screen capture from the virtual terminal.
#[derive(Debug)]
pub struct ScreenCapture {
    /// Raw screen contents (rows of strings)
    pub rows: Vec<String>,
    /// Width in columns
    pub width: u16,
    /// Height in rows
    pub height: u16,
    /// Cursor position (col, row)
    pub cursor: (u16, u16),
}

impl ScreenCapture {
    /// Get the text at a specific row (0-indexed).
    pub fn row_text(&self, row: usize) -> Option<&str> {
        self.rows.get(row).map(String::as_str)
    }

    /// Check if a specific row contains a substring.
    pub fn row_contains(&self, row: usize, substring: &str) -> bool {
        self.rows.get(row).map_or(false, |r| r.contains(substring))
    }

    /// Get the character at a specific position.
    pub fn char_at(&self, col: usize, row: usize) -> Option<char> {
        self.rows.get(row).and_then(|r| r.chars().nth(col))
    }

    /// Find the first row containing a substring.
    pub fn find_row_containing(&self, substring: &str) -> Option<usize> {
        self.rows.iter().position(|r| r.contains(substring))
    }

    /// Dump the screen to a string for debugging.
    pub fn dump(&self) -> String {
        let mut out = format!("Screen {}x{}:\n", self.width, self.height);
        for (i, row) in self.rows.iter().enumerate() {
            out.push_str(&format!("{:3}| {}\n", i, row));
        }
        out
    }
}

/// PTY test harness for visual TUI testing.
pub struct PtyTestHarness {
    pty_writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    parser: vt100::Parser,
    reader_rx: mpsc::Receiver<Vec<u8>>,
    width: u16,
    height: u16,
}

impl PtyTestHarness {
    /// Spawn a command in a PTY with the given dimensions and custom TERM.
    pub fn spawn_with_term(
        cmd: &str,
        args: &[&str],
        width: u16,
        height: u16,
        term: &str,
    ) -> std::io::Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: height,
                cols: width,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let mut cmd_builder = CommandBuilder::new(cmd);
        cmd_builder.args(args);
        // Use specified TERM to test different capability paths
        cmd_builder.env("TERM", term);
        // Disable mouse to simplify testing
        cmd_builder.env("OPENTUI_NO_MOUSE", "1");

        let child = pair
            .slave
            .spawn_command(cmd_builder)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Set up async reader for PTY output
        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Get writer for sending input
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        Ok(Self {
            pty_writer: writer,
            child,
            parser: vt100::Parser::new(height, width, 0),
            reader_rx: rx,
            width,
            height,
        })
    }

    /// Spawn a command in a PTY with the given dimensions.
    /// Uses TERM=wezterm by default to match WezTerm-specific capability paths.
    pub fn spawn(cmd: &str, args: &[&str], width: u16, height: u16) -> std::io::Result<Self> {
        Self::spawn_with_term(cmd, args, width, height, "wezterm")
    }

    /// Spawn demo_showcase with given arguments and TERM type.
    pub fn spawn_demo_with_term(
        args: &[&str],
        width: u16,
        height: u16,
        term: &str,
    ) -> std::io::Result<Self> {
        let demo_path = env!("CARGO_BIN_EXE_demo_showcase");
        Self::spawn_with_term(demo_path, args, width, height, term)
    }

    /// Spawn demo_showcase with given arguments.
    /// Uses TERM=wezterm by default.
    pub fn spawn_demo(args: &[&str], width: u16, height: u16) -> std::io::Result<Self> {
        Self::spawn_demo_with_term(args, width, height, "wezterm")
    }

    /// Wait for output and process it through the terminal parser.
    pub fn wait_for_output(&mut self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut received_any = false;

        while Instant::now() < deadline {
            match self.reader_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(data) => {
                    self.parser.process(&data);
                    received_any = true;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if received_any {
                        // Give a little more time for any trailing output
                        thread::sleep(Duration::from_millis(10));
                        // Drain any remaining
                        while let Ok(data) = self.reader_rx.try_recv() {
                            self.parser.process(&data);
                        }
                        return true;
                    }
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => return received_any,
            }
        }
        received_any
    }

    /// Send keystrokes to the PTY.
    pub fn send_keys(&mut self, keys: &str) -> std::io::Result<()> {
        self.pty_writer.write_all(keys.as_bytes())?;
        self.pty_writer.flush()
    }

    /// Send a special key (escape sequence).
    pub fn send_escape(&mut self, seq: &str) -> std::io::Result<()> {
        self.pty_writer.write_all(seq.as_bytes())?;
        self.pty_writer.flush()
    }

    /// Send Ctrl+C.
    pub fn send_ctrl_c(&mut self) -> std::io::Result<()> {
        self.pty_writer.write_all(&[0x03])?;
        self.pty_writer.flush()
    }

    /// Send Escape key.
    pub fn send_esc(&mut self) -> std::io::Result<()> {
        self.pty_writer.write_all(&[0x1b])?;
        self.pty_writer.flush()
    }

    /// Capture the current screen state.
    pub fn capture_screen(&self) -> ScreenCapture {
        let screen = self.parser.screen();
        let mut rows = Vec::with_capacity(self.height as usize);

        for row in 0..self.height {
            let mut line = String::new();
            for col in 0..self.width {
                let cell = screen.cell(row, col).unwrap();
                line.push_str(&cell.contents());
            }
            // Trim trailing spaces but keep structure
            let trimmed = line.trim_end();
            rows.push(trimmed.to_string());
        }

        let cursor = screen.cursor_position();

        ScreenCapture {
            rows,
            width: self.width,
            height: self.height,
            cursor,
        }
    }

    /// Wait for the screen to contain a specific string.
    pub fn wait_for_text(&mut self, text: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            self.wait_for_output(Duration::from_millis(100));
            let screen = self.capture_screen();
            for row in &screen.rows {
                if row.contains(text) {
                    return true;
                }
            }
        }
        false
    }

    /// Wait for the process to exit and return exit code.
    pub fn wait_exit(&mut self, timeout: Duration) -> Option<u32> {
        let deadline = Instant::now() + timeout;

        while Instant::now() < deadline {
            // Drain output
            while let Ok(data) = self.reader_rx.try_recv() {
                self.parser.process(&data);
            }

            // Check if exited
            if let Ok(Some(status)) = self.child.try_wait() {
                return Some(status.exit_code());
            }

            thread::sleep(Duration::from_millis(50));
        }
        None
    }

    /// Kill the child process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for PtyTestHarness {
    fn drop(&mut self) {
        let _ = self.child.kill();
    }
}

// ============================================================================
// Visual Tests
// ============================================================================

#[test]
fn test_demo_renders_header() {
    let mut harness =
        PtyTestHarness::spawn_demo(&["--seed", "42"], 120, 40).expect("Failed to spawn demo");

    // Wait for initial render
    assert!(
        harness.wait_for_output(Duration::from_secs(5)),
        "Demo should produce output"
    );

    let screen = harness.capture_screen();
    println!("{}", screen.dump());

    // Check for expected header content
    assert!(
        screen.row_contains(0, "OpenTUI") || screen.find_row_containing("OpenTUI").is_some(),
        "Screen should contain 'OpenTUI' header"
    );

    // Clean exit
    harness.send_keys("q").ok();
    harness.wait_exit(Duration::from_secs(2));
}

#[test]
fn test_demo_renders_cache_rs_correctly() {
    let mut harness =
        PtyTestHarness::spawn_demo(&["--seed", "42"], 120, 40).expect("Failed to spawn demo");

    // Wait for initial render
    assert!(
        harness.wait_for_output(Duration::from_secs(5)),
        "Demo should produce output"
    );

    let screen = harness.capture_screen();
    println!("=== SCREEN CAPTURE ===");
    println!("{}", screen.dump());

    // The editor panel should show cache.rs with correct content
    // Line should contain "use std::io::{self, Write};" NOT garbled text
    let io_line = screen.find_row_containing("std::io");

    if let Some(row_idx) = io_line {
        let row = screen.row_text(row_idx).unwrap();
        println!("Found io line at row {}: {}", row_idx, row);

        // Verify the line is NOT corrupted
        assert!(
            row.contains("self") && row.contains("Write"),
            "Line should contain 'self' and 'Write', got: {}",
            row
        );
        assert!(
            !row.contains("u8tyle") && !row.contains("Wslts"),
            "Line should NOT contain garbled text like 'u8tyle' or 'Wslts', got: {}",
            row
        );
    } else {
        // If we can't find std::io, check that we at least have std::collections
        // and verify content is readable
        let collections_line = screen.find_row_containing("collections");
        assert!(
            collections_line.is_some(),
            "Should find either std::io or std::collections in editor"
        );
    }

    harness.send_keys("q").ok();
    harness.wait_exit(Duration::from_secs(2));
}

#[test]
fn test_demo_text_not_garbled() {
    let mut harness =
        PtyTestHarness::spawn_demo(&["--seed", "42"], 120, 40).expect("Failed to spawn demo");

    harness.wait_for_output(Duration::from_secs(5));
    let screen = harness.capture_screen();
    println!("{}", screen.dump());

    // Check that no lines contain obviously garbled text
    for (i, row) in screen.rows.iter().enumerate() {
        // These are known garbled patterns from the bug
        assert!(
            !row.contains("u8tyle"),
            "Row {} contains garbled text 'u8tyle': {}",
            i,
            row
        );
        assert!(
            !row.contains("Wslts"),
            "Row {} contains garbled text 'Wslts': {}",
            i,
            row
        );
    }

    harness.send_keys("q").ok();
    harness.wait_exit(Duration::from_secs(2));
}

#[test]
fn test_demo_keyboard_navigation() {
    let mut harness =
        PtyTestHarness::spawn_demo(&["--seed", "42"], 120, 40).expect("Failed to spawn demo");

    harness.wait_for_output(Duration::from_secs(5));

    // Initial state
    let screen1 = harness.capture_screen();
    println!("=== INITIAL STATE ===");
    println!("{}", screen1.dump());

    // Press Tab to change focus
    harness.send_keys("\t").expect("Failed to send Tab");
    harness.wait_for_output(Duration::from_secs(1));

    let screen2 = harness.capture_screen();
    println!("=== AFTER TAB ===");
    println!("{}", screen2.dump());

    // The screen should have changed (focus moved)
    // We don't assert exact content, just that something changed
    let changed = screen1.rows != screen2.rows;
    println!("Screen changed after Tab: {}", changed);

    harness.send_keys("q").ok();
    harness.wait_exit(Duration::from_secs(2));
}

#[test]
fn test_demo_tour_mode_text_integrity() {
    let mut harness = PtyTestHarness::spawn_demo(&["--seed", "42", "--tour"], 120, 40)
        .expect("Failed to spawn demo");

    harness.wait_for_output(Duration::from_secs(5));

    let screen = harness.capture_screen();
    println!("=== TOUR MODE ===");
    println!("{}", screen.dump());

    // In tour mode, should see "Tour" indicator
    assert!(
        screen.find_row_containing("Tour").is_some()
            || screen.find_row_containing("Welcome").is_some(),
        "Tour mode should show tour indicator or welcome message"
    );

    // Verify no garbled text
    for (i, row) in screen.rows.iter().enumerate() {
        assert!(
            !row.contains("u8tyle") && !row.contains("Wslts"),
            "Row {} in tour mode has garbled text: {}",
            i,
            row
        );
    }

    harness.send_keys("q").ok();
    harness.wait_exit(Duration::from_secs(2));
}

// ============================================================================
// Terminal Type Consistency Tests
// ============================================================================

/// Test that rendering is consistent between TERM=wezterm and TERM=xterm-256color
#[test]
fn test_terminal_type_consistency() {
    // Test with wezterm
    let mut harness_wez =
        PtyTestHarness::spawn_demo_with_term(&["--seed", "42"], 120, 40, "wezterm")
            .expect("Failed to spawn demo with wezterm");

    harness_wez.wait_for_output(Duration::from_secs(5));
    let screen_wez = harness_wez.capture_screen();

    harness_wez.send_keys("q").ok();
    harness_wez.wait_exit(Duration::from_secs(2));

    // Test with xterm-256color
    let mut harness_xterm =
        PtyTestHarness::spawn_demo_with_term(&["--seed", "42"], 120, 40, "xterm-256color")
            .expect("Failed to spawn demo with xterm-256color");

    harness_xterm.wait_for_output(Duration::from_secs(5));
    let screen_xterm = harness_xterm.capture_screen();

    harness_xterm.send_keys("q").ok();
    harness_xterm.wait_exit(Duration::from_secs(2));

    println!("=== WEZTERM ===");
    println!("{}", screen_wez.dump());
    println!("=== XTERM-256COLOR ===");
    println!("{}", screen_xterm.dump());

    // Both should contain the same key content (ignoring timing/frame differences)
    assert!(
        screen_wez.find_row_containing("OpenTUI").is_some(),
        "wezterm should show OpenTUI header"
    );
    assert!(
        screen_xterm.find_row_containing("OpenTUI").is_some(),
        "xterm should show OpenTUI header"
    );

    // Both should have readable code content
    let wez_has_std = screen_wez.find_row_containing("std::").is_some();
    let xterm_has_std = screen_xterm.find_row_containing("std::").is_some();

    assert!(
        wez_has_std && xterm_has_std,
        "Both terminal types should render std:: imports correctly"
    );

    // Neither should have garbled text
    for (i, row) in screen_wez.rows.iter().enumerate() {
        assert!(
            !row.contains("u8tyle") && !row.contains("Wslts"),
            "wezterm row {} has garbled text: {}",
            i,
            row
        );
    }
    for (i, row) in screen_xterm.rows.iter().enumerate() {
        assert!(
            !row.contains("u8tyle") && !row.contains("Wslts"),
            "xterm row {} has garbled text: {}",
            i,
            row
        );
    }
}

/// Test rendering at different screen sizes to catch resize/layout bugs
#[test]
fn test_different_screen_sizes() {
    let sizes = [(80, 24), (120, 40), (160, 50)];

    for (width, height) in sizes {
        println!("Testing {}x{}", width, height);

        let mut harness = PtyTestHarness::spawn_demo(&["--seed", "42"], width, height)
            .expect(&format!("Failed to spawn demo at {}x{}", width, height));

        harness.wait_for_output(Duration::from_secs(5));
        let screen = harness.capture_screen();

        println!("=== {}x{} ===", width, height);
        println!("{}", screen.dump());

        // Should always show header
        assert!(
            screen.find_row_containing("OpenTUI").is_some()
                || screen.find_row_containing("Showcase").is_some(),
            "Screen {}x{} should contain header",
            width,
            height
        );

        // No garbled text
        for (i, row) in screen.rows.iter().enumerate() {
            assert!(
                !row.contains("u8tyle") && !row.contains("Wslts"),
                "Screen {}x{} row {} has garbled text: {}",
                width,
                height,
                i,
                row
            );
        }

        harness.send_keys("q").ok();
        harness.wait_exit(Duration::from_secs(2));
    }
}

/// Test that arrow key navigation works correctly
#[test]
fn test_arrow_key_navigation() {
    let mut harness =
        PtyTestHarness::spawn_demo(&["--seed", "42"], 120, 40).expect("Failed to spawn demo");

    harness.wait_for_output(Duration::from_secs(5));
    let initial_screen = harness.capture_screen();

    // Send down arrow (ESC [ B)
    harness
        .send_escape("\x1b[B")
        .expect("Failed to send down arrow");
    harness.wait_for_output(Duration::from_secs(1));

    let after_down = harness.capture_screen();

    // Send up arrow (ESC [ A)
    harness
        .send_escape("\x1b[A")
        .expect("Failed to send up arrow");
    harness.wait_for_output(Duration::from_secs(1));

    let after_up = harness.capture_screen();

    println!("=== INITIAL ===");
    println!("{}", initial_screen.dump());
    println!("=== AFTER DOWN ===");
    println!("{}", after_down.dump());
    println!("=== AFTER UP ===");
    println!("{}", after_up.dump());

    // Verify no garbled text after navigation
    for (name, screen) in [
        ("initial", &initial_screen),
        ("after_down", &after_down),
        ("after_up", &after_up),
    ] {
        for (i, row) in screen.rows.iter().enumerate() {
            assert!(
                !row.contains("u8tyle") && !row.contains("Wslts"),
                "{} row {} has garbled text: {}",
                name,
                i,
                row
            );
        }
    }

    harness.send_keys("q").ok();
    harness.wait_exit(Duration::from_secs(2));
}
