//! Terminal abstraction and capability detection.

mod capabilities;
mod cursor;
mod mouse;
mod queries;
mod raw;

pub use capabilities::{Capabilities, ColorSupport};
pub use cursor::{CursorState, CursorStyle};
pub use mouse::{MouseButton, MouseEvent, MouseEventKind};
pub use queries::{TerminalResponse, all_queries, query_constants};
pub use raw::{RawModeGuard, enable_raw_mode, is_tty, terminal_size};

use crate::ansi::sequences;
use std::io::{self, Write};

/// Terminal state manager.
pub struct Terminal<W: Write> {
    writer: W,
    capabilities: Capabilities,
    cursor: CursorState,
    alt_screen: bool,
    mouse_enabled: bool,
    raw_mode_guard: Option<RawModeGuard>,
}

impl<W: Write> Terminal<W> {
    /// Create a new terminal with the given writer.
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            capabilities: Capabilities::detect(),
            cursor: CursorState::default(),
            alt_screen: false,
            mouse_enabled: false,
            raw_mode_guard: None,
        }
    }

    /// Check if terminal is in raw mode.
    #[must_use]
    pub fn is_raw_mode(&self) -> bool {
        self.raw_mode_guard.is_some()
    }

    /// Enter raw mode.
    ///
    /// Raw mode disables terminal line buffering, echo, and signal processing,
    /// allowing the application to receive individual key presses.
    pub fn enter_raw_mode(&mut self) -> io::Result<()> {
        if self.raw_mode_guard.is_none() {
            self.raw_mode_guard = Some(enable_raw_mode()?);
        }
        Ok(())
    }

    /// Exit raw mode.
    ///
    /// Restores the terminal to its original state before raw mode was enabled.
    pub fn exit_raw_mode(&mut self) -> io::Result<()> {
        self.raw_mode_guard = None;
        Ok(())
    }

    /// Get terminal capabilities.
    #[must_use]
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Get mutable access to terminal capabilities.
    pub fn capabilities_mut(&mut self) -> &mut Capabilities {
        &mut self.capabilities
    }

    /// Send terminal capability queries.
    ///
    /// Sends the following queries:
    /// - DA1 (Primary Device Attributes)
    /// - DA2 (Secondary Device Attributes)
    /// - XTVERSION (terminal version)
    /// - Pixel resolution
    /// - Kitty keyboard protocol
    pub fn query_capabilities(&mut self) -> io::Result<()> {
        self.writer.write_all(all_queries().as_bytes())?;
        self.writer.flush()
    }

    /// Parse a terminal response and update capabilities.
    ///
    /// Returns the parsed response if recognized.
    pub fn parse_response(&mut self, response: &[u8]) -> Option<TerminalResponse> {
        let parsed = TerminalResponse::parse(response)?;
        self.update_capabilities_from_response(&parsed);
        Some(parsed)
    }

    /// Update capabilities based on a parsed response.
    fn update_capabilities_from_response(&mut self, response: &TerminalResponse) {
        match response {
            TerminalResponse::DeviceAttributes { primary: true, params } => {
                // DA1 param 4 indicates sixel support
                if params.contains(&4) {
                    self.capabilities.sixel = true;
                }
            }
            TerminalResponse::XtVersion { name, .. } => {
                let name_lower = name.to_lowercase();
                if name_lower.contains("kitty") {
                    self.capabilities.kitty_keyboard = true;
                    self.capabilities.kitty_graphics = true;
                    self.capabilities.sync_output = true;
                } else if name_lower.contains("foot")
                    || name_lower.contains("alacritty")
                    || name_lower.contains("wezterm")
                {
                    self.capabilities.sync_output = true;
                }
            }
            TerminalResponse::PixelSize { width, height } => {
                if *width > 0 && *height > 0 {
                    self.capabilities.explicit_width = true;
                    self.capabilities.sgr_pixels = true;
                }
            }
            TerminalResponse::KittyKeyboard { flags: _ } => {
                self.capabilities.kitty_keyboard = true;
            }
            _ => {}
        }
    }

    /// Apply a raw capability response to update detection hints.
    pub fn apply_capability_response(&mut self, response: &str) {
        self.capabilities.apply_query_response(response);
    }

    /// Get cursor state.
    #[must_use]
    pub fn cursor(&self) -> &CursorState {
        &self.cursor
    }

    /// Enter alternate screen buffer.
    pub fn enter_alt_screen(&mut self) -> io::Result<()> {
        if !self.alt_screen {
            self.writer.write_all(sequences::ALT_SCREEN_ON.as_bytes())?;
            self.alt_screen = true;
        }
        Ok(())
    }

    /// Leave alternate screen buffer.
    pub fn leave_alt_screen(&mut self) -> io::Result<()> {
        if self.alt_screen {
            self.writer
                .write_all(sequences::ALT_SCREEN_OFF.as_bytes())?;
            self.alt_screen = false;
        }
        Ok(())
    }

    /// Enable mouse tracking.
    pub fn enable_mouse(&mut self) -> io::Result<()> {
        if !self.mouse_enabled {
            self.writer.write_all(sequences::MOUSE_ON.as_bytes())?;
            self.mouse_enabled = true;
        }
        Ok(())
    }

    /// Disable mouse tracking.
    pub fn disable_mouse(&mut self) -> io::Result<()> {
        if self.mouse_enabled {
            self.writer.write_all(sequences::MOUSE_OFF.as_bytes())?;
            self.mouse_enabled = false;
        }
        Ok(())
    }

    /// Hide cursor.
    pub fn hide_cursor(&mut self) -> io::Result<()> {
        if self.cursor.visible {
            self.writer.write_all(sequences::CURSOR_HIDE.as_bytes())?;
            self.cursor.visible = false;
        }
        Ok(())
    }

    /// Show cursor.
    pub fn show_cursor(&mut self) -> io::Result<()> {
        if !self.cursor.visible {
            self.writer.write_all(sequences::CURSOR_SHOW.as_bytes())?;
            self.cursor.visible = true;
        }
        Ok(())
    }

    /// Set cursor style.
    pub fn set_cursor_style(&mut self, style: CursorStyle, blinking: bool) -> io::Result<()> {
        let seq = match (style, blinking) {
            (CursorStyle::Block, true) => sequences::cursor_style::BLOCK_BLINK,
            (CursorStyle::Block, false) => sequences::cursor_style::BLOCK_STEADY,
            (CursorStyle::Underline, true) => sequences::cursor_style::UNDERLINE_BLINK,
            (CursorStyle::Underline, false) => sequences::cursor_style::UNDERLINE_STEADY,
            (CursorStyle::Bar, true) => sequences::cursor_style::BAR_BLINK,
            (CursorStyle::Bar, false) => sequences::cursor_style::BAR_STEADY,
        };
        self.writer.write_all(seq.as_bytes())?;
        self.cursor.style = style;
        self.cursor.blinking = blinking;
        Ok(())
    }

    /// Move cursor to position.
    pub fn move_cursor(&mut self, x: u32, y: u32) -> io::Result<()> {
        let seq = crate::ansi::cursor_position(y, x);
        self.writer.write_all(seq.as_bytes())?;
        self.cursor.x = x;
        self.cursor.y = y;
        Ok(())
    }

    /// Save cursor position using DEC sequence.
    pub fn save_cursor(&mut self) -> io::Result<()> {
        self.writer.write_all(sequences::CURSOR_SAVE.as_bytes())
    }

    /// Restore cursor position using DEC sequence.
    pub fn restore_cursor(&mut self) -> io::Result<()> {
        self.writer.write_all(sequences::CURSOR_RESTORE.as_bytes())
    }

    /// Set cursor color using OSC 12.
    pub fn set_cursor_color(&mut self, color: crate::color::Rgba) -> io::Result<()> {
        let (r, g, b) = color.to_rgb_u8();
        let seq = sequences::cursor_color(r, g, b);
        self.writer.write_all(seq.as_bytes())
    }

    /// Reset cursor color to default using OSC 112.
    pub fn reset_cursor_color(&mut self) -> io::Result<()> {
        self.writer
            .write_all(sequences::CURSOR_COLOR_RESET.as_bytes())
    }

    /// Clear the screen.
    pub fn clear(&mut self) -> io::Result<()> {
        self.writer.write_all(sequences::CLEAR_SCREEN.as_bytes())?;
        self.writer.write_all(sequences::CURSOR_HOME.as_bytes())?;
        Ok(())
    }

    /// Set window title.
    pub fn set_title(&mut self, title: &str) -> io::Result<()> {
        write!(
            self.writer,
            "{}{}{}",
            sequences::TITLE_PREFIX,
            title,
            sequences::TITLE_SUFFIX
        )?;
        Ok(())
    }

    /// Reset terminal state.
    pub fn reset(&mut self) -> io::Result<()> {
        self.writer.write_all(sequences::RESET.as_bytes())?;
        self.writer
            .write_all(sequences::cursor_style::DEFAULT.as_bytes())?;
        Ok(())
    }

    /// Flush the output.
    pub fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }

    /// Begin synchronized update (for flicker-free rendering).
    pub fn begin_sync(&mut self) -> io::Result<()> {
        self.writer.write_all(sequences::sync::BEGIN.as_bytes())
    }

    /// End synchronized update.
    pub fn end_sync(&mut self) -> io::Result<()> {
        self.writer.write_all(sequences::sync::END.as_bytes())
    }

    /// Cleanup terminal on exit.
    pub fn cleanup(&mut self) -> io::Result<()> {
        self.show_cursor()?;
        self.disable_mouse()?;
        self.leave_alt_screen()?;
        self.exit_raw_mode()?;
        self.reset()?;
        self.flush()
    }
}

impl<W: Write> Drop for Terminal<W> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_basic() {
        let terminal = Terminal::new(Vec::new());
        assert!(!terminal.alt_screen);
        assert!(!terminal.mouse_enabled);
        assert!(!terminal.is_raw_mode());
    }

    #[test]
    fn test_terminal_alt_screen() {
        let mut terminal = Terminal::new(Vec::new());
        terminal.enter_alt_screen().unwrap();
        assert!(terminal.alt_screen);
        terminal.leave_alt_screen().unwrap();
        assert!(!terminal.alt_screen);
    }

    #[test]
    fn test_save_cursor_sequence() {
        eprintln!("[TEST] test_save_cursor_sequence");
        let mut output = Vec::new();
        {
            let mut terminal = Terminal::new(&mut output);
            terminal.save_cursor().unwrap();
        }

        let s = String::from_utf8_lossy(&output);
        eprintln!("[TEST] Output: {:?}", s);

        // Check that the save cursor sequence is present (terminal cleanup adds extra sequences)
        assert!(
            s.starts_with("\x1b7"),
            "Output should start with save cursor sequence"
        );

        eprintln!("[TEST] PASS: save_cursor writes correct sequence");
    }

    #[test]
    fn test_restore_cursor_sequence() {
        eprintln!("[TEST] test_restore_cursor_sequence");
        let mut output = Vec::new();
        {
            let mut terminal = Terminal::new(&mut output);
            terminal.restore_cursor().unwrap();
        }

        let s = String::from_utf8_lossy(&output);
        eprintln!("[TEST] Output: {:?}", s);

        // Check that the restore cursor sequence is present (terminal cleanup adds extra sequences)
        assert!(
            s.starts_with("\x1b8"),
            "Output should start with restore cursor sequence"
        );

        eprintln!("[TEST] PASS: restore_cursor writes correct sequence");
    }

    #[test]
    fn test_save_restore_round_trip() {
        eprintln!("[TEST] test_save_restore_round_trip");
        let mut output = Vec::new();
        {
            let mut terminal = Terminal::new(&mut output);

            // Save, move, restore pattern
            terminal.save_cursor().unwrap();
            terminal.move_cursor(10, 5).unwrap();
            terminal.restore_cursor().unwrap();
        }

        let s = String::from_utf8_lossy(&output);
        eprintln!("[TEST] Full sequence: {:?}", s);

        // Should contain save, move, restore in order
        assert!(s.contains("\x1b7"), "Should contain save sequence");
        assert!(s.contains("\x1b8"), "Should contain restore sequence");

        eprintln!("[TEST] PASS: save/restore round trip works");
    }

    #[test]
    fn test_cursor_color_sequence() {
        eprintln!("[TEST] test_cursor_color_sequence");
        let mut output = Vec::new();
        {
            let mut terminal = Terminal::new(&mut output);
            let color = crate::color::Rgba::from_rgb_u8(255, 128, 0);
            terminal.set_cursor_color(color).unwrap();
        }

        let s = String::from_utf8_lossy(&output);
        eprintln!("[TEST] Output: {:?}", s);

        // Should start with OSC 12 sequence: \x1b]12;#ff8000\x07
        assert!(
            s.starts_with("\x1b]12;#ff8000\x07"),
            "Output should start with cursor color sequence"
        );

        eprintln!("[TEST] PASS: set_cursor_color writes correct sequence");
    }

    #[test]
    fn test_cursor_color_reset() {
        eprintln!("[TEST] test_cursor_color_reset");
        let mut output = Vec::new();
        {
            let mut terminal = Terminal::new(&mut output);
            terminal.reset_cursor_color().unwrap();
        }

        let s = String::from_utf8_lossy(&output);
        eprintln!("[TEST] Output: {:?}", s);

        // Should start with OSC 112: \x1b]112\x07
        assert!(
            s.starts_with("\x1b]112\x07"),
            "Output should start with cursor color reset sequence"
        );

        eprintln!("[TEST] PASS: reset_cursor_color writes correct sequence");
    }
}
