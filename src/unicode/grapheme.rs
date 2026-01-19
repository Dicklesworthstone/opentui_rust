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
#[must_use]
pub fn grapheme_info(s: &str, tab_width: u32, method: WidthMethod) -> Vec<GraphemeInfo> {
    let mut infos = Vec::new();
    let mut col = 0u32;
    let tab_width = tab_width.max(1);

    for (byte_offset, grapheme) in s.grapheme_indices(true) {
        let width = if grapheme == "\t" {
            let spaces = tab_width - (col % tab_width);
            spaces as u8
        } else {
            display_width_with_method(grapheme, method) as u8
        };

        let info = GraphemeInfo {
            byte_offset: byte_offset as u32,
            byte_len: grapheme.len() as u8,
            col_offset: col,
            width,
        };
        infos.push(info);
        col += width as u32;
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
}
