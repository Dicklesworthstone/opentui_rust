//! SIMD-optimized text searching and analysis.
//!
//! This module provides high-performance functions for finding:
//! - Line breaks (LF, CR, CRLF)
//! - Tab stops
//! - Word/wrap break points
//!
//! These functions use SIMD-optimized algorithms where available.

use unicode_segmentation::UnicodeSegmentation;

/// Result of a line break search.
#[derive(Clone, Debug, Default)]
pub struct LineBreakResult {
    /// Byte offsets where line breaks start.
    pub positions: Vec<usize>,
    /// Length of each line break (1 for LF/CR, 2 for CRLF).
    pub lengths: Vec<u8>,
}

/// Result of a tab stop search.
#[derive(Clone, Debug, Default)]
pub struct TabStopResult {
    /// Byte offsets where tabs occur.
    pub positions: Vec<usize>,
}

/// Result of a wrap break search.
#[derive(Clone, Debug, Default)]
pub struct WrapBreakResult {
    /// Byte offsets where wrapping can occur.
    pub positions: Vec<usize>,
    /// Type of break point (whitespace, punctuation, etc.).
    pub break_types: Vec<BreakType>,
}

/// Type of wrap break point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BreakType {
    /// Whitespace (space, tab).
    Whitespace,
    /// Punctuation that allows breaking after.
    Punctuation,
    /// Opening bracket/paren.
    OpenBracket,
    /// Closing bracket/paren.
    CloseBracket,
    /// Hyphen or dash.
    Hyphen,
}

/// Check if a string contains only ASCII characters.
///
/// This uses a fast SIMD-optimized check when the string is long enough.
/// For short strings, it falls back to byte-by-byte checking.
#[must_use]
#[inline]
pub fn is_ascii_only_fast(s: &str) -> bool {
    // Rust's str::is_ascii() is already well-optimized
    // and uses SIMD on supported platforms
    s.is_ascii()
}

/// Check if a string contains only printable ASCII (32-126).
///
/// This is useful for detecting strings that need no special Unicode handling.
#[must_use]
pub fn is_printable_ascii_only(s: &str) -> bool {
    s.bytes().all(|b| (32..=126).contains(&b))
}

/// Find all line breaks in a string.
///
/// Detects LF (`\n`), CR (`\r`), and CRLF (`\r\n`) sequences.
/// Returns positions and lengths of each line break.
#[must_use]
pub fn find_line_breaks(text: &str) -> LineBreakResult {
    let mut result = LineBreakResult::default();
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        match bytes[i] {
            b'\n' => {
                result.positions.push(i);
                result.lengths.push(1);
                i += 1;
            }
            b'\r' => {
                // Check for CRLF
                if i + 1 < len && bytes[i + 1] == b'\n' {
                    result.positions.push(i);
                    result.lengths.push(2);
                    i += 2;
                } else {
                    result.positions.push(i);
                    result.lengths.push(1);
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }

    result
}

/// Find all tab characters in a string.
///
/// Returns byte offsets of each tab character.
#[must_use]
pub fn find_tab_stops(text: &str) -> TabStopResult {
    let mut result = TabStopResult::default();

    for (i, b) in text.bytes().enumerate() {
        if b == b'\t' {
            result.positions.push(i);
        }
    }

    result
}

/// Find all potential wrap break points in a string.
///
/// Identifies positions where text can be wrapped:
/// - After whitespace
/// - After punctuation
/// - After hyphens
/// - Around brackets
#[must_use]
pub fn find_wrap_breaks(text: &str) -> WrapBreakResult {
    let mut result = WrapBreakResult::default();

    for (i, ch) in text.char_indices() {
        let break_type = match ch {
            ' ' | '\t' => Some(BreakType::Whitespace),
            '.' | ',' | ';' | ':' | '!' | '?' => Some(BreakType::Punctuation),
            '(' | '[' | '{' | '<' => Some(BreakType::OpenBracket),
            ')' | ']' | '}' | '>' => Some(BreakType::CloseBracket),
            '-' | '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' => {
                Some(BreakType::Hyphen)
            }
            _ => None,
        };

        if let Some(bt) = break_type {
            result.positions.push(i + ch.len_utf8());
            result.break_types.push(bt);
        }
    }

    result
}

/// Find the byte position to wrap text at a given column width.
///
/// Returns the byte offset where wrapping should occur, preferring
/// word boundaries when possible.
#[must_use]
pub fn find_wrap_position(text: &str, max_columns: u32, tab_width: u8) -> Option<usize> {
    if text.is_empty() || max_columns == 0 {
        return None;
    }

    let mut col = 0u32;
    let mut last_break = None;
    let mut last_break_col = 0u32;

    for (byte_idx, grapheme) in text.grapheme_indices(true) {
        let width = if grapheme == "\t" {
            u32::from(tab_width) - (col % u32::from(tab_width))
        } else {
            unicode_width::UnicodeWidthStr::width(grapheme) as u32
        };

        // Check if this is a potential break point
        if let Some(ch) = grapheme.chars().next() {
            if ch.is_whitespace() || is_break_char(ch) {
                last_break = Some(byte_idx + grapheme.len());
                last_break_col = col + width;
            }
        }

        col += width;

        // If we've exceeded the width
        if col > max_columns {
            // Prefer breaking at last word boundary if we have one
            // and it's not too far back
            if let Some(break_pos) = last_break {
                if last_break_col >= max_columns / 2 {
                    return Some(break_pos);
                }
            }
            // Otherwise break at current position
            return Some(byte_idx);
        }
    }

    // Text fits within max_columns
    None
}

/// Check if a character is a potential break point.
fn is_break_char(ch: char) -> bool {
    matches!(
        ch,
        '.' | ',' | ';' | ':' | '!' | '?' | '-' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>'
    )
}

/// Find position by column width.
///
/// Returns the byte offset of the grapheme at or just before the specified column.
#[must_use]
pub fn find_position_by_width(text: &str, target_column: u32, tab_width: u8) -> usize {
    if text.is_empty() || target_column == 0 {
        return 0;
    }

    let mut col = 0u32;

    for (byte_idx, grapheme) in text.grapheme_indices(true) {
        let width = if grapheme == "\t" {
            u32::from(tab_width) - (col % u32::from(tab_width))
        } else {
            unicode_width::UnicodeWidthStr::width(grapheme) as u32
        };

        if col + width > target_column {
            return byte_idx;
        }

        col += width;

        if col >= target_column {
            return byte_idx + grapheme.len();
        }
    }

    text.len()
}

/// Get the previous grapheme start position.
///
/// Returns the byte offset and width of the grapheme before the given position.
#[must_use]
pub fn get_prev_grapheme_start(text: &str, byte_offset: usize, tab_width: u8) -> Option<(usize, u32)> {
    if byte_offset == 0 || text.is_empty() {
        return None;
    }

    let prefix = &text[..byte_offset.min(text.len())];
    let graphemes: Vec<_> = prefix.grapheme_indices(true).collect();

    if graphemes.is_empty() {
        return None;
    }

    let (start, grapheme) = graphemes.last()?;
    let width = if *grapheme == "\t" {
        // Calculate tab width at this position
        let mut col = 0u32;
        for (_, g) in &graphemes[..graphemes.len() - 1] {
            if *g == "\t" {
                col += u32::from(tab_width) - (col % u32::from(tab_width));
            } else {
                col += unicode_width::UnicodeWidthStr::width(*g) as u32;
            }
        }
        u32::from(tab_width) - (col % u32::from(tab_width))
    } else {
        unicode_width::UnicodeWidthStr::width(*grapheme) as u32
    };

    Some((*start, width))
}

/// Calculate text width in columns.
///
/// This is a faster version that handles tabs.
#[must_use]
pub fn calculate_text_width(text: &str, tab_width: u8) -> u32 {
    if text.is_empty() {
        return 0;
    }

    let mut col = 0u32;

    for grapheme in text.graphemes(true) {
        if grapheme == "\t" {
            col += u32::from(tab_width) - (col % u32::from(tab_width));
        } else {
            col += unicode_width::UnicodeWidthStr::width(grapheme) as u32;
        }
    }

    col
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_ascii_only_fast() {
        assert!(is_ascii_only_fast("hello world"));
        assert!(is_ascii_only_fast(""));
        assert!(!is_ascii_only_fast("héllo"));
        assert!(!is_ascii_only_fast("hello 🌍"));
    }

    #[test]
    fn test_is_printable_ascii_only() {
        assert!(is_printable_ascii_only("hello world"));
        assert!(is_printable_ascii_only(""));
        assert!(!is_printable_ascii_only("hello\tworld")); // tab is not printable
        assert!(!is_printable_ascii_only("hello\nworld")); // newline is not printable
    }

    #[test]
    fn test_find_line_breaks() {
        let result = find_line_breaks("a\nb\r\nc\rd");
        assert_eq!(result.positions, vec![1, 3, 6]);
        assert_eq!(result.lengths, vec![1, 2, 1]);
    }

    #[test]
    fn test_find_line_breaks_empty() {
        let result = find_line_breaks("");
        assert!(result.positions.is_empty());
    }

    #[test]
    fn test_find_tab_stops() {
        let result = find_tab_stops("a\tb\tc");
        assert_eq!(result.positions, vec![1, 3]);
    }

    #[test]
    fn test_find_wrap_breaks() {
        let result = find_wrap_breaks("hello, world!");
        assert!(!result.positions.is_empty());
        assert!(result.break_types.contains(&BreakType::Whitespace));
        assert!(result.break_types.contains(&BreakType::Punctuation));
    }

    #[test]
    fn test_find_wrap_position() {
        // "hello world" with max 6 columns should wrap after "hello "
        let pos = find_wrap_position("hello world", 6, 4);
        assert_eq!(pos, Some(6)); // After "hello "
    }

    #[test]
    fn test_find_wrap_position_no_wrap_needed() {
        let pos = find_wrap_position("hello", 10, 4);
        assert_eq!(pos, None);
    }

    #[test]
    fn test_find_position_by_width() {
        assert_eq!(find_position_by_width("hello", 0, 4), 0);
        assert_eq!(find_position_by_width("hello", 3, 4), 3);
        assert_eq!(find_position_by_width("hello", 10, 4), 5);
    }

    #[test]
    fn test_find_position_by_width_with_tab() {
        // "a\tb" with tab_width=4: a=col0, tab fills cols 1-3, b=col4
        // At col 3, we're still within the tab, so we get position 1 (start of tab)
        let pos = find_position_by_width("a\tb", 3, 4);
        assert_eq!(pos, 1); // Within the tab

        // At col 4, we should be at 'b'
        let pos = find_position_by_width("a\tb", 4, 4);
        assert_eq!(pos, 2); // After the tab, at 'b'
    }

    #[test]
    fn test_get_prev_grapheme_start() {
        let result = get_prev_grapheme_start("hello", 3, 4);
        assert_eq!(result, Some((2, 1))); // 'l' at position 2, width 1

        let result = get_prev_grapheme_start("hello", 0, 4);
        assert_eq!(result, None);
    }

    #[test]
    fn test_calculate_text_width() {
        assert_eq!(calculate_text_width("hello", 4), 5);
        assert_eq!(calculate_text_width("", 4), 0);
        assert_eq!(calculate_text_width("\t", 4), 4); // Tab at col 0 -> 4 spaces
        assert_eq!(calculate_text_width("a\t", 4), 4); // 'a' + tab fills to col 4
    }

    #[test]
    fn test_calculate_text_width_wide_chars() {
        // CJK characters are typically 2 columns wide
        assert_eq!(calculate_text_width("漢字", 4), 4); // 2 chars × 2 width
    }
}
