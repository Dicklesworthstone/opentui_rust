//! Assertion helpers for testing OpenTUI buffers and cells.
//!
//! This module provides assertion functions and macros for comparing
//! buffers, cells, and ANSI sequences in tests.

use opentui::buffer::OptimizedBuffer;
use opentui::cell::{Cell, CellContent};
use opentui::color::Rgba;
use opentui::style::Style;
use std::fmt::Write;

/// Result of a buffer comparison.
#[derive(Debug)]
pub struct BufferDiff {
    /// Whether the buffers are equal.
    pub equal: bool,
    /// List of differing cells with positions.
    pub differences: Vec<CellDifference>,
    /// Human-readable summary.
    pub summary: String,
}

/// A single cell difference between two buffers.
#[derive(Debug, Clone)]
pub struct CellDifference {
    pub x: u32,
    pub y: u32,
    pub expected: CellSnapshot,
    pub actual: CellSnapshot,
}

/// A snapshot of a cell's state for comparison.
#[derive(Debug, Clone, PartialEq)]
pub struct CellSnapshot {
    pub content: String,
    pub fg: (u8, u8, u8, u8),
    pub bg: (u8, u8, u8, u8),
}

impl CellSnapshot {
    /// Create a snapshot from a Cell.
    pub fn from_cell(cell: &Cell) -> Self {
        let content = match cell.content {
            CellContent::Char(c) => c.to_string(),
            CellContent::Empty => " ".to_string(),
            CellContent::Continuation => "".to_string(),
            CellContent::Grapheme(id) => format!("<grapheme:{}>", id.pool_id()),
        };

        let fg = cell.fg.to_rgba_u8();
        let bg = cell.bg.to_rgba_u8();

        Self {
            content,
            fg: (fg.0, fg.1, fg.2, (cell.fg.a * 255.0) as u8),
            bg: (bg.0, bg.1, bg.2, (cell.bg.a * 255.0) as u8),
        }
    }
}

/// Compare two buffers and return detailed differences.
pub fn compare_buffers(expected: &OptimizedBuffer, actual: &OptimizedBuffer) -> BufferDiff {
    let mut differences = Vec::new();
    let mut summary = String::new();

    // Check dimensions
    if expected.width() != actual.width() || expected.height() != actual.height() {
        writeln!(
            summary,
            "Dimension mismatch: expected {}x{}, got {}x{}",
            expected.width(),
            expected.height(),
            actual.width(),
            actual.height()
        )
        .unwrap();

        return BufferDiff {
            equal: false,
            differences,
            summary,
        };
    }

    // Compare cells
    for y in 0..expected.height() {
        for x in 0..expected.width() {
            let expected_cell = expected.get(x, y);
            let actual_cell = actual.get(x, y);

            match (expected_cell, actual_cell) {
                (Some(e), Some(a)) => {
                    let e_snap = CellSnapshot::from_cell(e);
                    let a_snap = CellSnapshot::from_cell(a);

                    if e_snap != a_snap {
                        differences.push(CellDifference {
                            x,
                            y,
                            expected: e_snap,
                            actual: a_snap,
                        });
                    }
                }
                (None, Some(_)) => {
                    differences.push(CellDifference {
                        x,
                        y,
                        expected: CellSnapshot {
                            content: "<none>".to_string(),
                            fg: (0, 0, 0, 0),
                            bg: (0, 0, 0, 0),
                        },
                        actual: CellSnapshot::from_cell(actual_cell.unwrap()),
                    });
                }
                (Some(_), None) => {
                    differences.push(CellDifference {
                        x,
                        y,
                        expected: CellSnapshot::from_cell(expected_cell.unwrap()),
                        actual: CellSnapshot {
                            content: "<none>".to_string(),
                            fg: (0, 0, 0, 0),
                            bg: (0, 0, 0, 0),
                        },
                    });
                }
                (None, None) => {}
            }
        }
    }

    if differences.is_empty() {
        summary = "Buffers are identical".to_string();
    } else {
        writeln!(summary, "{} cells differ:", differences.len()).unwrap();
        for (i, diff) in differences.iter().take(10).enumerate() {
            writeln!(
                summary,
                "  {}. ({}, {}): expected '{}', got '{}'",
                i + 1,
                diff.x,
                diff.y,
                diff.expected.content,
                diff.actual.content
            )
            .unwrap();
        }
        if differences.len() > 10 {
            writeln!(summary, "  ... and {} more", differences.len() - 10).unwrap();
        }
    }

    BufferDiff {
        equal: differences.is_empty(),
        differences,
        summary,
    }
}

/// Assert that two buffers are equal.
///
/// Panics with a detailed diff if they differ.
pub fn assert_buffers_equal(expected: &OptimizedBuffer, actual: &OptimizedBuffer) {
    let diff = compare_buffers(expected, actual);
    if !diff.equal {
        panic!("Buffer assertion failed:\n{}", diff.summary);
    }
}

/// Assert that a cell at position (x, y) has the expected character.
pub fn assert_cell_char(buffer: &OptimizedBuffer, x: u32, y: u32, expected: char) {
    let cell = buffer
        .get(x, y)
        .unwrap_or_else(|| panic!("No cell at ({}, {})", x, y));

    match cell.content {
        CellContent::Char(c) => {
            if c != expected {
                panic!("Cell ({}, {}) expected '{}', got '{}'", x, y, expected, c);
            }
        }
        other => {
            panic!(
                "Cell ({}, {}) expected Char('{}'), got {:?}",
                x, y, expected, other
            );
        }
    }
}

/// Assert that a cell at position (x, y) is empty.
pub fn assert_cell_empty(buffer: &OptimizedBuffer, x: u32, y: u32) {
    let cell = buffer
        .get(x, y)
        .unwrap_or_else(|| panic!("No cell at ({}, {})", x, y));

    if !matches!(cell.content, CellContent::Empty) {
        panic!("Cell ({}, {}) expected Empty, got {:?}", x, y, cell.content);
    }
}

/// Assert that a cell has the expected foreground color.
pub fn assert_cell_fg(buffer: &OptimizedBuffer, x: u32, y: u32, expected: Rgba) {
    let cell = buffer
        .get(x, y)
        .unwrap_or_else(|| panic!("No cell at ({}, {})", x, y));

    let actual = cell.fg;
    if !colors_equal(actual, expected) {
        panic!(
            "Cell ({}, {}) fg expected {:?}, got {:?}",
            x, y, expected, actual
        );
    }
}

/// Assert that a cell has the expected background color.
pub fn assert_cell_bg(buffer: &OptimizedBuffer, x: u32, y: u32, expected: Rgba) {
    let cell = buffer
        .get(x, y)
        .unwrap_or_else(|| panic!("No cell at ({}, {})", x, y));

    let actual = cell.bg;
    if !colors_equal(actual, expected) {
        panic!(
            "Cell ({}, {}) bg expected {:?}, got {:?}",
            x, y, expected, actual
        );
    }
}

/// Compare two colors with epsilon tolerance for floating point.
fn colors_equal(a: Rgba, b: Rgba) -> bool {
    const EPSILON: f32 = 0.001;
    (a.r - b.r).abs() < EPSILON
        && (a.g - b.g).abs() < EPSILON
        && (a.b - b.b).abs() < EPSILON
        && (a.a - b.a).abs() < EPSILON
}

/// Assert that a string appears at the given position in the buffer.
pub fn assert_text_at(buffer: &OptimizedBuffer, x: u32, y: u32, expected: &str) {
    for (i, expected_char) in expected.chars().enumerate() {
        let cell_x = x + i as u32;
        assert_cell_char(buffer, cell_x, y, expected_char);
    }
}

/// Assert that a row contains the expected string (starting at x=0).
pub fn assert_row_text(buffer: &OptimizedBuffer, y: u32, expected: &str) {
    assert_text_at(buffer, 0, y, expected);
}

/// Assert that an ANSI sequence is present in the output.
pub fn assert_sequence_present(output: &[u8], sequence: &[u8]) {
    if !output.windows(sequence.len()).any(|w| w == sequence) {
        let output_hex: String = output.iter().map(|b| format!("{:02x} ", b)).collect();
        let seq_hex: String = sequence.iter().map(|b| format!("{:02x} ", b)).collect();
        panic!(
            "Expected sequence not found:\n  sequence: {}\n  output: {}",
            seq_hex.trim(),
            output_hex.trim()
        );
    }
}

/// Assert that an ANSI sequence is NOT present in the output.
pub fn assert_sequence_absent(output: &[u8], sequence: &[u8]) {
    if output.windows(sequence.len()).any(|w| w == sequence) {
        let seq_hex: String = sequence.iter().map(|b| format!("{:02x} ", b)).collect();
        panic!("Unexpected sequence found: {}", seq_hex.trim());
    }
}

/// Create a test buffer with a simple pattern for visual comparison.
///
/// Fills the buffer with a grid pattern of characters and colors.
pub fn create_test_pattern_buffer(width: u32, height: u32) -> OptimizedBuffer {
    let mut buffer = OptimizedBuffer::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let c = match (x + y) % 4 {
                0 => '.',
                1 => '+',
                2 => 'x',
                _ => 'o',
            };

            let fg = match x % 3 {
                0 => Rgba::RED,
                1 => Rgba::GREEN,
                _ => Rgba::BLUE,
            };

            buffer.set(x, y, Cell::new(c, Style::fg(fg)));
        }
    }

    buffer
}

/// Render a buffer to a string for debugging.
///
/// Returns a simple text representation of the buffer content.
pub fn buffer_to_string(buffer: &OptimizedBuffer) -> String {
    let mut result = String::new();

    for y in 0..buffer.height() {
        for x in 0..buffer.width() {
            if let Some(cell) = buffer.get(x, y) {
                match cell.content {
                    CellContent::Char(c) => result.push(c),
                    CellContent::Empty => result.push(' '),
                    CellContent::Continuation => {} // Skip continuation cells
                    CellContent::Grapheme(_) => result.push('?'),
                }
            } else {
                result.push('?');
            }
        }
        result.push('\n');
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_equal_buffers() {
        let buf1 = OptimizedBuffer::new(10, 5);
        let buf2 = OptimizedBuffer::new(10, 5);

        let diff = compare_buffers(&buf1, &buf2);
        assert!(diff.equal);
        assert!(diff.differences.is_empty());
    }

    #[test]
    fn test_compare_different_dimensions() {
        let buf1 = OptimizedBuffer::new(10, 5);
        let buf2 = OptimizedBuffer::new(20, 5);

        let diff = compare_buffers(&buf1, &buf2);
        assert!(!diff.equal);
        assert!(diff.summary.contains("Dimension mismatch"));
    }

    #[test]
    fn test_compare_different_content() {
        let mut buf1 = OptimizedBuffer::new(10, 5);
        let mut buf2 = OptimizedBuffer::new(10, 5);

        buf1.set(0, 0, Cell::new('A', Style::default()));
        buf2.set(0, 0, Cell::new('B', Style::default()));

        let diff = compare_buffers(&buf1, &buf2);
        assert!(!diff.equal);
        assert_eq!(diff.differences.len(), 1);
        assert_eq!(diff.differences[0].x, 0);
        assert_eq!(diff.differences[0].y, 0);
    }

    #[test]
    fn test_assert_cell_char() {
        let mut buffer = OptimizedBuffer::new(10, 5);
        buffer.set(3, 2, Cell::new('X', Style::default()));

        assert_cell_char(&buffer, 3, 2, 'X');
    }

    #[test]
    #[should_panic(expected = "expected 'Y', got 'X'")]
    fn test_assert_cell_char_fails() {
        let mut buffer = OptimizedBuffer::new(10, 5);
        buffer.set(3, 2, Cell::new('X', Style::default()));

        assert_cell_char(&buffer, 3, 2, 'Y');
    }

    #[test]
    fn test_assert_text_at() {
        let mut buffer = OptimizedBuffer::new(20, 5);
        for (i, c) in "Hello".chars().enumerate() {
            buffer.set(5 + i as u32, 2, Cell::new(c, Style::default()));
        }

        assert_text_at(&buffer, 5, 2, "Hello");
    }

    #[test]
    fn test_assert_sequence_present() {
        let output = b"\x1b[2J\x1b[H\x1b[31mHello";
        assert_sequence_present(output, b"\x1b[2J");
        assert_sequence_present(output, b"\x1b[31m");
    }

    #[test]
    #[should_panic(expected = "Expected sequence not found")]
    fn test_assert_sequence_present_fails() {
        let output = b"\x1b[2JHello";
        assert_sequence_present(output, b"\x1b[H");
    }

    #[test]
    fn test_create_test_pattern_buffer() {
        let buffer = create_test_pattern_buffer(20, 10);
        assert_eq!(buffer.width(), 20);
        assert_eq!(buffer.height(), 10);

        // Check pattern
        assert_cell_char(&buffer, 0, 0, '.');
        assert_cell_char(&buffer, 1, 0, '+');
    }

    #[test]
    fn test_buffer_to_string() {
        let mut buffer = OptimizedBuffer::new(5, 2);
        for (i, c) in "Hello".chars().enumerate() {
            buffer.set(i as u32, 0, Cell::new(c, Style::default()));
        }
        for (i, c) in "World".chars().enumerate() {
            buffer.set(i as u32, 1, Cell::new(c, Style::default()));
        }

        let s = buffer_to_string(&buffer);
        assert!(s.contains("Hello"));
        assert!(s.contains("World"));
    }
}
