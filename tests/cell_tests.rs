//! Comprehensive unit tests for the Cell module.
//!
//! Tests cover cell creation, comparison, wide characters, grapheme handling,
//! style application, blending, and content operations.

#![allow(clippy::float_cmp)] // Exact float comparison is intentional in tests

use opentui::cell::{Cell, CellContent, GraphemeId};
use opentui::color::Rgba;
use opentui::style::{Style, TextAttributes};
use opentui_rust as opentui;

// =============================================================================
// Cell Creation Tests
// =============================================================================

#[test]
fn test_cell_default() {
    // Cell::default() should create an empty cell with default colors
    let cell = Cell::default();
    assert!(cell.is_empty());
    assert_eq!(cell.display_width(), 1);
    // Default style should have transparent/default colors
    assert_eq!(cell.attributes, TextAttributes::empty());
}

#[test]
fn test_cell_with_char() {
    let cell = Cell::new('X', Style::NONE);
    assert!(matches!(cell.content, CellContent::Char('X')));
    assert_eq!(cell.display_width(), 1);
    assert!(!cell.is_empty());
    assert!(!cell.is_continuation());
}

#[test]
fn test_cell_with_style() {
    let style = Style::bold().with_attributes(TextAttributes::ITALIC);
    let cell = Cell::new('A', style);

    assert!(cell.attributes.contains(TextAttributes::BOLD));
    assert!(cell.attributes.contains(TextAttributes::ITALIC));
}

#[test]
fn test_cell_with_fg_bg() {
    let style = Style::builder().fg(Rgba::RED).bg(Rgba::BLUE).build();
    let cell = Cell::new('Z', style);

    assert_eq!(cell.fg, Rgba::RED);
    assert_eq!(cell.bg, Rgba::BLUE);
}

#[test]
fn test_cell_clear_creates_empty() {
    let cell = Cell::clear(Rgba::GREEN);
    assert!(cell.is_empty());
    assert_eq!(cell.bg, Rgba::GREEN);
    assert_eq!(cell.display_width(), 1);
}

#[test]
fn test_cell_continuation_creates_continuation() {
    let cell = Cell::continuation(Rgba::RED);
    assert!(cell.is_continuation());
    assert_eq!(cell.bg, Rgba::RED);
    assert_eq!(cell.display_width(), 0);
}

// =============================================================================
// Cell Comparison Tests
// =============================================================================

#[test]
fn test_cell_eq_same() {
    let cell1 = Cell::new('A', Style::fg(Rgba::RED));
    let cell2 = Cell::new('A', Style::fg(Rgba::RED));

    assert_eq!(cell1, cell2);
    assert!(cell1.bits_eq(&cell2));
}

#[test]
fn test_cell_eq_different_char() {
    let cell1 = Cell::new('A', Style::NONE);
    let cell2 = Cell::new('B', Style::NONE);

    assert_ne!(cell1, cell2);
    assert!(!cell1.bits_eq(&cell2));
}

#[test]
fn test_cell_eq_different_fg() {
    let cell1 = Cell::new('A', Style::fg(Rgba::RED));
    let cell2 = Cell::new('A', Style::fg(Rgba::BLUE));

    assert_ne!(cell1, cell2);
}

#[test]
fn test_cell_eq_different_bg() {
    let cell1 = Cell::new('A', Style::bg(Rgba::RED));
    let cell2 = Cell::new('A', Style::bg(Rgba::BLUE));

    assert_ne!(cell1, cell2);
}

#[test]
fn test_cell_eq_different_style() {
    let cell1 = Cell::new('A', Style::bold());
    let cell2 = Cell::new('A', Style::italic());

    assert_ne!(cell1, cell2);
    assert!(!cell1.bits_eq(&cell2));
}

#[test]
fn test_cell_eq_empty_cells() {
    let cell1 = Cell::clear(Rgba::BLACK);
    let cell2 = Cell::clear(Rgba::BLACK);

    assert_eq!(cell1, cell2);
}

#[test]
fn test_cell_eq_continuation_cells() {
    let cell1 = Cell::continuation(Rgba::WHITE);
    let cell2 = Cell::continuation(Rgba::WHITE);

    assert_eq!(cell1, cell2);
}

// =============================================================================
// Wide Character Tests
// =============================================================================

#[test]
fn test_cell_wide_char_cjk() {
    // CJK characters are typically 2 cells wide
    let cell = Cell::new('漢', Style::NONE);
    assert_eq!(cell.display_width(), 2);
    assert!(matches!(cell.content, CellContent::Char('漢')));
}

#[test]
fn test_cell_wide_char_hiragana() {
    let cell = Cell::new('あ', Style::NONE);
    assert_eq!(cell.display_width(), 2);
}

#[test]
fn test_cell_wide_char_katakana() {
    let cell = Cell::new('ア', Style::NONE);
    assert_eq!(cell.display_width(), 2);
}

#[test]
fn test_cell_wide_char_fullwidth_latin() {
    // Fullwidth Latin letters (used in CJK contexts)
    let cell = Cell::new('Ａ', Style::NONE); // U+FF21 FULLWIDTH LATIN CAPITAL LETTER A
    assert_eq!(cell.display_width(), 2);
}

#[test]
fn test_cell_halfwidth_ascii() {
    // Regular ASCII is single-width
    let cell = Cell::new('A', Style::NONE);
    assert_eq!(cell.display_width(), 1);
}

// =============================================================================
// Emoji Handling Tests
// =============================================================================

#[test]
fn test_cell_emoji_simple() {
    // Simple emoji should be width 2
    let cell = Cell::from_grapheme("🎉", Style::NONE);
    assert_eq!(cell.display_width(), 2);
}

#[test]
fn test_cell_emoji_zwj_family() {
    // ZWJ sequences like 👨‍👩‍👧 are complex graphemes
    let cell = Cell::from_grapheme("👨‍👩‍👧", Style::NONE);
    // Should be stored as grapheme since it's multi-codepoint
    assert!(matches!(cell.content, CellContent::Grapheme(_)));
    assert_eq!(cell.display_width(), 2);
}

#[test]
fn test_cell_emoji_skin_tone() {
    // Emoji with skin tone modifier
    let cell = Cell::from_grapheme("👋🏽", Style::NONE);
    assert!(matches!(cell.content, CellContent::Grapheme(_)));
    assert_eq!(cell.display_width(), 2);
}

#[test]
fn test_cell_emoji_flag() {
    // Flag emoji (regional indicators)
    let cell = Cell::from_grapheme("🇺🇸", Style::NONE);
    assert!(matches!(cell.content, CellContent::Grapheme(_)));
    assert_eq!(cell.display_width(), 2);
}

// =============================================================================
// Combining Characters Tests
// =============================================================================

#[test]
fn test_cell_combining_accent() {
    // e + combining acute accent = é as a grapheme
    let cell = Cell::from_grapheme("é", Style::NONE); // NFD form: e + ́
    // If this is precomposed (NFC), it's a single char
    // If decomposed (NFD), it's a grapheme
    assert_eq!(cell.display_width(), 1);
}

#[test]
fn test_cell_combining_diaeresis() {
    // o with combining diaeresis
    let cell = Cell::from_grapheme("ö", Style::NONE);
    assert_eq!(cell.display_width(), 1);
}

// =============================================================================
// Grapheme Storage Tests
// =============================================================================

#[test]
fn test_cell_grapheme_cluster_storage() {
    // Multi-codepoint graphemes use Grapheme variant
    let cell = Cell::from_grapheme("👨‍💻", Style::NONE); // Man technologist ZWJ

    assert!(matches!(cell.content, CellContent::Grapheme(_)));
    assert!(cell.content.is_grapheme());
    assert!(cell.content.grapheme_id().is_some());
}

#[test]
fn test_cell_single_char_optimization() {
    // Single codepoint should use Char variant, not Grapheme
    let cell = Cell::from_grapheme("A", Style::NONE);
    assert!(matches!(cell.content, CellContent::Char('A')));
    assert!(!cell.content.is_grapheme());
}

#[test]
fn test_grapheme_id_width_calculation() {
    let id = GraphemeId::new(100, 2);
    let content = CellContent::Grapheme(id);

    assert_eq!(content.display_width(), 2);
    assert_eq!(content.grapheme_id().unwrap().width(), 2);
}

// =============================================================================
// Style Application Tests
// =============================================================================

#[test]
fn test_cell_apply_style_fg() {
    let mut cell = Cell::new('A', Style::NONE);
    cell.apply_style(Style::fg(Rgba::RED));

    assert_eq!(cell.fg, Rgba::RED);
}

#[test]
fn test_cell_apply_style_bg() {
    let mut cell = Cell::new('A', Style::NONE);
    cell.apply_style(Style::bg(Rgba::BLUE));

    assert_eq!(cell.bg, Rgba::BLUE);
}

#[test]
fn test_cell_apply_style_attributes() {
    let mut cell = Cell::new('A', Style::NONE);
    cell.apply_style(Style::bold());

    assert!(cell.attributes.contains(TextAttributes::BOLD));
}

#[test]
fn test_cell_apply_style_overwrites() {
    let mut cell = Cell::new('A', Style::fg(Rgba::RED));
    cell.apply_style(Style::fg(Rgba::BLUE));

    // Should overwrite with new style's fg
    assert_eq!(cell.fg, Rgba::BLUE);
}

// =============================================================================
// Blending Tests
// =============================================================================

#[test]
fn test_cell_blend_opaque_over_transparent() {
    let bg = Cell::new('A', Style::fg(Rgba::RED));
    let fg = Cell::new('B', Style::fg(Rgba::BLUE));

    let blended = fg.blend_over(&bg);

    // Opaque foreground should completely replace
    assert!(matches!(blended.content, CellContent::Char('B')));
    assert_eq!(blended.fg, Rgba::BLUE);
}

#[test]
fn test_cell_blend_empty_preserves_background() {
    let bg = Cell::new(
        'A',
        Style::builder().fg(Rgba::RED).bg(Rgba::BLUE).bold().build(),
    );
    let fg = Cell::transparent();

    let blended = fg.blend_over(&bg);

    // Empty foreground should preserve background content
    assert!(matches!(blended.content, CellContent::Char('A')));
    assert!(blended.attributes.contains(TextAttributes::BOLD));
    assert_eq!(blended.fg, Rgba::RED);
    assert_eq!(blended.bg, Rgba::BLUE);
}

#[test]
fn test_cell_blend_with_opacity() {
    let mut cell = Cell::new('A', Style::fg(Rgba::WHITE));
    cell.blend_with_opacity(0.5);

    // Colors should be reduced by opacity
    assert!(cell.fg.a < 1.0);
}

#[test]
fn test_cell_blend_with_full_opacity() {
    let mut cell = Cell::new('A', Style::fg(Rgba::WHITE));
    let original_fg = cell.fg;
    cell.blend_with_opacity(1.0);

    // Full opacity should not change colors
    assert_eq!(cell.fg, original_fg);
}

#[test]
fn test_cell_blend_with_zero_opacity() {
    let mut cell = Cell::new('A', Style::fg(Rgba::WHITE));
    cell.blend_with_opacity(0.0);

    // Zero opacity should make colors fully transparent
    assert_eq!(cell.fg.a, 0.0);
}

// =============================================================================
// Content Access Tests
// =============================================================================

#[test]
fn test_cell_as_char() {
    let cell = Cell::new('X', Style::NONE);
    assert_eq!(cell.content.as_char(), Some('X'));

    let empty_cell = Cell::clear(Rgba::BLACK);
    assert_eq!(empty_cell.content.as_char(), None);

    let grapheme_cell = Cell::from_grapheme("👨‍👩‍👧", Style::NONE);
    assert_eq!(grapheme_cell.content.as_char(), None);
}

#[test]
fn test_cell_content_as_str_char() {
    let content = CellContent::Char('A');
    let s = content.as_str_without_pool();
    assert!(s.is_some());
    assert_eq!(s.unwrap().as_ref(), "A");
}

#[test]
fn test_cell_content_as_str_empty() {
    let content = CellContent::Empty;
    let s = content.as_str_without_pool();
    assert!(s.is_some());
    assert_eq!(s.unwrap().as_ref(), " ");
}

#[test]
fn test_cell_content_as_str_continuation() {
    let content = CellContent::Continuation;
    let s = content.as_str_without_pool();
    assert!(s.is_some());
    assert_eq!(s.unwrap().as_ref(), "");
}

#[test]
fn test_cell_content_as_str_grapheme_needs_pool() {
    let id = GraphemeId::new(42, 2);
    let content = CellContent::Grapheme(id);

    // Grapheme content requires pool lookup
    assert!(content.as_str_without_pool().is_none());
}

// =============================================================================
// Write Content Tests
// =============================================================================

#[test]
fn test_cell_write_content_char() {
    let cell = Cell::new('H', Style::NONE);
    let mut buf = Vec::new();
    cell.write_content(&mut buf).unwrap();

    assert_eq!(String::from_utf8(buf).unwrap(), "H");
}

#[test]
fn test_cell_write_content_empty() {
    let cell = Cell::clear(Rgba::BLACK);
    let mut buf = Vec::new();
    cell.write_content(&mut buf).unwrap();

    assert_eq!(String::from_utf8(buf).unwrap(), " ");
}

#[test]
fn test_cell_write_content_continuation() {
    let cell = Cell::continuation(Rgba::BLACK);
    let mut buf = Vec::new();
    cell.write_content(&mut buf).unwrap();

    assert_eq!(String::from_utf8(buf).unwrap(), "");
}

#[test]
fn test_cell_write_content_unicode() {
    let cell = Cell::new('日', Style::NONE);
    let mut buf = Vec::new();
    cell.write_content(&mut buf).unwrap();

    assert_eq!(String::from_utf8(buf).unwrap(), "日");
}

#[test]
fn test_cell_write_content_with_pool_lookup() {
    let id = GraphemeId::new(123, 2);
    let cell = Cell {
        content: CellContent::Grapheme(id),
        fg: Rgba::WHITE,
        bg: Rgba::BLACK,
        attributes: TextAttributes::empty(),
    };

    let mut buf = Vec::new();
    cell.write_content_with_pool(&mut buf, |gid| {
        if gid.pool_id() == 123 {
            Some("🎯".to_string())
        } else {
            None
        }
    })
    .unwrap();

    assert_eq!(String::from_utf8(buf).unwrap(), "🎯");
}

// =============================================================================
// Copy/Clone Tests
// =============================================================================

#[test]
fn test_cell_is_copy() {
    let cell = Cell::new('A', Style::bold());
    let cell2 = cell; // Copy
    let cell3 = cell; // Another copy

    assert_eq!(cell, cell2);
    assert_eq!(cell, cell3);
    assert_eq!(cell2, cell3);
}

#[test]
fn test_cell_content_is_copy() {
    let content = CellContent::Char('Z');
    let content2 = content; // Copy

    assert_eq!(content, content2);
}

#[test]
fn test_grapheme_id_is_copy() {
    let id = GraphemeId::new(999, 2);
    let id2 = id; // Copy

    assert_eq!(id, id2);
    assert_eq!(id.pool_id(), id2.pool_id());
    assert_eq!(id.width(), id2.width());
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_cell_nul_char() {
    let cell = Cell::new('\0', Style::NONE);
    assert!(matches!(cell.content, CellContent::Char('\0')));
    assert_eq!(cell.display_width(), 0); // NUL is zero-width
}

#[test]
fn test_cell_tab_char() {
    let cell = Cell::new('\t', Style::NONE);
    assert!(matches!(cell.content, CellContent::Char('\t')));
    // Tab width depends on position, but the cell itself is typically 0-1
}

#[test]
fn test_cell_newline_char() {
    let cell = Cell::new('\n', Style::NONE);
    assert!(matches!(cell.content, CellContent::Char('\n')));
    assert_eq!(cell.display_width(), 0); // Control chars are zero-width
}

#[test]
fn test_grapheme_id_zero_width() {
    let id = GraphemeId::new(1, 0);
    assert_eq!(id.width(), 0);
}

#[test]
fn test_grapheme_id_max_width() {
    let id = GraphemeId::new(1, 127);
    assert_eq!(id.width(), 127);
}
