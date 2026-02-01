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

    /// Maximum width value that can be stored (7 bits = 127).
    pub const MAX_WIDTH: u8 = 127;

    /// Create a new grapheme ID with cached width.
    ///
    /// # Arguments
    ///
    /// * `pool_id` - The pool slot index (must be <= 0x00FF_FFFF)
    /// * `width` - Display width to cache (saturates to 127 if larger)
    #[must_use]
    pub const fn new(pool_id: u32, width: u8) -> Self {
        // Saturate width to 127 to prevent silent truncation
        // (only 7 bits available in the encoding)
        let safe_width = if width > Self::MAX_WIDTH {
            Self::MAX_WIDTH
        } else {
            width
        };
        Self((pool_id & Self::ID_MASK) | ((safe_width as u32) << Self::WIDTH_SHIFT))
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
/// Multi-codepoint graphemes (emoji, ZWJ sequences) are stored in a [`crate::GraphemePool`]
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
    /// width but pool_id 0. Use [`crate::GraphemePool::intern`] to get a real ID that
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

    /// Create a fully transparent cell.
    ///
    /// This is a true no-op for compositing: blending this cell over another cell
    /// leaves the background cell unchanged.
    ///
    /// Prefer this over `Cell::clear(Rgba::TRANSPARENT)` when you need a layer or
    /// buffer to start out fully transparent without unintentionally tinting the
    /// underlying foreground.
    #[must_use]
    pub fn transparent() -> Self {
        Self {
            content: CellContent::Empty,
            fg: Rgba::TRANSPARENT,
            bg: Rgba::TRANSPARENT,
            attributes: TextAttributes::empty(),
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
    /// since the actual string requires a [`crate::GraphemePool`] lookup. Use
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

    /// Fast bitwise equality check for cell diffing.
    ///
    /// This is optimized for the diff detection hot path, using integer
    /// comparison for colors instead of floating-point. This is typically
    /// faster than derived `PartialEq` because:
    /// - Integer comparison is simpler than float comparison
    /// - No special handling for NaN needed in this context
    /// - Better branch prediction on simple integer ops
    #[inline]
    #[must_use]
    pub fn bits_eq(&self, other: &Self) -> bool {
        self.content == other.content
            && self.fg.bits_eq(other.fg)
            && self.bg.bits_eq(other.bg)
            && self.attributes == other.attributes
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
    fn test_grapheme_id_width_saturation() {
        // Width > 127 should saturate to 127 (only 7 bits available)
        let id128 = GraphemeId::new(1, 128);
        assert_eq!(id128.width(), 127, "width 128 should saturate to 127");

        let id255 = GraphemeId::new(1, 255);
        assert_eq!(id255.width(), 127, "width 255 should saturate to 127");

        // Width <= 127 should be preserved
        let id127 = GraphemeId::new(1, 127);
        assert_eq!(id127.width(), 127);

        let id0 = GraphemeId::new(1, 0);
        assert_eq!(id0.width(), 0);
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
        let bg = Cell::new(
            'A',
            Style::builder()
                .fg(Rgba::RED)
                .bg(Rgba::BLACK)
                .bold()
                .link(7)
                .build(),
        );
        let fg = Cell::transparent();
        let blended = fg.blend_over(&bg);

        assert_eq!(blended, bg);
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

    // =========================================================================
    // Additional tests per bd-2ei4 spec
    // =========================================================================

    // Cell Creation Tests
    #[test]
    fn test_cell_default() {
        let cell = Cell::default();
        assert!(cell.content.is_empty());
        assert_eq!(cell.fg, Rgba::default());
        assert_eq!(cell.bg, Rgba::default());
        assert_eq!(cell.attributes, TextAttributes::empty());
    }

    #[test]
    fn test_cell_with_style() {
        let style = Style::fg(Rgba::RED)
            .with_bg(Rgba::BLUE)
            .with_bold()
            .with_italic();
        let cell = Cell::new('X', style);
        assert_eq!(cell.fg, Rgba::RED);
        assert_eq!(cell.bg, Rgba::BLUE);
        assert!(cell.attributes.contains(TextAttributes::BOLD));
        assert!(cell.attributes.contains(TextAttributes::ITALIC));
    }

    #[test]
    fn test_cell_with_fg_bg() {
        let cell = Cell::new('A', Style::fg(Rgba::GREEN).with_bg(Rgba::BLACK));
        assert_eq!(cell.fg, Rgba::GREEN);
        assert_eq!(cell.bg, Rgba::BLACK);
    }

    // Cell Comparison Tests
    #[test]
    fn test_cell_eq_same() {
        let cell1 = Cell::new('A', Style::fg(Rgba::RED));
        let cell2 = Cell::new('A', Style::fg(Rgba::RED));
        assert_eq!(cell1, cell2);
        assert!(cell1.bits_eq(&cell2));
    }

    #[test]
    fn test_cell_eq_different_char() {
        let cell1 = Cell::new('A', Style::fg(Rgba::RED));
        let cell2 = Cell::new('B', Style::fg(Rgba::RED));
        assert_ne!(cell1, cell2);
        assert!(!cell1.bits_eq(&cell2));
    }

    #[test]
    fn test_cell_eq_different_style() {
        let cell1 = Cell::new('A', Style::fg(Rgba::RED));
        let cell2 = Cell::new('A', Style::fg(Rgba::BLUE));
        assert_ne!(cell1, cell2);
        assert!(!cell1.bits_eq(&cell2));
    }

    #[test]
    fn test_cell_eq_different_attributes() {
        let cell1 = Cell::new('A', Style::bold());
        let cell2 = Cell::new('A', Style::italic());
        assert_ne!(cell1, cell2);
        assert!(!cell1.bits_eq(&cell2));
    }

    // Wide Character Tests
    #[test]
    fn test_cell_cjk_characters() {
        // Chinese
        assert_eq!(Cell::new('中', Style::NONE).display_width(), 2);
        // Japanese
        assert_eq!(Cell::new('日', Style::NONE).display_width(), 2);
        // Korean
        assert_eq!(Cell::new('한', Style::NONE).display_width(), 2);
    }

    #[test]
    fn test_cell_emoji_handling() {
        // Simple emoji
        let cell = Cell::from_grapheme("😀", Style::NONE);
        assert_eq!(cell.display_width(), 2);

        // Thumbs up
        let cell = Cell::from_grapheme("👍", Style::NONE);
        assert_eq!(cell.display_width(), 2);
    }

    #[test]
    fn test_cell_combining_chars() {
        // e with combining acute accent: e + ́ = é
        let cell = Cell::from_grapheme("é", Style::NONE); // precomposed
        assert_eq!(cell.display_width(), 1);

        // "e\u{0301}" is 'e' followed by combining acute accent
        // This should be a grapheme cluster
        let combined = Cell::from_grapheme("e\u{0301}", Style::NONE);
        // The display width should be 1 (one visual character)
        assert_eq!(combined.display_width(), 1);
    }

    // CellContent Tests
    #[test]
    fn test_cell_content_display_width_all_variants() {
        // Single-width char
        assert_eq!(CellContent::Char('a').display_width(), 1);
        // Wide char
        assert_eq!(CellContent::Char('中').display_width(), 2);
        // Empty
        assert_eq!(CellContent::Empty.display_width(), 1);
        // Continuation
        assert_eq!(CellContent::Continuation.display_width(), 0);
        // Grapheme (width from ID)
        let id = GraphemeId::new(1, 3);
        assert_eq!(CellContent::Grapheme(id).display_width(), 3);
    }

    #[test]
    fn test_cell_content_is_empty() {
        assert!(CellContent::Empty.is_empty());
        assert!(!CellContent::Char('A').is_empty());
        assert!(!CellContent::Continuation.is_empty());
        assert!(!CellContent::Grapheme(GraphemeId::placeholder(2)).is_empty());
    }

    #[test]
    fn test_cell_content_is_continuation() {
        assert!(CellContent::Continuation.is_continuation());
        assert!(!CellContent::Empty.is_continuation());
        assert!(!CellContent::Char('A').is_continuation());
        assert!(!CellContent::Grapheme(GraphemeId::placeholder(2)).is_continuation());
    }

    #[test]
    fn test_cell_content_as_char() {
        assert_eq!(CellContent::Char('A').as_char(), Some('A'));
        assert_eq!(CellContent::Char('中').as_char(), Some('中'));
        assert_eq!(CellContent::Empty.as_char(), None);
        assert_eq!(CellContent::Continuation.as_char(), None);
        assert_eq!(
            CellContent::Grapheme(GraphemeId::placeholder(2)).as_char(),
            None
        );
    }

    // Cell Method Tests
    #[test]
    fn test_cell_apply_style() {
        let mut cell = Cell::new('A', Style::NONE);
        assert_eq!(cell.fg, Rgba::WHITE);

        cell.apply_style(Style::fg(Rgba::RED).with_bold());
        assert_eq!(cell.fg, Rgba::RED);
        assert!(cell.attributes.contains(TextAttributes::BOLD));
    }

    #[test]
    fn test_cell_apply_style_partial() {
        // apply_style should only change set fields
        let mut cell = Cell::new('A', Style::fg(Rgba::RED).with_bg(Rgba::BLUE));
        cell.apply_style(Style::fg(Rgba::GREEN)); // Only set fg
        assert_eq!(cell.fg, Rgba::GREEN);
        assert_eq!(cell.bg, Rgba::BLUE); // bg unchanged
    }

    #[test]
    fn test_cell_blend_with_opacity() {
        let mut cell = Cell::new('A', Style::fg(Rgba::WHITE).with_bg(Rgba::BLACK));
        cell.blend_with_opacity(0.5);
        assert!(cell.fg.a < 1.0);
        assert!(cell.bg.a < 1.0);
    }

    #[test]
    fn test_cell_bits_eq_vs_eq() {
        // bits_eq should behave same as PartialEq for normal cases
        let cell1 = Cell::new('A', Style::fg(Rgba::RED));
        let cell2 = Cell::new('A', Style::fg(Rgba::RED));
        assert_eq!(cell1 == cell2, cell1.bits_eq(&cell2));

        let cell3 = Cell::new('B', Style::fg(Rgba::RED));
        assert_eq!(cell1 == cell3, cell1.bits_eq(&cell3));
    }

    #[test]
    fn test_cell_write_content_empty() {
        let cell = Cell::clear(Rgba::BLACK);
        let mut buf = Vec::new();
        cell.write_content(&mut buf).unwrap();
        assert_eq!(&buf, b" ");
    }

    #[test]
    fn test_cell_write_content_continuation() {
        let cell = Cell::continuation(Rgba::BLACK);
        let mut buf = Vec::new();
        cell.write_content(&mut buf).unwrap();
        assert!(buf.is_empty()); // Continuation writes nothing
    }

    #[test]
    fn test_cell_write_content_grapheme_placeholder() {
        // Without pool, grapheme writes spaces matching width
        let id = GraphemeId::new(42, 2);
        let cell = Cell {
            content: CellContent::Grapheme(id),
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            attributes: TextAttributes::empty(),
        };
        let mut buf = Vec::new();
        cell.write_content(&mut buf).unwrap();
        assert_eq!(&buf, b"  "); // Two spaces for width 2
    }

    // Grapheme Storage Tests
    #[test]
    fn test_grapheme_cluster_storage() {
        // Family emoji (multi-codepoint ZWJ sequence)
        let cell = Cell::from_grapheme("👨‍👩‍👧‍👦", Style::NONE);
        assert!(matches!(cell.content, CellContent::Grapheme(_)));
        assert_eq!(cell.display_width(), 2);

        // Flag emoji (regional indicator sequence)
        let flag = Cell::from_grapheme("🇺🇸", Style::NONE);
        assert!(matches!(flag.content, CellContent::Grapheme(_)));
        assert_eq!(flag.display_width(), 2);
    }

    #[test]
    fn test_grapheme_width_calculation() {
        // Verify width is correctly cached in GraphemeId
        let id = GraphemeId::new(100, 4);
        assert_eq!(id.width(), 4);

        let content = CellContent::Grapheme(id);
        assert_eq!(content.display_width(), 4);

        // Cell should use content's display_width
        let cell = Cell {
            content,
            fg: Rgba::WHITE,
            bg: Rgba::BLACK,
            attributes: TextAttributes::empty(),
        };
        assert_eq!(cell.display_width(), 4);
    }

    #[test]
    fn test_grapheme_id_default() {
        let id = GraphemeId::default();
        assert_eq!(id.pool_id(), 0);
        assert_eq!(id.width(), 0);
    }

    // Edge Cases
    #[test]
    fn test_cell_zero_width_chars() {
        // Zero-width joiner and other invisible characters
        let cell = Cell::new('\u{200B}', Style::NONE); // Zero-width space
        assert_eq!(cell.display_width(), 0);
    }

    #[test]
    fn test_cell_blend_over_transparent() {
        let bg = Cell::new('A', Style::bg(Rgba::RED));
        // Note: blend_over preserves background content only for CellContent::Empty,
        // not for space characters. Use Cell::transparent() to get Empty content.
        let fg = Cell::transparent();
        let blended = fg.blend_over(&bg);
        // Foreground is Empty so should preserve background content
        assert_eq!(blended.content, CellContent::Char('A'));
        assert_eq!(blended.fg, bg.fg);
        assert_eq!(blended.bg, bg.bg);
    }
}
