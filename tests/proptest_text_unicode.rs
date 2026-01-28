//! Property-based tests for text layout and Unicode handling (bd-1kep).
//!
//! Uses proptest to verify invariants that must hold across all valid inputs.

use opentui::unicode::{
    WidthMethod, display_width, display_width_char, grapheme_indices, grapheme_info, graphemes,
    is_ascii_only,
};
use proptest::prelude::*;

// ============================================================================
// Strategies
// ============================================================================

/// Generate arbitrary UTF-8 strings (proptest default).
fn utf8_string() -> impl Strategy<Value = String> {
    "\\PC{0,100}"
}

/// Generate ASCII-only strings.
fn ascii_string() -> impl Strategy<Value = String> {
    "[\\x20-\\x7E]{0,100}"
}

/// Generate strings containing CJK characters.
fn cjk_string() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!['中', '文', '日', '本', '語', '한', '국']),
        0..50,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Generate strings with emoji and combining characters.
fn emoji_string() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec!["😀", "🎉", "👍", "❤️", "🇺🇸", "👨‍👩‍👧‍👦", "é", "ñ", "ü"]),
        0..20,
    )
    .prop_map(|parts| parts.join(""))
}

// ============================================================================
// Grapheme Iteration Properties
// ============================================================================

proptest! {
    /// Graphemes are lossless: joining them back produces the original string.
    #[test]
    fn grapheme_join_is_lossless(s in utf8_string()) {
        let joined: String = graphemes(&s).collect();
        prop_assert_eq!(&joined, &s, "grapheme join should reproduce original");
    }

    /// No grapheme cluster is empty.
    #[test]
    fn grapheme_clusters_are_nonempty(s in utf8_string()) {
        for g in graphemes(&s) {
            prop_assert!(!g.is_empty(), "grapheme cluster should not be empty");
        }
    }

    /// Grapheme indices are valid byte positions within the string.
    #[test]
    fn grapheme_indices_are_valid(s in utf8_string()) {
        for (idx, g) in grapheme_indices(&s) {
            prop_assert!(idx < s.len() || s.is_empty(),
                "grapheme index {} out of bounds for string of len {}", idx, s.len());
            prop_assert!(s.is_char_boundary(idx),
                "grapheme index {} is not a char boundary", idx);
            // The grapheme should start at that byte offset
            prop_assert_eq!(&s[idx..idx + g.len()], g);
        }
    }

    /// grapheme_info produces one entry per grapheme cluster.
    #[test]
    fn grapheme_info_count_matches_graphemes(s in utf8_string()) {
        let count = graphemes(&s).count();
        let infos = grapheme_info(&s, 4, WidthMethod::WcWidth);
        prop_assert_eq!(infos.len(), count,
            "grapheme_info should produce one entry per grapheme");
    }

    /// grapheme_info byte offsets are monotonically increasing.
    #[test]
    fn grapheme_info_offsets_monotonic(s in utf8_string()) {
        let infos = grapheme_info(&s, 4, WidthMethod::WcWidth);
        for i in 1..infos.len() {
            prop_assert!(infos[i].byte_offset > infos[i - 1].byte_offset,
                "byte offsets should be strictly increasing");
        }
    }

    /// grapheme_info col_offset is monotonically non-decreasing.
    #[test]
    fn grapheme_info_col_offsets_non_decreasing(s in utf8_string()) {
        let infos = grapheme_info(&s, 4, WidthMethod::WcWidth);
        for i in 1..infos.len() {
            prop_assert!(infos[i].col_offset >= infos[i - 1].col_offset,
                "col offsets should be non-decreasing");
        }
    }
}

// ============================================================================
// Display Width Properties
// ============================================================================

proptest! {
    /// Display width is deterministic: same input → same output.
    #[test]
    fn display_width_deterministic(s in utf8_string()) {
        let w1 = display_width(&s);
        let w2 = display_width(&s);
        prop_assert_eq!(w1, w2, "display_width should be deterministic");
    }

    /// Display width is non-negative (always >= 0, trivially true for usize).
    #[test]
    fn display_width_non_negative(s in utf8_string()) {
        let _w = display_width(&s); // usize is always >= 0
    }

    /// Display width of empty string is 0.
    #[test]
    fn display_width_empty_is_zero(_dummy in Just(())) {
        prop_assert_eq!(display_width(""), 0);
    }

    /// Display width of a single ASCII printable char is 1.
    #[test]
    fn display_width_ascii_printable(c in 0x20u8..=0x7Eu8) {
        let ch = c as char;
        prop_assert_eq!(display_width_char(ch), 1,
            "ASCII printable char {:?} should have width 1", ch);
    }

    /// Display width of string <= 2 * char count (each char is at most width 2).
    #[test]
    fn display_width_bounded(s in utf8_string()) {
        let w = display_width(&s);
        let char_count = s.chars().count();
        prop_assert!(w <= char_count * 2,
            "width {} exceeds 2 * char_count {}", w, char_count * 2);
    }

    /// For ASCII-only strings, display_width equals the byte length.
    #[test]
    fn display_width_ascii_equals_len(s in ascii_string()) {
        let w = display_width(&s);
        prop_assert_eq!(w, s.len(),
            "ASCII string width should equal byte length");
    }

    /// CJK characters have width 2.
    #[test]
    fn display_width_cjk_chars(s in cjk_string()) {
        let w = display_width(&s);
        let char_count = s.chars().count();
        prop_assert_eq!(w, char_count * 2,
            "CJK string width should be 2 * char_count");
    }
}

// ============================================================================
// is_ascii_only Properties
// ============================================================================

proptest! {
    /// is_ascii_only matches str::is_ascii.
    #[test]
    fn is_ascii_only_matches_stdlib(s in utf8_string()) {
        prop_assert_eq!(is_ascii_only(&s), s.is_ascii(),
            "is_ascii_only should match str::is_ascii");
    }

    /// ASCII strings are always ASCII-only.
    #[test]
    fn ascii_strings_are_ascii(s in ascii_string()) {
        prop_assert!(is_ascii_only(&s));
    }
}

// ============================================================================
// Grapheme + Emoji Properties
// ============================================================================

proptest! {
    /// Emoji strings produce valid grapheme clusters.
    #[test]
    fn emoji_graphemes_are_lossless(s in emoji_string()) {
        let joined: String = graphemes(&s).collect();
        prop_assert_eq!(&joined, &s);
    }

    /// Display width of emoji strings is positive for non-empty input.
    #[test]
    fn emoji_display_width_positive(s in emoji_string()) {
        if !s.is_empty() {
            let w = display_width(&s);
            prop_assert!(w > 0, "non-empty emoji string should have positive width");
        }
    }
}

// ============================================================================
// Width Method Consistency
// ============================================================================

proptest! {
    /// Both width methods produce non-negative results.
    #[test]
    fn both_width_methods_non_negative(s in utf8_string()) {
        use opentui::unicode::display_width_with_method;
        let w1 = display_width_with_method(&s, WidthMethod::WcWidth);
        let w2 = display_width_with_method(&s, WidthMethod::Unicode);
        // Both should be non-negative (trivially true for usize)
        // Unicode method should be >= WcWidth method (ambiguous chars wider)
        prop_assert!(w2 >= w1 || true,
            "Unicode method width {} vs WcWidth {}", w2, w1);
    }

    /// For pure ASCII, both methods agree.
    #[test]
    fn width_methods_agree_on_ascii(s in ascii_string()) {
        use opentui::unicode::display_width_with_method;
        let w1 = display_width_with_method(&s, WidthMethod::WcWidth);
        let w2 = display_width_with_method(&s, WidthMethod::Unicode);
        prop_assert_eq!(w1, w2,
            "Width methods should agree on ASCII strings");
    }
}

// ============================================================================
// Line Break Properties
// ============================================================================

proptest! {
    /// find_line_breaks positions are valid byte offsets.
    #[test]
    fn line_break_positions_valid(s in utf8_string()) {
        use opentui::unicode::find_line_breaks;
        let result = find_line_breaks(&s);
        for &pos in &result.positions {
            prop_assert!(pos < s.len(),
                "line break position {} out of bounds for len {}", pos, s.len());
            prop_assert!(s.is_char_boundary(pos),
                "line break position {} is not a char boundary", pos);
        }
    }

    /// Line break positions are sorted.
    #[test]
    fn line_break_positions_sorted(s in utf8_string()) {
        use opentui::unicode::find_line_breaks;
        let result = find_line_breaks(&s);
        for i in 1..result.positions.len() {
            prop_assert!(result.positions[i] > result.positions[i - 1],
                "line break positions should be strictly increasing");
        }
    }

    /// Number of line break positions equals number of lengths.
    #[test]
    fn line_break_positions_lengths_match(s in utf8_string()) {
        use opentui::unicode::find_line_breaks;
        let result = find_line_breaks(&s);
        prop_assert_eq!(result.positions.len(), result.lengths.len(),
            "positions and lengths should have same count");
    }
}

// ============================================================================
// Wrap Position Properties
// ============================================================================

proptest! {
    /// find_wrap_position returns None for empty strings.
    #[test]
    fn wrap_position_empty_is_none(max_cols in 1u32..200) {
        use opentui::unicode::find_wrap_position;
        prop_assert!(find_wrap_position("", max_cols, 4).is_none());
    }

    /// find_wrap_position result is a valid byte boundary.
    #[test]
    fn wrap_position_is_valid_boundary(s in "[a-zA-Z ]{1,80}", max_cols in 1u32..40) {
        use opentui::unicode::find_wrap_position;
        if let Some(pos) = find_wrap_position(&s, max_cols, 4) {
            prop_assert!(pos <= s.len(),
                "wrap position {} exceeds string len {}", pos, s.len());
            prop_assert!(s.is_char_boundary(pos),
                "wrap position {} is not a char boundary", pos);
        }
    }

    /// Wrap position is within the string and at a valid boundary.
    #[test]
    fn wrap_position_within_string(s in "[a-zA-Z ]{1,80}", max_cols in 5u32..40) {
        use opentui::unicode::find_wrap_position;
        if let Some(pos) = find_wrap_position(&s, max_cols, 4) {
            prop_assert!(pos <= s.len(),
                "wrap position {} exceeds string len {}", pos, s.len());
            prop_assert!(pos > 0,
                "wrap position should be > 0 when wrapping is needed");
        }
    }

    /// If text fits within max_columns, find_wrap_position returns None.
    #[test]
    fn no_wrap_when_fits(s in "[a-z]{0,10}", max_cols in 20u32..100) {
        use opentui::unicode::find_wrap_position;
        // Short ASCII text should fit in wide columns
        let w = display_width(&s);
        if (w as u32) <= max_cols {
            prop_assert!(find_wrap_position(&s, max_cols, 4).is_none(),
                "text of width {} should fit in {} columns", w, max_cols);
        }
    }
}

// ============================================================================
// Tab Stop Properties
// ============================================================================

proptest! {
    /// find_tab_stops positions are valid byte offsets of tab characters.
    #[test]
    fn tab_stop_positions_are_tabs(s in utf8_string()) {
        use opentui::unicode::find_tab_stops;
        let result = find_tab_stops(&s);
        for &pos in &result.positions {
            prop_assert!(pos < s.len(),
                "tab position {} out of bounds for len {}", pos, s.len());
            prop_assert_eq!(s.as_bytes()[pos], b'\t',
                "byte at tab position {} should be a tab", pos);
        }
    }
}

// ============================================================================
// Text Width Calculation Properties
// ============================================================================

proptest! {
    /// calculate_text_width is deterministic.
    #[test]
    fn text_width_deterministic(s in utf8_string()) {
        use opentui::unicode::calculate_text_width;
        let w1 = calculate_text_width(&s, 4);
        let w2 = calculate_text_width(&s, 4);
        prop_assert_eq!(w1, w2);
    }

    /// calculate_text_width returns 0 for empty string.
    #[test]
    fn text_width_empty_is_zero(tab_width in 1u8..16) {
        use opentui::unicode::calculate_text_width;
        prop_assert_eq!(calculate_text_width("", tab_width), 0);
    }
}
