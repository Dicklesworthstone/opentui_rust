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
        let kitty_present = kitty_window_id.is_some();
        let hyperlinks = Self::detect_hyperlinks(&term, &term_program, kitty_present);
        let sync_output = Self::detect_sync(&term, &term_program, kitty_present);
        let kitty_keyboard = kitty_present;
        let kitty_graphics = kitty_present;

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

    /// Detect hyperlink support from multiple signals.
    ///
    /// Considers:
    /// - `TERM_PROGRAM`: WezTerm, Alacritty, kitty, ghostty, iTerm.app, Apple_Terminal, Hyper
    /// - `TERM`: kitty, ghostty, wezterm, alacritty, xterm-kitty
    /// - `KITTY_WINDOW_ID` presence
    fn detect_hyperlinks(term: &str, term_program: &str, kitty_present: bool) -> bool {
        // KITTY_WINDOW_ID present -> kitty features supported
        if kitty_present {
            return true;
        }

        // Known terminals with hyperlink support via TERM_PROGRAM
        let supported_programs = [
            "iTerm.app",
            "Apple_Terminal",
            "WezTerm",
            "Hyper",
            "Alacritty",
            "kitty",
            "ghostty",
        ];
        if supported_programs
            .iter()
            .any(|t| term_program.eq_ignore_ascii_case(t) || term_program.contains(t))
        {
            return true;
        }

        // Known terminals via TERM value
        let term_lower = term.to_lowercase();
        let supported_terms = ["kitty", "ghostty", "wezterm", "alacritty"];
        supported_terms.iter().any(|t| term_lower.contains(t))
    }

    /// Detect synchronized output support from multiple signals.
    ///
    /// Considers:
    /// - `TERM_PROGRAM`: kitty, Alacritty, WezTerm, ghostty
    /// - `TERM`: kitty, ghostty, wezterm, alacritty
    /// - `KITTY_WINDOW_ID` presence
    fn detect_sync(term: &str, term_program: &str, kitty_present: bool) -> bool {
        // KITTY_WINDOW_ID present -> kitty features supported
        if kitty_present {
            return true;
        }

        // Terminals known to support synchronized output via TERM_PROGRAM
        let supported_programs = ["kitty", "Alacritty", "WezTerm", "ghostty"];
        if supported_programs
            .iter()
            .any(|t| term_program.eq_ignore_ascii_case(t) || term_program.contains(t))
        {
            return true;
        }

        // Known terminals via TERM value
        let term_lower = term.to_lowercase();
        let supported_terms = ["kitty", "ghostty", "wezterm", "alacritty"];
        supported_terms.iter().any(|t| term_lower.contains(t))
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

    // === Hyperlink detection tests ===

    #[test]
    fn test_hyperlinks_via_term_program() {
        // WezTerm
        assert!(Capabilities::detect_hyperlinks(
            "xterm-256color",
            "WezTerm",
            false
        ));
        // Alacritty
        assert!(Capabilities::detect_hyperlinks(
            "alacritty",
            "Alacritty",
            false
        ));
        // kitty
        assert!(Capabilities::detect_hyperlinks(
            "xterm-256color",
            "kitty",
            false
        ));
        // ghostty
        assert!(Capabilities::detect_hyperlinks(
            "xterm-256color",
            "ghostty",
            false
        ));
        // iTerm
        assert!(Capabilities::detect_hyperlinks(
            "xterm-256color",
            "iTerm.app",
            false
        ));
        // Apple Terminal
        assert!(Capabilities::detect_hyperlinks(
            "xterm-256color",
            "Apple_Terminal",
            false
        ));
        // Hyper
        assert!(Capabilities::detect_hyperlinks(
            "xterm-256color",
            "Hyper",
            false
        ));
    }

    #[test]
    fn test_hyperlinks_via_term() {
        // kitty via TERM (common when TERM_PROGRAM is unset)
        assert!(Capabilities::detect_hyperlinks("xterm-kitty", "", false));
        assert!(Capabilities::detect_hyperlinks("kitty", "", false));
        // ghostty via TERM
        assert!(Capabilities::detect_hyperlinks("ghostty", "", false));
        // wezterm via TERM
        assert!(Capabilities::detect_hyperlinks("wezterm", "", false));
        // alacritty via TERM
        assert!(Capabilities::detect_hyperlinks("alacritty", "", false));
    }

    #[test]
    fn test_hyperlinks_via_kitty_window_id() {
        // KITTY_WINDOW_ID present should enable hyperlinks even with unknown term
        assert!(Capabilities::detect_hyperlinks("xterm-256color", "", true));
        assert!(Capabilities::detect_hyperlinks("linux", "", true));
    }

    #[test]
    fn test_hyperlinks_false_for_unknown() {
        // Unknown terminal with no special env vars
        assert!(!Capabilities::detect_hyperlinks(
            "xterm-256color",
            "",
            false
        ));
        assert!(!Capabilities::detect_hyperlinks(
            "screen-256color",
            "",
            false
        ));
        assert!(!Capabilities::detect_hyperlinks("linux", "", false));
        assert!(!Capabilities::detect_hyperlinks("vt100", "", false));
        // Empty values
        assert!(!Capabilities::detect_hyperlinks("", "", false));
    }

    // === Sync output detection tests ===

    #[test]
    fn test_sync_via_term_program() {
        // kitty
        assert!(Capabilities::detect_sync("xterm-256color", "kitty", false));
        // Alacritty
        assert!(Capabilities::detect_sync(
            "xterm-256color",
            "Alacritty",
            false
        ));
        // WezTerm
        assert!(Capabilities::detect_sync(
            "xterm-256color",
            "WezTerm",
            false
        ));
        // ghostty
        assert!(Capabilities::detect_sync(
            "xterm-256color",
            "ghostty",
            false
        ));
    }

    #[test]
    fn test_sync_via_term() {
        // kitty via TERM
        assert!(Capabilities::detect_sync("xterm-kitty", "", false));
        assert!(Capabilities::detect_sync("kitty", "", false));
        // ghostty via TERM
        assert!(Capabilities::detect_sync("ghostty", "", false));
        // wezterm via TERM
        assert!(Capabilities::detect_sync("wezterm", "", false));
        // alacritty via TERM
        assert!(Capabilities::detect_sync("alacritty", "", false));
    }

    #[test]
    fn test_sync_via_kitty_window_id() {
        // KITTY_WINDOW_ID present should enable sync even with unknown term
        assert!(Capabilities::detect_sync("xterm-256color", "", true));
        assert!(Capabilities::detect_sync("linux", "", true));
    }

    #[test]
    fn test_sync_false_for_unknown() {
        // Unknown terminal with no special env vars
        assert!(!Capabilities::detect_sync("xterm-256color", "", false));
        assert!(!Capabilities::detect_sync("screen-256color", "", false));
        assert!(!Capabilities::detect_sync("tmux-256color", "", false));
        assert!(!Capabilities::detect_sync("linux", "", false));
        // Note: iTerm and Apple_Terminal don't support sync output
        assert!(!Capabilities::detect_sync(
            "xterm-256color",
            "iTerm.app",
            false
        ));
        assert!(!Capabilities::detect_sync(
            "xterm-256color",
            "Apple_Terminal",
            false
        ));
    }

    #[test]
    fn test_case_insensitive_term_matching() {
        // TERM values should match case-insensitively
        assert!(Capabilities::detect_hyperlinks("KITTY", "", false));
        assert!(Capabilities::detect_hyperlinks("Kitty", "", false));
        assert!(Capabilities::detect_sync("ALACRITTY", "", false));
        assert!(Capabilities::detect_sync("Alacritty", "", false));
    }
}
