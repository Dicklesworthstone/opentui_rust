//! ANSI sequence parser for terminal input.
//!
//! Parses raw bytes from the terminal into structured events. Supports:
//! - Standard VT sequences (arrows, function keys)
//! - CSI sequences with modifiers
//! - SGR mouse encoding (1006)
//! - Legacy X10/X11 mouse encoding
//! - Bracketed paste mode
//! - Focus events

// Parser has many match arms for different terminal sequences
#![allow(clippy::match_same_arms)]
// Self is used for consistency with other methods even when not needed
#![allow(clippy::unused_self)]
// Result wrapping is for consistency in the parsing API
#![allow(clippy::unnecessary_wraps)]
// Mutable reference needed for future state handling
#![allow(clippy::needless_pass_by_ref_mut)]

use crate::input::event::{Event, PasteEvent, ResizeEvent};
use crate::input::keyboard::{KeyCode, KeyEvent, KeyModifiers};
use crate::terminal::{MouseButton, MouseEvent, MouseEventKind};

/// Error type for input parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Input buffer is empty.
    Empty,
    /// Incomplete escape sequence (need more bytes).
    Incomplete,
    /// Unrecognized escape sequence.
    UnrecognizedSequence(Vec<u8>),
    /// Invalid UTF-8 in input.
    InvalidUtf8,
    /// Paste buffer exceeded maximum size limit.
    ///
    /// The paste operation was aborted because the incoming paste data
    /// exceeded [`MAX_PASTE_BUFFER_SIZE`] (10 MB). This prevents unbounded
    /// memory growth from malformed or malicious input.
    PasteBufferOverflow,
    /// Invalid resize event format.
    ///
    /// The resize sequence (CSI 8;height;width t) contained non-numeric
    /// values for width or height.
    InvalidResizeFormat,
}

/// Result of parsing input.
pub type ParseResult = Result<(Event, usize), ParseError>;

/// Maximum size for paste buffer to prevent unbounded memory growth (10 MB).
const MAX_PASTE_BUFFER_SIZE: usize = 10 * 1024 * 1024;

/// Parser state for multi-byte sequences.
#[derive(Clone, Debug, Default)]
pub struct InputParser {
    /// Whether we're in bracketed paste mode.
    in_paste: bool,
    /// Accumulated paste content.
    paste_buffer: Vec<u8>,
}

impl InputParser {
    /// Create a new input parser.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse bytes into an event.
    ///
    /// Returns the event and number of bytes consumed, or an error.
    /// Call repeatedly with the same buffer until `Err(ParseError::Empty)`
    /// or `Err(ParseError::Incomplete)` is returned.
    pub fn parse(&mut self, input: &[u8]) -> ParseResult {
        if input.is_empty() {
            return Err(ParseError::Empty);
        }

        // Handle bracketed paste mode
        if self.in_paste {
            return self.parse_paste(input);
        }

        let first = input[0];

        match first {
            // Escape sequence
            0x1b => self.parse_escape(input),
            // Control characters
            0x00 => Ok((KeyEvent::key(KeyCode::Null).into(), 1)),
            0x01..=0x1a => {
                // Ctrl+A through Ctrl+Z
                let c = (first - 1 + b'a') as char;
                Ok((
                    KeyEvent::new(KeyCode::Char(c), KeyModifiers::CTRL).into(),
                    1,
                ))
            }
            0x7f => Ok((KeyEvent::key(KeyCode::Backspace).into(), 1)),
            // Regular characters (ASCII)
            0x20..=0x7e => Ok((KeyEvent::char(first as char).into(), 1)),
            // UTF-8 sequences
            0x80..=0xff => self.parse_utf8(input),
            _ => Ok((KeyEvent::char(first as char).into(), 1)),
        }
    }

    /// Parse an escape sequence.
    fn parse_escape(&mut self, input: &[u8]) -> ParseResult {
        if input.len() == 1 {
            // Could be just Escape or start of sequence
            return Err(ParseError::Incomplete);
        }

        match input[1] {
            // CSI sequence: ESC [
            b'[' => self.parse_csi(input),
            // SS3 sequence: ESC O (alternate function keys)
            b'O' => self.parse_ss3(input),
            // DCS sequence: ESC P (Device Control String)
            b'P' => self.parse_dcs(input),
            // Alt+key: ESC <char>
            0x20..=0x7e => {
                let c = input[1] as char;
                Ok((KeyEvent::new(KeyCode::Char(c), KeyModifiers::ALT).into(), 2))
            }
            // Double escape
            0x1b => Ok((KeyEvent::key(KeyCode::Esc).into(), 1)),
            _ => Ok((KeyEvent::key(KeyCode::Esc).into(), 1)),
        }
    }

    /// Parse a CSI sequence (ESC [ ...).
    fn parse_csi(&mut self, input: &[u8]) -> ParseResult {
        if input.len() < 3 {
            return Err(ParseError::Incomplete);
        }

        // Find the final byte (0x40-0x7e)
        let mut end = 2;
        while end < input.len() {
            let b = input[end];
            if (0x40..=0x7e).contains(&b) {
                break;
            }
            end += 1;
        }

        if end >= input.len() {
            return Err(ParseError::Incomplete);
        }

        let final_byte = input[end];
        let params = &input[2..end];

        match final_byte {
            // Arrow keys and navigation
            b'A' => self.parse_modified_key(params, KeyCode::Up, end + 1),
            b'B' => self.parse_modified_key(params, KeyCode::Down, end + 1),
            b'C' => self.parse_modified_key(params, KeyCode::Right, end + 1),
            b'D' => self.parse_modified_key(params, KeyCode::Left, end + 1),
            b'H' => self.parse_modified_key(params, KeyCode::Home, end + 1),
            b'F' => self.parse_modified_key(params, KeyCode::End, end + 1),
            b'E' => self.parse_modified_key(params, KeyCode::KeypadBegin, end + 1),

            // Tilde sequences: ESC [ <number> ~
            b'~' => self.parse_tilde_key(params, end + 1),

            // Mouse events
            b'M' => {
                // Distinguish SGR (<prefix) from X11 mouse
                if params.first() == Some(&b'<') {
                    self.parse_sgr_mouse(input)
                } else {
                    self.parse_x11_mouse(input, end + 1)
                }
            }
            b'm' => self.parse_sgr_mouse(input),

            // Focus events
            b'I' => Ok((Event::FocusGained, end + 1)),
            b'O' => Ok((Event::FocusLost, end + 1)),

            // Resize (some terminals)
            b't' => self.parse_resize(params, end + 1),

            _ => Err(ParseError::UnrecognizedSequence(input[..=end].to_vec())),
        }
    }

    /// Parse DCS sequence (ESC P ... ST).
    fn parse_dcs(&self, input: &[u8]) -> ParseResult {
        // Search for ST (String Terminator)
        // ST can be ESC \ (0x1b 0x5c) or 0x9c
        let mut i = 2; // Skip ESC P
        while i < input.len() {
            match input[i] {
                0x1b => {
                    // Check for ESC \
                    if i + 1 < input.len() {
                        if input[i + 1] == b'\\' {
                            // Found ESC \
                            return Err(ParseError::UnrecognizedSequence(input[..=i + 1].to_vec()));
                        }
                        // Other escape sequence inside DCS? Should not happen in valid DCS but possible in noise.
                        // Continue searching.
                    } else {
                        // ESC at end of buffer, might be start of ST
                        return Err(ParseError::Incomplete);
                    }
                }
                0x9c => {
                    // Found 8-bit ST
                    return Err(ParseError::UnrecognizedSequence(input[..=i].to_vec()));
                }
                _ => {}
            }
            i += 1;
        }

        Err(ParseError::Incomplete)
    }

    /// Parse a key with modifiers from CSI params.
    fn parse_modified_key(&self, params: &[u8], base_key: KeyCode, consumed: usize) -> ParseResult {
        let modifiers = if params.is_empty() {
            KeyModifiers::empty()
        } else {
            self.parse_modifiers(params)?
        };
        Ok((KeyEvent::new(base_key, modifiers).into(), consumed))
    }

    /// Parse modifiers from CSI parameter bytes.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::InvalidUtf8`] if the parameter bytes are not valid UTF-8.
    fn parse_modifiers(&self, params: &[u8]) -> Result<KeyModifiers, ParseError> {
        // Format: 1;N where N encodes modifiers
        // N = 1 + (shift ? 1 : 0) + (alt ? 2 : 0) + (ctrl ? 4 : 0)
        let s = std::str::from_utf8(params).map_err(|_| ParseError::InvalidUtf8)?;
        let parts: Vec<&str> = s.split(';').collect();
        if parts.len() >= 2 {
            if let Ok(n) = parts[1].parse::<u8>() {
                let n = n.saturating_sub(1);
                let mut mods = KeyModifiers::empty();
                if n & 1 != 0 {
                    mods |= KeyModifiers::SHIFT;
                }
                if n & 2 != 0 {
                    mods |= KeyModifiers::ALT;
                }
                if n & 4 != 0 {
                    mods |= KeyModifiers::CTRL;
                }
                return Ok(mods);
            }
        }
        Ok(KeyModifiers::empty())
    }

    /// Parse tilde key sequences (Insert, Delete, Page Up/Down, F5+).
    fn parse_tilde_key(&mut self, params: &[u8], consumed: usize) -> ParseResult {
        let s = std::str::from_utf8(params).map_err(|_| ParseError::InvalidUtf8)?;
        let parts: Vec<&str> = s.split(';').collect();
        let num: u8 = parts.first().and_then(|p| p.parse().ok()).unwrap_or(0);

        let modifiers = if parts.len() >= 2 {
            self.parse_modifiers(params)?
        } else {
            KeyModifiers::empty()
        };

        let code = match num {
            1 => KeyCode::Home,
            2 => KeyCode::Insert,
            3 => KeyCode::Delete,
            4 => KeyCode::End,
            5 => KeyCode::PageUp,
            6 => KeyCode::PageDown,
            7 => KeyCode::Home,
            8 => KeyCode::End,
            11 => KeyCode::F(1),
            12 => KeyCode::F(2),
            13 => KeyCode::F(3),
            14 => KeyCode::F(4),
            15 => KeyCode::F(5),
            17 => KeyCode::F(6),
            18 => KeyCode::F(7),
            19 => KeyCode::F(8),
            20 => KeyCode::F(9),
            21 => KeyCode::F(10),
            23 => KeyCode::F(11),
            24 => KeyCode::F(12),
            25 => KeyCode::F(13),
            26 => KeyCode::F(14),
            28 => KeyCode::F(15),
            29 => KeyCode::F(16),
            31 => KeyCode::F(17),
            32 => KeyCode::F(18),
            33 => KeyCode::F(19),
            34 => KeyCode::F(20),
            200 => {
                // Bracketed paste start - enter paste mode
                self.in_paste = true;
                return Err(ParseError::Incomplete);
            }
            201 => {
                // Bracketed paste end - shouldn't happen here
                return Err(ParseError::UnrecognizedSequence(params.to_vec()));
            }
            _ => return Err(ParseError::UnrecognizedSequence(params.to_vec())),
        };

        Ok((KeyEvent::new(code, modifiers).into(), consumed))
    }

    /// Parse SS3 sequences (ESC O ...).
    fn parse_ss3(&mut self, input: &[u8]) -> ParseResult {
        if input.len() < 3 {
            return Err(ParseError::Incomplete);
        }

        let code = match input[2] {
            b'P' => KeyCode::F(1),
            b'Q' => KeyCode::F(2),
            b'R' => KeyCode::F(3),
            b'S' => KeyCode::F(4),
            b'A' => KeyCode::Up,
            b'B' => KeyCode::Down,
            b'C' => KeyCode::Right,
            b'D' => KeyCode::Left,
            b'H' => KeyCode::Home,
            b'F' => KeyCode::End,
            b'M' => KeyCode::Enter,
            _ => return Err(ParseError::UnrecognizedSequence(input[..3].to_vec())),
        };

        Ok((KeyEvent::key(code).into(), 3))
    }

    /// Parse X11 mouse encoding (ESC [ M <button+mods> <x+33> <y+33>).
    ///
    /// X11 encoding adds 32 to avoid control characters, and coordinates are 1-indexed.
    /// So we subtract 33 (32 + 1) to get 0-indexed coordinates matching SGR output.
    fn parse_x11_mouse(&self, input: &[u8], start: usize) -> ParseResult {
        if input.len() < start + 3 {
            return Err(ParseError::Incomplete);
        }

        let cb = input[start];
        let cx = input[start + 1].saturating_sub(33);
        let cy = input[start + 2].saturating_sub(33);

        let (button, kind) = decode_x11_button(cb);
        let (shift, alt, ctrl) = decode_x11_modifiers(cb);

        let event = MouseEvent::new(u32::from(cx), u32::from(cy), button, kind)
            .with_modifiers(shift, ctrl, alt);

        Ok((Event::Mouse(event), start + 3))
    }

    /// Parse SGR mouse encoding (ESC [ < Pb ; Px ; Py M/m).
    fn parse_sgr_mouse(&self, input: &[u8]) -> ParseResult {
        // Find 'M' or 'm' terminator
        let term_pos = input.iter().position(|&b| b == b'M' || b == b'm');
        let Some(term_pos) = term_pos else {
            return Err(ParseError::Incomplete);
        };

        let is_release = input[term_pos] == b'm';

        // Parse parameters: ESC [ < Pb ; Px ; Py [Mm]
        // Start after "ESC [ <" (positions 0, 1, 2)
        let params_start = if input.len() > 2 && input[2] == b'<' {
            3
        } else {
            2
        };
        let params = &input[params_start..term_pos];

        let s = std::str::from_utf8(params).map_err(|_| ParseError::InvalidUtf8)?;
        let parts: Vec<&str> = s.split(';').collect();

        if parts.len() < 3 {
            return Err(ParseError::UnrecognizedSequence(
                input[..=term_pos].to_vec(),
            ));
        }

        let cb: u8 = parts[0].parse().unwrap_or(0);
        let cx: u32 = parts[1].parse::<u32>().unwrap_or(1).saturating_sub(1);
        let cy: u32 = parts[2].parse::<u32>().unwrap_or(1).saturating_sub(1);

        let (button, mut kind) = decode_sgr_button(cb);
        if is_release {
            kind = MouseEventKind::Release;
        }
        let (shift, alt, ctrl) = decode_sgr_modifiers(cb);

        let event = MouseEvent::new(cx, cy, button, kind).with_modifiers(shift, ctrl, alt);

        Ok((Event::Mouse(event), term_pos + 1))
    }

    /// Parse resize sequence (CSI 8 ; height ; width t).
    ///
    /// Only handles XTWINOPS format. Other formats (e.g., CSI 4 for pixel size)
    /// are returned as unrecognized.
    fn parse_resize(&self, params: &[u8], consumed: usize) -> ParseResult {
        let s = std::str::from_utf8(params).map_err(|_| ParseError::InvalidUtf8)?;
        let parts: Vec<&str> = s.split(';').collect();

        if parts.len() >= 3 && parts[0] == "8" {
            // Parse height and width, returning error on invalid values
            // rather than falling back to arbitrary defaults
            let height: u16 = parts[1]
                .parse()
                .map_err(|_| ParseError::InvalidResizeFormat)?;
            let width: u16 = parts[2]
                .parse()
                .map_err(|_| ParseError::InvalidResizeFormat)?;
            Ok((Event::Resize(ResizeEvent::new(width, height)), consumed))
        } else {
            Err(ParseError::UnrecognizedSequence(params.to_vec()))
        }
    }

    /// Parse bracketed paste content.
    ///
    /// Note: Paste buffer is limited to [`MAX_PASTE_BUFFER_SIZE`] to prevent
    /// unbounded memory growth from malformed or malicious input.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError::PasteBufferOverflow`] if the paste data would
    /// exceed the maximum buffer size. The parser state is reset when this
    /// occurs.
    fn parse_paste(&mut self, input: &[u8]) -> ParseResult {
        // Start and end sequences for bracketed paste
        const START_SEQ: &[u8] = b"\x1b[200~";
        const END_SEQ: &[u8] = b"\x1b[201~";

        // Skip start sequence if present at beginning
        // (can happen when retrying after Incomplete or when full sequence arrives at once)
        let content_start = if input.starts_with(START_SEQ) {
            START_SEQ.len()
        } else {
            0
        };
        let effective_input = &input[content_start..];

        if let Some(pos) = find_subsequence(effective_input, END_SEQ) {
            // Check if adding this content would exceed the limit
            let available = MAX_PASTE_BUFFER_SIZE.saturating_sub(self.paste_buffer.len());
            if pos > available {
                // Paste would overflow - reset state and return error
                self.in_paste = false;
                self.paste_buffer.clear();
                return Err(ParseError::PasteBufferOverflow);
            }

            self.paste_buffer.extend_from_slice(&effective_input[..pos]);
            self.in_paste = false;

            let content = String::from_utf8_lossy(&self.paste_buffer).into_owned();
            self.paste_buffer.clear();

            Ok((
                Event::Paste(PasteEvent::new(content)),
                content_start + pos + END_SEQ.len(),
            ))
        } else {
            // Check if adding this content would exceed the limit
            let available = MAX_PASTE_BUFFER_SIZE.saturating_sub(self.paste_buffer.len());
            if effective_input.len() > available {
                // Paste would overflow - reset state and return error
                self.in_paste = false;
                self.paste_buffer.clear();
                return Err(ParseError::PasteBufferOverflow);
            }

            self.paste_buffer.extend_from_slice(effective_input);
            Err(ParseError::Incomplete)
        }
    }

    /// Parse a UTF-8 character sequence.
    fn parse_utf8(&self, input: &[u8]) -> ParseResult {
        let first = input[0];

        // Determine expected byte length
        let expected_len = if first & 0b1110_0000 == 0b1100_0000 {
            2
        } else if first & 0b1111_0000 == 0b1110_0000 {
            3
        } else if first & 0b1111_1000 == 0b1111_0000 {
            4
        } else {
            return Err(ParseError::InvalidUtf8);
        };

        if input.len() < expected_len {
            return Err(ParseError::Incomplete);
        }

        let s = std::str::from_utf8(&input[..expected_len]).map_err(|_| ParseError::InvalidUtf8)?;
        let c = s.chars().next().ok_or(ParseError::InvalidUtf8)?;

        Ok((KeyEvent::char(c).into(), expected_len))
    }

    /// Clear any buffered state.
    pub fn clear(&mut self) {
        self.in_paste = false;
        self.paste_buffer.clear();
    }
}

/// Decode X11 mouse button and event kind from button byte.
fn decode_x11_button(cb: u8) -> (MouseButton, MouseEventKind) {
    let low = cb & 0b0000_0011;
    let motion = cb & 0b0010_0000 != 0;
    let scroll = cb & 0b0100_0000 != 0;

    if scroll {
        let kind = match low {
            0 => MouseEventKind::ScrollUp,
            1 => MouseEventKind::ScrollDown,
            2 => MouseEventKind::ScrollLeft,
            3 => MouseEventKind::ScrollRight,
            _ => MouseEventKind::ScrollUp,
        };
        (MouseButton::None, kind)
    } else if motion {
        let button = match low {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::None,
        };
        (button, MouseEventKind::Move)
    } else {
        let button = match low {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            3 => return (MouseButton::None, MouseEventKind::Release),
            _ => MouseButton::None,
        };
        (button, MouseEventKind::Press)
    }
}

/// Decode X11 mouse modifiers from button byte.
fn decode_x11_modifiers(cb: u8) -> (bool, bool, bool) {
    let shift = cb & 0b0000_0100 != 0;
    let alt = cb & 0b0000_1000 != 0;
    let ctrl = cb & 0b0001_0000 != 0;
    (shift, alt, ctrl)
}

/// Decode SGR mouse button and event kind.
fn decode_sgr_button(cb: u8) -> (MouseButton, MouseEventKind) {
    let low = cb & 0b0000_0011;
    let motion = cb & 0b0010_0000 != 0;
    let scroll = cb & 0b0100_0000 != 0;

    if scroll {
        let kind = match low {
            0 => MouseEventKind::ScrollUp,
            1 => MouseEventKind::ScrollDown,
            2 => MouseEventKind::ScrollLeft,
            3 => MouseEventKind::ScrollRight,
            _ => MouseEventKind::ScrollUp,
        };
        (MouseButton::None, kind)
    } else if motion {
        let button = match low {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::None,
        };
        (button, MouseEventKind::Move)
    } else {
        let button = match low {
            0 => MouseButton::Left,
            1 => MouseButton::Middle,
            2 => MouseButton::Right,
            _ => MouseButton::None,
        };
        (button, MouseEventKind::Press)
    }
}

/// Decode SGR mouse modifiers.
fn decode_sgr_modifiers(cb: u8) -> (bool, bool, bool) {
    let shift = cb & 0b0000_0100 != 0;
    let alt = cb & 0b0000_1000 != 0;
    let ctrl = cb & 0b0001_0000 != 0;
    (shift, alt, ctrl)
}

/// Find a subsequence in a slice.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
#[allow(clippy::uninlined_format_args)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_char() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse(b"a").unwrap();
        assert_eq!(consumed, 1);
        assert_eq!(event, Event::Key(KeyEvent::char('a')));
    }

    #[test]
    fn test_parse_ctrl_c() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse(&[0x03]).unwrap();
        assert_eq!(consumed, 1);
        let key = event.key().unwrap();
        assert!(key.is_ctrl_c());
    }

    #[test]
    fn test_parse_escape() {
        let mut parser = InputParser::new();
        let result = parser.parse(b"\x1b");
        assert_eq!(result, Err(ParseError::Incomplete));
    }

    #[test]
    fn test_parse_arrow_up() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse(b"\x1b[A").unwrap();
        assert_eq!(consumed, 3);
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Up);
    }

    #[test]
    fn test_parse_arrow_with_modifiers() {
        let mut parser = InputParser::new();
        // Shift+Up: ESC [ 1 ; 2 A
        let (event, _) = parser.parse(b"\x1b[1;2A").unwrap();
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Up);
        assert!(key.shift());
    }

    #[test]
    fn test_parse_f1() {
        let mut parser = InputParser::new();
        let (event, _) = parser.parse(b"\x1bOP").unwrap();
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::F(1));
    }

    #[test]
    fn test_parse_delete() {
        let mut parser = InputParser::new();
        let (event, _) = parser.parse(b"\x1b[3~").unwrap();
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Delete);
    }

    #[test]
    fn test_parse_alt_key() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse(b"\x1bx").unwrap();
        assert_eq!(consumed, 2);
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Char('x'));
        assert!(key.alt());
    }

    #[test]
    fn test_parse_sgr_mouse() {
        let mut parser = InputParser::new();
        // Left click at (10, 5): ESC [ < 0 ; 11 ; 6 M
        let (event, _) = parser.parse(b"\x1b[<0;11;6M").unwrap();
        let mouse = event.mouse().unwrap();
        assert_eq!(mouse.x, 10);
        assert_eq!(mouse.y, 5);
        assert_eq!(mouse.button, MouseButton::Left);
        assert_eq!(mouse.kind, MouseEventKind::Press);
    }

    #[test]
    fn test_parse_sgr_mouse_release() {
        let mut parser = InputParser::new();
        // Left release at (10, 5): ESC [ < 0 ; 11 ; 6 m
        let (event, _) = parser.parse(b"\x1b[<0;11;6m").unwrap();
        let mouse = event.mouse().unwrap();
        assert_eq!(mouse.kind, MouseEventKind::Release);
    }

    #[test]
    fn test_parse_utf8() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse("日".as_bytes()).unwrap();
        assert_eq!(consumed, 3);
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Char('日'));
    }

    #[test]
    fn test_parse_focus() {
        let mut parser = InputParser::new();
        let (event, _) = parser.parse(b"\x1b[I").unwrap();
        assert_eq!(event, Event::FocusGained);

        let (event, _) = parser.parse(b"\x1b[O").unwrap();
        assert_eq!(event, Event::FocusLost);
    }

    #[test]
    fn test_parse_backspace() {
        let mut parser = InputParser::new();
        let (event, _) = parser.parse(&[0x7f]).unwrap();
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Backspace);
    }

    #[test]
    fn test_bracketed_paste_simple() {
        eprintln!("[TEST] test_bracketed_paste_simple: Testing basic paste flow");
        let mut parser = InputParser::new();

        // Send paste start sequence: ESC [ 200 ~
        eprintln!("[TEST] Sending paste start sequence ESC[200~");
        let result = parser.parse(b"\x1b[200~");
        eprintln!("[TEST] Result after paste start: {result:?}");
        assert_eq!(
            result,
            Err(ParseError::Incomplete),
            "Paste start should return Incomplete"
        );
        assert!(parser.in_paste, "Parser should enter paste mode");

        // Send paste content with end sequence: hello ESC [ 201 ~
        eprintln!("[TEST] Sending paste content 'hello' with end sequence");
        let (event, consumed) = parser.parse(b"hello\x1b[201~").unwrap();
        eprintln!("[TEST] Consumed {consumed} bytes, event: {event:?}");
        assert_eq!(consumed, 11); // 5 for "hello" + 6 for end sequence
        let paste = event.paste().expect("Should be a paste event");
        assert_eq!(paste.content, "hello");
        assert!(!parser.in_paste, "Parser should exit paste mode");
        eprintln!("[TEST] SUCCESS: Basic paste flow works correctly");
    }

    #[test]
    fn test_bracketed_paste_multiline() {
        eprintln!("[TEST] test_bracketed_paste_multiline: Testing multiline paste");
        let mut parser = InputParser::new();

        // Start paste mode
        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Multi-line content
        let content = b"line1\nline2\nline3\x1b[201~";
        eprintln!(
            "[TEST] Sending multiline content: {:?}",
            String::from_utf8_lossy(&content[..15])
        );
        let (event, _) = parser.parse(content).unwrap();
        let paste = event.paste().expect("Should be a paste event");
        eprintln!("[TEST] Received paste content: {:?}", paste.content);
        assert_eq!(paste.content, "line1\nline2\nline3");
        eprintln!("[TEST] SUCCESS: Multiline paste works correctly");
    }

    #[test]
    fn test_bracketed_paste_with_escapes() {
        eprintln!("[TEST] test_bracketed_paste_with_escapes: Testing paste with embedded escapes");
        let mut parser = InputParser::new();

        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Paste content containing escape characters (but not the end sequence)
        let content = b"text with \x1b escape\x1b[201~";
        eprintln!("[TEST] Sending content with embedded escape byte");
        let (event, _) = parser.parse(content).unwrap();
        let paste = event.paste().expect("Should be a paste event");
        eprintln!("[TEST] Received paste content: {:?}", paste.content);
        assert!(paste.content.contains('\x1b'));
        assert_eq!(paste.content, "text with \x1b escape");
        eprintln!("[TEST] SUCCESS: Paste with embedded escapes works correctly");
    }

    #[test]
    fn test_bracketed_paste_chunked() {
        eprintln!("[TEST] test_bracketed_paste_chunked: Testing chunked paste delivery");
        let mut parser = InputParser::new();

        // Start paste
        eprintln!("[TEST] Sending paste start");
        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // First chunk (no end sequence)
        eprintln!("[TEST] Sending first chunk 'hello '");
        let result = parser.parse(b"hello ");
        eprintln!("[TEST] First chunk result: {result:?}");
        assert_eq!(result, Err(ParseError::Incomplete));
        assert!(parser.in_paste, "Should still be in paste mode");

        // Second chunk with end sequence
        eprintln!("[TEST] Sending second chunk 'world' with end sequence");
        let (event, _) = parser.parse(b"world\x1b[201~").unwrap();
        let paste = event.paste().expect("Should be a paste event");
        eprintln!("[TEST] Final paste content: {:?}", paste.content);
        assert_eq!(paste.content, "hello world");
        eprintln!("[TEST] SUCCESS: Chunked paste works correctly");
    }

    #[test]
    fn test_bracketed_paste_empty() {
        eprintln!("[TEST] test_bracketed_paste_empty: Testing empty paste");
        let mut parser = InputParser::new();

        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Immediate end sequence (empty paste)
        eprintln!("[TEST] Sending immediate end sequence (empty paste)");
        let (event, _) = parser.parse(b"\x1b[201~").unwrap();
        let paste = event.paste().expect("Should be a paste event");
        eprintln!("[TEST] Paste content: {:?}", paste.content);
        assert_eq!(paste.content, "");
        assert!(!parser.in_paste);
        eprintln!("[TEST] SUCCESS: Empty paste works correctly");
    }

    #[test]
    fn test_bracketed_paste_with_unicode() {
        eprintln!("[TEST] test_bracketed_paste_with_unicode: Testing unicode paste");
        let mut parser = InputParser::new();

        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Unicode content
        let content = "日本語テスト🎉\x1b[201~".as_bytes();
        eprintln!("[TEST] Sending unicode content");
        let (event, _) = parser.parse(content).unwrap();
        let paste = event.paste().expect("Should be a paste event");
        eprintln!("[TEST] Paste content: {:?}", paste.content);
        assert_eq!(paste.content, "日本語テスト🎉");
        eprintln!("[TEST] SUCCESS: Unicode paste works correctly");
    }

    #[test]
    fn test_bracketed_paste_clear_resets_state() {
        eprintln!(
            "[TEST] test_bracketed_paste_clear_resets_state: Testing clear() resets paste state"
        );
        let mut parser = InputParser::new();

        // Enter paste mode
        let _ = parser.parse(b"\x1b[200~");
        let _ = parser.parse(b"partial content");
        assert!(parser.in_paste);

        // Clear should reset state
        eprintln!("[TEST] Calling clear()");
        parser.clear();
        assert!(!parser.in_paste, "clear() should exit paste mode");
        eprintln!("[TEST] SUCCESS: clear() properly resets paste state");
    }

    #[test]
    fn test_paste_end_without_start() {
        eprintln!("[TEST] test_paste_end_without_start: Testing paste end without start");
        let mut parser = InputParser::new();

        // Paste end sequence without being in paste mode (CSI 201~)
        eprintln!("[TEST] Sending paste end without being in paste mode");
        let result = parser.parse(b"\x1b[201~");
        eprintln!("[TEST] Result: {result:?}");
        // Should be unrecognized sequence since we're not in paste mode
        assert!(matches!(result, Err(ParseError::UnrecognizedSequence(_))));
        eprintln!("[TEST] SUCCESS: Paste end without start handled correctly");
    }

    #[test]
    fn test_bracketed_paste_full_sequence_at_once() {
        eprintln!(
            "[TEST] test_bracketed_paste_full_sequence_at_once: Testing full paste sequence in single call"
        );
        let mut parser = InputParser::new();

        // First call parses the CSI 200~ and enters paste mode, returns Incomplete
        let full_input = b"\x1b[200~hello world\x1b[201~";
        eprintln!("[TEST] Sending full paste sequence: {:?}", full_input);

        let result = parser.parse(full_input);
        eprintln!("[TEST] First parse result: {:?}", result);

        // First parse enters paste mode and returns Incomplete
        assert_eq!(result, Err(ParseError::Incomplete));
        assert!(parser.in_paste, "Parser should be in paste mode");

        // Second parse (with same input) should strip start sequence and return paste event
        let result = parser.parse(full_input);
        eprintln!("[TEST] Second parse result: {:?}", result);

        let (event, consumed) = result.expect("Should parse paste event");
        let paste = event.paste().expect("Should be a paste event");

        eprintln!("[TEST] Paste content: {:?}", paste.content);
        eprintln!("[TEST] Consumed: {} bytes", consumed);

        // Content should NOT include the start sequence
        assert_eq!(
            paste.content, "hello world",
            "Start sequence should be stripped"
        );
        assert!(
            !paste.content.contains("\x1b[200~"),
            "Content should not contain start sequence"
        );
        assert_eq!(consumed, full_input.len(), "Should consume entire input");

        eprintln!("[TEST] SUCCESS: Full sequence at once correctly strips start sequence");
    }

    // =========================================================================
    // Comprehensive Mouse Input Tests (bd-vde)
    // =========================================================================

    #[test]
    fn test_parse_sgr_mouse_middle_click() {
        eprintln!("[TEST] test_parse_sgr_mouse_middle_click: Testing middle mouse button");
        let mut parser = InputParser::new();

        // Middle click: button=1
        let input = b"\x1b[<1;20;10M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 1 (middle)");

        let (event, consumed) = parser.parse(input).unwrap();
        eprintln!("[TEST] Consumed: {} bytes", consumed);

        let mouse = event.mouse().expect("Should be a mouse event");
        eprintln!(
            "[TEST] Mouse: button={:?} kind={:?} at ({}, {})",
            mouse.button, mouse.kind, mouse.x, mouse.y
        );

        assert_eq!(mouse.button, MouseButton::Middle);
        assert_eq!(mouse.kind, MouseEventKind::Press);
        assert_eq!(mouse.x, 19); // 20 - 1 (0-indexed)
        assert_eq!(mouse.y, 9); // 10 - 1 (0-indexed)
        eprintln!("[TEST] PASS: Middle click detected correctly");
    }

    #[test]
    fn test_parse_sgr_mouse_right_click() {
        eprintln!("[TEST] test_parse_sgr_mouse_right_click: Testing right mouse button");
        let mut parser = InputParser::new();

        // Right click: button=2
        let input = b"\x1b[<2;30;15M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 2 (right)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!(
            "[TEST] Mouse: button={:?} at ({}, {})",
            mouse.button, mouse.x, mouse.y
        );
        assert_eq!(mouse.button, MouseButton::Right);
        assert_eq!(mouse.kind, MouseEventKind::Press);
        assert_eq!(mouse.x, 29);
        assert_eq!(mouse.y, 14);
        eprintln!("[TEST] PASS: Right click detected correctly");
    }

    #[test]
    fn test_parse_sgr_mouse_with_shift() {
        eprintln!("[TEST] test_parse_sgr_mouse_with_shift: Testing Shift modifier");
        let mut parser = InputParser::new();

        // Shift+Left click: button=0 + shift(4) = 4
        let input = b"\x1b[<4;10;5M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 4 = 0(left) + 4(shift)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!(
            "[TEST] Modifiers: shift={} ctrl={} alt={}",
            mouse.shift, mouse.ctrl, mouse.alt
        );
        assert_eq!(mouse.button, MouseButton::Left);
        assert!(mouse.shift, "Shift modifier should be set");
        assert!(!mouse.ctrl, "Ctrl should not be set");
        assert!(!mouse.alt, "Alt should not be set");
        eprintln!("[TEST] PASS: Shift modifier detected");
    }

    #[test]
    fn test_parse_sgr_mouse_with_ctrl() {
        eprintln!("[TEST] test_parse_sgr_mouse_with_ctrl: Testing Ctrl modifier");
        let mut parser = InputParser::new();

        // Ctrl+Left click: button=0 + ctrl(16) = 16
        let input = b"\x1b[<16;30;15M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 16 = 0(left) + 16(ctrl)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!(
            "[TEST] Modifiers: shift={} ctrl={} alt={}",
            mouse.shift, mouse.ctrl, mouse.alt
        );
        assert_eq!(mouse.button, MouseButton::Left);
        assert!(mouse.ctrl, "Ctrl modifier should be set");
        assert!(!mouse.shift, "Shift should not be set");
        assert!(!mouse.alt, "Alt should not be set");
        eprintln!("[TEST] PASS: Ctrl modifier detected");
    }

    #[test]
    fn test_parse_sgr_mouse_with_alt() {
        eprintln!("[TEST] test_parse_sgr_mouse_with_alt: Testing Alt modifier");
        let mut parser = InputParser::new();

        // Alt+Left click: button=0 + alt(8) = 8
        let input = b"\x1b[<8;20;10M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 8 = 0(left) + 8(alt)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!(
            "[TEST] Modifiers: shift={} ctrl={} alt={}",
            mouse.shift, mouse.ctrl, mouse.alt
        );
        assert!(mouse.alt, "Alt modifier should be set");
        assert!(!mouse.shift, "Shift should not be set");
        assert!(!mouse.ctrl, "Ctrl should not be set");
        eprintln!("[TEST] PASS: Alt modifier detected");
    }

    #[test]
    fn test_parse_sgr_mouse_with_multiple_modifiers() {
        eprintln!(
            "[TEST] test_parse_sgr_mouse_with_multiple_modifiers: Testing combined modifiers"
        );
        let mut parser = InputParser::new();

        // Ctrl+Shift+Left click: 0 + 4(shift) + 16(ctrl) = 20
        let input = b"\x1b[<20;15;8M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 20 = 0(left) + 4(shift) + 16(ctrl)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!(
            "[TEST] Modifiers: shift={} ctrl={} alt={}",
            mouse.shift, mouse.ctrl, mouse.alt
        );
        assert!(mouse.shift, "Shift should be set");
        assert!(mouse.ctrl, "Ctrl should be set");
        assert!(!mouse.alt, "Alt should not be set");
        eprintln!("[TEST] PASS: Multiple modifiers detected");
    }

    #[test]
    fn test_parse_sgr_mouse_scroll_up() {
        eprintln!("[TEST] test_parse_sgr_mouse_scroll_up: Testing scroll wheel up");
        let mut parser = InputParser::new();

        // Scroll up: 64
        let input = b"\x1b[<64;10;5M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 64 (scroll up)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!("[TEST] Event kind: {:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::ScrollUp);
        assert!(mouse.is_scroll());
        eprintln!("[TEST] PASS: Scroll up detected");
    }

    #[test]
    fn test_parse_sgr_mouse_scroll_down() {
        eprintln!("[TEST] test_parse_sgr_mouse_scroll_down: Testing scroll wheel down");
        let mut parser = InputParser::new();

        // Scroll down: 65
        let input = b"\x1b[<65;10;5M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 65 (scroll down)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!("[TEST] Event kind: {:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::ScrollDown);
        eprintln!("[TEST] PASS: Scroll down detected");
    }

    #[test]
    fn test_parse_sgr_mouse_scroll_left() {
        eprintln!("[TEST] test_parse_sgr_mouse_scroll_left: Testing horizontal scroll left");
        let mut parser = InputParser::new();

        // Scroll left: 66
        let input = b"\x1b[<66;10;5M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 66 (scroll left)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!("[TEST] Event kind: {:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::ScrollLeft);
        eprintln!("[TEST] PASS: Scroll left detected");
    }

    #[test]
    fn test_parse_sgr_mouse_scroll_right() {
        eprintln!("[TEST] test_parse_sgr_mouse_scroll_right: Testing horizontal scroll right");
        let mut parser = InputParser::new();

        // Scroll right: 67
        let input = b"\x1b[<67;10;5M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 67 (scroll right)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!("[TEST] Event kind: {:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::ScrollRight);
        eprintln!("[TEST] PASS: Scroll right detected");
    }

    #[test]
    fn test_parse_sgr_mouse_motion() {
        eprintln!("[TEST] test_parse_sgr_mouse_motion: Testing mouse motion (drag)");
        let mut parser = InputParser::new();

        // Motion with left button held: 32 (motion flag) + 0 (left) = 32
        let input = b"\x1b[<32;50;25M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Button byte: 32 (motion with left button)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!(
            "[TEST] Kind: {:?}, Button: {:?}, Position: ({}, {})",
            mouse.kind, mouse.button, mouse.x, mouse.y
        );
        assert_eq!(mouse.kind, MouseEventKind::Move);
        assert_eq!(mouse.x, 49);
        assert_eq!(mouse.y, 24);
        eprintln!("[TEST] PASS: Motion event detected");
    }

    #[test]
    fn test_parse_sgr_mouse_large_coordinates() {
        eprintln!(
            "[TEST] test_parse_sgr_mouse_large_coordinates: Testing large terminal coordinates"
        );
        let mut parser = InputParser::new();

        // Large coordinates (common in high-res terminals)
        let input = b"\x1b[<0;999;500M";
        eprintln!("[TEST] Input: {:?}", input);
        eprintln!("[TEST] Testing coordinates (999, 500)");

        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().expect("Should be a mouse event");

        eprintln!("[TEST] Parsed coordinates: ({}, {})", mouse.x, mouse.y);
        assert_eq!(mouse.x, 998, "X should be 999-1=998 (0-indexed)");
        assert_eq!(mouse.y, 499, "Y should be 500-1=499 (0-indexed)");
        eprintln!("[TEST] PASS: Large coordinates handled correctly");
    }

    #[test]
    fn test_parse_sgr_mouse_all_buttons_release() {
        eprintln!("[TEST] test_parse_sgr_mouse_all_buttons_release: Testing button release");
        let mut parser = InputParser::new();

        // Left release (lowercase m)
        let (event, _) = parser.parse(b"\x1b[<0;10;5m").unwrap();
        let mouse = event.mouse().unwrap();
        eprintln!("[TEST] Left release: kind={:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::Release);
        assert_eq!(mouse.button, MouseButton::Left);

        // Middle release
        let (event, _) = parser.parse(b"\x1b[<1;10;5m").unwrap();
        let mouse = event.mouse().unwrap();
        eprintln!("[TEST] Middle release: kind={:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::Release);
        assert_eq!(mouse.button, MouseButton::Middle);

        // Right release
        let (event, _) = parser.parse(b"\x1b[<2;10;5m").unwrap();
        let mouse = event.mouse().unwrap();
        eprintln!("[TEST] Right release: kind={:?}", mouse.kind);
        assert_eq!(mouse.kind, MouseEventKind::Release);
        assert_eq!(mouse.button, MouseButton::Right);

        eprintln!("[TEST] PASS: All button releases detected");
    }

    #[test]
    fn test_parse_x11_mouse_basic() {
        eprintln!("[TEST] test_parse_x11_mouse_basic: Testing legacy X11 mouse encoding");
        let mut parser = InputParser::new();

        // X11 encoding: ESC[M followed by button+32, x+33, y+33
        // The x and y values are 1-indexed in protocol, +32 to avoid control chars
        // We subtract 33 to get 0-indexed coords (consistent with SGR)
        // Left click at (10, 5) in 0-indexed coords: button=0+32=32=' ', x=10+33=43='+', y=5+33=38='&'
        let input = b"\x1b[M +&";
        eprintln!("[TEST] Input bytes: {:02x?}", input);
        eprintln!("[TEST] X11 encoding: button=' '(32), x='+'(43-33=10), y='&'(38-33=5)");

        let (event, consumed) = parser.parse(input).unwrap();
        eprintln!("[TEST] Consumed: {} bytes", consumed);

        let mouse = event.mouse().expect("Should be a mouse event");
        eprintln!(
            "[TEST] Parsed: button={:?} at ({}, {})",
            mouse.button, mouse.x, mouse.y
        );

        // X11 now returns 0-indexed coords (consistent with SGR)
        assert_eq!(mouse.button, MouseButton::Left);
        assert_eq!(mouse.x, 10);
        assert_eq!(mouse.y, 5);
        eprintln!("[TEST] PASS: X11 mouse encoding parsed correctly");
    }

    #[test]
    fn test_parse_sgr_mouse_coordinate_boundary() {
        eprintln!("[TEST] test_parse_sgr_mouse_coordinate_boundary: Testing coordinate edge cases");
        let mut parser = InputParser::new();

        // Minimum coordinates (1,1) -> (0,0)
        let (event, _) = parser.parse(b"\x1b[<0;1;1M").unwrap();
        let mouse = event.mouse().unwrap();
        eprintln!("[TEST] Min coords (1,1) -> ({}, {})", mouse.x, mouse.y);
        assert_eq!(mouse.x, 0);
        assert_eq!(mouse.y, 0);

        // Very large coordinates
        let (event, _) = parser.parse(b"\x1b[<0;9999;9999M").unwrap();
        let mouse = event.mouse().unwrap();
        eprintln!(
            "[TEST] Large coords (9999,9999) -> ({}, {})",
            mouse.x, mouse.y
        );
        assert_eq!(mouse.x, 9998);
        assert_eq!(mouse.y, 9998);

        eprintln!("[TEST] PASS: Coordinate boundaries handled correctly");
    }

    // =========================================================================
    // Additional Input Event Parsing Tests (bd-2a56)
    // =========================================================================

    #[test]
    fn test_parse_ctrl_sequences_all() {
        // Test Ctrl+A through Ctrl+Z (bytes 0x01 to 0x1a)
        let mut parser = InputParser::new();

        for (i, expected_char) in ('a'..='z').enumerate() {
            let ctrl_byte = (i + 1) as u8;
            let (event, consumed) = parser.parse(&[ctrl_byte]).unwrap();
            assert_eq!(consumed, 1);

            let key = event.key().expect("Should be key event");
            assert_eq!(key.code, KeyCode::Char(expected_char));
            assert!(
                key.ctrl(),
                "Ctrl modifier should be set for byte 0x{:02x}",
                ctrl_byte
            );
        }
    }

    #[test]
    fn test_parse_function_keys_f1_f12() {
        let mut parser = InputParser::new();

        // F1-F4 use SS3 sequences (ESC O P/Q/R/S)
        let ss3_keys = [(b"P", 1), (b"Q", 2), (b"R", 3), (b"S", 4)];
        for (suffix, num) in ss3_keys {
            let mut input = vec![0x1b, b'O'];
            input.push(suffix[0]);
            let (event, _) = parser.parse(&input).unwrap();
            let key = event.key().unwrap();
            assert_eq!(
                key.code,
                KeyCode::F(num),
                "F{} should be parsed from SS3",
                num
            );
        }

        // F5-F12 use CSI sequences with tilde
        let csi_keys = [
            (15, 5),
            (17, 6),
            (18, 7),
            (19, 8),
            (20, 9),
            (21, 10),
            (23, 11),
            (24, 12),
        ];
        for (num_code, f_num) in csi_keys {
            let input = format!("\x1b[{}~", num_code);
            let (event, _) = parser.parse(input.as_bytes()).unwrap();
            let key = event.key().unwrap();
            assert_eq!(
                key.code,
                KeyCode::F(f_num),
                "F{} should be parsed from CSI {}~",
                f_num,
                num_code
            );
        }
    }

    #[test]
    fn test_parse_special_keys_navigation() {
        let mut parser = InputParser::new();

        // Home, End via CSI H/F
        let (event, _) = parser.parse(b"\x1b[H").unwrap();
        assert_eq!(event.key().unwrap().code, KeyCode::Home);

        let (event, _) = parser.parse(b"\x1b[F").unwrap();
        assert_eq!(event.key().unwrap().code, KeyCode::End);

        // Insert, Delete, PageUp, PageDown via tilde sequences
        let tilde_keys = [
            (2, KeyCode::Insert),
            (3, KeyCode::Delete),
            (5, KeyCode::PageUp),
            (6, KeyCode::PageDown),
        ];
        for (num, expected_code) in tilde_keys {
            let input = format!("\x1b[{}~", num);
            let (event, _) = parser.parse(input.as_bytes()).unwrap();
            assert_eq!(event.key().unwrap().code, expected_code);
        }
    }

    #[test]
    fn test_parse_all_arrow_keys() {
        let mut parser = InputParser::new();

        let arrows = [
            (b'A', KeyCode::Up),
            (b'B', KeyCode::Down),
            (b'C', KeyCode::Right),
            (b'D', KeyCode::Left),
        ];

        for (char_code, expected_key) in arrows {
            let input = [0x1b, b'[', char_code];
            let (event, consumed) = parser.parse(&input).unwrap();
            assert_eq!(consumed, 3);
            assert_eq!(event.key().unwrap().code, expected_key);
        }
    }

    #[test]
    fn test_parse_malformed_sequence() {
        let mut parser = InputParser::new();

        // Unknown CSI sequence
        let result = parser.parse(b"\x1b[999Z");
        assert!(matches!(result, Err(ParseError::UnrecognizedSequence(_))));

        // Unknown tilde sequence
        let result = parser.parse(b"\x1b[999~");
        assert!(matches!(result, Err(ParseError::UnrecognizedSequence(_))));
    }

    #[test]
    fn test_parse_empty_input() {
        let mut parser = InputParser::new();
        let result = parser.parse(&[]);
        assert_eq!(result, Err(ParseError::Empty));
    }

    #[test]
    fn test_parse_incomplete_csi() {
        let mut parser = InputParser::new();

        // Just ESC [
        let result = parser.parse(b"\x1b[");
        assert_eq!(result, Err(ParseError::Incomplete));

        // ESC [ with parameters but no terminator
        let result = parser.parse(b"\x1b[1;2");
        assert_eq!(result, Err(ParseError::Incomplete));
    }

    #[test]
    fn test_parse_invalid_utf8() {
        let mut parser = InputParser::new();

        // Invalid UTF-8 continuation byte without start
        let result = parser.parse(&[0x80]);
        assert!(matches!(result, Err(ParseError::InvalidUtf8)));

        // Incomplete UTF-8 (2-byte sequence with only first byte)
        let result = parser.parse(&[0xc3]); // Start of 2-byte sequence
        assert_eq!(result, Err(ParseError::Incomplete));
    }

    #[test]
    fn test_parse_null_character() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse(&[0x00]).unwrap();
        assert_eq!(consumed, 1);
        assert_eq!(event.key().unwrap().code, KeyCode::Null);
    }

    #[test]
    fn test_parse_double_escape() {
        let mut parser = InputParser::new();
        let (event, consumed) = parser.parse(b"\x1b\x1b").unwrap();
        assert_eq!(consumed, 1);
        assert_eq!(event.key().unwrap().code, KeyCode::Esc);
    }

    #[test]
    fn test_parse_resize_event() {
        let mut parser = InputParser::new();
        let (event, _) = parser.parse(b"\x1b[8;50;120t").unwrap();
        if let Event::Resize(resize) = event {
            assert_eq!(resize.width, 120);
            assert_eq!(resize.height, 50);
        } else {
            panic!("Expected Resize event");
        }
    }

    // =========================================================================
    // Resize parsing error tests (bd-1nv2)
    // =========================================================================

    #[test]
    fn test_parse_resize_invalid_height() {
        let mut parser = InputParser::new();
        // Height with non-digit CSI parameter bytes should fail
        // Using '<' (0x3C) which is a valid CSI parameter byte but not a digit
        let result = parser.parse(b"\x1b[8;<10;120t");
        assert_eq!(
            result,
            Err(ParseError::InvalidResizeFormat),
            "Non-numeric height should return error"
        );
    }

    #[test]
    fn test_parse_resize_invalid_width() {
        let mut parser = InputParser::new();
        // Width with non-digit CSI parameter bytes should fail
        let result = parser.parse(b"\x1b[8;50;>80t");
        assert_eq!(
            result,
            Err(ParseError::InvalidResizeFormat),
            "Non-numeric width should return error"
        );
    }

    #[test]
    fn test_parse_resize_overflow_dimensions() {
        let mut parser = InputParser::new();
        // Dimensions that overflow u16 should return error
        let result = parser.parse(b"\x1b[8;99999;120t");
        assert_eq!(
            result,
            Err(ParseError::InvalidResizeFormat),
            "Overflow height should return error"
        );
    }

    #[test]
    fn test_parse_resize_empty_values() {
        let mut parser = InputParser::new();
        // Empty height/width should return error
        let result = parser.parse(b"\x1b[8;;t");
        assert_eq!(
            result,
            Err(ParseError::InvalidResizeFormat),
            "Empty height/width should return error"
        );
    }

    #[test]
    fn test_parse_resize_zero_dimensions() {
        let mut parser = InputParser::new();
        // Zero dimensions should be parsed as valid (terminal might report this)
        let (event, _) = parser.parse(b"\x1b[8;0;0t").unwrap();
        if let Event::Resize(resize) = event {
            assert_eq!(resize.width, 0);
            assert_eq!(resize.height, 0);
        } else {
            panic!("Expected Resize event");
        }
    }

    #[test]
    fn test_parse_resize_large_dimensions() {
        let mut parser = InputParser::new();
        // Large valid dimensions should work
        let (event, _) = parser.parse(b"\x1b[8;1000;2000t").unwrap();
        if let Event::Resize(resize) = event {
            assert_eq!(resize.width, 2000);
            assert_eq!(resize.height, 1000);
        } else {
            panic!("Expected Resize event");
        }
    }

    #[test]
    fn test_parse_keyboard_with_all_modifiers() {
        let mut parser = InputParser::new();

        // Ctrl+Shift+Alt+Up: ESC [ 1 ; 8 A (8 = 1 + 1(shift) + 2(alt) + 4(ctrl))
        let (event, _) = parser.parse(b"\x1b[1;8A").unwrap();
        let key = event.key().unwrap();
        assert_eq!(key.code, KeyCode::Up);
        assert!(key.shift(), "Shift should be set");
        assert!(key.alt(), "Alt should be set");
        assert!(key.ctrl(), "Ctrl should be set");
    }

    #[test]
    fn test_x11_mouse_with_modifiers() {
        let mut parser = InputParser::new();

        // X11 with Shift: button=0 + shift(4) + offset(32) = 36
        // '$'=36, '+'=43, '&'=38
        let input = b"\x1b[M$+&";
        let (event, _) = parser.parse(input).unwrap();
        let mouse = event.mouse().unwrap();
        assert!(
            mouse.shift,
            "Shift modifier should be detected in X11 encoding"
        );
    }

    // =========================================================================
    // Paste Buffer Overflow Tests (bd-nkgh)
    // =========================================================================

    #[test]
    fn test_paste_buffer_overflow_single_chunk() {
        eprintln!(
            "[TEST] test_paste_buffer_overflow_single_chunk: Testing overflow in single chunk"
        );
        let mut parser = InputParser::new();

        // Enter paste mode
        let result = parser.parse(b"\x1b[200~");
        assert_eq!(result, Err(ParseError::Incomplete));
        assert!(parser.in_paste, "Should be in paste mode");

        // Create content larger than MAX_PASTE_BUFFER_SIZE (10 MB)
        let oversized_content: Vec<u8> = vec![b'X'; MAX_PASTE_BUFFER_SIZE + 1];
        let result = parser.parse(&oversized_content);

        eprintln!("[TEST] Result: {:?}", result);
        assert_eq!(
            result,
            Err(ParseError::PasteBufferOverflow),
            "Should return PasteBufferOverflow error"
        );
        assert!(
            !parser.in_paste,
            "Parser should exit paste mode after overflow"
        );
        assert!(
            parser.paste_buffer.is_empty(),
            "Paste buffer should be cleared after overflow"
        );
        eprintln!("[TEST] PASS: Single chunk overflow handled correctly");
    }

    #[test]
    fn test_paste_buffer_overflow_incremental() {
        eprintln!(
            "[TEST] test_paste_buffer_overflow_incremental: Testing overflow across multiple chunks"
        );
        let mut parser = InputParser::new();

        // Enter paste mode
        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Fill the buffer close to the limit (leave room for just a few bytes)
        let almost_full: Vec<u8> = vec![b'A'; MAX_PASTE_BUFFER_SIZE - 10];
        let result = parser.parse(&almost_full);
        assert_eq!(
            result,
            Err(ParseError::Incomplete),
            "Should still be accumulating"
        );
        assert!(parser.in_paste, "Should still be in paste mode");

        // Now send more than the remaining 10 bytes (but without end sequence)
        let overflow_chunk: Vec<u8> = vec![b'B'; 20];
        let result = parser.parse(&overflow_chunk);

        eprintln!("[TEST] Result after overflow chunk: {:?}", result);
        assert_eq!(
            result,
            Err(ParseError::PasteBufferOverflow),
            "Should return PasteBufferOverflow"
        );
        assert!(!parser.in_paste, "Parser should exit paste mode");
        assert!(
            parser.paste_buffer.is_empty(),
            "Buffer should be cleared after overflow"
        );
        eprintln!("[TEST] PASS: Incremental overflow handled correctly");
    }

    #[test]
    fn test_paste_buffer_overflow_with_end_sequence() {
        eprintln!(
            "[TEST] test_paste_buffer_overflow_with_end_sequence: Testing overflow when end sequence present"
        );
        let mut parser = InputParser::new();

        // Enter paste mode
        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Fill buffer close to limit
        let almost_full: Vec<u8> = vec![b'X'; MAX_PASTE_BUFFER_SIZE - 5];
        let _ = parser.parse(&almost_full);

        // Send content that overflows even though end sequence is present
        let mut final_chunk = vec![b'Y'; 20]; // 20 bytes > remaining 5
        final_chunk.extend_from_slice(b"\x1b[201~"); // Add end sequence
        let result = parser.parse(&final_chunk);

        eprintln!("[TEST] Result: {:?}", result);
        assert_eq!(
            result,
            Err(ParseError::PasteBufferOverflow),
            "Should return overflow error even with end sequence"
        );
        assert!(!parser.in_paste, "Should exit paste mode");
        assert!(parser.paste_buffer.is_empty(), "Buffer should be cleared");
        eprintln!("[TEST] PASS: Overflow with end sequence handled correctly");
    }

    #[test]
    fn test_paste_buffer_exactly_at_limit() {
        eprintln!("[TEST] test_paste_buffer_exactly_at_limit: Testing paste exactly at size limit");
        let mut parser = InputParser::new();

        // Enter paste mode
        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste);

        // Fill buffer to exactly the limit
        let exact_limit: Vec<u8> = vec![b'E'; MAX_PASTE_BUFFER_SIZE];
        let result = parser.parse(&exact_limit);
        assert_eq!(
            result,
            Err(ParseError::Incomplete),
            "Should accept exactly at limit"
        );
        assert!(parser.in_paste, "Should still be in paste mode");

        // Now send end sequence
        let (event, _) = parser.parse(b"\x1b[201~").expect("Should complete paste");
        let paste = event.paste().expect("Should be paste event");
        assert_eq!(
            paste.content.len(),
            MAX_PASTE_BUFFER_SIZE,
            "Content should be exactly at limit"
        );
        eprintln!("[TEST] PASS: Exactly at limit works correctly");
    }

    #[test]
    fn test_paste_buffer_overflow_resets_for_next_paste() {
        eprintln!(
            "[TEST] test_paste_buffer_overflow_resets_for_next_paste: Testing recovery after overflow"
        );
        let mut parser = InputParser::new();

        // First paste: overflow
        let _ = parser.parse(b"\x1b[200~");
        let oversized: Vec<u8> = vec![b'X'; MAX_PASTE_BUFFER_SIZE + 100];
        let result = parser.parse(&oversized);
        assert_eq!(result, Err(ParseError::PasteBufferOverflow));
        assert!(!parser.in_paste, "Should exit paste mode after overflow");

        // Second paste: should work normally
        let _ = parser.parse(b"\x1b[200~");
        assert!(parser.in_paste, "Should enter paste mode again");

        let (event, _) = parser
            .parse(b"normal paste content\x1b[201~")
            .expect("Normal paste should work after overflow");
        let paste = event.paste().expect("Should be paste event");
        assert_eq!(
            paste.content, "normal paste content",
            "Normal paste should work after previous overflow"
        );
        eprintln!("[TEST] PASS: Recovery after overflow works correctly");
    }
}
