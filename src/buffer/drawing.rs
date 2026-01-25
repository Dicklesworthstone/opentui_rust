//! Text and box drawing operations.

use crate::buffer::OptimizedBuffer;
use crate::cell::{Cell, CellContent};
use crate::color::Rgba;
use crate::grapheme_pool::GraphemePool;
use crate::style::Style;
use unicode_segmentation::UnicodeSegmentation;

/// Box drawing style with corner and edge characters.
#[derive(Clone, Debug)]
pub struct BoxStyle {
    pub top_left: char,
    pub top_right: char,
    pub bottom_left: char,
    pub bottom_right: char,
    pub horizontal: char,
    pub vertical: char,
    pub style: Style,
}

/// Box side visibility.
#[derive(Clone, Copy, Debug)]
pub struct BoxSides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Default for BoxSides {
    fn default() -> Self {
        Self {
            top: true,
            right: true,
            bottom: true,
            left: true,
        }
    }
}

/// Title alignment for boxed titles.
#[derive(Clone, Copy, Debug, Default)]
pub enum TitleAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Extended box drawing options.
#[derive(Clone, Debug)]
pub struct BoxOptions {
    pub style: BoxStyle,
    pub sides: BoxSides,
    pub fill: Option<Rgba>,
    pub title: Option<String>,
    pub title_align: TitleAlign,
}

impl BoxOptions {
    #[must_use]
    pub fn new(style: BoxStyle) -> Self {
        Self {
            style,
            sides: BoxSides::default(),
            fill: None,
            title: None,
            title_align: TitleAlign::Left,
        }
    }
}

impl BoxStyle {
    /// Single-line box drawing characters.
    #[must_use]
    pub fn single(style: Style) -> Self {
        Self {
            top_left: '┌',
            top_right: '┐',
            bottom_left: '└',
            bottom_right: '┘',
            horizontal: '─',
            vertical: '│',
            style,
        }
    }

    /// Double-line box drawing characters.
    #[must_use]
    pub fn double(style: Style) -> Self {
        Self {
            top_left: '╔',
            top_right: '╗',
            bottom_left: '╚',
            bottom_right: '╝',
            horizontal: '═',
            vertical: '║',
            style,
        }
    }

    /// Rounded corner box drawing characters.
    #[must_use]
    pub fn rounded(style: Style) -> Self {
        Self {
            top_left: '╭',
            top_right: '╮',
            bottom_left: '╰',
            bottom_right: '╯',
            horizontal: '─',
            vertical: '│',
            style,
        }
    }

    /// Heavy (bold) box drawing characters.
    #[must_use]
    pub fn heavy(style: Style) -> Self {
        Self {
            top_left: '┏',
            top_right: '┓',
            bottom_left: '┗',
            bottom_right: '┛',
            horizontal: '━',
            vertical: '┃',
            style,
        }
    }

    /// ASCII box drawing characters (works in all terminals).
    #[must_use]
    pub fn ascii(style: Style) -> Self {
        Self {
            top_left: '+',
            top_right: '+',
            bottom_left: '+',
            bottom_right: '+',
            horizontal: '-',
            vertical: '|',
            style,
        }
    }
}

impl Default for BoxStyle {
    fn default() -> Self {
        Self::single(Style::NONE)
    }
}

/// Draw text at position, handling grapheme clusters and wide characters.
///
/// **Note:** Multi-codepoint graphemes are stored with placeholder IDs.
/// For proper grapheme pool integration, use [`draw_text_with_pool`].
pub fn draw_text(buffer: &mut OptimizedBuffer, x: u32, y: u32, text: &str, style: Style) {
    let mut col = x;

    for grapheme in text.graphemes(true) {
        if grapheme == "\n" || grapheme == "\r" {
            continue;
        }

        let cell = Cell::from_grapheme(grapheme, style);
        let width = cell.display_width();

        buffer.set_blended(col, y, cell);

        // Add continuation cells for wide characters
        for i in 1..width {
            buffer.set_blended(
                col + i as u32,
                y,
                Cell::continuation(style.bg.unwrap_or(Rgba::TRANSPARENT)),
            );
        }

        col += width as u32;
    }
}

/// Draw text at position, allocating grapheme IDs from the pool.
///
/// This version properly allocates multi-codepoint graphemes (emoji, ZWJ sequences)
/// in the pool, allowing them to be resolved during rendering.
///
/// # Arguments
///
/// * `buffer` - The buffer to draw to
/// * `pool` - The grapheme pool for allocating multi-codepoint graphemes
/// * `x` - Starting X position
/// * `y` - Y position
/// * `text` - The text to draw
/// * `style` - Style to apply to the text
pub fn draw_text_with_pool(
    buffer: &mut OptimizedBuffer,
    pool: &mut GraphemePool,
    x: u32,
    y: u32,
    text: &str,
    style: Style,
) {
    let mut col = x;
    let fg = style.fg.unwrap_or(Rgba::WHITE);
    let bg = style.bg.unwrap_or(Rgba::TRANSPARENT);
    let attrs = style.attributes;

    for grapheme in text.graphemes(true) {
        if grapheme == "\n" || grapheme == "\r" {
            continue;
        }

        // Determine cell content and width
        let (content, width) = if grapheme.chars().count() == 1 {
            // Single codepoint - store directly as Char
            let ch = grapheme.chars().next().unwrap();
            let w = crate::unicode::display_width_char(ch);
            (CellContent::Char(ch), w)
        } else {
            // Multi-codepoint grapheme - allocate from pool
            let id = pool.intern(grapheme);
            (CellContent::Grapheme(id), id.width())
        };

        let cell = Cell {
            content,
            fg,
            bg,
            attributes: attrs,
        };

        buffer.set_blended(col, y, cell);

        // Add continuation cells for wide characters
        for i in 1..width {
            buffer.set_blended(col + i as u32, y, Cell::continuation(bg));
        }

        col += width as u32;
    }
}

/// Draw a single character at position, allocating from pool if needed.
///
/// For single codepoints, stores directly. For multi-codepoint graphemes,
/// allocates from the pool.
pub fn draw_char_with_pool(
    buffer: &mut OptimizedBuffer,
    pool: &mut GraphemePool,
    x: u32,
    y: u32,
    grapheme: &str,
    style: Style,
) {
    let fg = style.fg.unwrap_or(Rgba::WHITE);
    let bg = style.bg.unwrap_or(Rgba::TRANSPARENT);
    let attrs = style.attributes;

    let (content, width) = if grapheme.chars().count() == 1 {
        let ch = grapheme.chars().next().unwrap();
        let w = crate::unicode::display_width_char(ch);
        (CellContent::Char(ch), w)
    } else {
        let id = pool.intern(grapheme);
        (CellContent::Grapheme(id), id.width())
    };

    let cell = Cell {
        content,
        fg,
        bg,
        attributes: attrs,
    };

    buffer.set_blended(x, y, cell);

    // Add continuation cells for wide characters
    for i in 1..width {
        buffer.set_blended(x + i as u32, y, Cell::continuation(bg));
    }
}

/// Draw a box border.
pub fn draw_box(buffer: &mut OptimizedBuffer, x: u32, y: u32, w: u32, h: u32, box_style: BoxStyle) {
    if w < 2 || h < 2 {
        return;
    }

    let style = box_style.style;

    // Corners
    buffer.set_blended(x, y, Cell::new(box_style.top_left, style));
    buffer.set_blended(x + w - 1, y, Cell::new(box_style.top_right, style));
    buffer.set_blended(x, y + h - 1, Cell::new(box_style.bottom_left, style));
    buffer.set_blended(
        x + w - 1,
        y + h - 1,
        Cell::new(box_style.bottom_right, style),
    );

    // Horizontal edges
    for col in (x + 1)..(x + w - 1) {
        buffer.set_blended(col, y, Cell::new(box_style.horizontal, style));
        buffer.set_blended(col, y + h - 1, Cell::new(box_style.horizontal, style));
    }

    // Vertical edges
    for row in (y + 1)..(y + h - 1) {
        buffer.set_blended(x, row, Cell::new(box_style.vertical, style));
        buffer.set_blended(x + w - 1, row, Cell::new(box_style.vertical, style));
    }
}

/// Draw a box border with extended options.
pub fn draw_box_with_options(
    buffer: &mut OptimizedBuffer,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    options: BoxOptions,
) {
    if w < 2 || h < 2 {
        return;
    }

    let style = options.style.style;

    // Optional fill
    if let Some(bg) = options.fill {
        if w > 2 && h > 2 {
            buffer.fill_rect(x + 1, y + 1, w - 2, h - 2, bg);
        }
    }

    // Corners
    if options.sides.top && options.sides.left {
        buffer.set_blended(x, y, Cell::new(options.style.top_left, style));
    }
    if options.sides.top && options.sides.right {
        buffer.set_blended(x + w - 1, y, Cell::new(options.style.top_right, style));
    }
    if options.sides.bottom && options.sides.left {
        buffer.set_blended(x, y + h - 1, Cell::new(options.style.bottom_left, style));
    }
    if options.sides.bottom && options.sides.right {
        buffer.set_blended(
            x + w - 1,
            y + h - 1,
            Cell::new(options.style.bottom_right, style),
        );
    }

    // Horizontal edges
    if options.sides.top {
        for col in (x + 1)..(x + w - 1) {
            buffer.set_blended(col, y, Cell::new(options.style.horizontal, style));
        }
    }
    if options.sides.bottom {
        for col in (x + 1)..(x + w - 1) {
            buffer.set_blended(col, y + h - 1, Cell::new(options.style.horizontal, style));
        }
    }

    // Vertical edges
    if options.sides.left {
        for row in (y + 1)..(y + h - 1) {
            buffer.set_blended(x, row, Cell::new(options.style.vertical, style));
        }
    }
    if options.sides.right {
        for row in (y + 1)..(y + h - 1) {
            buffer.set_blended(x + w - 1, row, Cell::new(options.style.vertical, style));
        }
    }

    // Title
    if let Some(title) = options.title {
        if options.sides.top && w > 2 {
            let available = (w - 2) as usize;
            let title_text = if title.len() > available {
                title.chars().take(available).collect::<String>()
            } else {
                title
            };
            let start_offset = match options.title_align {
                TitleAlign::Left => 0,
                TitleAlign::Center => (available.saturating_sub(title_text.len())) / 2,
                TitleAlign::Right => available.saturating_sub(title_text.len()),
            };
            let title_x = x + 1 + start_offset as u32;
            buffer.draw_text(title_x, y, &title_text, style);
        }
    }
}

/// Draw a horizontal line.
pub fn draw_hline(buffer: &mut OptimizedBuffer, x: u32, y: u32, len: u32, ch: char, style: Style) {
    for col in x..x.saturating_add(len) {
        buffer.set_blended(col, y, Cell::new(ch, style));
    }
}

/// Draw a vertical line.
pub fn draw_vline(buffer: &mut OptimizedBuffer, x: u32, y: u32, len: u32, ch: char, style: Style) {
    for row in y..y.saturating_add(len) {
        buffer.set_blended(x, row, Cell::new(ch, style));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_text() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        draw_text(&mut buffer, 0, 0, "Hello", Style::fg(Rgba::RED));

        assert_eq!(
            buffer.get(0, 0).unwrap().content,
            crate::cell::CellContent::Char('H')
        );
        assert_eq!(
            buffer.get(4, 0).unwrap().content,
            crate::cell::CellContent::Char('o')
        );
    }

    #[test]
    fn test_draw_wide_char() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        draw_text(&mut buffer, 0, 0, "漢字", Style::NONE);

        // First character at 0, continuation at 1
        // Second character at 2, continuation at 3
        assert!(!buffer.get(0, 0).unwrap().is_continuation());
        assert!(buffer.get(1, 0).unwrap().is_continuation());
        assert!(!buffer.get(2, 0).unwrap().is_continuation());
        assert!(buffer.get(3, 0).unwrap().is_continuation());
    }

    #[test]
    fn test_draw_box() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        draw_box(&mut buffer, 0, 0, 10, 5, BoxStyle::single(Style::NONE));

        // Check corners
        assert_eq!(
            buffer.get(0, 0).unwrap().content,
            crate::cell::CellContent::Char('┌')
        );
        assert_eq!(
            buffer.get(9, 0).unwrap().content,
            crate::cell::CellContent::Char('┐')
        );
        assert_eq!(
            buffer.get(0, 4).unwrap().content,
            crate::cell::CellContent::Char('└')
        );
        assert_eq!(
            buffer.get(9, 4).unwrap().content,
            crate::cell::CellContent::Char('┘')
        );
    }

    #[test]
    fn test_draw_box_with_options_title() {
        let mut buffer = OptimizedBuffer::new(20, 5);
        let options = BoxOptions {
            style: BoxStyle::single(Style::NONE),
            sides: BoxSides::default(),
            fill: None,
            title: Some("Title".to_string()),
            title_align: TitleAlign::Left,
        };
        draw_box_with_options(&mut buffer, 0, 0, 10, 4, options);
        assert_eq!(
            buffer.get(1, 0).unwrap().content,
            crate::cell::CellContent::Char('T')
        );
    }

    #[test]
    fn test_draw_text_with_pool_ascii() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut pool = GraphemePool::new();

        draw_text_with_pool(&mut buffer, &mut pool, 0, 0, "Hello", Style::fg(Rgba::RED));

        assert_eq!(buffer.get(0, 0).unwrap().content, CellContent::Char('H'));
        assert_eq!(buffer.get(4, 0).unwrap().content, CellContent::Char('o'));

        // No graphemes should be allocated for ASCII
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_draw_text_with_pool_emoji() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut pool = GraphemePool::new();

        // Use a ZWJ family emoji which is multi-codepoint
        draw_text_with_pool(&mut buffer, &mut pool, 0, 0, "Hi 👨‍👩‍👧!", Style::NONE);

        // H, i, space should be Char
        assert!(matches!(
            buffer.get(0, 0).unwrap().content,
            CellContent::Char('H')
        ));
        assert!(matches!(
            buffer.get(1, 0).unwrap().content,
            CellContent::Char('i')
        ));
        assert!(matches!(
            buffer.get(2, 0).unwrap().content,
            CellContent::Char(' ')
        ));

        // 👨‍👩‍👧 should be Grapheme with width 2 (multi-codepoint ZWJ sequence)
        let emoji_cell = buffer.get(3, 0).unwrap();
        assert!(matches!(emoji_cell.content, CellContent::Grapheme(_)));
        assert_eq!(emoji_cell.display_width(), 2);

        // Cell 4 should be continuation
        assert!(buffer.get(4, 0).unwrap().is_continuation());

        // ! at position 5
        assert!(matches!(
            buffer.get(5, 0).unwrap().content,
            CellContent::Char('!')
        ));

        // One grapheme should be allocated
        assert_eq!(pool.active_count(), 1);

        // Can resolve the grapheme from the pool
        if let CellContent::Grapheme(id) = emoji_cell.content {
            assert_eq!(pool.get(id), Some("👨‍👩‍👧"));
        }
    }

    #[test]
    fn test_draw_text_with_pool_single_codepoint_emoji() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut pool = GraphemePool::new();

        // Single codepoint emoji (👍) should be stored as Char, not Grapheme
        draw_text_with_pool(&mut buffer, &mut pool, 0, 0, "👍", Style::NONE);

        let cell = buffer.get(0, 0).unwrap();
        // Single codepoint emoji stored as Char (the codepoint fits in char)
        assert!(matches!(cell.content, CellContent::Char('👍')));
        assert_eq!(cell.display_width(), 2);
        assert!(buffer.get(1, 0).unwrap().is_continuation());

        // No graphemes allocated for single codepoint
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_draw_text_with_pool_deduplication() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut pool = GraphemePool::new();

        // Draw the same multi-codepoint grapheme twice (family emoji)
        draw_text_with_pool(&mut buffer, &mut pool, 0, 0, "👨‍👩‍👧👨‍👩‍👧", Style::NONE);

        // Only one grapheme should be allocated (intern deduplicates)
        assert_eq!(pool.active_count(), 1);

        // Both cells should reference the same grapheme with refcount 2
        // First family at 0, continuation at 1; second family at 2, continuation at 3
        let cell1 = buffer.get(0, 0).unwrap();
        let cell2 = buffer.get(2, 0).unwrap();

        if let (CellContent::Grapheme(id1), CellContent::Grapheme(id2)) =
            (cell1.content, cell2.content)
        {
            assert_eq!(id1, id2);
            assert_eq!(pool.refcount(id1), 2);
        } else {
            panic!("Expected Grapheme content");
        }
    }

    #[test]
    fn test_draw_char_with_pool() {
        let mut buffer = OptimizedBuffer::new(80, 24);
        let mut pool = GraphemePool::new();

        // Single codepoint
        draw_char_with_pool(&mut buffer, &mut pool, 0, 0, "A", Style::NONE);
        assert!(matches!(
            buffer.get(0, 0).unwrap().content,
            CellContent::Char('A')
        ));

        // Multi-codepoint grapheme
        draw_char_with_pool(&mut buffer, &mut pool, 5, 0, "👨‍👩‍👧", Style::NONE);
        let cell = buffer.get(5, 0).unwrap();
        assert!(cell.content.is_grapheme());
        assert_eq!(cell.display_width(), 2);
        assert!(buffer.get(6, 0).unwrap().is_continuation());

        // Can resolve from pool
        if let CellContent::Grapheme(id) = cell.content {
            assert_eq!(pool.get(id), Some("👨‍👩‍👧"));
        }
    }
}
