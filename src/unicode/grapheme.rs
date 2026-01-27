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
}
