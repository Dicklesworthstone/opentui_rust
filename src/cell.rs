//! Terminal cell type representing a single character position.
//!
//! A terminal display is a grid of cells, where each cell contains a single
//! character (or grapheme cluster) along with styling information. This module
//! provides [`Cell`] and [`CellContent`] types for representing this data.
//!
//! # Wide Characters and Graphemes
//!
//! Some characters (CJK, emoji) have display width 2. When a wide character
//! is placed in a cell, the following cell becomes a [`CellContent::Continuation`]
//! to indicate it's occupied by the previous character.
//!
//! # Examples
//!
//! ```
//! use opentui::{Cell, Style, Rgba};
//!
//! // Create a simple character cell
//! let cell = Cell::new('A', Style::fg(Rgba::GREEN));
//!
//! // Create a cell with an emoji (grapheme cluster)
//! let emoji = Cell::from_grapheme("👍", Style::NONE);
//! assert_eq!(emoji.display_width(), 2);
//!
//! // Clear a cell (renders as space)
//! let empty = Cell::clear(Rgba::BLACK);
//! ```

use crate::color::Rgba;
use crate::style::{Style, TextAttributes};
use std::borrow::Cow;
use std::sync::Arc;

/// Content of a terminal cell.
///
/// Represents what is displayed in a single cell position. Most cells contain
/// either a simple character or are empty. Wide characters and emoji use
/// grapheme clusters and leave continuation markers in following cells.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum CellContent {
    /// Simple ASCII or single-codepoint character (display width 1-2).
    Char(char),
    /// Grapheme cluster (emoji, combining chars, ZWJ sequences).
    Grapheme(Arc<str>),
    /// Empty/cleared cell.
    #[default]
    Empty,
    /// Continuation of a wide character from the previous cell.
    Continuation,
}

impl CellContent {
    /// Get the display width of this content.
    #[must_use]
    pub fn display_width(&self) -> usize {
        match self {
            Self::Char(c) => crate::unicode::display_width_char(*c),
            Self::Grapheme(s) => crate::unicode::display_width(s.as_ref()),
            Self::Empty => 1,
            Self::Continuation => 0,
        }
    }

    /// Check if this is a continuation cell.
    #[must_use]
    pub fn is_continuation(&self) -> bool {
        matches!(self, Self::Continuation)
    }

    /// Check if this is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Get the string representation of this content.
    ///
    /// Returns a `Cow<str>` to handle both borrowed (grapheme, empty, continuation)
    /// and owned (single char) cases efficiently.
    #[must_use]
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            Self::Char(c) => {
                let mut buf = [0u8; 4];
                Cow::Owned(c.encode_utf8(&mut buf).to_owned())
            }
            Self::Grapheme(s) => Cow::Borrowed(s.as_ref()),
            Self::Empty => Cow::Borrowed(" "),
            Self::Continuation => Cow::Borrowed(""),
        }
    }
}

/// A single terminal cell with content and styling.
///
/// Cells are the fundamental unit of terminal rendering. Each cell occupies
/// one column position and contains:
/// - Content: A character, grapheme cluster, or empty/continuation marker
/// - Foreground and background colors (with alpha for blending)
/// - Text attributes (bold, italic, etc.)
/// - Optional hyperlink ID for OSC 8 links
///
/// # Alpha Blending
///
/// Cells support alpha blending via [`Cell::blend_over`], which composites
/// one cell on top of another using Porter-Duff "over" compositing. This
/// enables transparent overlays and layered UI elements.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    /// The character or grapheme content.
    pub content: CellContent,
    /// Foreground color.
    pub fg: Rgba,
    /// Background color.
    pub bg: Rgba,
    /// Text rendering attributes.
    pub attributes: TextAttributes,
    /// Optional hyperlink ID.
    pub link_id: Option<u32>,
}

impl Cell {
    /// Create a new cell with a single character.
    #[must_use]
    pub fn new(ch: char, style: Style) -> Self {
        Self {
            content: CellContent::Char(ch),
            fg: style.fg.unwrap_or(Rgba::WHITE),
            bg: style.bg.unwrap_or(Rgba::TRANSPARENT),
            attributes: style.attributes,
            link_id: style.link_id,
        }
    }

    /// Create a cell from a grapheme cluster string.
    #[must_use]
    pub fn from_grapheme(s: &str, style: Style) -> Self {
        let content = if s.chars().count() == 1 {
            CellContent::Char(s.chars().next().unwrap())
        } else {
            CellContent::Grapheme(Arc::from(s))
        };

        Self {
            content,
            fg: style.fg.unwrap_or(Rgba::WHITE),
            bg: style.bg.unwrap_or(Rgba::TRANSPARENT),
            attributes: style.attributes,
            link_id: style.link_id,
        }
    }

    /// Create a cleared/empty cell with the specified background.
    #[must_use]
    pub fn clear(bg: Rgba) -> Self {
        Self {
            content: CellContent::Empty,
            fg: Rgba::WHITE,
            bg,
            attributes: TextAttributes::empty(),
            link_id: None,
        }
    }

    /// Create a continuation cell (placeholder for wide characters).
    #[must_use]
    pub fn continuation(bg: Rgba) -> Self {
        Self {
            content: CellContent::Continuation,
            fg: Rgba::WHITE,
            bg,
            attributes: TextAttributes::empty(),
            link_id: None,
        }
    }

    /// Get the display width of this cell.
    #[must_use]
    pub fn display_width(&self) -> usize {
        self.content.display_width()
    }

    /// Check if this is a continuation cell.
    #[must_use]
    pub fn is_continuation(&self) -> bool {
        self.content.is_continuation()
    }

    /// Check if this cell is empty/cleared.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Write the cell content to a writer.
    pub fn write_content<W: std::io::Write>(&self, w: &mut W) -> std::io::Result<()> {
        match &self.content {
            CellContent::Char(c) => write!(w, "{c}"),
            CellContent::Grapheme(s) => write!(w, "{s}"),
            CellContent::Empty => write!(w, " "),
            CellContent::Continuation => Ok(()),
        }
    }

    /// Apply a style to this cell.
    pub fn apply_style(&mut self, style: Style) {
        if let Some(fg) = style.fg {
            self.fg = fg;
        }
        if let Some(bg) = style.bg {
            self.bg = bg;
        }
        self.attributes |= style.attributes;
        if style.link_id.is_some() {
            self.link_id = style.link_id;
        }
    }

    /// Blend this cell's colors with a global opacity factor.
    pub fn blend_with_opacity(&mut self, opacity: f32) {
        self.fg = self.fg.multiply_alpha(opacity);
        self.bg = self.bg.multiply_alpha(opacity);
    }

    /// Blend this cell over a background cell using alpha compositing.
    #[must_use]
    pub fn blend_over(self, background: &Cell) -> Cell {
        Cell {
            content: if self.content.is_empty() {
                background.content.clone()
            } else {
                self.content
            },
            fg: self.fg.blend_over(background.fg),
            bg: self.bg.blend_over(background.bg),
            attributes: self.attributes | background.attributes,
            link_id: self.link_id.or(background.link_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_new() {
        let cell = Cell::new('A', Style::fg(Rgba::RED));
        assert!(matches!(cell.content, CellContent::Char('A')));
        assert_eq!(cell.fg, Rgba::RED);
        assert_eq!(cell.display_width(), 1);
    }

    #[test]
    fn test_cell_grapheme() {
        let cell = Cell::from_grapheme("👨‍👩‍👧", Style::NONE);
        assert!(matches!(cell.content, CellContent::Grapheme(_)));
        // ZWJ family emoji has width 2
        assert_eq!(cell.display_width(), 2);
    }

    #[test]
    fn test_cell_clear() {
        let cell = Cell::clear(Rgba::BLACK);
        assert!(cell.is_empty());
        assert_eq!(cell.bg, Rgba::BLACK);
    }

    #[test]
    fn test_cell_continuation() {
        let cell = Cell::continuation(Rgba::BLACK);
        assert!(cell.is_continuation());
        assert_eq!(cell.display_width(), 0);
    }

    #[test]
    fn test_wide_char() {
        let cell = Cell::new('漢', Style::NONE);
        assert_eq!(cell.display_width(), 2);
    }
}
