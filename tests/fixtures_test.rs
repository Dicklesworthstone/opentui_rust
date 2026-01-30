//! Integration tests for the test fixtures module.
//!
//! This file verifies that the fixtures module compiles and works correctly.

#![allow(clippy::float_cmp)] // Exact float comparison is intentional in tests

mod fixtures;

use fixtures::*;

#[test]
fn test_mock_terminal_basic() {
    use std::io::Write;

    let mut term = MockTerminal::new(80, 24);
    write!(term, "Hello, World!").unwrap();
    assert_eq!(term.output_str(), "Hello, World!");
}

#[test]
fn test_mock_terminal_ansi_detection() {
    use std::io::Write;

    let mut term = MockTerminal::new(80, 24);
    write!(term, "\x1b[?1049h\x1b[2J\x1b[?25l").unwrap();

    assert!(term.entered_alt_screen());
    assert!(term.screen_cleared());
    assert!(term.cursor_hidden());
}

#[test]
fn test_mock_terminal_sequence_extraction() {
    use std::io::Write;

    let mut term = MockTerminal::new(80, 24);
    write!(term, "\x1b[31m\x1b[1m\x1b[0m").unwrap();

    let sgr = term.extract_sgr_sequences();
    assert_eq!(sgr.len(), 3);
}

#[test]
fn test_mock_input_key_queue() {
    let mut input = MockInput::new();
    input.queue_string("abc");

    assert_eq!(input.event_count(), 3);
    assert!(input.has_events());
}

#[test]
fn test_mock_input_builder() {
    use opentui::input::KeyCode;

    let input = InputSequenceBuilder::new()
        .string("hello")
        .enter()
        .ctrl('c')
        .build();

    assert_eq!(input.event_count(), 7);
}

#[test]
fn test_buffer_comparison() {
    use opentui::buffer::OptimizedBuffer;
    use opentui::cell::Cell;
    use opentui::style::Style;

    let mut buf1 = OptimizedBuffer::new(10, 5);
    let mut buf2 = OptimizedBuffer::new(10, 5);

    buf1.set(0, 0, Cell::new('A', Style::default()));
    buf2.set(0, 0, Cell::new('A', Style::default()));

    let diff = compare_buffers(&buf1, &buf2);
    assert!(diff.equal);
}

#[test]
fn test_buffer_comparison_different() {
    use opentui::buffer::OptimizedBuffer;
    use opentui::cell::Cell;
    use opentui::style::Style;

    let mut buf1 = OptimizedBuffer::new(10, 5);
    let mut buf2 = OptimizedBuffer::new(10, 5);

    buf1.set(0, 0, Cell::new('A', Style::default()));
    buf2.set(0, 0, Cell::new('B', Style::default()));

    let diff = compare_buffers(&buf1, &buf2);
    assert!(!diff.equal);
    assert_eq!(diff.differences.len(), 1);
}

#[test]
fn test_assert_cell_char() {
    use opentui::buffer::OptimizedBuffer;
    use opentui::cell::Cell;
    use opentui::style::Style;

    let mut buffer = OptimizedBuffer::new(10, 5);
    buffer.set(3, 2, Cell::new('X', Style::default()));

    assert_cell_char(&buffer, 3, 2, 'X');
}

#[test]
fn test_assert_text_at() {
    use opentui::buffer::OptimizedBuffer;
    use opentui::cell::Cell;
    use opentui::style::Style;

    let mut buffer = OptimizedBuffer::new(20, 5);
    for (i, c) in "Hello".chars().enumerate() {
        buffer.set(5 + i as u32, 2, Cell::new(c, Style::default()));
    }

    assert_text_at(&buffer, 5, 2, "Hello");
}

#[test]
fn test_ansi_sequence_builders() {
    let pos = ansi_sequences::cursor_position(5, 10);
    assert_eq!(pos, b"\x1b[5;10H");

    let fg = ansi_sequences::fg_rgb(255, 128, 64);
    assert_eq!(fg, b"\x1b[38;2;255;128;64m");
}

#[test]
fn test_buffer_generators() {
    let empty = buffers::empty(80, 24);
    assert_eq!(empty.width(), 80);
    assert_eq!(empty.height(), 24);

    let checker = buffers::checkerboard(10, 10, 'X', 'O');
    assert_eq!(checker.width(), 10);
}

#[test]
fn test_color_generators() {
    use opentui::color::Rgba;

    let gray = colors::grayscale(0.5);
    assert!((gray.r - 0.5).abs() < 0.001);
    assert!((gray.g - 0.5).abs() < 0.001);
    assert!((gray.b - 0.5).abs() < 0.001);

    // Deterministic
    let c1 = colors::from_seed(42);
    let c2 = colors::from_seed(42);
    assert_eq!(c1.r, c2.r);
}

#[test]
fn test_sample_text_available() {
    assert!(!sample_text::SHORT_ASCII.is_empty());
    assert!(!sample_text::LONG_ASCII.is_empty());
    assert!(!sample_text::UNICODE_MIXED.is_empty());
    assert!(!sample_text::EMOJI_BASIC.is_empty());
}

#[test]
fn test_style_generators() {
    use opentui::style::TextAttributes;

    let bold = styles::bold();
    assert!(bold.attributes.contains(TextAttributes::BOLD));

    let italic = styles::italic();
    assert!(italic.attributes.contains(TextAttributes::ITALIC));
}
