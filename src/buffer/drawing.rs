//! Text and box drawing operations.

use crate::buffer::OptimizedBuffer;
use crate::cell::Cell;
use crate::color::Rgba;
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
pub fn draw_text(buffer: &mut OptimizedBuffer, x: u32, y: u32, text: &str, style: Style) {
    let mut col = x;

    for grapheme in text.graphemes(true) {
        if grapheme == "\n" || grapheme == "\r" {
            continue;
        }

        let cell = Cell::from_grapheme(grapheme, style);
        let width = cell.display_width();

        buffer.set(col, y, cell);

        // Add continuation cells for wide characters
        for i in 1..width {
            buffer.set(
                col + i as u32,
                y,
                Cell::continuation(style.bg.unwrap_or(Rgba::TRANSPARENT)),
            );
        }

        col += width as u32;
    }
}

/// Draw a box border.
pub fn draw_box(buffer: &mut OptimizedBuffer, x: u32, y: u32, w: u32, h: u32, box_style: BoxStyle) {
    if w < 2 || h < 2 {
        return;
    }

    let style = box_style.style;

    // Corners
    buffer.set(x, y, Cell::new(box_style.top_left, style));
    buffer.set(x + w - 1, y, Cell::new(box_style.top_right, style));
    buffer.set(x, y + h - 1, Cell::new(box_style.bottom_left, style));
    buffer.set(
        x + w - 1,
        y + h - 1,
        Cell::new(box_style.bottom_right, style),
    );

    // Horizontal edges
    for col in (x + 1)..(x + w - 1) {
        buffer.set(col, y, Cell::new(box_style.horizontal, style));
        buffer.set(col, y + h - 1, Cell::new(box_style.horizontal, style));
    }

    // Vertical edges
    for row in (y + 1)..(y + h - 1) {
        buffer.set(x, row, Cell::new(box_style.vertical, style));
        buffer.set(x + w - 1, row, Cell::new(box_style.vertical, style));
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
        buffer.set(x, y, Cell::new(options.style.top_left, style));
    }
    if options.sides.top && options.sides.right {
        buffer.set(x + w - 1, y, Cell::new(options.style.top_right, style));
    }
    if options.sides.bottom && options.sides.left {
        buffer.set(x, y + h - 1, Cell::new(options.style.bottom_left, style));
    }
    if options.sides.bottom && options.sides.right {
        buffer.set(
            x + w - 1,
            y + h - 1,
            Cell::new(options.style.bottom_right, style),
        );
    }

    // Horizontal edges
    if options.sides.top {
        for col in (x + 1)..(x + w - 1) {
            buffer.set(col, y, Cell::new(options.style.horizontal, style));
        }
    }
    if options.sides.bottom {
        for col in (x + 1)..(x + w - 1) {
            buffer.set(col, y + h - 1, Cell::new(options.style.horizontal, style));
        }
    }

    // Vertical edges
    if options.sides.left {
        for row in (y + 1)..(y + h - 1) {
            buffer.set(x, row, Cell::new(options.style.vertical, style));
        }
    }
    if options.sides.right {
        for row in (y + 1)..(y + h - 1) {
            buffer.set(x + w - 1, row, Cell::new(options.style.vertical, style));
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
        buffer.set(col, y, Cell::new(ch, style));
    }
}

/// Draw a vertical line.
pub fn draw_vline(buffer: &mut OptimizedBuffer, x: u32, y: u32, len: u32, ch: char, style: Style) {
    for row in y..y.saturating_add(len) {
        buffer.set(x, row, Cell::new(ch, style));
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
}
