//! Constant ANSI escape sequences.

/// Reset all attributes to default.
pub const RESET: &str = "\x1b[0m";

/// Clear entire screen.
pub const CLEAR_SCREEN: &str = "\x1b[2J";

/// Erase scrollback buffer (may not be supported by all terminals).
pub const ERASE_SCROLLBACK: &str = "\x1b[3J";

/// Clear from cursor to end of screen.
pub const CLEAR_SCREEN_BELOW: &str = "\x1b[J";

/// Clear from cursor to beginning of screen.
pub const CLEAR_SCREEN_ABOVE: &str = "\x1b[1J";

/// Clear entire line.
pub const CLEAR_LINE: &str = "\x1b[2K";

/// Clear from cursor to end of line.
pub const CLEAR_LINE_RIGHT: &str = "\x1b[K";

/// Clear from cursor to beginning of line.
pub const CLEAR_LINE_LEFT: &str = "\x1b[1K";

/// Hide cursor.
pub const CURSOR_HIDE: &str = "\x1b[?25l";

/// Show cursor.
pub const CURSOR_SHOW: &str = "\x1b[?25h";

/// Save cursor position (DEC).
pub const CURSOR_SAVE: &str = "\x1b7";

/// Restore cursor position (DEC).
pub const CURSOR_RESTORE: &str = "\x1b8";

/// Move cursor to home position (1,1).
pub const CURSOR_HOME: &str = "\x1b[H";

/// Reset cursor color to default (OSC 112).
pub const CURSOR_COLOR_RESET: &str = "\x1b]112\x07";

/// Generate cursor color sequence (OSC 12).
///
/// Uses the OSC 12 sequence to set cursor color to an RGB value.
#[must_use]
pub fn cursor_color(r: u8, g: u8, b: u8) -> String {
    format!("\x1b]12;#{r:02x}{g:02x}{b:02x}\x07")
}

/// Enable alternative screen buffer.
pub const ALT_SCREEN_ON: &str = "\x1b[?1049h";

/// Disable alternative screen buffer.
pub const ALT_SCREEN_OFF: &str = "\x1b[?1049l";

/// Enable mouse tracking (all events).
pub const MOUSE_ON: &str = "\x1b[?1003h\x1b[?1006h";

/// Disable mouse tracking.
pub const MOUSE_OFF: &str = "\x1b[?1003l\x1b[?1006l";

/// Enable bracketed paste mode.
pub const BRACKETED_PASTE_ON: &str = "\x1b[?2004h";

/// Disable bracketed paste mode.
pub const BRACKETED_PASTE_OFF: &str = "\x1b[?2004l";

/// Enable focus tracking.
pub const FOCUS_ON: &str = "\x1b[?1004h";

/// Disable focus tracking.
pub const FOCUS_OFF: &str = "\x1b[?1004l";

/// Request terminal size (XTWINOPS).
pub const REQUEST_SIZE: &str = "\x1b[18t";

/// Terminal capability query sequences.
pub mod query {
    /// Primary device attributes (DA1).
    pub const DEVICE_ATTRIBUTES: &str = "\x1b[c";
    /// Secondary device attributes (DA2).
    pub const DEVICE_ATTRIBUTES_SECONDARY: &str = "\x1b[>c";
    /// XTVERSION query.
    pub const XTVERSION: &str = "\x1b[>0q";
    /// Pixel resolution query.
    pub const PIXEL_RESOLUTION: &str = "\x1b[14t";
    /// Kitty keyboard protocol query.
    pub const KITTY_KEYBOARD: &str = "\x1b[?u";
}

/// Set window title prefix.
pub const TITLE_PREFIX: &str = "\x1b]0;";

/// Set window title suffix.
pub const TITLE_SUFFIX: &str = "\x1b\\";

/// Soft reset (RIS).
pub const SOFT_RESET: &str = "\x1bc";

/// Cursor style constants.
pub mod cursor_style {
    /// Block cursor (blinking).
    pub const BLOCK_BLINK: &str = "\x1b[1 q";
    /// Block cursor (steady).
    pub const BLOCK_STEADY: &str = "\x1b[2 q";
    /// Underline cursor (blinking).
    pub const UNDERLINE_BLINK: &str = "\x1b[3 q";
    /// Underline cursor (steady).
    pub const UNDERLINE_STEADY: &str = "\x1b[4 q";
    /// Bar cursor (blinking).
    pub const BAR_BLINK: &str = "\x1b[5 q";
    /// Bar cursor (steady).
    pub const BAR_STEADY: &str = "\x1b[6 q";
    /// Default cursor style.
    pub const DEFAULT: &str = "\x1b[0 q";
}

/// Synchronous update sequences (for flicker-free rendering).
pub mod sync {
    /// Begin synchronized update.
    pub const BEGIN: &str = "\x1b[?2026h";
    /// End synchronized update.
    pub const END: &str = "\x1b[?2026l";
}

/// Color reset sequences.
pub mod color {
    /// Reset foreground to default.
    pub const FG_DEFAULT: &str = "\x1b[39m";
    /// Reset background to default.
    pub const BG_DEFAULT: &str = "\x1b[49m";
}

/// Attribute reset sequences.
pub mod attr {
    /// Reset bold/dim.
    pub const RESET_INTENSITY: &str = "\x1b[22m";
    /// Reset italic.
    pub const RESET_ITALIC: &str = "\x1b[23m";
    /// Reset underline.
    pub const RESET_UNDERLINE: &str = "\x1b[24m";
    /// Reset blink.
    pub const RESET_BLINK: &str = "\x1b[25m";
    /// Reset inverse.
    pub const RESET_INVERSE: &str = "\x1b[27m";
    /// Reset hidden.
    pub const RESET_HIDDEN: &str = "\x1b[28m";
    /// Reset strikethrough.
    pub const RESET_STRIKETHROUGH: &str = "\x1b[29m";
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // ANSI Escape Sequence Format Tests
    // All CSI sequences start with ESC [ (0x1B 0x5B)
    // All OSC sequences start with ESC ] (0x1B 0x5D) and end with BEL (0x07) or ST
    // =========================================================================

    /// Helper to verify a string starts with CSI (ESC [)
    fn starts_with_csi(s: &str) -> bool {
        s.starts_with("\x1b[")
    }

    /// Helper to verify a string starts with OSC (ESC ])
    fn starts_with_osc(s: &str) -> bool {
        s.starts_with("\x1b]")
    }

    // =========================================================================
    // Screen Control Sequences (ECMA-48 Section 8.3)
    // =========================================================================

    #[test]
    fn test_reset_sgr0() {
        // SGR 0 - Reset all attributes (ECMA-48 8.3.117)
        assert_eq!(RESET, "\x1b[0m");
        assert!(starts_with_csi(RESET));
        assert!(RESET.ends_with('m'));
    }

    #[test]
    fn test_clear_screen_ed2() {
        // ED 2 - Erase entire display (ECMA-48 8.3.39)
        assert_eq!(CLEAR_SCREEN, "\x1b[2J");
        assert!(starts_with_csi(CLEAR_SCREEN));
        assert!(CLEAR_SCREEN.ends_with('J'));
    }

    #[test]
    fn test_clear_screen_below_ed0() {
        // ED 0 - Erase from cursor to end of display (ECMA-48 8.3.39)
        assert_eq!(CLEAR_SCREEN_BELOW, "\x1b[J");
        assert!(starts_with_csi(CLEAR_SCREEN_BELOW));
    }

    #[test]
    fn test_clear_screen_above_ed1() {
        // ED 1 - Erase from start of display to cursor (ECMA-48 8.3.39)
        assert_eq!(CLEAR_SCREEN_ABOVE, "\x1b[1J");
        assert!(starts_with_csi(CLEAR_SCREEN_ABOVE));
    }

    #[test]
    fn test_clear_line_el2() {
        // EL 2 - Erase entire line (ECMA-48 8.3.41)
        assert_eq!(CLEAR_LINE, "\x1b[2K");
        assert!(starts_with_csi(CLEAR_LINE));
        assert!(CLEAR_LINE.ends_with('K'));
    }

    #[test]
    fn test_clear_line_right_el0() {
        // EL 0 - Erase from cursor to end of line (ECMA-48 8.3.41)
        assert_eq!(CLEAR_LINE_RIGHT, "\x1b[K");
        assert!(starts_with_csi(CLEAR_LINE_RIGHT));
    }

    #[test]
    fn test_clear_line_left_el1() {
        // EL 1 - Erase from start of line to cursor (ECMA-48 8.3.41)
        assert_eq!(CLEAR_LINE_LEFT, "\x1b[1K");
        assert!(starts_with_csi(CLEAR_LINE_LEFT));
    }

    // =========================================================================
    // Cursor Control Sequences
    // =========================================================================

    #[test]
    fn test_cursor_hide_dectcem() {
        // DECTCEM - DEC Text Cursor Enable Mode (hide)
        // CSI ? 25 l
        assert_eq!(CURSOR_HIDE, "\x1b[?25l");
        assert!(starts_with_csi(CURSOR_HIDE));
        assert!(CURSOR_HIDE.contains("?25"));
        assert!(CURSOR_HIDE.ends_with('l'));
    }

    #[test]
    fn test_cursor_show_dectcem() {
        // DECTCEM - DEC Text Cursor Enable Mode (show)
        // CSI ? 25 h
        assert_eq!(CURSOR_SHOW, "\x1b[?25h");
        assert!(starts_with_csi(CURSOR_SHOW));
        assert!(CURSOR_SHOW.contains("?25"));
        assert!(CURSOR_SHOW.ends_with('h'));
    }

    #[test]
    fn test_cursor_save_decsc() {
        // DECSC - DEC Save Cursor (ESC 7)
        assert_eq!(CURSOR_SAVE, "\x1b7");
        assert!(CURSOR_SAVE.starts_with('\x1b'));
        assert_eq!(CURSOR_SAVE.len(), 2);
    }

    #[test]
    fn test_cursor_restore_decrc() {
        // DECRC - DEC Restore Cursor (ESC 8)
        assert_eq!(CURSOR_RESTORE, "\x1b8");
        assert!(CURSOR_RESTORE.starts_with('\x1b'));
        assert_eq!(CURSOR_RESTORE.len(), 2);
    }

    #[test]
    fn test_cursor_home_cup() {
        // CUP - Cursor Position to (1,1) (ECMA-48 8.3.21)
        // CSI H with no parameters defaults to (1,1)
        assert_eq!(CURSOR_HOME, "\x1b[H");
        assert!(starts_with_csi(CURSOR_HOME));
        assert!(CURSOR_HOME.ends_with('H'));
    }

    #[test]
    fn test_cursor_color_reset_osc112() {
        // OSC 112 - Reset cursor color to default
        assert_eq!(CURSOR_COLOR_RESET, "\x1b]112\x07");
        assert!(starts_with_osc(CURSOR_COLOR_RESET));
        assert!(CURSOR_COLOR_RESET.ends_with('\x07')); // BEL terminator
    }

    #[test]
    fn test_cursor_color_function_osc12() {
        // OSC 12 - Set cursor color to RGB
        let result = cursor_color(255, 128, 0);
        assert!(starts_with_osc(&result));
        assert!(result.contains("12;"));
        assert!(result.contains("#ff8000"));
        assert!(result.ends_with('\x07'));

        // Test black
        let black = cursor_color(0, 0, 0);
        assert!(black.contains("#000000"));

        // Test white
        let white = cursor_color(255, 255, 255);
        assert!(white.contains("#ffffff"));
    }

    // =========================================================================
    // Alternative Screen Buffer (xterm)
    // =========================================================================

    #[test]
    fn test_alt_screen_on_dec1049() {
        // DECSET 1049 - Enable alternative screen buffer
        // CSI ? 1049 h
        assert_eq!(ALT_SCREEN_ON, "\x1b[?1049h");
        assert!(starts_with_csi(ALT_SCREEN_ON));
        assert!(ALT_SCREEN_ON.contains("?1049"));
        assert!(ALT_SCREEN_ON.ends_with('h'));
    }

    #[test]
    fn test_alt_screen_off_dec1049() {
        // DECRST 1049 - Disable alternative screen buffer
        // CSI ? 1049 l
        assert_eq!(ALT_SCREEN_OFF, "\x1b[?1049l");
        assert!(starts_with_csi(ALT_SCREEN_OFF));
        assert!(ALT_SCREEN_OFF.contains("?1049"));
        assert!(ALT_SCREEN_OFF.ends_with('l'));
    }

    // =========================================================================
    // Mouse Tracking (xterm)
    // =========================================================================

    #[test]
    fn test_mouse_on_sgr1006() {
        // Enable mouse tracking:
        // - 1003: Any-event tracking
        // - 1006: SGR extended coordinates
        assert_eq!(MOUSE_ON, "\x1b[?1003h\x1b[?1006h");
        assert!(MOUSE_ON.contains("?1003h")); // Any-event mode
        assert!(MOUSE_ON.contains("?1006h")); // SGR extended
    }

    #[test]
    fn test_mouse_off() {
        // Disable mouse tracking (reverse order)
        assert_eq!(MOUSE_OFF, "\x1b[?1003l\x1b[?1006l");
        assert!(MOUSE_OFF.contains("?1003l"));
        assert!(MOUSE_OFF.contains("?1006l"));
    }

    // =========================================================================
    // Bracketed Paste Mode (xterm)
    // =========================================================================

    #[test]
    fn test_bracketed_paste_on() {
        // DECSET 2004 - Enable bracketed paste
        assert_eq!(BRACKETED_PASTE_ON, "\x1b[?2004h");
        assert!(starts_with_csi(BRACKETED_PASTE_ON));
        assert!(BRACKETED_PASTE_ON.contains("?2004"));
    }

    #[test]
    fn test_bracketed_paste_off() {
        // DECRST 2004 - Disable bracketed paste
        assert_eq!(BRACKETED_PASTE_OFF, "\x1b[?2004l");
        assert!(starts_with_csi(BRACKETED_PASTE_OFF));
    }

    // =========================================================================
    // Focus Tracking (xterm)
    // =========================================================================

    #[test]
    fn test_focus_on() {
        // DECSET 1004 - Enable focus reporting
        assert_eq!(FOCUS_ON, "\x1b[?1004h");
        assert!(starts_with_csi(FOCUS_ON));
        assert!(FOCUS_ON.contains("?1004"));
    }

    #[test]
    fn test_focus_off() {
        // DECRST 1004 - Disable focus reporting
        assert_eq!(FOCUS_OFF, "\x1b[?1004l");
        assert!(starts_with_csi(FOCUS_OFF));
    }

    // =========================================================================
    // Terminal Size Query
    // =========================================================================

    #[test]
    fn test_request_size_xtwinops() {
        // XTWINOPS 18 - Report text area size in characters
        assert_eq!(REQUEST_SIZE, "\x1b[18t");
        assert!(starts_with_csi(REQUEST_SIZE));
        assert!(REQUEST_SIZE.ends_with('t'));
    }

    // =========================================================================
    // Query Sequences Module
    // =========================================================================

    #[test]
    fn test_query_device_attributes_da1() {
        // DA1 - Primary Device Attributes
        // CSI c (or CSI 0 c)
        assert_eq!(query::DEVICE_ATTRIBUTES, "\x1b[c");
        assert!(starts_with_csi(query::DEVICE_ATTRIBUTES));
        assert!(query::DEVICE_ATTRIBUTES.ends_with('c'));
    }

    #[test]
    fn test_query_device_attributes_secondary_da2() {
        // DA2 - Secondary Device Attributes
        // CSI > c (or CSI > 0 c)
        assert_eq!(query::DEVICE_ATTRIBUTES_SECONDARY, "\x1b[>c");
        assert!(starts_with_csi(query::DEVICE_ATTRIBUTES_SECONDARY));
        assert!(query::DEVICE_ATTRIBUTES_SECONDARY.contains('>'));
    }

    #[test]
    fn test_query_xtversion() {
        // XTVERSION - Request xterm version string
        // CSI > 0 q
        assert_eq!(query::XTVERSION, "\x1b[>0q");
        assert!(starts_with_csi(query::XTVERSION));
        assert!(query::XTVERSION.ends_with('q'));
    }

    #[test]
    fn test_query_pixel_resolution() {
        // XTWINOPS 14 - Report text area size in pixels
        assert_eq!(query::PIXEL_RESOLUTION, "\x1b[14t");
        assert!(starts_with_csi(query::PIXEL_RESOLUTION));
        assert!(query::PIXEL_RESOLUTION.ends_with('t'));
    }

    #[test]
    fn test_query_kitty_keyboard() {
        // Kitty keyboard protocol query
        // CSI ? u
        assert_eq!(query::KITTY_KEYBOARD, "\x1b[?u");
        assert!(starts_with_csi(query::KITTY_KEYBOARD));
        assert!(query::KITTY_KEYBOARD.ends_with('u'));
    }

    // =========================================================================
    // Title Sequences (OSC)
    // =========================================================================

    #[test]
    fn test_title_prefix_osc0() {
        // OSC 0 - Set window title
        // OSC 0 ; <title> ST
        assert_eq!(TITLE_PREFIX, "\x1b]0;");
        assert!(starts_with_osc(TITLE_PREFIX));
        assert!(TITLE_PREFIX.contains("0;"));
    }

    #[test]
    fn test_title_suffix_st() {
        // ST - String Terminator (ESC \)
        assert_eq!(TITLE_SUFFIX, "\x1b\\");
        assert!(TITLE_SUFFIX.starts_with('\x1b'));
        assert_eq!(TITLE_SUFFIX.len(), 2);
    }

    #[test]
    fn test_title_sequence_complete() {
        // Verify we can construct a complete title sequence
        let title = "Test Title";
        let full = format!("{TITLE_PREFIX}{title}{TITLE_SUFFIX}");
        assert!(full.starts_with("\x1b]0;"));
        assert!(full.contains("Test Title"));
        assert!(full.ends_with("\x1b\\"));
    }

    // =========================================================================
    // Soft Reset
    // =========================================================================

    #[test]
    fn test_soft_reset_ris() {
        // RIS - Reset to Initial State (ESC c)
        assert_eq!(SOFT_RESET, "\x1bc");
        assert!(SOFT_RESET.starts_with('\x1b'));
        assert_eq!(SOFT_RESET.len(), 2);
    }

    // =========================================================================
    // Cursor Style Module (DECSCUSR)
    // =========================================================================

    #[test]
    fn test_cursor_style_block_blink() {
        // DECSCUSR 1 - Blinking block cursor
        assert_eq!(cursor_style::BLOCK_BLINK, "\x1b[1 q");
        assert!(starts_with_csi(cursor_style::BLOCK_BLINK));
        assert!(cursor_style::BLOCK_BLINK.ends_with(" q"));
    }

    #[test]
    fn test_cursor_style_block_steady() {
        // DECSCUSR 2 - Steady block cursor
        assert_eq!(cursor_style::BLOCK_STEADY, "\x1b[2 q");
        assert!(cursor_style::BLOCK_STEADY.contains('2'));
    }

    #[test]
    fn test_cursor_style_underline_blink() {
        // DECSCUSR 3 - Blinking underline cursor
        assert_eq!(cursor_style::UNDERLINE_BLINK, "\x1b[3 q");
        assert!(cursor_style::UNDERLINE_BLINK.contains('3'));
    }

    #[test]
    fn test_cursor_style_underline_steady() {
        // DECSCUSR 4 - Steady underline cursor
        assert_eq!(cursor_style::UNDERLINE_STEADY, "\x1b[4 q");
        assert!(cursor_style::UNDERLINE_STEADY.contains('4'));
    }

    #[test]
    fn test_cursor_style_bar_blink() {
        // DECSCUSR 5 - Blinking bar cursor
        assert_eq!(cursor_style::BAR_BLINK, "\x1b[5 q");
        assert!(cursor_style::BAR_BLINK.contains('5'));
    }

    #[test]
    fn test_cursor_style_bar_steady() {
        // DECSCUSR 6 - Steady bar cursor
        assert_eq!(cursor_style::BAR_STEADY, "\x1b[6 q");
        assert!(cursor_style::BAR_STEADY.contains('6'));
    }

    #[test]
    fn test_cursor_style_default() {
        // DECSCUSR 0 - Default cursor style
        assert_eq!(cursor_style::DEFAULT, "\x1b[0 q");
        assert!(cursor_style::DEFAULT.contains('0'));
    }

    // =========================================================================
    // Synchronized Output Module
    // =========================================================================

    #[test]
    fn test_sync_begin() {
        // DECSET 2026 - Begin synchronized update
        assert_eq!(sync::BEGIN, "\x1b[?2026h");
        assert!(starts_with_csi(sync::BEGIN));
        assert!(sync::BEGIN.contains("?2026"));
        assert!(sync::BEGIN.ends_with('h'));
    }

    #[test]
    fn test_sync_end() {
        // DECRST 2026 - End synchronized update
        assert_eq!(sync::END, "\x1b[?2026l");
        assert!(starts_with_csi(sync::END));
        assert!(sync::END.contains("?2026"));
        assert!(sync::END.ends_with('l'));
    }

    // =========================================================================
    // Color Reset Module
    // =========================================================================

    #[test]
    fn test_color_fg_default_sgr39() {
        // SGR 39 - Default foreground color
        assert_eq!(color::FG_DEFAULT, "\x1b[39m");
        assert!(starts_with_csi(color::FG_DEFAULT));
        assert!(color::FG_DEFAULT.ends_with('m'));
    }

    #[test]
    fn test_color_bg_default_sgr49() {
        // SGR 49 - Default background color
        assert_eq!(color::BG_DEFAULT, "\x1b[49m");
        assert!(starts_with_csi(color::BG_DEFAULT));
        assert!(color::BG_DEFAULT.ends_with('m'));
    }

    // =========================================================================
    // Attribute Reset Module
    // =========================================================================

    #[test]
    fn test_attr_reset_intensity_sgr22() {
        // SGR 22 - Normal intensity (neither bold nor dim)
        assert_eq!(attr::RESET_INTENSITY, "\x1b[22m");
        assert!(starts_with_csi(attr::RESET_INTENSITY));
    }

    #[test]
    fn test_attr_reset_italic_sgr23() {
        // SGR 23 - Not italic
        assert_eq!(attr::RESET_ITALIC, "\x1b[23m");
        assert!(starts_with_csi(attr::RESET_ITALIC));
    }

    #[test]
    fn test_attr_reset_underline_sgr24() {
        // SGR 24 - Not underlined
        assert_eq!(attr::RESET_UNDERLINE, "\x1b[24m");
        assert!(starts_with_csi(attr::RESET_UNDERLINE));
    }

    #[test]
    fn test_attr_reset_blink_sgr25() {
        // SGR 25 - Not blinking
        assert_eq!(attr::RESET_BLINK, "\x1b[25m");
        assert!(starts_with_csi(attr::RESET_BLINK));
    }

    #[test]
    fn test_attr_reset_inverse_sgr27() {
        // SGR 27 - Not reversed
        assert_eq!(attr::RESET_INVERSE, "\x1b[27m");
        assert!(starts_with_csi(attr::RESET_INVERSE));
    }

    #[test]
    fn test_attr_reset_hidden_sgr28() {
        // SGR 28 - Not hidden
        assert_eq!(attr::RESET_HIDDEN, "\x1b[28m");
        assert!(starts_with_csi(attr::RESET_HIDDEN));
    }

    #[test]
    fn test_attr_reset_strikethrough_sgr29() {
        // SGR 29 - Not crossed out
        assert_eq!(attr::RESET_STRIKETHROUGH, "\x1b[29m");
        assert!(starts_with_csi(attr::RESET_STRIKETHROUGH));
    }

    // =========================================================================
    // Sequence Structure Validation
    // =========================================================================

    #[test]
    fn test_all_csi_sequences_have_terminator() {
        // CSI sequences must end with a letter
        let csi_sequences = [
            RESET,
            CLEAR_SCREEN,
            CLEAR_SCREEN_BELOW,
            CLEAR_SCREEN_ABOVE,
            CLEAR_LINE,
            CLEAR_LINE_RIGHT,
            CLEAR_LINE_LEFT,
            CURSOR_HIDE,
            CURSOR_SHOW,
            CURSOR_HOME,
            ALT_SCREEN_ON,
            ALT_SCREEN_OFF,
            BRACKETED_PASTE_ON,
            BRACKETED_PASTE_OFF,
            FOCUS_ON,
            FOCUS_OFF,
            REQUEST_SIZE,
        ];

        for seq in &csi_sequences {
            assert!(
                starts_with_csi(seq),
                "Sequence {seq:?} should start with CSI"
            );
            let last_char = seq.chars().last().unwrap();
            assert!(
                last_char.is_ascii_alphabetic(),
                "Sequence {seq:?} should end with letter, got {last_char:?}"
            );
        }
    }

    #[test]
    fn test_all_sequences_are_valid_utf8() {
        // All sequences should be valid UTF-8 strings
        let all_sequences: &[&str] = &[
            RESET,
            CLEAR_SCREEN,
            CLEAR_SCREEN_BELOW,
            CLEAR_SCREEN_ABOVE,
            CLEAR_LINE,
            CLEAR_LINE_RIGHT,
            CLEAR_LINE_LEFT,
            CURSOR_HIDE,
            CURSOR_SHOW,
            CURSOR_SAVE,
            CURSOR_RESTORE,
            CURSOR_HOME,
            CURSOR_COLOR_RESET,
            ALT_SCREEN_ON,
            ALT_SCREEN_OFF,
            MOUSE_ON,
            MOUSE_OFF,
            BRACKETED_PASTE_ON,
            BRACKETED_PASTE_OFF,
            FOCUS_ON,
            FOCUS_OFF,
            REQUEST_SIZE,
            TITLE_PREFIX,
            TITLE_SUFFIX,
            SOFT_RESET,
            query::DEVICE_ATTRIBUTES,
            query::DEVICE_ATTRIBUTES_SECONDARY,
            query::XTVERSION,
            query::PIXEL_RESOLUTION,
            query::KITTY_KEYBOARD,
            cursor_style::BLOCK_BLINK,
            cursor_style::BLOCK_STEADY,
            cursor_style::UNDERLINE_BLINK,
            cursor_style::UNDERLINE_STEADY,
            cursor_style::BAR_BLINK,
            cursor_style::BAR_STEADY,
            cursor_style::DEFAULT,
            sync::BEGIN,
            sync::END,
            color::FG_DEFAULT,
            color::BG_DEFAULT,
            attr::RESET_INTENSITY,
            attr::RESET_ITALIC,
            attr::RESET_UNDERLINE,
            attr::RESET_BLINK,
            attr::RESET_INVERSE,
            attr::RESET_HIDDEN,
            attr::RESET_STRIKETHROUGH,
        ];

        for seq in all_sequences {
            // If we got here, the string is valid UTF-8 (Rust guarantees this)
            assert!(!seq.is_empty(), "Sequence should not be empty");
            // Verify ESC character is present
            assert!(
                seq.contains('\x1b'),
                "Sequence {seq:?} should contain ESC character"
            );
        }
    }

    #[test]
    fn test_set_reset_pairs() {
        // Verify that SET/RESET pairs use h/l consistently
        let pairs = [
            (CURSOR_HIDE, CURSOR_SHOW),
            (ALT_SCREEN_ON, ALT_SCREEN_OFF),
            (BRACKETED_PASTE_ON, BRACKETED_PASTE_OFF),
            (FOCUS_ON, FOCUS_OFF),
            (sync::BEGIN, sync::END),
        ];

        for (set_seq, reset_seq) in &pairs {
            let set_last = set_seq.chars().last().unwrap();
            let reset_last = reset_seq.chars().last().unwrap();

            // SET ends with 'h', RESET ends with 'l'
            assert!(
                set_last == 'h' || set_last == 'l',
                "SET sequence should end with h or l"
            );
            assert!(
                reset_last == 'h' || reset_last == 'l',
                "RESET sequence should end with h or l"
            );
            assert_ne!(
                set_last, reset_last,
                "SET and RESET should have opposite terminators"
            );
        }
    }
}
