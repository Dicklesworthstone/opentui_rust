//! Styled text segments for rich text rendering.

use crate::style::Style;
use std::ops::Range;

/// A segment of text with associated style.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledSegment {
    /// Byte range in the source text.
    pub range: Range<usize>,
    /// Style applied to this segment.
    pub style: Style,
    /// Priority for overlapping segments (higher wins).
    pub priority: u8,
    /// Optional highlight reference ID for batch removal.
    pub ref_id: Option<u16>,
    /// Optional source line for line-based highlights.
    pub line: Option<usize>,
}

impl StyledSegment {
    /// Create a new styled segment.
    #[must_use]
    pub fn new(range: Range<usize>, style: Style) -> Self {
        Self {
            range,
            style,
            priority: 0,
            ref_id: None,
            line: None,
        }
    }

    /// Create a segment with priority.
    #[must_use]
    pub fn with_priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Attach a highlight reference ID.
    #[must_use]
    pub fn with_ref(mut self, ref_id: u16) -> Self {
        self.ref_id = Some(ref_id);
        self
    }

    /// Attach a source line for line-based highlights.
    #[must_use]
    pub fn with_line(mut self, line: usize) -> Self {
        self.line = Some(line);
        self
    }

    /// Check if this segment overlaps with another.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.range.start < other.range.end && other.range.start < self.range.end
    }

    /// Check if this segment contains a position.
    #[must_use]
    pub fn contains(&self, pos: usize) -> bool {
        self.range.contains(&pos)
    }

    /// Get the length in bytes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.range.end - self.range.start
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.range.start >= self.range.end
    }
}

/// A chunk of styled text for building TextBuffer content.
#[derive(Clone, Debug)]
pub struct StyledChunk<'a> {
    /// The text content.
    pub text: &'a str,
    /// The style to apply.
    pub style: Style,
}

impl<'a> StyledChunk<'a> {
    /// Create a new styled chunk.
    #[must_use]
    pub fn new(text: &'a str, style: Style) -> Self {
        Self { text, style }
    }

    /// Create an unstyled chunk.
    #[must_use]
    pub fn plain(text: &'a str) -> Self {
        Self {
            text,
            style: Style::NONE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_overlap() {
        let a = StyledSegment::new(0..10, Style::NONE);
        let b = StyledSegment::new(5..15, Style::NONE);
        let c = StyledSegment::new(10..20, Style::NONE);

        assert!(a.overlaps(&b));
        assert!(b.overlaps(&a));
        assert!(!a.overlaps(&c)); // adjacent, not overlapping
    }

    #[test]
    fn test_segment_contains() {
        let seg = StyledSegment::new(5..10, Style::NONE);
        assert!(!seg.contains(4));
        assert!(seg.contains(5));
        assert!(seg.contains(9));
        assert!(!seg.contains(10));
    }
}
