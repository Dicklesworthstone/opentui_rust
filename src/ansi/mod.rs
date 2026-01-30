//! ANSI escape sequence generation.

pub mod output;
pub mod sequences;

pub use output::AnsiWriter;
pub use sequences::*;

use crate::color::Rgba;
use crate::style::TextAttributes;
use crate::terminal::ColorSupport;
use std::io::{self, Write};

/// Color output mode for ANSI sequences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColorMode {
    /// True color (24-bit RGB).
    #[default]
    TrueColor,
    /// 256-color palette.
    Color256,
    /// 16-color (basic ANSI).
    Color16,
    /// No color output.
    NoColor,
}

impl From<ColorSupport> for ColorMode {
    fn from(support: ColorSupport) -> Self {
        match support {
            ColorSupport::TrueColor => ColorMode::TrueColor,
            ColorSupport::Extended => ColorMode::Color256,
            ColorSupport::Basic => ColorMode::Color16,
            ColorSupport::None => ColorMode::NoColor,
        }
    }
}

/// Generate SGR (Select Graphic Rendition) sequence for foreground color.
#[must_use]
pub fn fg_color(color: Rgba) -> String {
    fg_color_with_mode(color, ColorMode::TrueColor)
}

/// Generate SGR sequence for background color.
#[must_use]
pub fn bg_color(color: Rgba) -> String {
    bg_color_with_mode(color, ColorMode::TrueColor)
}

/// Generate SGR sequence for foreground color with specified color mode.
#[must_use]
pub fn fg_color_with_mode(color: Rgba, mode: ColorMode) -> String {
    let mut buf = Vec::new();
    write_fg_color_with_mode(&mut buf, color, mode).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write a u8 as decimal digits to a writer without formatting overhead.
#[inline]
fn write_u8_decimal(w: &mut impl Write, n: u8) -> io::Result<()> {
    if n >= 100 {
        w.write_all(&[b'0' + n / 100, b'0' + (n / 10) % 10, b'0' + n % 10])
    } else if n >= 10 {
        w.write_all(&[b'0' + n / 10, b'0' + n % 10])
    } else {
        w.write_all(&[b'0' + n])
    }
}

/// Write a u32 as decimal digits to a writer without formatting overhead.
///
/// Stack buffer is sized for max u32 digits (10) to avoid heap allocation.
#[inline]
fn write_u32_decimal(w: &mut impl Write, n: u32) -> io::Result<()> {
    // Fast paths for common small values (most cursor positions)
    if n < 10 {
        return w.write_all(&[b'0' + n as u8]);
    }
    if n < 100 {
        return w.write_all(&[b'0' + (n / 10) as u8, b'0' + (n % 10) as u8]);
    }
    if n < 1000 {
        return w.write_all(&[
            b'0' + (n / 100) as u8,
            b'0' + ((n / 10) % 10) as u8,
            b'0' + (n % 10) as u8,
        ]);
    }

    // General case: build digits in reverse on stack
    let mut buf = [0u8; 10]; // max u32 is 4294967295 (10 digits)
    let mut i = buf.len();
    let mut val = n;
    while val > 0 {
        i -= 1;
        buf[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    w.write_all(&buf[i..])
}

/// Write SGR sequence for foreground color to a writer.
///
/// Uses direct byte writes to avoid `write!` formatting overhead on hot paths.
pub fn write_fg_color_with_mode(
    w: &mut impl Write,
    color: Rgba,
    mode: ColorMode,
) -> io::Result<()> {
    match mode {
        ColorMode::TrueColor => {
            let (r, g, b) = color.to_rgb_u8();
            w.write_all(b"\x1b[38;2;")?;
            write_u8_decimal(w, r)?;
            w.write_all(b";")?;
            write_u8_decimal(w, g)?;
            w.write_all(b";")?;
            write_u8_decimal(w, b)?;
            w.write_all(b"m")
        }
        ColorMode::Color256 => {
            let idx = color.to_256_color();
            w.write_all(b"\x1b[38;5;")?;
            write_u8_decimal(w, idx)?;
            w.write_all(b"m")
        }
        ColorMode::Color16 => {
            let idx = color.to_16_color();
            // ANSI 16 colors: 30-37 for normal, 90-97 for bright
            let code = if idx < 8 { 30 + idx } else { 90 + idx - 8 };
            w.write_all(b"\x1b[")?;
            write_u8_decimal(w, code)?;
            w.write_all(b"m")
        }
        ColorMode::NoColor => Ok(()),
    }
}

/// Generate SGR sequence for background color with specified color mode.
#[must_use]
pub fn bg_color_with_mode(color: Rgba, mode: ColorMode) -> String {
    let mut buf = Vec::new();
    write_bg_color_with_mode(&mut buf, color, mode).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write SGR sequence for background color to a writer.
///
/// Uses direct byte writes to avoid `write!` formatting overhead on hot paths.
pub fn write_bg_color_with_mode(
    w: &mut impl Write,
    color: Rgba,
    mode: ColorMode,
) -> io::Result<()> {
    match mode {
        ColorMode::TrueColor => {
            let (r, g, b) = color.to_rgb_u8();
            w.write_all(b"\x1b[48;2;")?;
            write_u8_decimal(w, r)?;
            w.write_all(b";")?;
            write_u8_decimal(w, g)?;
            w.write_all(b";")?;
            write_u8_decimal(w, b)?;
            w.write_all(b"m")
        }
        ColorMode::Color256 => {
            let idx = color.to_256_color();
            w.write_all(b"\x1b[48;5;")?;
            write_u8_decimal(w, idx)?;
            w.write_all(b"m")
        }
        ColorMode::Color16 => {
            let idx = color.to_16_color();
            // ANSI 16 colors: 40-47 for normal, 100-107 for bright
            let code = if idx < 8 { 40 + idx } else { 100 + idx - 8 };
            w.write_all(b"\x1b[")?;
            write_u8_decimal(w, code)?;
            w.write_all(b"m")
        }
        ColorMode::NoColor => Ok(()),
    }
}

/// Generate SGR sequence for text attributes.
#[must_use]
pub fn attributes(attrs: TextAttributes) -> String {
    let mut buf = Vec::new();
    write_attributes(&mut buf, attrs).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write SGR sequence for text attributes to a writer.
///
/// Uses a stack-allocated array to avoid heap allocation on every call.
pub fn write_attributes(w: &mut impl Write, attrs: TextAttributes) -> io::Result<()> {
    // Stack-allocated array - max 8 attribute codes possible
    let mut codes: [&str; 8] = [""; 8];
    let mut count = 0;

    if attrs.contains(TextAttributes::BOLD) {
        codes[count] = "1";
        count += 1;
    }
    if attrs.contains(TextAttributes::DIM) {
        codes[count] = "2";
        count += 1;
    }
    if attrs.contains(TextAttributes::ITALIC) {
        codes[count] = "3";
        count += 1;
    }
    if attrs.contains(TextAttributes::UNDERLINE) {
        codes[count] = "4";
        count += 1;
    }
    if attrs.contains(TextAttributes::BLINK) {
        codes[count] = "5";
        count += 1;
    }
    if attrs.contains(TextAttributes::INVERSE) {
        codes[count] = "7";
        count += 1;
    }
    if attrs.contains(TextAttributes::HIDDEN) {
        codes[count] = "8";
        count += 1;
    }
    if attrs.contains(TextAttributes::STRIKETHROUGH) {
        codes[count] = "9";
        count += 1;
    }

    if count == 0 {
        Ok(())
    } else {
        // Write CSI sequence manually to avoid format! overhead
        w.write_all(b"\x1b[")?;
        for (i, code) in codes[..count].iter().enumerate() {
            if i > 0 {
                w.write_all(b";")?;
            }
            w.write_all(code.as_bytes())?;
        }
        w.write_all(b"m")
    }
}

/// Generate cursor position sequence (1-indexed).
#[must_use]
pub fn cursor_position(row: u32, col: u32) -> String {
    let mut buf = Vec::new();
    write_cursor_position(&mut buf, row, col).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write cursor position sequence to a writer.
///
/// Uses direct byte writes to avoid `write!` formatting overhead on hot paths.
pub fn write_cursor_position(w: &mut impl Write, row: u32, col: u32) -> io::Result<()> {
    w.write_all(b"\x1b[")?;
    write_u32_decimal(w, row + 1)?;
    w.write_all(b";")?;
    write_u32_decimal(w, col + 1)?;
    w.write_all(b"H")
}

/// Generate relative cursor movement.
#[must_use]
pub fn cursor_move(dx: i32, dy: i32) -> String {
    let mut buf = Vec::new();
    write_cursor_move(&mut buf, dx, dy).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write relative cursor movement to a writer.
///
/// Uses direct byte writes to avoid `write!` formatting overhead on hot paths.
pub fn write_cursor_move(w: &mut impl Write, dx: i32, dy: i32) -> io::Result<()> {
    if dy < 0 {
        w.write_all(b"\x1b[")?;
        write_u32_decimal(w, (-dy) as u32)?;
        w.write_all(b"A")?;
    } else if dy > 0 {
        w.write_all(b"\x1b[")?;
        write_u32_decimal(w, dy as u32)?;
        w.write_all(b"B")?;
    }

    if dx > 0 {
        w.write_all(b"\x1b[")?;
        write_u32_decimal(w, dx as u32)?;
        w.write_all(b"C")?;
    } else if dx < 0 {
        w.write_all(b"\x1b[")?;
        write_u32_decimal(w, (-dx) as u32)?;
        w.write_all(b"D")?;
    }
    Ok(())
}

/// Escape a URL for safe inclusion in OSC 8 hyperlink sequences.
///
/// Control characters are percent-encoded to prevent escape sequence injection:
/// - C0 controls (U+0000-U+001F): Contains ESC (0x1B) and BEL (0x07)
/// - DEL (U+007F)
/// - C1 controls (U+0080-U+009F): Contains CSI (U+009B), ST (U+009C), OSC (U+009D)
///
/// This is critical because unescaped control characters could terminate the
/// OSC sequence early and allow arbitrary terminal command injection.
#[must_use]
pub fn escape_url_for_osc8(url: &str) -> String {
    let mut escaped = String::with_capacity(url.len());

    for ch in url.chars() {
        if ch.is_control() {
            // Percent-encode all control characters (C0, DEL, and C1)
            // This handles both single-byte ASCII controls and multi-byte C1 controls
            for byte in ch.to_string().bytes() {
                escaped.push('%');
                // Use uppercase hex for RFC 3986 compatibility
                let high = (byte >> 4) & 0x0F;
                let low = byte & 0x0F;
                escaped.push(if high < 10 {
                    char::from(b'0' + high)
                } else {
                    char::from(b'A' + high - 10)
                });
                escaped.push(if low < 10 {
                    char::from(b'0' + low)
                } else {
                    char::from(b'A' + low - 10)
                });
            }
        } else {
            escaped.push(ch);
        }
    }

    escaped
}

/// Generate OSC 8 hyperlink start sequence.
#[must_use]
pub fn hyperlink_start(id: u32, url: &str) -> String {
    let mut buf = Vec::new();
    write_hyperlink_start(&mut buf, id, url).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write OSC 8 hyperlink start sequence to a writer.
///
/// The URL is automatically escaped to prevent control character injection.
pub fn write_hyperlink_start(w: &mut impl Write, id: u32, url: &str) -> io::Result<()> {
    let escaped_url = escape_url_for_osc8(url);
    write!(w, "\x1b]8;id={id};{escaped_url}\x1b\\")
}

/// OSC 8 hyperlink end sequence.
pub const HYPERLINK_END: &str = "\x1b]8;;\x1b\\";

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_json_snapshot;
    use serde::Serialize;

    /// Wrapper for snapshot testing escape sequences.
    /// Converts raw escape sequences to readable format.
    #[derive(Serialize)]
    struct AnsiSequence {
        /// Human-readable description
        description: &'static str,
        /// Raw bytes as hex for exact verification
        hex: String,
        /// Readable representation with escapes shown
        readable: String,
    }

    impl AnsiSequence {
        fn new(description: &'static str, sequence: &str) -> Self {
            Self {
                description,
                hex: sequence
                    .bytes()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                readable: sequence
                    .replace('\x1b', "ESC")
                    .replace('\x07', "BEL")
                    .replace('\\', "ST"),
            }
        }
    }

    #[test]
    fn snapshot_fg_colors_truecolor() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("red", &fg_color_with_mode(Rgba::RED, ColorMode::TrueColor)),
            AnsiSequence::new(
                "green",
                &fg_color_with_mode(Rgba::GREEN, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "blue",
                &fg_color_with_mode(Rgba::BLUE, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "white",
                &fg_color_with_mode(Rgba::WHITE, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "black",
                &fg_color_with_mode(Rgba::BLACK, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "transparent",
                &fg_color_with_mode(Rgba::TRANSPARENT, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "custom_rgb",
                &fg_color_with_mode(Rgba::new(0.5, 0.25, 0.75, 1.0), ColorMode::TrueColor),
            ),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_fg_colors_256() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("red", &fg_color_with_mode(Rgba::RED, ColorMode::Color256)),
            AnsiSequence::new(
                "green",
                &fg_color_with_mode(Rgba::GREEN, ColorMode::Color256),
            ),
            AnsiSequence::new("blue", &fg_color_with_mode(Rgba::BLUE, ColorMode::Color256)),
            AnsiSequence::new(
                "white",
                &fg_color_with_mode(Rgba::WHITE, ColorMode::Color256),
            ),
            AnsiSequence::new(
                "black",
                &fg_color_with_mode(Rgba::BLACK, ColorMode::Color256),
            ),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_fg_colors_16() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("red", &fg_color_with_mode(Rgba::RED, ColorMode::Color16)),
            AnsiSequence::new(
                "green",
                &fg_color_with_mode(Rgba::GREEN, ColorMode::Color16),
            ),
            AnsiSequence::new("blue", &fg_color_with_mode(Rgba::BLUE, ColorMode::Color16)),
            AnsiSequence::new(
                "white",
                &fg_color_with_mode(Rgba::WHITE, ColorMode::Color16),
            ),
            AnsiSequence::new(
                "black",
                &fg_color_with_mode(Rgba::BLACK, ColorMode::Color16),
            ),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_fg_colors_nocolor() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new(
                "red_nocolor",
                &fg_color_with_mode(Rgba::RED, ColorMode::NoColor),
            ),
            AnsiSequence::new(
                "any_nocolor",
                &fg_color_with_mode(Rgba::new(0.5, 0.5, 0.5, 1.0), ColorMode::NoColor),
            ),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_bg_colors_truecolor() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("red", &bg_color_with_mode(Rgba::RED, ColorMode::TrueColor)),
            AnsiSequence::new(
                "green",
                &bg_color_with_mode(Rgba::GREEN, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "blue",
                &bg_color_with_mode(Rgba::BLUE, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "white",
                &bg_color_with_mode(Rgba::WHITE, ColorMode::TrueColor),
            ),
            AnsiSequence::new(
                "black",
                &bg_color_with_mode(Rgba::BLACK, ColorMode::TrueColor),
            ),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_bg_colors_256() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("red", &bg_color_with_mode(Rgba::RED, ColorMode::Color256)),
            AnsiSequence::new(
                "green",
                &bg_color_with_mode(Rgba::GREEN, ColorMode::Color256),
            ),
            AnsiSequence::new("blue", &bg_color_with_mode(Rgba::BLUE, ColorMode::Color256)),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_bg_colors_16() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("red", &bg_color_with_mode(Rgba::RED, ColorMode::Color16)),
            AnsiSequence::new(
                "green",
                &bg_color_with_mode(Rgba::GREEN, ColorMode::Color16),
            ),
            AnsiSequence::new("blue", &bg_color_with_mode(Rgba::BLUE, ColorMode::Color16)),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_text_attributes() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("bold", &attributes(TextAttributes::BOLD)),
            AnsiSequence::new("dim", &attributes(TextAttributes::DIM)),
            AnsiSequence::new("italic", &attributes(TextAttributes::ITALIC)),
            AnsiSequence::new("underline", &attributes(TextAttributes::UNDERLINE)),
            AnsiSequence::new("blink", &attributes(TextAttributes::BLINK)),
            AnsiSequence::new("inverse", &attributes(TextAttributes::INVERSE)),
            AnsiSequence::new("hidden", &attributes(TextAttributes::HIDDEN)),
            AnsiSequence::new("strikethrough", &attributes(TextAttributes::STRIKETHROUGH)),
            AnsiSequence::new(
                "bold_italic",
                &attributes(TextAttributes::BOLD | TextAttributes::ITALIC),
            ),
            AnsiSequence::new(
                "bold_underline_inverse",
                &attributes(
                    TextAttributes::BOLD | TextAttributes::UNDERLINE | TextAttributes::INVERSE,
                ),
            ),
            AnsiSequence::new("empty", &attributes(TextAttributes::empty())),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_cursor_position() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("origin", &cursor_position(0, 0)),
            AnsiSequence::new("row_5_col_10", &cursor_position(5, 10)),
            AnsiSequence::new("large_position", &cursor_position(100, 200)),
            AnsiSequence::new("max_u32", &cursor_position(u32::MAX - 1, u32::MAX - 1)),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_cursor_move() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("no_move", &cursor_move(0, 0)),
            AnsiSequence::new("right_5", &cursor_move(5, 0)),
            AnsiSequence::new("left_5", &cursor_move(-5, 0)),
            AnsiSequence::new("down_3", &cursor_move(0, 3)),
            AnsiSequence::new("up_3", &cursor_move(0, -3)),
            AnsiSequence::new("right_down", &cursor_move(5, 3)),
            AnsiSequence::new("left_up", &cursor_move(-5, -3)),
            AnsiSequence::new("right_up", &cursor_move(5, -3)),
            AnsiSequence::new("left_down", &cursor_move(-5, 3)),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn snapshot_hyperlinks() {
        let sequences: Vec<AnsiSequence> = vec![
            AnsiSequence::new("simple_link", &hyperlink_start(1, "https://example.com")),
            AnsiSequence::new(
                "link_with_path",
                &hyperlink_start(42, "https://example.com/path/to/file.txt"),
            ),
            AnsiSequence::new("link_end", HYPERLINK_END),
        ];
        assert_json_snapshot!(sequences);
    }

    #[test]
    fn test_osc8_url_escaping() {
        // Normal URLs should pass through unchanged
        assert_eq!(
            escape_url_for_osc8("https://example.com/path?query=value"),
            "https://example.com/path?query=value"
        );

        // ESC (0x1B) must be escaped - this is the critical injection vector
        assert_eq!(escape_url_for_osc8("http://x\x1b"), "http://x%1B");

        // BEL (0x07) must be escaped - another OSC terminator
        assert_eq!(escape_url_for_osc8("http://x\x07"), "http://x%07");

        // NUL (0x00) must be escaped
        assert_eq!(escape_url_for_osc8("http://x\x00"), "http://x%00");

        // DEL (0x7F) must be escaped
        assert_eq!(escape_url_for_osc8("http://x\x7f"), "http://x%7F");

        // All control characters should be escaped
        for byte in 0x00u8..=0x1F {
            let url = format!("http://x{}", byte as char);
            let escaped = escape_url_for_osc8(&url);
            assert!(
                !escaped.contains(byte as char),
                "Control char 0x{byte:02x} should be escaped"
            );
            assert!(
                escaped.contains('%'),
                "Control char 0x{byte:02x} should be percent-encoded"
            );
        }
    }

    #[test]
    fn test_osc8_url_preserves_unicode() {
        // URLs with Unicode characters should be preserved exactly
        let unicode_url = "https://example.com/日本語/path";
        assert_eq!(
            escape_url_for_osc8(unicode_url),
            unicode_url,
            "Unicode URLs should pass through unchanged"
        );

        // Emoji in URLs
        let emoji_url = "https://example.com/🎉/celebration";
        assert_eq!(
            escape_url_for_osc8(emoji_url),
            emoji_url,
            "Emoji URLs should pass through unchanged"
        );

        // Mixed ASCII and Unicode
        let mixed_url = "https://日本.example.com/path?q=テスト";
        assert_eq!(
            escape_url_for_osc8(mixed_url),
            mixed_url,
            "Mixed URLs should pass through unchanged"
        );
    }

    #[test]
    fn test_osc8_injection_prevention() {
        // Attempt to inject an escape sequence that would close OSC 8 early
        // and execute arbitrary terminal commands.
        // Malicious URL: tries to inject ST (ESC \) to end OSC, then clear screen
        let malicious_url = "http://evil\x1b\\x1b[2J";
        let escaped = escape_url_for_osc8(malicious_url);

        // The escaped URL should NOT contain raw ESC bytes
        assert!(
            !escaped.bytes().any(|b| b == 0x1B),
            "Escaped URL must not contain raw ESC bytes"
        );

        // The hyperlink start should be safe
        let output = hyperlink_start(1, malicious_url);
        let esc_count = output.bytes().filter(|&b| b == 0x1B).count();
        // Should only have 2 ESC bytes: one for OSC start, one for ST terminator
        assert_eq!(
            esc_count, 2,
            "Hyperlink output should only have opening and closing ESC, not injected ones"
        );
    }

    #[test]
    fn test_osc8_c1_control_escaping() {
        // C1 control characters (U+0080-U+009F) must also be escaped
        // These are interpreted as control sequences by some terminals:
        // - U+009B (CSI) is equivalent to ESC[
        // - U+009C (ST) is equivalent to ESC\ (string terminator)
        // - U+009D (OSC) is equivalent to ESC]

        // CSI (U+009B) - Control Sequence Introducer
        let url_with_csi = "http://evil\u{009B}2J";
        let escaped = escape_url_for_osc8(url_with_csi);
        assert!(
            !escaped.contains('\u{009B}'),
            "CSI (U+009B) must be escaped"
        );
        assert!(
            escaped.contains("%C2%9B"),
            "CSI should be percent-encoded as %C2%9B"
        );

        // ST (U+009C) - String Terminator
        let url_with_st = "http://evil\u{009C}inject";
        let escaped = escape_url_for_osc8(url_with_st);
        assert!(!escaped.contains('\u{009C}'), "ST (U+009C) must be escaped");

        // OSC (U+009D) - Operating System Command
        let url_with_osc = "http://evil\u{009D}0;title\u{009C}";
        let escaped = escape_url_for_osc8(url_with_osc);
        assert!(
            !escaped.contains('\u{009D}'),
            "OSC (U+009D) must be escaped"
        );
    }

    // ============================================
    // Cursor Control Tests (4 tests per spec)
    // ============================================

    #[test]
    fn test_cursor_move_to_absolute() {
        // Test absolute cursor positioning ESC[row;colH
        let seq = cursor_position(0, 0);
        assert_eq!(seq, "\x1b[1;1H", "Origin should be 1,1 (1-indexed)");

        let seq = cursor_position(5, 10);
        assert_eq!(seq, "\x1b[6;11H", "Position 5,10 -> 6,11 (1-indexed)");

        let seq = cursor_position(99, 199);
        assert_eq!(seq, "\x1b[100;200H", "Large position check");
    }

    #[test]
    fn test_cursor_move_relative() {
        // Test relative cursor movements: up (A), down (B), forward (C), back (D)
        let up = cursor_move(0, -3);
        assert!(up.contains("3A"), "Up 3 should use ESC[3A");

        let down = cursor_move(0, 3);
        assert!(down.contains("3B"), "Down 3 should use ESC[3B");

        let right = cursor_move(5, 0);
        assert!(right.contains("5C"), "Right 5 should use ESC[5C");

        let left = cursor_move(-5, 0);
        assert!(left.contains("5D"), "Left 5 should use ESC[5D");
    }

    #[test]
    fn test_cursor_hide_show() {
        // Test cursor visibility control sequences
        use super::sequences::{CURSOR_HIDE, CURSOR_SHOW};

        assert_eq!(CURSOR_HIDE, "\x1b[?25l", "Hide cursor sequence");
        assert_eq!(CURSOR_SHOW, "\x1b[?25h", "Show cursor sequence");
    }

    #[test]
    fn test_cursor_save_restore() {
        // Test DEC cursor save/restore
        use super::sequences::{CURSOR_RESTORE, CURSOR_SAVE};

        assert_eq!(CURSOR_SAVE, "\x1b7", "DEC save cursor position");
        assert_eq!(CURSOR_RESTORE, "\x1b8", "DEC restore cursor position");
    }

    // ============================================
    // Screen Control Tests (4 tests per spec)
    // ============================================

    #[test]
    fn test_clear_screen() {
        // Test clear screen sequences
        use super::sequences::{CLEAR_SCREEN, CLEAR_SCREEN_ABOVE, CLEAR_SCREEN_BELOW};

        assert_eq!(CLEAR_SCREEN, "\x1b[2J", "Clear entire screen");
        assert_eq!(CLEAR_SCREEN_BELOW, "\x1b[J", "Clear from cursor down");
        assert_eq!(CLEAR_SCREEN_ABOVE, "\x1b[1J", "Clear from cursor up");
    }

    #[test]
    fn test_clear_line() {
        // Test clear line variants: 0=right, 1=left, 2=all
        use super::sequences::{CLEAR_LINE, CLEAR_LINE_LEFT, CLEAR_LINE_RIGHT};

        assert_eq!(
            CLEAR_LINE_RIGHT, "\x1b[K",
            "Clear to end of line (default 0)"
        );
        assert_eq!(CLEAR_LINE_LEFT, "\x1b[1K", "Clear to beginning of line");
        assert_eq!(CLEAR_LINE, "\x1b[2K", "Clear entire line");
    }

    #[test]
    fn test_alt_screen_enter_leave() {
        // Test alternate screen buffer sequences
        use super::sequences::{ALT_SCREEN_OFF, ALT_SCREEN_ON};

        assert_eq!(ALT_SCREEN_ON, "\x1b[?1049h", "Enter alternate screen");
        assert_eq!(ALT_SCREEN_OFF, "\x1b[?1049l", "Leave alternate screen");
    }

    #[test]
    fn test_cursor_home() {
        // Test cursor home position
        use super::sequences::CURSOR_HOME;

        assert_eq!(CURSOR_HOME, "\x1b[H", "Cursor home (no params = 1,1)");
    }

    // ============================================
    // Color Output Tests (8 tests per spec)
    // ============================================

    #[test]
    fn test_sgr_colors_16_mapping() {
        // Test 16-color mapping for basic ANSI colors
        // Normal colors: 30-37, bright: 90-97
        let black = fg_color_with_mode(Rgba::BLACK, ColorMode::Color16);
        assert!(black.contains("\x1b["), "Should have CSI prefix");
        assert!(black.ends_with('m'), "Should end with m");

        let white = fg_color_with_mode(Rgba::WHITE, ColorMode::Color16);
        // White maps to bright white (97)
        let code: u8 = white
            .trim_start_matches("\x1b[")
            .trim_end_matches('m')
            .parse()
            .unwrap_or(0);
        assert!(
            code >= 30 && code <= 97,
            "16-color code should be in valid range"
        );
    }

    #[test]
    fn test_sgr_colors_256_format() {
        // Test 256-color format: ESC[38;5;Nm
        let color = fg_color_with_mode(Rgba::new(0.5, 0.5, 0.5, 1.0), ColorMode::Color256);
        assert!(color.starts_with("\x1b[38;5;"), "256-color fg format");
        assert!(color.ends_with('m'), "Should end with m");

        let bg = bg_color_with_mode(Rgba::RED, ColorMode::Color256);
        assert!(bg.starts_with("\x1b[48;5;"), "256-color bg format");
    }

    #[test]
    fn test_sgr_colors_rgb_format() {
        // Test 24-bit true color format: ESC[38;2;R;G;Bm
        let color = fg_color_with_mode(Rgba::new(0.5, 0.25, 0.75, 1.0), ColorMode::TrueColor);
        assert!(color.starts_with("\x1b[38;2;"), "True color fg format");

        // Parse out the RGB values
        let parts: Vec<&str> = color
            .trim_start_matches("\x1b[38;2;")
            .trim_end_matches('m')
            .split(';')
            .collect();
        assert_eq!(parts.len(), 3, "Should have 3 color components");

        // Verify RGB values parse correctly (0-255 range implicit in u8)
        for part in parts {
            let _val: u8 = part.parse().expect("Should be valid u8");
            // Parsing succeeded - value is valid u8 (0-255)
        }
    }

    #[test]
    fn test_color_foreground_vs_background() {
        // Test that fg uses 38 and bg uses 48
        let fg = fg_color_with_mode(Rgba::RED, ColorMode::TrueColor);
        let bg = bg_color_with_mode(Rgba::RED, ColorMode::TrueColor);

        assert!(fg.contains("38;2;"), "Foreground uses SGR 38");
        assert!(bg.contains("48;2;"), "Background uses SGR 48");
    }

    #[test]
    fn test_color_reset() {
        // Test color reset sequences
        use super::sequences::{RESET, color};

        assert_eq!(RESET, "\x1b[0m", "Full reset sequence");
        assert_eq!(color::FG_DEFAULT, "\x1b[39m", "Foreground default");
        assert_eq!(color::BG_DEFAULT, "\x1b[49m", "Background default");
    }

    #[test]
    fn test_color_no_color_mode() {
        // In NoColor mode, no escape sequences should be emitted
        let fg = fg_color_with_mode(Rgba::RED, ColorMode::NoColor);
        let bg = bg_color_with_mode(Rgba::BLUE, ColorMode::NoColor);

        assert!(fg.is_empty(), "NoColor fg should be empty");
        assert!(bg.is_empty(), "NoColor bg should be empty");
    }

    #[test]
    fn test_color_mode_from_support() {
        // Test ColorMode::from(ColorSupport)
        use crate::terminal::ColorSupport;

        assert_eq!(
            ColorMode::from(ColorSupport::TrueColor),
            ColorMode::TrueColor
        );
        assert_eq!(ColorMode::from(ColorSupport::Extended), ColorMode::Color256);
        assert_eq!(ColorMode::from(ColorSupport::Basic), ColorMode::Color16);
        assert_eq!(ColorMode::from(ColorSupport::None), ColorMode::NoColor);
    }

    #[test]
    fn test_color_boundary_values() {
        // Test boundary RGB values
        let min = Rgba::new(0.0, 0.0, 0.0, 1.0);
        let max = Rgba::new(1.0, 1.0, 1.0, 1.0);

        let min_seq = fg_color_with_mode(min, ColorMode::TrueColor);
        assert!(
            min_seq.contains(";0;0;0m") || min_seq.ends_with("0m"),
            "Min RGB"
        );

        let max_seq = fg_color_with_mode(max, ColorMode::TrueColor);
        assert!(max_seq.contains(";255;255;255m"), "Max RGB");
    }

    // ============================================
    // Text Attributes Tests (7 tests per spec)
    // ============================================

    #[test]
    fn test_sgr_bold() {
        let seq = attributes(TextAttributes::BOLD);
        assert_eq!(seq, "\x1b[1m", "Bold is SGR 1");
    }

    #[test]
    fn test_sgr_italic() {
        let seq = attributes(TextAttributes::ITALIC);
        assert_eq!(seq, "\x1b[3m", "Italic is SGR 3");
    }

    #[test]
    fn test_sgr_underline() {
        let seq = attributes(TextAttributes::UNDERLINE);
        assert_eq!(seq, "\x1b[4m", "Underline is SGR 4");
    }

    #[test]
    fn test_sgr_strikethrough() {
        let seq = attributes(TextAttributes::STRIKETHROUGH);
        assert_eq!(seq, "\x1b[9m", "Strikethrough is SGR 9");
    }

    #[test]
    fn test_sgr_multiple_attributes() {
        // Multiple attributes should be combined with semicolons
        let seq = attributes(TextAttributes::BOLD | TextAttributes::ITALIC);
        assert!(seq.starts_with("\x1b["), "CSI prefix");
        assert!(seq.contains("1"), "Has bold");
        assert!(seq.contains("3"), "Has italic");
        assert!(seq.contains(";"), "Semicolon separator");
        assert!(seq.ends_with('m'), "SGR terminator");
    }

    #[test]
    fn test_sgr_reset_full() {
        use super::sequences::RESET;
        assert_eq!(RESET, "\x1b[0m", "Full SGR reset");
    }

    #[test]
    fn test_attribute_empty() {
        let seq = attributes(TextAttributes::empty());
        assert!(seq.is_empty(), "Empty attributes produce no sequence");
    }

    // ============================================
    // Mouse & Extended Tests (4 tests per spec)
    // ============================================

    #[test]
    fn test_mouse_enable_disable() {
        use super::sequences::{MOUSE_OFF, MOUSE_ON};

        // Mouse tracking modes: 1003 = all, 1006 = SGR extended
        assert!(MOUSE_ON.contains("1003h"), "Enable all mouse events");
        assert!(MOUSE_ON.contains("1006h"), "Enable SGR mouse format");
        assert!(MOUSE_OFF.contains("1003l"), "Disable all mouse events");
        assert!(MOUSE_OFF.contains("1006l"), "Disable SGR mouse format");
    }

    #[test]
    fn test_bracketed_paste_mode() {
        use super::sequences::{BRACKETED_PASTE_OFF, BRACKETED_PASTE_ON};

        assert_eq!(BRACKETED_PASTE_ON, "\x1b[?2004h", "Enable bracketed paste");
        assert_eq!(
            BRACKETED_PASTE_OFF, "\x1b[?2004l",
            "Disable bracketed paste"
        );
    }

    #[test]
    fn test_focus_events() {
        use super::sequences::{FOCUS_OFF, FOCUS_ON};

        assert_eq!(FOCUS_ON, "\x1b[?1004h", "Enable focus tracking");
        assert_eq!(FOCUS_OFF, "\x1b[?1004l", "Disable focus tracking");
    }

    #[test]
    fn test_sync_output() {
        use super::sequences::sync;

        assert_eq!(sync::BEGIN, "\x1b[?2026h", "Begin synchronized output");
        assert_eq!(sync::END, "\x1b[?2026l", "End synchronized output");
    }

    // ============================================
    // OSC Sequences Tests (2 tests per spec)
    // ============================================

    #[test]
    fn test_osc8_hyperlink_structure() {
        // OSC 8 format: ESC]8;params;URLESC\
        let link = hyperlink_start(42, "https://example.com");
        assert!(link.starts_with("\x1b]8;"), "OSC 8 prefix");
        assert!(link.contains("id=42"), "Contains link ID");
        assert!(link.contains("https://example.com"), "Contains URL");
        assert!(link.ends_with("\x1b\\"), "String terminator");

        // End hyperlink
        assert_eq!(HYPERLINK_END, "\x1b]8;;\x1b\\", "Hyperlink end sequence");
    }

    #[test]
    fn test_osc_title() {
        use super::sequences::{TITLE_PREFIX, TITLE_SUFFIX};

        // Window title: OSC 0;titleST
        assert_eq!(TITLE_PREFIX, "\x1b]0;", "Title OSC prefix");
        assert_eq!(TITLE_SUFFIX, "\x1b\\", "String terminator");

        // Full title sequence would be: TITLE_PREFIX + "My Title" + TITLE_SUFFIX
        let full_title = format!("{TITLE_PREFIX}Test Window{TITLE_SUFFIX}");
        assert_eq!(full_title, "\x1b]0;Test Window\x1b\\");
    }

    // ============================================
    // State Tracking Tests (4 tests per spec)
    // ============================================

    #[test]
    fn test_write_u8_decimal() {
        // Test the internal decimal writer
        fn verify_u8(n: u8) -> String {
            let mut buf = Vec::new();
            write_u8_decimal(&mut buf, n).unwrap();
            String::from_utf8(buf).unwrap()
        }

        assert_eq!(verify_u8(0), "0");
        assert_eq!(verify_u8(9), "9");
        assert_eq!(verify_u8(10), "10");
        assert_eq!(verify_u8(99), "99");
        assert_eq!(verify_u8(100), "100");
        assert_eq!(verify_u8(255), "255");
    }

    #[test]
    fn test_write_u32_decimal() {
        // Test the internal u32 decimal writer
        fn verify_u32(n: u32) -> String {
            let mut buf = Vec::new();
            write_u32_decimal(&mut buf, n).unwrap();
            String::from_utf8(buf).unwrap()
        }

        assert_eq!(verify_u32(0), "0");
        assert_eq!(verify_u32(9), "9");
        assert_eq!(verify_u32(10), "10");
        assert_eq!(verify_u32(99), "99");
        assert_eq!(verify_u32(100), "100");
        assert_eq!(verify_u32(999), "999");
        assert_eq!(verify_u32(1000), "1000");
        assert_eq!(verify_u32(u32::MAX), "4294967295");
    }

    #[test]
    fn test_cursor_position_1_indexed() {
        // Cursor positions are converted from 0-indexed to 1-indexed
        let seq = cursor_position(0, 0);
        assert!(seq.contains("1;1"), "0,0 becomes 1;1");

        let seq = cursor_position(9, 19);
        assert!(seq.contains("10;20"), "9,19 becomes 10;20");
    }

    #[test]
    fn test_cursor_move_zero_no_output() {
        // Zero movement should produce minimal output
        let seq = cursor_move(0, 0);
        assert!(seq.is_empty(), "No movement = no sequence");
    }

    // ============================================
    // Edge Cases Tests (4 tests per spec)
    // ============================================

    #[test]
    fn test_large_coordinate_values() {
        // Test handling of large coordinate values
        let large = cursor_position(u32::MAX - 1, u32::MAX - 1);
        assert!(large.contains("H"), "Still produces valid sequence");

        // Verify it contains the large numbers
        let expected_row = u32::MAX.to_string();
        let expected_col = u32::MAX.to_string();
        assert!(large.contains(&expected_row), "Contains large row");
        assert!(large.contains(&expected_col), "Contains large col");
    }

    #[test]
    fn test_cursor_move_large_values() {
        // Test large relative movements
        let large_up = cursor_move(0, -10000);
        assert!(large_up.contains("10000A"), "Large up movement");

        let large_right = cursor_move(50000, 0);
        assert!(large_right.contains("50000C"), "Large right movement");
    }

    #[test]
    fn test_combined_cursor_move() {
        // Combined movements produce multiple sequences
        let combined = cursor_move(5, -3);
        assert!(combined.contains("3A"), "Up component");
        assert!(combined.contains("5C"), "Right component");
    }

    #[test]
    fn test_cursor_style_sequences() {
        // Test cursor style sequences
        use super::sequences::cursor_style;

        assert_eq!(cursor_style::BLOCK_BLINK, "\x1b[1 q");
        assert_eq!(cursor_style::BLOCK_STEADY, "\x1b[2 q");
        assert_eq!(cursor_style::UNDERLINE_BLINK, "\x1b[3 q");
        assert_eq!(cursor_style::UNDERLINE_STEADY, "\x1b[4 q");
        assert_eq!(cursor_style::BAR_BLINK, "\x1b[5 q");
        assert_eq!(cursor_style::BAR_STEADY, "\x1b[6 q");
        assert_eq!(cursor_style::DEFAULT, "\x1b[0 q");
    }
}
