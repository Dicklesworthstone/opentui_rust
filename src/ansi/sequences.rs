//! Constant ANSI escape sequences.

/// Reset all attributes to default.
pub const RESET: &str = "\x1b[0m";

/// Clear entire screen.
pub const CLEAR_SCREEN: &str = "\x1b[2J";

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
