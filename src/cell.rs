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

/// Encoded grapheme reference with cached display width.
///
/// Graphemes (multi-codepoint characters like emoji and ZWJ sequences) are stored
/// in a pool and referenced by ID. The ID encodes both the pool slot and the
/// display width to avoid lookups on the hot path.
///
/// # Encoding (per Zig spec)
///
/// ```text
/// [31: reserved][30-24: width (7 bits)][23-0: pool ID (24 bits)]
/// ```
///
/// - **Bits 0-23**: Pool slot ID (~16M possible slots)
/// - **Bits 24-30**: Cached display width (0-127, typically 1-2)
/// - **Bit 31**: Reserved (always 0)
///
/// # Performance
///
/// `GraphemeId` is `Copy`, enabling zero-allocation cell operations.
/// Display width is cached in the ID, avoiding pool lookups during rendering.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct GraphemeId(u32);

impl GraphemeId {
    const WIDTH_SHIFT: u32 = 24;
    const WIDTH_MASK: u32 = 0x7F << Self::WIDTH_SHIFT;
    const ID_MASK: u32 = 0x00FF_FFFF;

    /// Create a new grapheme ID with cached width.
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The pool slot index (must be <= 0x00FF_FFFF)
    /// * `width` - Display width to cache (must be <= 127)
    #[must_use]
    pub const fn new(pool_id: u32, width: u8) -> Self {
        // Note: debug_assert! not available in const fn, validation happens at pool level
        Self((pool_id & Self::ID_MASK) | ((width as u32) << Self::WIDTH_SHIFT))
    }

    /// Create an invalid/placeholder grapheme ID.
    ///
    /// Used for testing or when the pool is not yet available.
    #[must_use]
    pub const fn placeholder(width: u8) -> Self {
        Self::new(0, width)
    }

    /// Get the pool slot ID.
    #[must_use]
    pub const fn pool_id(self) -> u32 {
        self.0 & Self::ID_MASK
    }

    /// Get the cached display width.
    #[must_use]
    pub const fn width(self) -> usize {
        ((self.0 & Self::WIDTH_MASK) >> Self::WIDTH_SHIFT) as usize
    }

    /// Get the raw encoded value.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Create from raw encoded value.
    #[must_use]
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }
}

/// Content of a terminal cell.
///
/// Represents what is displayed in a single cell position. Most cells contain
/// either a simple character or are empty. Wide characters and emoji use
/// grapheme clusters and leave continuation markers in following cells.
///
/// # Grapheme Pool Integration
///
/// Multi-codepoint graphemes (emoji, ZWJ sequences) are stored in a [`GraphemePool`]
/// and referenced by [`GraphemeId`]. The actual string data is resolved via the pool
/// during rendering. This enables `Copy` semantics for cells.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CellContent {
    /// Simple ASCII or single-codepoint character (display width 1-2).
    Char(char),
    /// Reference to a grapheme cluster in the pool.
    ///
    /// The `GraphemeId` contains both the pool slot ID and cached display width.
    /// To get the actual string, resolve via `GraphemePool::get(id)`.
    Grapheme(GraphemeId),
    /// Empty/cleared cell.
    #[default]
    Empty,
    /// Continuation of a wide character from the previous cell.
    Continuation,
}

impl CellContent {
    /// Get the display width of this content.
    ///
    /// For graphemes, returns the cached width from the [`GraphemeId`].
    /// This avoids pool lookups on the hot rendering path.
    #[must_use]
    pub fn display_width(&self) -> usize {
        match self {
            Self::Char(c) => crate::unicode::display_width_char(*c),
            Self::Grapheme(id) => id.width(),
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

    /// Check if this is a grapheme reference.
    #[must_use]
    pub fn is_grapheme(&self) -> bool {
        matches!(self, Self::Grapheme(_))
    }

    /// Get the grapheme ID if this is a grapheme reference.
    #[must_use]
    pub fn grapheme_id(&self) -> Option<GraphemeId> {
        match self {
            Self::Grapheme(id) => Some(*id),
            _ => None,
        }
    }

    /// Get the character if this is a single char.
    #[must_use]
    pub fn as_char(&self) -> Option<char> {
        match self {
            Self::Char(c) => Some(*c),
            _ => None,
        }
    }

    /// Get the string representation for non-grapheme content.
    ///
    /// Returns `None` for [`CellContent::Grapheme`] - use the pool to resolve
    /// grapheme strings via [`GraphemeId::pool_id`].
    ///
    /// # Returns
    ///
    /// - `Char`: The character as a string
    /// - `Empty`: A space character
    /// - `Continuation`: Empty string
    /// - `Grapheme`: `None` (requires pool lookup)
    #[must_use]
    pub fn as_str_without_pool(&self) -> Option<Cow<'static, str>> {
        match self {
            Self::Char(c) => {
                let mut buf = [0u8; 4];
                Some(Cow::Owned(c.encode_utf8(&mut buf).to_owned()))
            }
            Self::Grapheme(_) => None, // Requires pool lookup
            Self::Empty => Some(Cow::Borrowed(" ")),
            Self::Continuation => Some(Cow::Borrowed("")),
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
/// - Hyperlink ID packed into attributes for OSC 8 links
///
/// # Alpha Blending
///
/// Cells support alpha blending via [`Cell::blend_over`], which composites
/// one cell on top of another using Porter-Duff "over" compositing. This
/// enables transparent overlays and layered UI elements.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Cell {
    /// The character or grapheme content.
    pub content: CellContent,
    /// Foreground color.
    pub fg: Rgba,
    /// Background color.
    pub bg: Rgba,
    /// Text rendering attributes (includes packed link ID).
    pub attributes: TextAttributes,
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
        }
    }

    /// Create a cell from a grapheme cluster string.
    ///
    /// For single-codepoint strings, creates a `Char` cell directly.
    /// For multi-codepoint graphemes, creates a placeholder `Grapheme` cell.
    ///
    /// **Note:** This creates a placeholder `GraphemeId` with the correct display
    /// width but pool_id 0. Use [`GraphemePool::intern`] to get a real ID that
    /// can be resolved back to the string during rendering.
    #[must_use]
    pub fn from_grapheme(s: &str, style: Style) -> Self {
        let content = if s.chars().count() == 1 {
            CellContent::Char(s.chars().next().unwrap())
        } else {
            // Compute display width for the grapheme cluster
            let width = crate::unicode::display_width(s);
            // Create placeholder ID with correct width (pool integration in bd-2qg.4.3)
            CellContent::Grapheme(GraphemeId::placeholder(width as u8))
        };

        Self {
            content,
            fg: style.fg.unwrap_or(Rgba::WHITE),
            bg: style.bg.unwrap_or(Rgba::TRANSPARENT),
            attributes: style.attributes,
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

    /// Write the cell content to a writer (without pool lookup).
    ///
    /// **Note:** For [`CellContent::Grapheme`], this writes a placeholder character
    /// since the actual string requires a [`GraphemePool`] lookup. Use
    /// [`Cell::write_content_with_pool`] or the ANSI writer for proper grapheme rendering.
    pub fn write_content<W: std::io::Write>(&self, w: &mut W) -> std::io::Result<()> {
        match &self.content {
            CellContent::Char(c) => write!(w, "{c}"),
            CellContent::Grapheme(id) => {
                // Write placeholder spaces matching the display width
                // Proper rendering requires pool lookup (see write_content_with_pool)
                for _ in 0..id.width() {
                    write!(w, " ")?;
                }
                Ok(())
            }
            CellContent::Empty => write!(w, " "),
            CellContent::Continuation => Ok(()),
        }
    }

    /// Write the cell content to a writer with grapheme pool lookup.
    ///
    /// The `pool_lookup` function resolves a [`GraphemeId`] to its string representation.
    pub fn write_content_with_pool<W, F>(&self, w: &mut W, pool_lookup: F) -> std::io::Result<()>
    where
        W: std::io::Write,
        F: Fn(GraphemeId) -> Option<String>,
    {
        match &self.content {
            CellContent::Char(c) => write!(w, "{c}"),
            CellContent::Grapheme(id) => {
                if let Some(s) = pool_lookup(*id) {
                    write!(w, "{s}")
                } else {
                    // Fallback: write spaces matching display width
                    for _ in 0..id.width() {
                        write!(w, " ")?;
                    }
                    Ok(())
                }
            }
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
        self.attributes = self.attributes.merge(style.attributes);
    }

    /// Blend this cell's colors with a global opacity factor.
    pub fn blend_with_opacity(&mut self, opacity: f32) {
        self.fg = self.fg.multiply_alpha(opacity);
        self.bg = self.bg.multiply_alpha(opacity);
    }

    /// Blend this cell over a background cell using alpha compositing.
    #[must_use]
    pub fn blend_over(self, background: &Cell) -> Cell {
        let (content, attributes) = if self.content.is_empty() {
            (background.content, background.attributes)
        } else {
            (self.content, self.attributes)
        };

        Cell {
            content,
            fg: self.fg.blend_over(background.fg),
            bg: self.bg.blend_over(background.bg),
            attributes,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // GraphemeId tests
    #[test]
    fn test_grapheme_id_encoding() {
        let id = GraphemeId::new(0x0012_3456, 2);
        assert_eq!(id.pool_id(), 0x0012_3456);
        assert_eq!(id.width(), 2);
    }

    #[test]
    fn test_grapheme_id_max_values() {
        // Max pool ID is 24 bits
        let id = GraphemeId::new(0x00FF_FFFF, 127);
        assert_eq!(id.pool_id(), 0x00FF_FFFF);
        assert_eq!(id.width(), 127);
    }

    #[test]
    fn test_grapheme_id_overflow_masked() {
        // Values beyond 24 bits should be masked
        let id = GraphemeId::new(0x01FF_FFFF, 2);
        assert_eq!(id.pool_id(), 0x00FF_FFFF); // Upper bits masked
    }

    #[test]
    fn test_grapheme_id_placeholder() {
        let id = GraphemeId::placeholder(2);
        assert_eq!(id.pool_id(), 0);
        assert_eq!(id.width(), 2);
    }

    #[test]
    fn test_grapheme_id_roundtrip() {
        let id = GraphemeId::new(12345, 2);
        let raw = id.raw();
        let restored = GraphemeId::from_raw(raw);
        assert_eq!(id, restored);
    }

    #[test]
    fn test_grapheme_id_is_copy() {
        let id = GraphemeId::new(1, 2);
        let id2 = id; // Copy
        assert_eq!(id, id2);
    }

    // CellContent tests
    #[test]
    fn test_cell_content_is_copy() {
        let content = CellContent::Char('A');
        let content2 = content; // Copy
        assert_eq!(content, content2);
    }

    #[test]
    fn test_cell_content_grapheme_width() {
        let id = GraphemeId::new(42, 2);
        let content = CellContent::Grapheme(id);
        assert_eq!(content.display_width(), 2);
        assert!(content.is_grapheme());
        assert_eq!(content.grapheme_id(), Some(id));
    }

    #[test]
    fn test_cell_content_as_str_without_pool() {
        assert_eq!(
            CellContent::Char('A').as_str_without_pool(),
            Some(std::borrow::Cow::Owned("A".to_string()))
        );
        assert_eq!(
            CellContent::Empty.as_str_without_pool(),
            Some(std::borrow::Cow::Borrowed(" "))
        );
        assert_eq!(
            CellContent::Continuation.as_str_without_pool(),
            Some(std::borrow::Cow::Borrowed(""))
        );
        // Grapheme requires pool lookup
        assert!(
            CellContent::Grapheme(GraphemeId::placeholder(2))
                .as_str_without_pool()
                .is_none()
        );
    }

    // Cell tests
    #[test]
    fn test_cell_new() {
        let cell = Cell::new('A', Style::fg(Rgba::RED));
        assert!(matches!(cell.content, CellContent::Char('A')));
        assert_eq!(cell.fg, Rgba::RED);
        assert_eq!(cell.display_width(), 1);
    }

    #[test]
    fn test_cell_is_copy() {
        let cell = Cell::new('A', Style::NONE);
        let cell2 = cell; // Copy
        assert_eq!(cell, cell2);
    }

    #[test]
    fn test_cell_grapheme() {
        let cell = Cell::from_grapheme("👨‍👩‍👧", Style::NONE);
        assert!(matches!(cell.content, CellContent::Grapheme(_)));
        // ZWJ family emoji has width 2
        assert_eq!(cell.display_width(), 2);
    }

    #[test]
    fn test_cell_grapheme_single_char_optimization() {
        // Single char graphemes should use Char variant
        let cell = Cell::from_grapheme("A", Style::NONE);
        assert!(matches!(cell.content, CellContent::Char('A')));
    }

    #[test]
    fn test_blend_over_attributes_override_for_content() {
        let bg = Cell::new('A', Style::bold());
        let fg = Cell::new('B', Style::NONE);
        let fg_attrs = fg.attributes;
        let blended = fg.blend_over(&bg);

        assert_eq!(blended.content, CellContent::Char('B'));
        assert_eq!(blended.attributes, fg_attrs);
    }

    #[test]
    fn test_blend_over_empty_preserves_background_attrs_and_link() {
        let bg = Cell::new('A', Style::bold().with_link(7));
        let fg = Cell::clear(Rgba::TRANSPARENT);
        let blended = fg.blend_over(&bg);

        assert_eq!(blended.content, CellContent::Char('A'));
        assert_eq!(blended.attributes, bg.attributes);
        assert_eq!(blended.attributes.link_id(), Some(7));
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

    #[test]
    fn test_write_content_with_pool() {
        let cell = Cell::new('A', Style::NONE);
        let mut buf = Vec::new();
        cell.write_content_with_pool(&mut buf, |_| None).unwrap();
        assert_eq!(&buf, b"A");

        // Test grapheme with pool lookup
        let id = GraphemeId::new(42, 2);
        let grapheme_cell = Cell {
            content: CellContent::Grapheme(id),
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            attributes: TextAttributes::empty(),
        };
        buf.clear();
        grapheme_cell
            .write_content_with_pool(&mut buf, |gid| {
                if gid.pool_id() == 42 {
                    Some("👍".to_string())
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&buf), "👍");
    }
}
