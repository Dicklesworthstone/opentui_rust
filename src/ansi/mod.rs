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
pub fn write_cursor_position(w: &mut impl Write, row: u32, col: u32) -> io::Result<()> {
    write!(w, "\x1b[{};{}H", row + 1, col + 1)
}

/// Generate relative cursor movement.
#[must_use]
pub fn cursor_move(dx: i32, dy: i32) -> String {
    let mut buf = Vec::new();
    write_cursor_move(&mut buf, dx, dy).unwrap();
    String::from_utf8(buf).unwrap()
}

/// Write relative cursor movement to a writer.
pub fn write_cursor_move(w: &mut impl Write, dx: i32, dy: i32) -> io::Result<()> {
    if dy < 0 {
        write!(w, "\x1b[{}A", -dy)?;
    } else if dy > 0 {
        write!(w, "\x1b[{dy}B")?;
    }

    if dx > 0 {
        write!(w, "\x1b[{dx}C")?;
    } else if dx < 0 {
        write!(w, "\x1b[{}D", -dx)?;
    }
    Ok(())
}

/// Escape a URL for safe inclusion in OSC 8 hyperlink sequences.
///
/// Control characters (0x00-0x1F, 0x7F) are percent-encoded to prevent
/// escape sequence injection attacks. This is critical because an unescaped
/// ESC (0x1B) or BEL (0x07) could terminate the OSC sequence early and
/// allow arbitrary terminal command injection.
///
/// Non-ASCII bytes (UTF-8 sequences) are preserved unchanged.
#[must_use]
pub fn escape_url_for_osc8(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut escaped = Vec::with_capacity(bytes.len());

    for &byte in bytes {
        match byte {
            // Control characters (C0 and DEL) must be percent-encoded
            0x00..=0x1F | 0x7F => {
                escaped.push(b'%');
                // Use uppercase hex for RFC 3986 compatibility
                let high = (byte >> 4) & 0x0F;
                let low = byte & 0x0F;
                escaped.push(if high < 10 {
                    b'0' + high
                } else {
                    b'A' + high - 10
                });
                escaped.push(if low < 10 {
                    b'0' + low
                } else {
                    b'A' + low - 10
                });
            }
            // All other bytes pass through unchanged (preserves UTF-8)
            _ => escaped.push(byte),
        }
    }

    // SAFETY: Input is valid UTF-8, we only modified single-byte control characters
    // (0x00-0x1F, 0x7F) which are complete UTF-8 sequences, so output is valid UTF-8
    String::from_utf8(escaped).expect("escape_url_for_osc8 produced invalid UTF-8")
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
}
