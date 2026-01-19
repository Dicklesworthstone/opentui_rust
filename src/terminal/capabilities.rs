//! Terminal capability detection.

use crate::unicode::WidthMethod;
use std::env;

/// Color support level.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum ColorSupport {
    /// No color support.
    #[default]
    None,
    /// 16 colors (basic ANSI).
    Basic,
    /// 256 colors.
    Extended,
    /// True color (16 million colors).
    TrueColor,
}

/// Detected terminal capabilities.
#[derive(Clone, Debug)]
pub struct Capabilities {
    /// Color support level.
    pub color: ColorSupport,
    /// Terminal supports Unicode.
    pub unicode: bool,
    /// Preferred width calculation method.
    pub width_method: WidthMethod,
    /// Terminal supports hyperlinks (OSC 8).
    pub hyperlinks: bool,
    /// Terminal supports synchronized output.
    pub sync_output: bool,
    /// Terminal supports mouse tracking.
    pub mouse: bool,
    /// Terminal supports focus events.
    pub focus: bool,
    /// Terminal supports bracketed paste.
    pub bracketed_paste: bool,
    /// Kitty keyboard protocol.
    pub kitty_keyboard: bool,
    /// Kitty graphics protocol.
    pub kitty_graphics: bool,
    /// SGR pixel mouse mode.
    pub sgr_pixels: bool,
    /// Terminal supports dynamic color scheme updates.
    pub color_scheme_updates: bool,
    /// Terminal supports explicit width reporting.
    pub explicit_width: bool,
    /// Terminal supports scaled text.
    pub scaled_text: bool,
    /// Sixel graphics support.
    pub sixel: bool,
    /// Terminal supports explicit cursor positioning (DECCRA).
    pub explicit_cursor_positioning: bool,
    /// Terminal name if known.
    pub term_name: Option<String>,
}

impl Default for Capabilities {
    fn default() -> Self {
        Self {
            color: ColorSupport::TrueColor,
            unicode: true,
            width_method: WidthMethod::default(),
            hyperlinks: true,
            sync_output: true,
            mouse: true,
            focus: true,
            bracketed_paste: true,
            kitty_keyboard: false,
            kitty_graphics: false,
            sgr_pixels: false,
            color_scheme_updates: false,
            explicit_width: false,
            scaled_text: false,
            sixel: false,
            explicit_cursor_positioning: true,
            term_name: None,
        }
    }
}

impl Capabilities {
    /// Detect terminal capabilities from environment.
    #[must_use]
    pub fn detect() -> Self {
        let term = env::var("TERM").unwrap_or_default();
        let colorterm = env::var("COLORTERM").unwrap_or_default();
        let term_program = env::var("TERM_PROGRAM").unwrap_or_default();
        let kitty_window_id = env::var("KITTY_WINDOW_ID").ok();

        let color = Self::detect_color(&term, &colorterm);
        let unicode = Self::detect_unicode();
        let hyperlinks = Self::detect_hyperlinks(&term_program);
        let sync_output = Self::detect_sync(&term_program);
        let kitty_keyboard = kitty_window_id.is_some();
        let kitty_graphics = kitty_window_id.is_some();

        Self {
            color,
            unicode,
            width_method: WidthMethod::default(),
            hyperlinks,
            sync_output,
            mouse: true,
            focus: true,
            bracketed_paste: true,
            kitty_keyboard,
            kitty_graphics,
            sgr_pixels: false,
            color_scheme_updates: false,
            explicit_width: false,
            scaled_text: false,
            sixel: term.contains("sixel"),
            explicit_cursor_positioning: true,
            term_name: Some(term),
        }
    }

    /// Apply a best-effort capability response (from query output).
    pub fn apply_query_response(&mut self, response: &str) {
        if response.contains("[?u") {
            self.kitty_keyboard = true;
        }

        if let Some((width, height)) = parse_pixel_resolution(response) {
            if width > 0 && height > 0 {
                self.explicit_width = true;
                self.sgr_pixels = true;
            }
        }

        let lower = response.to_lowercase();
        if lower.contains("kitty") {
            self.kitty_graphics = true;
            self.kitty_keyboard = true;
        } else if lower.contains("wezterm") || lower.contains("alacritty") {
            self.sync_output = true;
        }
    }

    fn detect_color(term: &str, colorterm: &str) -> ColorSupport {
        // Check for explicit true color support
        if colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit") {
            return ColorSupport::TrueColor;
        }

        // Check term for true color indicators
        if term.contains("256color") || term.contains("24bit") || term.contains("truecolor") {
            return ColorSupport::TrueColor;
        }

        // Known true color terminals
        let truecolor_terms = [
            "xterm-256color",
            "screen-256color",
            "tmux-256color",
            "alacritty",
            "kitty",
            "wezterm",
            "ghostty",
        ];

        if truecolor_terms.iter().any(|t| term.contains(t)) {
            return ColorSupport::TrueColor;
        }

        // 256 color
        if term.contains("256") {
            return ColorSupport::Extended;
        }

        // Basic color
        if term.starts_with("xterm") || term.starts_with("screen") || term.starts_with("vt100") {
            return ColorSupport::Basic;
        }

        // Assume basic color if TERM is set
        if !term.is_empty() {
            return ColorSupport::Basic;
        }

        ColorSupport::None
    }

    fn detect_unicode() -> bool {
        // Check locale for UTF-8
        let lang = env::var("LANG").unwrap_or_default();
        let lc_all = env::var("LC_ALL").unwrap_or_default();
        let lc_ctype = env::var("LC_CTYPE").unwrap_or_default();

        lang.to_lowercase().contains("utf")
            || lc_all.to_lowercase().contains("utf")
            || lc_ctype.to_lowercase().contains("utf")
    }

    fn detect_hyperlinks(term_program: &str) -> bool {
        // Known terminals with hyperlink support
        let supported = [
            "iTerm.app",
            "Apple_Terminal",
            "WezTerm",
            "Hyper",
            "Alacritty",
            "kitty",
            "ghostty",
        ];
        supported.iter().any(|t| term_program.contains(t))
    }

    fn detect_sync(term_program: &str) -> bool {
        // Terminals known to support synchronized output
        let supported = ["kitty", "Alacritty", "WezTerm", "ghostty"];
        supported.iter().any(|t| term_program.contains(t))
    }

    /// Check if true color is supported.
    #[must_use]
    pub fn has_true_color(&self) -> bool {
        self.color >= ColorSupport::TrueColor
    }

    /// Check if 256 colors are supported.
    #[must_use]
    pub fn has_256_colors(&self) -> bool {
        self.color >= ColorSupport::Extended
    }
}

fn parse_pixel_resolution(response: &str) -> Option<(u32, u32)> {
    let start = response.find("[4;")?;
    let payload = &response[start + 3..];
    let end = payload.find('t')?;
    let payload = &payload[..end];
    let mut parts = payload.split(';');
    let height = parts.next()?.parse::<u32>().ok()?;
    let width = parts.next()?.parse::<u32>().ok()?;
    Some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pixel_resolution() {
        let response = "\x1b[4;900;1440t";
        assert_eq!(parse_pixel_resolution(response), Some((1440, 900)));
    }

    #[test]
    fn test_apply_query_response_flags() {
        let mut caps = Capabilities {
            kitty_keyboard: false,
            explicit_width: false,
            sgr_pixels: false,
            ..Capabilities::default()
        };

        caps.apply_query_response("\x1b[?u");
        caps.apply_query_response("\x1b[4;900;1440t");

        assert!(caps.kitty_keyboard);
        assert!(caps.explicit_width);
        assert!(caps.sgr_pixels);
    }

    #[test]
    fn test_color_support_ordering() {
        assert!(ColorSupport::TrueColor > ColorSupport::Extended);
        assert!(ColorSupport::Extended > ColorSupport::Basic);
        assert!(ColorSupport::Basic > ColorSupport::None);
    }

    #[test]
    fn test_capabilities_default() {
        let caps = Capabilities::default();
        assert_eq!(caps.color, ColorSupport::TrueColor);
        assert!(caps.unicode);
    }
}
