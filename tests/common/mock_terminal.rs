//! Mock terminal for testing renderer output without a real terminal.
//!
//! This module provides:
//! - `MockTerminal`: A terminal wrapper that captures ANSI output to a buffer
//! - `AnsiSequence`: Parsed ANSI escape sequences for verification
//! - Helper functions for asserting expected output
//!
//! # Example
//!
//! ```ignore
//! use tests::common::mock_terminal::{MockTerminal, AnsiSequence};
//!
//! let mut mock = MockTerminal::new(80, 24);
//! mock.terminal().hide_cursor()?;
//!
//! let sequences = mock.parse_sequences();
//! assert!(sequences.iter().any(|s| matches!(s, AnsiSequence::HideCursor)));
//! ```

#![allow(dead_code)] // Shared test helper; not every integration test uses every mock/utility

use opentui::color::Rgba;
use opentui::style::TextAttributes;
use opentui::terminal::{Capabilities, ColorSupport, Terminal};
use opentui_rust as opentui;
use std::io::{self, Write};

/// A mock terminal that captures output to an in-memory buffer.
///
/// Use this for testing terminal operations without a real TTY.
#[derive(Debug)]
pub struct MockTerminal {
    /// The captured output buffer.
    output: Vec<u8>,
    /// Terminal dimensions.
    width: u16,
    height: u16,
    /// Pre-configured capabilities for testing.
    capabilities: Capabilities,
}

impl MockTerminal {
    /// Create a new mock terminal with specified dimensions.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            output: Vec::with_capacity(4096),
            width,
            height,
            capabilities: Capabilities::default(),
        }
    }

    /// Create a mock terminal with specific capabilities.
    #[must_use]
    pub fn with_capabilities(width: u16, height: u16, capabilities: Capabilities) -> Self {
        Self {
            output: Vec::with_capacity(4096),
            width,
            height,
            capabilities,
        }
    }

    /// Create a mock terminal with no color support.
    #[must_use]
    pub fn no_color(width: u16, height: u16) -> Self {
        let caps = Capabilities {
            color: ColorSupport::None,
            ..Capabilities::default()
        };
        Self::with_capabilities(width, height, caps)
    }

    /// Create a mock terminal with 256-color support.
    #[must_use]
    pub fn color_256(width: u16, height: u16) -> Self {
        let caps = Capabilities {
            color: ColorSupport::Extended,
            ..Capabilities::default()
        };
        Self::with_capabilities(width, height, caps)
    }

    /// Get terminal dimensions.
    #[must_use]
    pub fn size(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Get the raw output buffer.
    #[must_use]
    pub fn output(&self) -> &[u8] {
        &self.output
    }

    /// Get the output as a string (lossy UTF-8 conversion).
    #[must_use]
    pub fn output_str(&self) -> String {
        String::from_utf8_lossy(&self.output).into_owned()
    }

    /// Clear the output buffer.
    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    /// Get the output length in bytes.
    #[must_use]
    pub fn output_len(&self) -> usize {
        self.output.len()
    }

    /// Check if output is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.output.is_empty()
    }

    /// Create a Terminal wrapper around this mock.
    ///
    /// Note: This consumes self. Use `into_terminal()` instead if you need
    /// to get the output back later via `MockTerminalWriter`.
    pub fn into_terminal(self) -> Terminal<MockTerminalWriter> {
        let writer = MockTerminalWriter {
            buffer: self.output,
        };
        let mut terminal = Terminal::new(writer);
        *terminal.capabilities_mut() = self.capabilities;
        terminal
    }

    /// Parse all ANSI sequences from the output buffer.
    #[must_use]
    pub fn parse_sequences(&self) -> Vec<AnsiSequence> {
        AnsiSequenceParser::parse_all(&self.output)
    }

    /// Check if output contains a specific sequence.
    #[must_use]
    pub fn contains_sequence(&self, seq: &AnsiSequence) -> bool {
        self.parse_sequences().contains(seq)
    }

    /// Get all cursor move sequences.
    #[must_use]
    pub fn cursor_moves(&self) -> Vec<(u32, u32)> {
        self.parse_sequences()
            .into_iter()
            .filter_map(|s| match s {
                AnsiSequence::CursorPosition { row, col } => Some((row, col)),
                _ => None,
            })
            .collect()
    }

    /// Get all colors set in the output.
    #[must_use]
    pub fn colors_used(&self) -> Vec<(bool, Rgba)> {
        self.parse_sequences()
            .into_iter()
            .filter_map(|s| match s {
                AnsiSequence::SetFgColor(c) => Some((true, c)),
                AnsiSequence::SetBgColor(c) => Some((false, c)),
                _ => None,
            })
            .collect()
    }

    /// Get all text content written (excluding control sequences).
    #[must_use]
    pub fn text_content(&self) -> String {
        self.parse_sequences()
            .into_iter()
            .filter_map(|s| match s {
                AnsiSequence::Text(t) => Some(t),
                _ => None,
            })
            .collect()
    }
}

impl Write for MockTerminal {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A writer that captures output, extractable after use with Terminal.
#[derive(Debug)]
pub struct MockTerminalWriter {
    buffer: Vec<u8>,
}

impl MockTerminalWriter {
    /// Create a new mock writer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Vec::with_capacity(4096),
        }
    }

    /// Get the captured output.
    #[must_use]
    pub fn output(&self) -> &[u8] {
        &self.buffer
    }

    /// Get output as string.
    #[must_use]
    pub fn output_str(&self) -> String {
        String::from_utf8_lossy(&self.buffer).into_owned()
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }

    /// Parse all sequences.
    #[must_use]
    pub fn parse_sequences(&self) -> Vec<AnsiSequence> {
        AnsiSequenceParser::parse_all(&self.buffer)
    }
}

impl Default for MockTerminalWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for MockTerminalWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Parsed ANSI escape sequence.
#[derive(Clone, Debug, PartialEq)]
pub enum AnsiSequence {
    /// Cursor position (CSI row;col H).
    CursorPosition { row: u32, col: u32 },
    /// Cursor up (CSI n A).
    CursorUp(u32),
    /// Cursor down (CSI n B).
    CursorDown(u32),
    /// Cursor forward (CSI n C).
    CursorForward(u32),
    /// Cursor back (CSI n D).
    CursorBack(u32),
    /// Set foreground color (SGR 38;2;r;g;b or 38;5;n).
    SetFgColor(Rgba),
    /// Set background color (SGR 48;2;r;g;b or 48;5;n).
    SetBgColor(Rgba),
    /// Set text attributes.
    SetAttributes(TextAttributes),
    /// Reset all attributes (SGR 0).
    Reset,
    /// Hide cursor (CSI ?25l).
    HideCursor,
    /// Show cursor (CSI ?25h).
    ShowCursor,
    /// Enter alternate screen (CSI ?1049h).
    EnterAltScreen,
    /// Exit alternate screen (CSI ?1049l).
    ExitAltScreen,
    /// Enable mouse tracking.
    EnableMouse,
    /// Disable mouse tracking.
    DisableMouse,
    /// Clear screen (CSI 2J).
    ClearScreen,
    /// Clear line (CSI 2K).
    ClearLine,
    /// Set window title (OSC 2).
    SetTitle(String),
    /// Hyperlink start (OSC 8).
    HyperlinkStart { id: String, url: String },
    /// Hyperlink end (OSC 8;;).
    HyperlinkEnd,
    /// Plain text content.
    Text(String),
    /// Unrecognized escape sequence.
    Unknown(Vec<u8>),
}

/// Parser for ANSI escape sequences.
struct AnsiSequenceParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> AnsiSequenceParser<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    /// Parse all sequences from a byte buffer.
    fn parse_all(data: &'a [u8]) -> Vec<AnsiSequence> {
        let mut parser = Self::new(data);
        let mut sequences = Vec::new();

        while let Some(seq) = parser.next_sequence() {
            sequences.push(seq);
        }

        sequences
    }

    fn remaining(&self) -> &[u8] {
        &self.data[self.pos..]
    }

    fn peek(&self) -> Option<u8> {
        self.data.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.data.get(self.pos).copied();
        if b.is_some() {
            self.pos += 1;
        }
        b
    }

    fn next_sequence(&mut self) -> Option<AnsiSequence> {
        if self.pos >= self.data.len() {
            return None;
        }

        match self.peek()? {
            0x1b => self.parse_escape(),
            _ => {
                // Collect text until next escape or end
                let start = self.pos;
                while self.pos < self.data.len() && self.data[self.pos] != 0x1b {
                    self.pos += 1;
                }
                let text = String::from_utf8_lossy(&self.data[start..self.pos]).into_owned();
                Some(AnsiSequence::Text(text))
            }
        }
    }

    fn parse_escape(&mut self) -> Option<AnsiSequence> {
        self.advance(); // consume ESC

        match self.peek()? {
            b'[' => self.parse_csi(),
            b']' => self.parse_osc(),
            _ => {
                // Unknown escape sequence - consume one more byte
                self.pos += 1;
                Some(AnsiSequence::Unknown(vec![0x1b]))
            }
        }
    }

    fn parse_csi(&mut self) -> Option<AnsiSequence> {
        self.advance(); // consume '['

        let _start = self.pos;

        // Check for private mode (?)
        let private = self.peek() == Some(b'?');
        if private {
            self.advance();
        }

        // Collect parameters
        let mut params = Vec::new();
        let mut current = 0u32;
        let mut has_digit = false;

        while let Some(b) = self.peek() {
            match b {
                b'0'..=b'9' => {
                    current = current.saturating_mul(10).saturating_add((b - b'0') as u32);
                    has_digit = true;
                    self.advance();
                }
                b';' => {
                    params.push(current);
                    current = 0;
                    has_digit = false;
                    self.advance();
                }
                _ => break,
            }
        }

        if has_digit || !params.is_empty() {
            params.push(current);
        }

        // Get final character
        let final_char = self.advance()?;

        // Interpret the sequence
        if private {
            return self.interpret_private_csi(final_char, &params);
        }

        self.interpret_csi(final_char, &params)
    }

    fn interpret_csi(&mut self, final_char: u8, params: &[u32]) -> Option<AnsiSequence> {
        match final_char {
            b'H' | b'f' => {
                // Cursor position
                let row = params.first().copied().unwrap_or(1);
                let col = params.get(1).copied().unwrap_or(1);
                Some(AnsiSequence::CursorPosition { row, col })
            }
            b'A' => Some(AnsiSequence::CursorUp(params.first().copied().unwrap_or(1))),
            b'B' => Some(AnsiSequence::CursorDown(
                params.first().copied().unwrap_or(1),
            )),
            b'C' => Some(AnsiSequence::CursorForward(
                params.first().copied().unwrap_or(1),
            )),
            b'D' => Some(AnsiSequence::CursorBack(
                params.first().copied().unwrap_or(1),
            )),
            b'J' => {
                if params.first().copied().unwrap_or(0) == 2 {
                    Some(AnsiSequence::ClearScreen)
                } else {
                    Some(AnsiSequence::Unknown(vec![]))
                }
            }
            b'K' => {
                if params.first().copied().unwrap_or(0) == 2 {
                    Some(AnsiSequence::ClearLine)
                } else {
                    Some(AnsiSequence::Unknown(vec![]))
                }
            }
            b'm' => self.interpret_sgr(params),
            _ => Some(AnsiSequence::Unknown(vec![])),
        }
    }

    fn interpret_private_csi(&mut self, final_char: u8, params: &[u32]) -> Option<AnsiSequence> {
        let param = params.first().copied().unwrap_or(0);

        match (param, final_char) {
            (25, b'l') => Some(AnsiSequence::HideCursor),
            (25, b'h') => Some(AnsiSequence::ShowCursor),
            (1049, b'h') => Some(AnsiSequence::EnterAltScreen),
            (1049, b'l') => Some(AnsiSequence::ExitAltScreen),
            (1000, b'h') | (1002, b'h') | (1003, b'h') | (1006, b'h') => {
                Some(AnsiSequence::EnableMouse)
            }
            (1000, b'l') | (1002, b'l') | (1003, b'l') | (1006, b'l') => {
                Some(AnsiSequence::DisableMouse)
            }
            _ => Some(AnsiSequence::Unknown(vec![])),
        }
    }

    fn interpret_sgr(&self, params: &[u32]) -> Option<AnsiSequence> {
        if params.is_empty() || params == [0] {
            return Some(AnsiSequence::Reset);
        }

        let mut i = 0;
        while i < params.len() {
            match params[i] {
                0 => return Some(AnsiSequence::Reset),
                38 => {
                    // Foreground color
                    if params.get(i + 1) == Some(&2) && i + 4 < params.len() {
                        // True color: 38;2;r;g;b
                        let r = params[i + 2] as u8;
                        let g = params[i + 3] as u8;
                        let b = params[i + 4] as u8;
                        return Some(AnsiSequence::SetFgColor(Rgba::from_rgb_u8(r, g, b)));
                    } else if params.get(i + 1) == Some(&5) && i + 2 < params.len() {
                        // 256 color: 38;5;n
                        let idx = params[i + 2] as u8;
                        return Some(AnsiSequence::SetFgColor(Rgba::from_256_color(idx)));
                    }
                }
                48 => {
                    // Background color
                    if params.get(i + 1) == Some(&2) && i + 4 < params.len() {
                        // True color: 48;2;r;g;b
                        let r = params[i + 2] as u8;
                        let g = params[i + 3] as u8;
                        let b = params[i + 4] as u8;
                        return Some(AnsiSequence::SetBgColor(Rgba::from_rgb_u8(r, g, b)));
                    } else if params.get(i + 1) == Some(&5) && i + 2 < params.len() {
                        // 256 color: 48;5;n
                        let idx = params[i + 2] as u8;
                        return Some(AnsiSequence::SetBgColor(Rgba::from_256_color(idx)));
                    }
                }
                1 => return Some(AnsiSequence::SetAttributes(TextAttributes::BOLD)),
                2 => return Some(AnsiSequence::SetAttributes(TextAttributes::DIM)),
                3 => return Some(AnsiSequence::SetAttributes(TextAttributes::ITALIC)),
                4 => return Some(AnsiSequence::SetAttributes(TextAttributes::UNDERLINE)),
                5 => return Some(AnsiSequence::SetAttributes(TextAttributes::BLINK)),
                7 => return Some(AnsiSequence::SetAttributes(TextAttributes::INVERSE)),
                8 => return Some(AnsiSequence::SetAttributes(TextAttributes::HIDDEN)),
                9 => return Some(AnsiSequence::SetAttributes(TextAttributes::STRIKETHROUGH)),
                30..=37 => {
                    // Basic foreground colors
                    let idx = (params[i] - 30) as u8;
                    return Some(AnsiSequence::SetFgColor(basic_color(idx)));
                }
                40..=47 => {
                    // Basic background colors
                    let idx = (params[i] - 40) as u8;
                    return Some(AnsiSequence::SetBgColor(basic_color(idx)));
                }
                90..=97 => {
                    // Bright foreground colors
                    let idx = (params[i] - 90 + 8) as u8;
                    return Some(AnsiSequence::SetFgColor(basic_color(idx)));
                }
                100..=107 => {
                    // Bright background colors
                    let idx = (params[i] - 100 + 8) as u8;
                    return Some(AnsiSequence::SetBgColor(basic_color(idx)));
                }
                _ => {}
            }
            i += 1;
        }

        Some(AnsiSequence::Unknown(vec![]))
    }

    fn parse_osc(&mut self) -> Option<AnsiSequence> {
        self.advance(); // consume ']'

        // Collect until ST (ESC \ or BEL)
        let start = self.pos;
        while self.pos < self.data.len() {
            if self.data[self.pos] == 0x07 {
                // BEL terminator
                let content = &self.data[start..self.pos];
                self.pos += 1;
                return self.interpret_osc(content);
            }
            if self.pos + 1 < self.data.len()
                && self.data[self.pos] == 0x1b
                && self.data[self.pos + 1] == b'\\'
            {
                // ST terminator
                let content = &self.data[start..self.pos];
                self.pos += 2;
                return self.interpret_osc(content);
            }
            self.pos += 1;
        }

        Some(AnsiSequence::Unknown(self.data[start - 2..].to_vec()))
    }

    fn interpret_osc(&self, content: &[u8]) -> Option<AnsiSequence> {
        let s = String::from_utf8_lossy(content);

        if let Some(rest) = s.strip_prefix("2;") {
            // Window title
            return Some(AnsiSequence::SetTitle(rest.to_string()));
        }

        if let Some(rest) = s.strip_prefix("8;") {
            // Hyperlink
            let parts: Vec<&str> = rest.splitn(2, ';').collect();
            if parts.len() == 2 {
                let params = parts[0];
                let url = parts[1];

                if url.is_empty() {
                    return Some(AnsiSequence::HyperlinkEnd);
                }

                // Extract id from params (id=xxx)
                let id = params
                    .split(':')
                    .find_map(|p| p.strip_prefix("id="))
                    .unwrap_or("")
                    .to_string();

                return Some(AnsiSequence::HyperlinkStart {
                    id,
                    url: url.to_string(),
                });
            }
        }

        Some(AnsiSequence::Unknown(content.to_vec()))
    }
}

/// Convert basic ANSI color index to Rgba.
fn basic_color(idx: u8) -> Rgba {
    match idx {
        0 => Rgba::BLACK,
        1 => Rgba::from_rgb_u8(128, 0, 0),     // Red
        2 => Rgba::from_rgb_u8(0, 128, 0),     // Green
        3 => Rgba::from_rgb_u8(128, 128, 0),   // Yellow
        4 => Rgba::from_rgb_u8(0, 0, 128),     // Blue
        5 => Rgba::from_rgb_u8(128, 0, 128),   // Magenta
        6 => Rgba::from_rgb_u8(0, 128, 128),   // Cyan
        7 => Rgba::from_rgb_u8(192, 192, 192), // White
        8 => Rgba::from_rgb_u8(128, 128, 128), // Bright Black
        9 => Rgba::RED,                        // Bright Red
        10 => Rgba::GREEN,                     // Bright Green
        11 => Rgba::from_rgb_u8(255, 255, 0),  // Bright Yellow
        12 => Rgba::BLUE,                      // Bright Blue
        13 => Rgba::from_rgb_u8(255, 0, 255),  // Bright Magenta
        14 => Rgba::from_rgb_u8(0, 255, 255),  // Bright Cyan
        15 => Rgba::WHITE,                     // Bright White
        _ => Rgba::WHITE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_terminal_basic() {
        let mut mock = MockTerminal::new(80, 24);
        write!(mock, "Hello").unwrap();
        assert_eq!(mock.output_str(), "Hello");
    }

    #[test]
    fn test_parse_cursor_position() {
        let data = b"\x1b[5;10H";
        let sequences = AnsiSequenceParser::parse_all(data);
        assert_eq!(sequences.len(), 1);
        assert_eq!(
            sequences[0],
            AnsiSequence::CursorPosition { row: 5, col: 10 }
        );
    }

    #[test]
    fn test_parse_true_color() {
        let data = b"\x1b[38;2;255;128;64m";
        let sequences = AnsiSequenceParser::parse_all(data);
        assert_eq!(sequences.len(), 1);
        assert_eq!(
            sequences[0],
            AnsiSequence::SetFgColor(Rgba::from_rgb_u8(255, 128, 64))
        );
    }

    #[test]
    fn test_parse_256_color() {
        let data = b"\x1b[48;5;196m";
        let sequences = AnsiSequenceParser::parse_all(data);
        assert_eq!(sequences.len(), 1);
        assert!(
            matches!(sequences[0], AnsiSequence::SetBgColor(_)),
            "Expected SetBgColor"
        );
    }

    #[test]
    fn test_parse_mixed_content() {
        let data = b"\x1b[?25lHello\x1b[38;2;255;0;0mWorld\x1b[0m";
        let sequences = AnsiSequenceParser::parse_all(data);

        assert!(sequences.contains(&AnsiSequence::HideCursor));
        assert!(
            sequences
                .iter()
                .any(|s| matches!(s, AnsiSequence::Text(t) if t == "Hello"))
        );
        assert!(
            sequences
                .iter()
                .any(|s| matches!(s, AnsiSequence::Text(t) if t == "World"))
        );
        assert!(sequences.contains(&AnsiSequence::SetFgColor(Rgba::RED)));
        assert!(sequences.contains(&AnsiSequence::Reset));
    }

    #[test]
    fn test_parse_alt_screen() {
        let data = b"\x1b[?1049h\x1b[?1049l";
        let sequences = AnsiSequenceParser::parse_all(data);

        assert_eq!(sequences.len(), 2);
        assert_eq!(sequences[0], AnsiSequence::EnterAltScreen);
        assert_eq!(sequences[1], AnsiSequence::ExitAltScreen);
    }

    #[test]
    fn test_parse_attributes() {
        let data = b"\x1b[1m\x1b[3m\x1b[4m";
        let sequences = AnsiSequenceParser::parse_all(data);

        assert!(sequences.contains(&AnsiSequence::SetAttributes(TextAttributes::BOLD)));
        assert!(sequences.contains(&AnsiSequence::SetAttributes(TextAttributes::ITALIC)));
        assert!(sequences.contains(&AnsiSequence::SetAttributes(TextAttributes::UNDERLINE)));
    }

    #[test]
    fn test_text_content_extraction() {
        let mut mock = MockTerminal::new(80, 24);
        write!(mock, "\x1b[1mBold\x1b[0m Normal").unwrap();

        let text = mock.text_content();
        assert_eq!(text, "Bold Normal");
    }

    #[test]
    fn test_cursor_moves_extraction() {
        let mut mock = MockTerminal::new(80, 24);
        write!(mock, "\x1b[1;1H\x1b[5;10H\x1b[10;20H").unwrap();

        let moves = mock.cursor_moves();
        assert_eq!(moves, vec![(1, 1), (5, 10), (10, 20)]);
    }

    #[test]
    fn test_colors_extraction() {
        let mut mock = MockTerminal::new(80, 24);
        write!(mock, "\x1b[38;2;255;0;0m\x1b[48;2;0;255;0m").unwrap();

        let colors = mock.colors_used();
        assert_eq!(colors.len(), 2);
        assert!(colors.iter().any(|(is_fg, c)| *is_fg && *c == Rgba::RED));
        assert!(colors.iter().any(|(is_fg, c)| !*is_fg && *c == Rgba::GREEN));
    }

    #[test]
    fn test_terminal_integration() {
        use std::cell::RefCell;
        use std::rc::Rc;

        // Use shared buffer to capture output
        let buffer = Rc::new(RefCell::new(Vec::new()));
        let writer = SharedWriter(Rc::clone(&buffer));

        let mut terminal = Terminal::new(writer);

        terminal.hide_cursor().unwrap();
        terminal.enter_alt_screen().unwrap();

        let output = buffer.borrow();
        let sequences = AnsiSequenceParser::parse_all(&output);
        assert!(sequences.contains(&AnsiSequence::HideCursor));
        assert!(sequences.contains(&AnsiSequence::EnterAltScreen));
    }
}

/// A writer that writes to a shared buffer for testing.
#[derive(Clone)]
struct SharedWriter(std::rc::Rc<std::cell::RefCell<Vec<u8>>>);

impl std::io::Write for SharedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
