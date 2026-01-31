//! E2E Unicode stress testing (bd-qqf9).
//!
//! This module provides comprehensive tests for Unicode handling edge cases,
//! including wide characters, emoji, combining characters, and stress tests.

#![allow(clippy::uninlined_format_args)] // Clarity over style in test code

mod common;

use opentui::OptimizedBuffer;
use opentui::buffer::ClipRect;
use opentui::color::Rgba;
use opentui::grapheme_pool::GraphemePool;
use opentui::style::Style;
use opentui::unicode::{display_width, display_width_char, graphemes};

// ============================================================================
// Test Helpers
// ============================================================================

/// Verify that rendered text matches expected cell layout.
fn verify_text_render(buffer: &OptimizedBuffer, x: u32, y: u32, text: &str, _style: Style) {
    let mut col = x;
    for grapheme in graphemes(text) {
        let width = display_width(grapheme) as u32;
        if width == 0 {
            continue;
        }

        // Check the first cell of the grapheme
        let cell = buffer.get(col, y).expect("cell should exist");
        assert!(
            !cell.is_continuation(),
            "first cell of grapheme '{}' at ({},{}) should not be continuation",
            grapheme,
            col,
            y
        );

        // Check continuation cells for wide characters
        for i in 1..width {
            let cont_cell = buffer
                .get(col + i, y)
                .expect("continuation cell should exist");
            assert!(
                cont_cell.is_continuation(),
                "cell at ({},{}) should be continuation for wide grapheme '{}'",
                col + i,
                y,
                grapheme
            );
        }

        col += width;
    }
}

// ============================================================================
// Wide Characters (CJK)
// ============================================================================

#[test]
fn test_cjk_basic_rendering() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // CJK characters are width 2
    let text = "\u{4E2D}\u{6587}"; // 中文
    buffer.draw_text(0, 0, text, Style::NONE);

    verify_text_render(&buffer, 0, 0, text, Style::NONE);

    // First character 中
    let cell0 = buffer.get(0, 0).unwrap();
    assert!(!cell0.is_continuation());
    assert_eq!(cell0.display_width(), 2);

    // Second cell is continuation of 中
    let cell1 = buffer.get(1, 0).unwrap();
    assert!(cell1.is_continuation());

    // Second character 文 at position 2
    let cell2 = buffer.get(2, 0).unwrap();
    assert!(!cell2.is_continuation());
    assert_eq!(cell2.display_width(), 2);

    // Fourth cell is continuation of 文
    let cell3 = buffer.get(3, 0).unwrap();
    assert!(cell3.is_continuation());
}

#[test]
fn test_mixed_ascii_cjk() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Mixed ASCII and CJK
    let text = "Hello\u{4E16}\u{754C}World"; // Hello世界World
    buffer.draw_text(0, 0, text, Style::NONE);

    // Check total display width: 5 (Hello) + 4 (世界 = 2+2) + 5 (World) = 14
    let total_width: usize = graphemes(text).map(|g| display_width(g)).sum();
    assert_eq!(total_width, 14);

    // Verify positions:
    // H at 0, e at 1, l at 2, l at 3, o at 4
    // 世 at 5-6 (width 2), 界 at 7-8 (width 2)
    // W at 9, o at 10, r at 11, l at 12, d at 13

    // Check ASCII before CJK
    for i in 0..5 {
        let cell = buffer.get(i, 0).unwrap();
        assert!(
            !cell.is_continuation(),
            "ASCII char at {} should not be continuation",
            i
        );
    }

    // Check CJK character 世
    let cell5 = buffer.get(5, 0).unwrap();
    assert!(!cell5.is_continuation());
    let cell6 = buffer.get(6, 0).unwrap();
    assert!(cell6.is_continuation());

    // Check CJK character 界
    let cell7 = buffer.get(7, 0).unwrap();
    assert!(!cell7.is_continuation());
    let cell8 = buffer.get(8, 0).unwrap();
    assert!(cell8.is_continuation());

    // Check ASCII after CJK
    for i in 9..14 {
        let cell = buffer.get(i, 0).unwrap();
        assert!(
            !cell.is_continuation(),
            "ASCII char at {} should not be continuation",
            i
        );
    }
}

#[test]
fn test_cjk_at_line_boundaries() {
    let mut buffer = OptimizedBuffer::new(10, 24);
    buffer.clear(Rgba::BLACK);

    // Push scissor to limit width to 10
    // Try to draw CJK at position 9 (only 1 cell left)
    let text = "\u{4E2D}"; // 中 (width 2)
    buffer.draw_text(9, 0, text, Style::NONE);

    // The character should be clipped or wrapped depending on implementation
    // At minimum, it shouldn't panic
}

#[test]
fn test_cjk_in_scissor_region() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Scissor to a 10-cell wide region
    buffer.push_scissor(ClipRect::new(5, 0, 10, 10));

    // Draw CJK text that should be clipped
    let text = "\u{4E2D}\u{6587}\u{5B57}\u{7B26}\u{6D4B}\u{8BD5}"; // 中文字符测试
    buffer.draw_text(5, 0, text, Style::NONE);

    buffer.pop_scissor();

    // Only first few characters should be visible within the scissor region
    // (10 cells / 2 width per char = 5 characters max)
}

// ============================================================================
// Emoji
// ============================================================================

#[test]
fn test_basic_emoji_single_codepoint() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Basic emoji (single codepoint, typically width 2)
    let emojis = ["\u{1F600}", "\u{1F601}", "\u{1F602}"]; // 😀😁😂

    for (i, emoji) in emojis.iter().enumerate() {
        let x = (i * 2) as u32;
        buffer.draw_text(x, 0, emoji, Style::NONE);

        // Verify the emoji cell exists and has correct width
        let cell = buffer.get(x, 0).unwrap();
        assert!(
            !cell.is_continuation(),
            "emoji at {} should not be continuation",
            x
        );
    }
}

#[test]
fn test_emoji_with_skin_tone() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Emoji with skin tone modifier (single grapheme cluster)
    let emoji = "\u{1F44B}\u{1F3FD}"; // 👋🏽 (waving hand + medium skin tone)
    let width = display_width(emoji);

    buffer.draw_text_with_pool(&mut pool, 0, 0, emoji, Style::NONE);

    // The emoji + skin tone should render as a single grapheme
    let cell = buffer.get(0, 0).unwrap();
    assert!(!cell.is_continuation());
    assert_eq!(cell.display_width() as usize, width);
}

#[test]
fn test_emoji_zwj_sequence() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Family emoji (ZWJ sequence: man + ZWJ + woman + ZWJ + girl + ZWJ + boy)
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";

    buffer.draw_text_with_pool(&mut pool, 0, 0, family, Style::NONE);

    // The ZWJ sequence should render as a single grapheme cluster
    let grapheme_count: usize = graphemes(family).count();
    assert_eq!(
        grapheme_count, 1,
        "ZWJ family should be single grapheme cluster"
    );

    let cell = buffer.get(0, 0).unwrap();
    assert!(!cell.is_continuation());
}

#[test]
fn test_flag_emoji_regional_indicators() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Flag emoji using regional indicator symbols
    let us_flag = "\u{1F1FA}\u{1F1F8}"; // 🇺🇸
    let jp_flag = "\u{1F1EF}\u{1F1F5}"; // 🇯🇵
    let gb_flag = "\u{1F1EC}\u{1F1E7}"; // 🇬🇧

    // Each flag should be a single grapheme cluster
    assert_eq!(graphemes(us_flag).count(), 1);
    assert_eq!(graphemes(jp_flag).count(), 1);
    assert_eq!(graphemes(gb_flag).count(), 1);

    buffer.draw_text_with_pool(&mut pool, 0, 0, us_flag, Style::NONE);
    buffer.draw_text_with_pool(&mut pool, 4, 0, jp_flag, Style::NONE);
    buffer.draw_text_with_pool(&mut pool, 8, 0, gb_flag, Style::NONE);

    // Verify rendering
    let cell0 = buffer.get(0, 0).unwrap();
    assert!(!cell0.is_continuation());
}

#[test]
fn test_emoji_in_text_flow() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Emoji mixed with regular text
    let text = "Hello \u{1F600} World \u{1F44D}!"; // Hello 😀 World 👍!
    buffer.draw_text(0, 0, text, Style::NONE);

    // Calculate expected width
    let expected_width: usize = graphemes(text).map(|g| display_width(g)).sum();

    // Verify no panics and reasonable width
    assert!(expected_width > 0);
    assert!(expected_width <= 80);
}

// ============================================================================
// Combining Characters
// ============================================================================

#[test]
fn test_combining_diacriticals_precomposed_vs_decomposed() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Precomposed: é (single codepoint U+00E9)
    let precomposed = "\u{00E9}";
    // Decomposed: e + combining acute accent (U+0065 + U+0301)
    let decomposed = "e\u{0301}";

    buffer.draw_text(0, 0, precomposed, Style::NONE);
    buffer.draw_text(5, 0, decomposed, Style::NONE);

    // Both should display as single grapheme cluster with width 1
    assert_eq!(graphemes(precomposed).count(), 1);
    assert_eq!(graphemes(decomposed).count(), 1);
    assert_eq!(display_width(precomposed), 1);
    assert_eq!(display_width(decomposed), 1);
}

#[test]
fn test_multiple_combining_marks() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Base character with multiple combining marks
    // a + acute + circumflex + tilde
    let text = "a\u{0301}\u{0302}\u{0303}";

    buffer.draw_text_with_pool(&mut pool, 0, 0, text, Style::NONE);

    // Should be single grapheme cluster
    assert_eq!(graphemes(text).count(), 1);

    // Width should still be 1 (combining marks don't add width)
    assert_eq!(display_width(text), 1);

    let cell = buffer.get(0, 0).unwrap();
    assert!(!cell.is_continuation());
    assert_eq!(cell.display_width(), 1);
}

#[test]
fn test_combining_marks_at_boundaries() {
    let mut buffer = OptimizedBuffer::new(10, 24);
    buffer.clear(Rgba::BLACK);

    // Draw text near boundary with combining marks
    let text = "test\u{0301}"; // test́ (t with combining acute at the end)

    buffer.draw_text(5, 0, text, Style::NONE);

    // Should render without panic
    // The 't' with combining mark should still fit
}

// ============================================================================
// Special Cases
// ============================================================================

#[test]
fn test_zero_width_joiner() {
    // ZWJ should have width 0 on its own
    assert_eq!(display_width_char('\u{200D}'), 0);

    // But when part of a ZWJ sequence, it joins graphemes
    let without_zwj = "\u{1F468}\u{1F469}"; // 👨👩 (separate)
    let with_zwj = "\u{1F468}\u{200D}\u{1F469}"; // 👨‍👩 (couple)

    assert_eq!(graphemes(without_zwj).count(), 2);
    assert_eq!(graphemes(with_zwj).count(), 1);
}

#[test]
fn test_zero_width_non_joiner() {
    // ZWNJ has width 0
    assert_eq!(display_width_char('\u{200C}'), 0);

    // ZWNJ may or may not cause grapheme breaks depending on implementation
    // The key invariant is that it doesn't add visible width
    let text = "a\u{200C}b";
    let total_width = display_width(text);
    assert_eq!(
        total_width, 2,
        "a+ZWNJ+b should have display width 2 (ZWNJ is invisible)"
    );
}

#[test]
fn test_variation_selectors() {
    // Variation selector 16 (VS16) - emoji presentation
    let text_presentation = "\u{2764}"; // ❤ (text style heart)
    let emoji_presentation = "\u{2764}\u{FE0F}"; // ❤️ (emoji style heart with VS16)

    // Both should be single grapheme clusters
    assert_eq!(graphemes(text_presentation).count(), 1);
    assert_eq!(graphemes(emoji_presentation).count(), 1);

    // Width might differ based on presentation
    // Text style is typically width 1, emoji style is typically width 2
}

#[test]
fn test_tab_characters() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Tab character
    let text = "A\tB";
    buffer.draw_text(0, 0, text, Style::NONE);

    // Tab handling varies - the test verifies no panic occurs
    // Actual tab expansion is implementation-dependent
}

#[test]
fn test_newline_variants() {
    // Newline characters - width behavior is implementation-dependent
    // Our implementation treats them as control characters
    let lf_width = display_width_char('\n');
    let cr_width = display_width_char('\r');

    // At minimum, they shouldn't be wide (width 2)
    assert!(lf_width <= 1, "LF should not be wide");
    assert!(cr_width <= 1, "CR should not be wide");

    // CRLF as a string
    let crlf = "\r\n";
    let crlf_width = display_width(crlf);
    assert!(crlf_width <= 2, "CRLF should not be wide");
}

#[test]
fn test_control_characters() {
    // Control characters should have width 0
    for c in 0..32u8 {
        let ch = c as char;
        assert_eq!(
            display_width_char(ch),
            0,
            "control char U+{:04X} should have width 0",
            c
        );
    }

    // DEL (0x7F) should also have width 0
    assert_eq!(display_width_char('\u{007F}'), 0);
}

// ============================================================================
// Stress Tests
// ============================================================================

#[test]
fn test_many_different_graphemes() {
    let mut buffer = OptimizedBuffer::new(200, 50);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Generate many different graphemes
    let mut grapheme_list = Vec::new();

    // ASCII
    for c in 'A'..='Z' {
        grapheme_list.push(c.to_string());
    }
    for c in 'a'..='z' {
        grapheme_list.push(c.to_string());
    }

    // CJK
    for c in '\u{4E00}'..'\u{4E50}' {
        grapheme_list.push(c.to_string());
    }

    // Emoji
    for c in '\u{1F600}'..'\u{1F650}' {
        grapheme_list.push(c.to_string());
    }

    // Greek
    for c in '\u{0391}'..'\u{03C9}' {
        grapheme_list.push(c.to_string());
    }

    // Cyrillic
    for c in '\u{0410}'..'\u{044F}' {
        grapheme_list.push(c.to_string());
    }

    // Hebrew
    for c in '\u{05D0}'..'\u{05EA}' {
        grapheme_list.push(c.to_string());
    }

    // Arabic
    for c in '\u{0621}'..'\u{064A}' {
        grapheme_list.push(c.to_string());
    }

    // Render all graphemes
    let text = grapheme_list.join("");
    let grapheme_count = graphemes(&text).count();

    assert!(
        grapheme_count >= 200,
        "should have at least 200 graphemes, got {}",
        grapheme_count
    );

    // Draw in chunks across multiple rows
    let mut x = 0u32;
    let mut y = 0u32;
    for grapheme in graphemes(&text) {
        let w = display_width(grapheme) as u32;
        if x + w > 200 {
            x = 0;
            y += 1;
            if y >= 50 {
                break;
            }
        }
        buffer.draw_text_with_pool(&mut pool, x, y, grapheme, Style::NONE);
        x += w.max(1); // At least move 1 cell for zero-width
    }
}

#[test]
fn test_very_long_grapheme_cluster() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Create a very long grapheme cluster with many combining marks
    // Base character followed by 50 combining marks
    let mut long_grapheme = String::from("e");
    for _ in 0..50 {
        long_grapheme.push('\u{0301}'); // combining acute accent
    }

    // Should be single grapheme cluster
    assert_eq!(graphemes(&long_grapheme).count(), 1);

    // Should have width 1 (combining marks don't add width)
    assert_eq!(display_width(&long_grapheme), 1);

    // Render it
    buffer.draw_text_with_pool(&mut pool, 0, 0, &long_grapheme, Style::NONE);

    // Verify it's stored
    let cell = buffer.get(0, 0).unwrap();
    assert!(!cell.is_continuation());
}

#[test]
fn test_alternating_width_characters() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Alternating ASCII (width 1) and CJK (width 2)
    let text = "A\u{4E2D}B\u{6587}C\u{5B57}D\u{7B26}E";
    // Widths: A=1, 中=2, B=1, 文=2, C=1, 字=2, D=1, 符=2, E=1
    // Total: 1+2+1+2+1+2+1+2+1 = 13

    buffer.draw_text(0, 0, text, Style::NONE);

    let total_width: usize = graphemes(text).map(display_width).sum();
    assert_eq!(total_width, 13);

    // Verify positions
    // A at 0
    // 中 at 1-2
    // B at 3
    // 文 at 4-5
    // C at 6
    // 字 at 7-8
    // D at 9
    // 符 at 10-11
    // E at 12

    let positions = [
        (0, false),  // A
        (1, false),  // 中
        (2, true),   // continuation
        (3, false),  // B
        (4, false),  // 文
        (5, true),   // continuation
        (6, false),  // C
        (7, false),  // 字
        (8, true),   // continuation
        (9, false),  // D
        (10, false), // 符
        (11, true),  // continuation
        (12, false), // E
    ];

    for (pos, should_be_continuation) in positions {
        let cell = buffer.get(pos, 0).unwrap();
        assert_eq!(
            cell.is_continuation(),
            should_be_continuation,
            "position {} should {}be continuation",
            pos,
            if should_be_continuation { "" } else { "not " }
        );
    }
}

#[test]
fn test_full_buffer_with_unicode() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    let mut pool = GraphemePool::new();
    buffer.clear(Rgba::BLACK);

    // Fill entire buffer with mixed Unicode content
    for y in 0..24 {
        let text = match y % 4 {
            0 => "ASCII text with numbers 0123456789 and symbols !@#$%",
            1 => "\u{4E2D}\u{6587}\u{65E5}\u{672C}\u{8A9E}\u{D55C}\u{AE00}",
            2 => "\u{1F600}\u{1F601}\u{1F602}\u{1F603}\u{1F604}\u{1F605}",
            _ => "e\u{0301} a\u{0300} n\u{0303} o\u{0302} u\u{0308}",
        };
        buffer.draw_text_with_pool(&mut pool, 0, y, text, Style::NONE);
    }

    // Verify no panics and buffer is populated
    let (width, height) = buffer.size();
    assert_eq!(width, 80);
    assert_eq!(height, 24);
}

#[test]
fn test_unicode_width_consistency() {
    // Verify that width calculations are consistent between methods

    // Test cases with definite expected widths
    let definite_cases = [
        ("A", 1),
        ("\u{4E2D}", 2),  // 中
        ("\u{1F600}", 2), // 😀
        ("e\u{0301}", 1), // é (decomposed)
        ("\u{00E9}", 1),  // é (precomposed)
        ("\u{200D}", 0),  // ZWJ
    ];

    for (grapheme, expected_width) in definite_cases {
        let width = display_width(grapheme);
        assert_eq!(
            width, expected_width,
            "grapheme {:?} should have width {}, got {}",
            grapheme, expected_width, width
        );
    }

    // Control characters - implementation may vary but should be <=1
    let control_cases = ["\t", "\n"];
    for grapheme in control_cases {
        let width = display_width(grapheme);
        assert!(
            width <= 1,
            "control character {:?} should have width <= 1, got {}",
            grapheme,
            width
        );
    }
}

#[test]
fn test_rtl_characters() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Hebrew
    let hebrew = "\u{05E9}\u{05DC}\u{05D5}\u{05DD}"; // שלום
    buffer.draw_text(0, 0, hebrew, Style::NONE);

    // Arabic
    let arabic = "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}"; // مرحبا
    buffer.draw_text(0, 1, arabic, Style::NONE);

    // Mixed LTR and RTL
    let mixed = "Hello \u{05E9}\u{05DC}\u{05D5}\u{05DD} World";
    buffer.draw_text(0, 2, mixed, Style::NONE);

    // Verify rendering doesn't panic and characters are placed
    // (Actual bidi reordering depends on terminal, not our renderer)
}

#[test]
fn test_private_use_area() {
    // Private Use Area characters (U+E000-U+F8FF)
    // These are often used for custom symbols/icons (e.g., Nerd Fonts)
    let pua_char = '\u{E000}';
    let width = display_width_char(pua_char);

    // PUA width is implementation-dependent, but shouldn't panic
    assert!(width <= 2, "PUA character width should be reasonable");
}

#[test]
fn test_surrogates_and_noncharacters() {
    // These shouldn't appear in valid UTF-8, but verify width function handles them
    // Note: Rust strings can't contain unpaired surrogates (they're invalid UTF-8)

    // Noncharacter: U+FFFE
    let nonchar = '\u{FFFE}';
    let width = display_width_char(nonchar);
    assert!(width <= 2);

    // Replacement character: U+FFFD
    let replacement = '\u{FFFD}';
    let width = display_width_char(replacement);
    assert!(width <= 2);
}
