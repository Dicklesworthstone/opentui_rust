//! Grapheme cluster iteration.

use crate::unicode::width::WidthMethod;
use crate::unicode::width::display_width_with_method;
use unicode_segmentation::UnicodeSegmentation;

/// Grapheme metadata for layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GraphemeInfo {
    pub byte_offset: u32,
    pub byte_len: u8,
    pub col_offset: u32,
    pub width: u8,
}

/// Iterator over grapheme clusters in a string.
pub struct GraphemeIterator<'a> {
    inner: unicode_segmentation::Graphemes<'a>,
}

impl<'a> Iterator for GraphemeIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Iterate over grapheme clusters in a string.
#[must_use]
pub fn graphemes(s: &str) -> GraphemeIterator<'_> {
    GraphemeIterator {
        inner: s.graphemes(true),
    }
}

/// Iterate over grapheme clusters with byte indices.
pub fn grapheme_indices(s: &str) -> impl Iterator<Item = (usize, &str)> {
    s.grapheme_indices(true)
}

/// Check if a string is ASCII-only.
#[must_use]
pub fn is_ascii_only(s: &str) -> bool {
    s.is_ascii()
}

/// Compute grapheme info for a string.
///
/// Note: `byte_len` and `width` are stored as `u8` for memory efficiency.
/// Values are clamped to `u8::MAX` (255) to prevent silent truncation.
/// This is safe because:
/// - Grapheme clusters rarely exceed 255 bytes (even complex ZWJ emoji are ~30 bytes)
/// - Display widths are almost always 0, 1, or 2 (tab stops are bounded)
#[must_use]
pub fn grapheme_info(s: &str, tab_width: u32, method: WidthMethod) -> Vec<GraphemeInfo> {
    let mut infos = Vec::new();
    let mut col = 0u32;
    let tab_width = tab_width.max(1);

    for (byte_offset, grapheme) in s.grapheme_indices(true) {
        let width = if grapheme == "\t" {
            let spaces = tab_width - (col % tab_width);
            // Saturate to u8::MAX to prevent silent truncation
            spaces.min(u32::from(u8::MAX)) as u8
        } else {
            let w = display_width_with_method(grapheme, method);
            // Saturate to u8::MAX (display widths are typically 0-2)
            w.min(usize::from(u8::MAX)) as u8
        };

        let info = GraphemeInfo {
            byte_offset: byte_offset as u32,
            // Saturate byte_len to u8::MAX - graphemes are rarely >255 bytes
            byte_len: grapheme.len().min(usize::from(u8::MAX)) as u8,
            col_offset: col,
            width,
        };
        infos.push(info);
        col += u32::from(width);
    }

    infos
}

/// Split text into grapheme clusters with their display widths.
///
/// Returns a vector of `(grapheme, width)` pairs where:
/// - `grapheme` is a string slice of the grapheme cluster
/// - `width` is the display width in terminal columns
///
/// This is useful for rendering and cursor positioning where you need
/// both the grapheme text and its display width.
///
/// # Example
///
/// ```
/// use opentui_rust::unicode::split_graphemes_with_widths;
///
/// let pairs = split_graphemes_with_widths("A世界");
/// assert_eq!(pairs.len(), 3);
/// assert_eq!(pairs[0], ("A", 1));
/// assert_eq!(pairs[1], ("世", 2));
/// assert_eq!(pairs[2], ("界", 2));
/// ```
#[must_use]
pub fn split_graphemes_with_widths(text: &str) -> Vec<(&str, usize)> {
    use crate::unicode::display_width;

    text.graphemes(true)
        .map(|g| (g, display_width(g)))
        .collect()
}

/// Find the nearest grapheme cluster boundary at or before a byte position.
///
/// Given a byte position in a string, returns the byte index of the start
/// of the grapheme cluster that contains (or starts at) that position.
/// This prevents splitting grapheme clusters during cursor movement or editing.
///
/// # Arguments
///
/// * `text` - The text to search
/// * `pos` - The byte position to find the boundary for
///
/// # Returns
///
/// The byte index of the grapheme boundary at or before `pos`.
/// - If `pos` is at a grapheme boundary, returns `pos`
/// - If `pos` is within a grapheme, returns the start of that grapheme
/// - If `pos` is beyond the string, returns `text.len()`
/// - If `text` is empty, returns 0
///
/// # Example
///
/// ```
/// use opentui_rust::unicode::find_grapheme_boundary;
///
/// // ASCII: each byte is a boundary
/// assert_eq!(find_grapheme_boundary("hello", 2), 2);
///
/// // Multi-byte char: position within returns start
/// let text = "世界"; // "世" is 3 bytes
/// assert_eq!(find_grapheme_boundary(text, 0), 0);
/// assert_eq!(find_grapheme_boundary(text, 1), 0); // Within "世"
/// assert_eq!(find_grapheme_boundary(text, 3), 3); // Start of "界"
///
/// // Combining character: treated as part of base
/// let combined = "e\u{0301}"; // e + combining acute = 3 bytes
/// assert_eq!(find_grapheme_boundary(combined, 0), 0);
/// assert_eq!(find_grapheme_boundary(combined, 1), 0); // Within grapheme
/// assert_eq!(find_grapheme_boundary(combined, 2), 0); // Within grapheme
/// ```
#[must_use]
pub fn find_grapheme_boundary(text: &str, pos: usize) -> usize {
    if text.is_empty() || pos == 0 {
        return 0;
    }

    if pos >= text.len() {
        return text.len();
    }

    // Find the grapheme that contains this position
    let mut last_boundary = 0;
    for (byte_offset, _grapheme) in text.grapheme_indices(true) {
        if byte_offset > pos {
            // We've passed the position, return the previous boundary
            return last_boundary;
        }
        if byte_offset == pos {
            // Exact match - this is a boundary
            return pos;
        }
        last_boundary = byte_offset;
    }

    // Position is in the last grapheme or beyond
    last_boundary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graphemes_ascii() {
        let g: Vec<_> = graphemes("hello").collect();
        assert_eq!(g, vec!["h", "e", "l", "l", "o"]);
    }

    #[test]
    fn test_graphemes_emoji() {
        // Family emoji (ZWJ sequence)
        assert_eq!(graphemes("👨‍👩‍👧").count(), 1);
    }

    #[test]
    fn test_graphemes_combining() {
        // e + combining acute accent
        assert_eq!(graphemes("e\u{0301}").count(), 1);
    }

    #[test]
    fn test_grapheme_info_basic() {
        let infos = grapheme_info("ab\tc", 4, WidthMethod::WcWidth);
        assert!(!infos.is_empty());
        assert_eq!(infos[0].byte_offset, 0);
        assert_eq!(infos[0].width, 1);
    }

    #[test]
    fn test_grapheme_info_clamping() {
        // Test clamping of byte_len
        // Create a fake grapheme > 255 bytes (not a real unicode grapheme, but treated as one block if we force it,
        // actually unicode segmentation will split it. So we construct a string where a single grapheme is huge.
        // A huge sequence of combining marks on a base char.
        let mut huge_grapheme = String::from("a");
        for _ in 0..300 {
            huge_grapheme.push('\u{0301}'); // combining acute accent
        }

        let infos = grapheme_info(&huge_grapheme, 4, WidthMethod::WcWidth);
        assert_eq!(infos.len(), 1); // Should be one huge grapheme
        assert_eq!(infos[0].byte_len, 255); // Clamped to u8::MAX

        // Test clamping of width (tab width > 255)
        // If tab width is huge, a single tab character should report width 255 max
        let infos_tab = grapheme_info("\t", 300, WidthMethod::WcWidth);
        assert_eq!(infos_tab.len(), 1);
        assert_eq!(infos_tab[0].width, 255); // Clamped to u8::MAX
    }

    #[test]
    fn test_split_graphemes_with_widths_empty() {
        let pairs = split_graphemes_with_widths("");
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_split_graphemes_with_widths_ascii() {
        let pairs = split_graphemes_with_widths("hello");
        assert_eq!(pairs.len(), 5);
        assert_eq!(pairs[0], ("h", 1));
        assert_eq!(pairs[1], ("e", 1));
        assert_eq!(pairs[2], ("l", 1));
        assert_eq!(pairs[3], ("l", 1));
        assert_eq!(pairs[4], ("o", 1));
    }

    #[test]
    fn test_split_graphemes_with_widths_cjk() {
        let pairs = split_graphemes_with_widths("世界");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("世", 2));
        assert_eq!(pairs[1], ("界", 2));
    }

    #[test]
    fn test_split_graphemes_with_widths_emoji() {
        let pairs = split_graphemes_with_widths("👍");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "👍");
        assert_eq!(pairs[0].1, 2);
    }

    #[test]
    fn test_split_graphemes_with_widths_zwj() {
        // Family emoji is a single grapheme
        let pairs = split_graphemes_with_widths("👨‍👩‍👧");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "👨‍👩‍👧");
        // Width is 2 (displayed as single wide char)
        assert_eq!(pairs[0].1, 2);
    }

    #[test]
    fn test_split_graphemes_with_widths_mixed() {
        let pairs = split_graphemes_with_widths("A世👍");
        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0], ("A", 1));
        assert_eq!(pairs[1], ("世", 2));
        assert_eq!(pairs[2].0, "👍");
        assert_eq!(pairs[2].1, 2);
    }

    #[test]
    fn test_split_graphemes_with_widths_combining() {
        // e + combining acute accent = single grapheme
        let pairs = split_graphemes_with_widths("e\u{0301}");
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].0, "e\u{0301}");
        // Width is 1 (base char width)
        assert_eq!(pairs[0].1, 1);
    }

    #[test]
    fn test_find_grapheme_boundary_empty() {
        assert_eq!(find_grapheme_boundary("", 0), 0);
        assert_eq!(find_grapheme_boundary("", 5), 0);
    }

    #[test]
    fn test_find_grapheme_boundary_ascii() {
        let text = "hello";
        assert_eq!(find_grapheme_boundary(text, 0), 0);
        assert_eq!(find_grapheme_boundary(text, 1), 1);
        assert_eq!(find_grapheme_boundary(text, 2), 2);
        assert_eq!(find_grapheme_boundary(text, 5), 5); // End of string
    }

    #[test]
    fn test_find_grapheme_boundary_beyond_string() {
        assert_eq!(find_grapheme_boundary("abc", 10), 3);
    }

    #[test]
    fn test_find_grapheme_boundary_multibyte() {
        // "世界" - each character is 3 bytes
        let text = "世界";
        assert_eq!(find_grapheme_boundary(text, 0), 0); // Start of "世"
        assert_eq!(find_grapheme_boundary(text, 1), 0); // Within "世"
        assert_eq!(find_grapheme_boundary(text, 2), 0); // Within "世"
        assert_eq!(find_grapheme_boundary(text, 3), 3); // Start of "界"
        assert_eq!(find_grapheme_boundary(text, 4), 3); // Within "界"
        assert_eq!(find_grapheme_boundary(text, 5), 3); // Within "界"
        assert_eq!(find_grapheme_boundary(text, 6), 6); // End of string
    }

    #[test]
    fn test_find_grapheme_boundary_combining() {
        // e + combining acute accent = 3 bytes total, single grapheme
        let text = "e\u{0301}";
        assert_eq!(text.len(), 3); // Verify length
        assert_eq!(find_grapheme_boundary(text, 0), 0);
        assert_eq!(find_grapheme_boundary(text, 1), 0); // Within grapheme
        assert_eq!(find_grapheme_boundary(text, 2), 0); // Within grapheme
        assert_eq!(find_grapheme_boundary(text, 3), 3); // End
    }

    #[test]
    fn test_find_grapheme_boundary_emoji() {
        // 👍 is 4 bytes
        let text = "👍";
        assert_eq!(find_grapheme_boundary(text, 0), 0);
        assert_eq!(find_grapheme_boundary(text, 1), 0);
        assert_eq!(find_grapheme_boundary(text, 2), 0);
        assert_eq!(find_grapheme_boundary(text, 3), 0);
        assert_eq!(find_grapheme_boundary(text, 4), 4); // End
    }

    #[test]
    fn test_find_grapheme_boundary_zwj() {
        // Family emoji is a single grapheme but many bytes
        let text = "👨‍👩‍👧";
        let len = text.len();
        assert!(len > 4); // ZWJ sequences are long

        // Position 0 is always 0
        assert_eq!(find_grapheme_boundary(text, 0), 0);

        // Any position within the grapheme should return 0
        for pos in 1..len {
            assert_eq!(
                find_grapheme_boundary(text, pos),
                0,
                "Position {pos} should return 0"
            );
        }

        // Position at end returns len
        assert_eq!(find_grapheme_boundary(text, len), len);
    }

    #[test]
    fn test_find_grapheme_boundary_mixed() {
        // "A世👍" - 1 + 3 + 4 = 8 bytes
        let text = "A世👍";
        assert_eq!(find_grapheme_boundary(text, 0), 0); // Start of "A"
        assert_eq!(find_grapheme_boundary(text, 1), 1); // Start of "世"
        assert_eq!(find_grapheme_boundary(text, 2), 1); // Within "世"
        assert_eq!(find_grapheme_boundary(text, 3), 1); // Within "世"
        assert_eq!(find_grapheme_boundary(text, 4), 4); // Start of "👍"
        assert_eq!(find_grapheme_boundary(text, 5), 4); // Within "👍"
        assert_eq!(find_grapheme_boundary(text, 7), 4); // Within "👍"
        assert_eq!(find_grapheme_boundary(text, 8), 8); // End
    }
}
