//! Test data generators for OpenTUI tests.
//!
//! Provides sample data, random generators, and common test scenarios.

use opentui::buffer::OptimizedBuffer;
use opentui::cell::Cell;
use opentui::color::Rgba;
use opentui::style::{Style, TextAttributes};

/// Sample ANSI escape sequences for testing.
pub mod ansi_sequences {
    /// Clear entire screen.
    pub const CLEAR_SCREEN: &[u8] = b"\x1b[2J";

    /// Move cursor to home position (1,1).
    pub const CURSOR_HOME: &[u8] = b"\x1b[H";

    /// Hide cursor.
    pub const CURSOR_HIDE: &[u8] = b"\x1b[?25l";

    /// Show cursor.
    pub const CURSOR_SHOW: &[u8] = b"\x1b[?25h";

    /// Enter alternate screen buffer.
    pub const ALT_SCREEN_ENTER: &[u8] = b"\x1b[?1049h";

    /// Leave alternate screen buffer.
    pub const ALT_SCREEN_LEAVE: &[u8] = b"\x1b[?1049l";

    /// Enable mouse button tracking (SGR mode).
    pub const MOUSE_ENABLE: &[u8] = b"\x1b[?1000h\x1b[?1006h";

    /// Disable mouse tracking.
    pub const MOUSE_DISABLE: &[u8] = b"\x1b[?1000l\x1b[?1006l";

    /// Enable mouse motion tracking.
    pub const MOUSE_MOTION_ENABLE: &[u8] = b"\x1b[?1003h";

    /// Reset all attributes.
    pub const SGR_RESET: &[u8] = b"\x1b[0m";

    /// Bold on.
    pub const SGR_BOLD: &[u8] = b"\x1b[1m";

    /// Italic on.
    pub const SGR_ITALIC: &[u8] = b"\x1b[3m";

    /// Underline on.
    pub const SGR_UNDERLINE: &[u8] = b"\x1b[4m";

    /// Red foreground.
    pub const SGR_FG_RED: &[u8] = b"\x1b[31m";

    /// Green foreground.
    pub const SGR_FG_GREEN: &[u8] = b"\x1b[32m";

    /// Blue foreground.
    pub const SGR_FG_BLUE: &[u8] = b"\x1b[34m";

    /// Red background.
    pub const SGR_BG_RED: &[u8] = b"\x1b[41m";

    /// Synchronized update begin.
    pub const SYNC_BEGIN: &[u8] = b"\x1b[?2026h";

    /// Synchronized update end.
    pub const SYNC_END: &[u8] = b"\x1b[?2026l";

    /// Build a cursor position sequence.
    pub fn cursor_position(row: u16, col: u16) -> Vec<u8> {
        format!("\x1b[{};{}H", row, col).into_bytes()
    }

    /// Build a 24-bit foreground color sequence.
    pub fn fg_rgb(r: u8, g: u8, b: u8) -> Vec<u8> {
        format!("\x1b[38;2;{};{};{}m", r, g, b).into_bytes()
    }

    /// Build a 24-bit background color sequence.
    pub fn bg_rgb(r: u8, g: u8, b: u8) -> Vec<u8> {
        format!("\x1b[48;2;{};{};{}m", r, g, b).into_bytes()
    }

    /// Build a 256-color foreground sequence.
    pub fn fg_256(color: u8) -> Vec<u8> {
        format!("\x1b[38;5;{}m", color).into_bytes()
    }

    /// Build a 256-color background sequence.
    pub fn bg_256(color: u8) -> Vec<u8> {
        format!("\x1b[48;5;{}m", color).into_bytes()
    }

    /// Build an SGR mouse event sequence.
    pub fn mouse_sgr(button: u8, x: u16, y: u16, press: bool) -> Vec<u8> {
        let suffix = if press { 'M' } else { 'm' };
        format!("\x1b[<{};{};{}{}", button, x + 1, y + 1, suffix).into_bytes()
    }
}

/// Sample text content for testing.
pub mod sample_text {
    /// Short ASCII text.
    pub const SHORT_ASCII: &str = "Hello, World!";

    /// Longer ASCII text for wrapping tests.
    pub const LONG_ASCII: &str = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs.";

    /// Multi-line text.
    pub const MULTILINE: &str = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";

    /// Text with tabs.
    pub const WITH_TABS: &str = "Col1\tCol2\tCol3\nA\tB\tC";

    /// Unicode text with various scripts.
    pub const UNICODE_MIXED: &str = "Hello 世界 مرحبا Привет 🌍";

    /// CJK text (full-width characters).
    pub const CJK: &str = "日本語テキスト";

    /// Emoji text.
    pub const EMOJI_BASIC: &str = "😀 🎉 🚀 ❤️ 🔥";

    /// ZWJ emoji sequences.
    pub const EMOJI_ZWJ: &str = "👨‍👩‍👧‍👦 👩‍💻 🏳️‍🌈";

    /// Text with combining characters.
    pub const COMBINING: &str = "café naïve résumé";

    /// RTL text (Arabic).
    pub const RTL_ARABIC: &str = "مرحبا بالعالم";

    /// Code snippet for syntax highlighting tests.
    pub const RUST_CODE: &str = r#"fn main() {
    let x = 42;
    println!("Hello, {}!", x);
}"#;

    /// JSON sample.
    pub const JSON_SAMPLE: &str = r#"{"name": "test", "value": 123, "active": true}"#;

    /// Markdown sample.
    pub const MARKDOWN_SAMPLE: &str = "# Heading\n\n**Bold** and *italic* text.\n\n- List item 1\n- List item 2";
}

/// Common color palettes for testing.
pub mod colors {
    use super::*;

    /// Basic ANSI colors.
    pub const ANSI_BLACK: Rgba = Rgba::rgb(0.0, 0.0, 0.0);
    pub const ANSI_RED: Rgba = Rgba::rgb(0.8, 0.0, 0.0);
    pub const ANSI_GREEN: Rgba = Rgba::rgb(0.0, 0.8, 0.0);
    pub const ANSI_YELLOW: Rgba = Rgba::rgb(0.8, 0.8, 0.0);
    pub const ANSI_BLUE: Rgba = Rgba::rgb(0.0, 0.0, 0.8);
    pub const ANSI_MAGENTA: Rgba = Rgba::rgb(0.8, 0.0, 0.8);
    pub const ANSI_CYAN: Rgba = Rgba::rgb(0.0, 0.8, 0.8);
    pub const ANSI_WHITE: Rgba = Rgba::rgb(0.8, 0.8, 0.8);

    /// Semi-transparent colors for blending tests.
    pub const SEMI_RED: Rgba = Rgba::new(1.0, 0.0, 0.0, 0.5);
    pub const SEMI_GREEN: Rgba = Rgba::new(0.0, 1.0, 0.0, 0.5);
    pub const SEMI_BLUE: Rgba = Rgba::new(0.0, 0.0, 1.0, 0.5);

    /// Generate a grayscale color.
    pub fn grayscale(level: f32) -> Rgba {
        Rgba::rgb(level, level, level)
    }

    /// Generate a color from HSV.
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Rgba {
        Rgba::from_hsv(h, s, v)
    }

    /// Generate a random-ish color from a seed (deterministic).
    pub fn from_seed(seed: u32) -> Rgba {
        let r = (seed.wrapping_mul(1103515245).wrapping_add(12345) % 256) as f32 / 255.0;
        let g = (seed.wrapping_mul(1103515245).wrapping_add(12346) % 256) as f32 / 255.0;
        let b = (seed.wrapping_mul(1103515245).wrapping_add(12347) % 256) as f32 / 255.0;
        Rgba::rgb(r, g, b)
    }
}

/// Buffer generators for common test scenarios.
pub mod buffers {
    use super::*;

    /// Create an empty buffer with default cells.
    pub fn empty(width: u32, height: u32) -> OptimizedBuffer {
        OptimizedBuffer::new(width, height)
    }

    /// Create a buffer cleared to a specific background color.
    pub fn cleared(width: u32, height: u32, bg: Rgba) -> OptimizedBuffer {
        let mut buffer = OptimizedBuffer::new(width, height);
        buffer.clear(bg);
        buffer
    }

    /// Create a buffer with a checkerboard pattern.
    pub fn checkerboard(width: u32, height: u32, c1: char, c2: char) -> OptimizedBuffer {
        let mut buffer = OptimizedBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let c = if (x + y) % 2 == 0 { c1 } else { c2 };
                buffer.set(x, y, Cell::new(c, Style::default()));
            }
        }
        buffer
    }

    /// Create a buffer with a gradient (for color testing).
    pub fn color_gradient(width: u32, height: u32) -> OptimizedBuffer {
        let mut buffer = OptimizedBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let r = x as f32 / width as f32;
                let g = y as f32 / height as f32;
                let b = 0.5;
                let color = Rgba::rgb(r, g, b);
                buffer.set(x, y, Cell::new(' ', Style::bg(color)));
            }
        }
        buffer
    }

    /// Create a buffer with text at specified positions.
    pub fn with_text(width: u32, height: u32, texts: &[((u32, u32), &str)]) -> OptimizedBuffer {
        let mut buffer = OptimizedBuffer::new(width, height);
        for ((x, y), text) in texts {
            for (i, c) in text.chars().enumerate() {
                let cell_x = x + i as u32;
                if cell_x < width {
                    buffer.set(cell_x, *y, Cell::new(c, Style::default()));
                }
            }
        }
        buffer
    }

    /// Create a buffer with a box drawn in it.
    pub fn with_box(
        width: u32,
        height: u32,
        box_x: u32,
        box_y: u32,
        box_w: u32,
        box_h: u32,
    ) -> OptimizedBuffer {
        let mut buffer = OptimizedBuffer::new(width, height);

        // Top edge
        for x in box_x..box_x + box_w {
            buffer.set(x, box_y, Cell::new('─', Style::default()));
        }
        // Bottom edge
        for x in box_x..box_x + box_w {
            buffer.set(x, box_y + box_h - 1, Cell::new('─', Style::default()));
        }
        // Left edge
        for y in box_y..box_y + box_h {
            buffer.set(box_x, y, Cell::new('│', Style::default()));
        }
        // Right edge
        for y in box_y..box_y + box_h {
            buffer.set(box_x + box_w - 1, y, Cell::new('│', Style::default()));
        }
        // Corners
        buffer.set(box_x, box_y, Cell::new('┌', Style::default()));
        buffer.set(box_x + box_w - 1, box_y, Cell::new('┐', Style::default()));
        buffer.set(box_x, box_y + box_h - 1, Cell::new('└', Style::default()));
        buffer.set(
            box_x + box_w - 1,
            box_y + box_h - 1,
            Cell::new('┘', Style::default()),
        );

        buffer
    }

    /// Create a buffer filled with styled cells.
    pub fn styled_fill(width: u32, height: u32, c: char, style: Style) -> OptimizedBuffer {
        let mut buffer = OptimizedBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                buffer.set(x, y, Cell::new(c, style));
            }
        }
        buffer
    }

    /// Standard terminal sizes for testing.
    pub fn standard_80x24() -> OptimizedBuffer {
        OptimizedBuffer::new(80, 24)
    }

    pub fn standard_132x43() -> OptimizedBuffer {
        OptimizedBuffer::new(132, 43)
    }

    pub fn minimal_40x12() -> OptimizedBuffer {
        OptimizedBuffer::new(40, 12)
    }

    pub fn large_200x60() -> OptimizedBuffer {
        OptimizedBuffer::new(200, 60)
    }
}

/// Style generators for testing.
pub mod styles {
    use super::*;

    /// Default style.
    pub fn default() -> Style {
        Style::default()
    }

    /// Bold style.
    pub fn bold() -> Style {
        Style::default().with_attributes(TextAttributes::BOLD)
    }

    /// Italic style.
    pub fn italic() -> Style {
        Style::default().with_attributes(TextAttributes::ITALIC)
    }

    /// Underline style.
    pub fn underline() -> Style {
        Style::default().with_attributes(TextAttributes::UNDERLINE)
    }

    /// Combined bold + italic.
    pub fn bold_italic() -> Style {
        Style::default().with_attributes(TextAttributes::BOLD | TextAttributes::ITALIC)
    }

    /// Style with foreground color.
    pub fn fg(color: Rgba) -> Style {
        Style::fg(color)
    }

    /// Style with background color.
    pub fn bg(color: Rgba) -> Style {
        Style::bg(color)
    }

    /// Style with both foreground and background.
    pub fn fg_bg(fg: Rgba, bg: Rgba) -> Style {
        Style::builder().fg(fg).bg(bg).build()
    }

    /// Error style (red foreground).
    pub fn error() -> Style {
        Style::fg(Rgba::RED).with_attributes(TextAttributes::BOLD)
    }

    /// Success style (green foreground).
    pub fn success() -> Style {
        Style::fg(Rgba::GREEN)
    }

    /// Warning style (yellow foreground).
    pub fn warning() -> Style {
        Style::fg(Rgba::new(1.0, 0.8, 0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ansi_cursor_position() {
        let seq = ansi_sequences::cursor_position(5, 10);
        assert_eq!(seq, b"\x1b[5;10H");
    }

    #[test]
    fn test_ansi_fg_rgb() {
        let seq = ansi_sequences::fg_rgb(255, 128, 64);
        assert_eq!(seq, b"\x1b[38;2;255;128;64m");
    }

    #[test]
    fn test_ansi_mouse_sgr() {
        let seq = ansi_sequences::mouse_sgr(0, 10, 20, true);
        assert_eq!(seq, b"\x1b[<0;11;21M");
    }

    #[test]
    fn test_buffer_checkerboard() {
        let buffer = buffers::checkerboard(4, 4, 'X', 'O');
        assert_eq!(buffer.width(), 4);
        assert_eq!(buffer.height(), 4);
    }

    #[test]
    fn test_buffer_with_text() {
        let buffer = buffers::with_text(20, 5, &[((0, 0), "Hello"), ((0, 1), "World")]);
        assert_eq!(buffer.width(), 20);
    }

    #[test]
    fn test_color_from_seed_deterministic() {
        let c1 = colors::from_seed(42);
        let c2 = colors::from_seed(42);
        assert_eq!(c1.r, c2.r);
        assert_eq!(c1.g, c2.g);
        assert_eq!(c1.b, c2.b);
    }

    #[test]
    fn test_style_combinations() {
        let s = styles::bold_italic();
        assert!(s.attributes.contains(TextAttributes::BOLD));
        assert!(s.attributes.contains(TextAttributes::ITALIC));
    }
}
