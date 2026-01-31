//! Bidirectional (BiDi) text resolution.
//!
//! This module provides a small wrapper around the Unicode Bidirectional Algorithm
//! (UAX #9), exposing a compact [`BidiInfo`] structure that is convenient for
//! terminal rendering and text layout.

use unicode_bidi::{BidiClass, BidiInfo as UnicodeBidiInfo};

/// Base paragraph direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Ltr,
    Rtl,
    /// No strong direction could be determined.
    Neutral,
}

/// Result of resolving BiDi embedding levels for a string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BidiInfo {
    /// Detected base direction.
    pub base_direction: Direction,
    /// Embedding level per Unicode scalar value (`char`).
    pub levels: Vec<u8>,
}

/// Determine the base/paragraph direction of `text` by finding the first strong
/// directional character.
#[must_use]
pub fn get_base_direction(text: &str) -> Direction {
    for ch in text.chars() {
        match unicode_bidi::bidi_class(ch) {
            BidiClass::L => return Direction::Ltr,
            BidiClass::R | BidiClass::AL => return Direction::Rtl,
            _ => {}
        }
    }
    Direction::Neutral
}

/// Return BiDi embedding levels for each `char` in `text` (UAX #9).
///
/// Even levels are LTR, odd levels are RTL. Levels are returned per Unicode
/// scalar value (`char`), not per byte.
#[must_use]
pub fn get_bidi_embedding_levels(text: &str) -> Vec<u8> {
    if text.is_empty() {
        return Vec::new();
    }

    let bidi = UnicodeBidiInfo::new(text, None);

    let mut levels = Vec::with_capacity(text.chars().count());
    for (byte_idx, _) in text.char_indices() {
        // `unicode-bidi` stores one level per byte; the level is repeated for all
        // bytes in a multi-byte code point. Using the starting byte index yields
        // a stable per-`char` level without additional lookups.
        levels.push(bidi.levels[byte_idx].number());
    }

    levels
}

/// Resolve bidirectional embedding levels for `text` (UAX #9).
///
/// The returned `levels` are per `char` (Unicode scalar value), not per byte.
#[must_use]
pub fn resolve_bidi(text: &str) -> BidiInfo {
    BidiInfo {
        base_direction: get_base_direction(text),
        levels: get_bidi_embedding_levels(text),
    }
}

/// Reorder text from logical order into visual display order (UAX #9).
///
/// This is a convenience wrapper intended for rendering: it applies BiDi line
/// reordering and also mirrors common bracket characters in RTL runs.
///
/// Notes:
/// - Reordering is done per paragraph as detected by `unicode-bidi`.
/// - Only a small ASCII bracket subset is mirrored (`()[]{}` and `<>`).
#[must_use]
pub fn reorder_for_display(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }

    let bidi = UnicodeBidiInfo::new(text, None);

    // Fast path: no RTL content means the display order is identical.
    if !bidi.has_rtl() {
        return text.to_owned();
    }

    let mut out = String::with_capacity(text.len());

    for para in &bidi.paragraphs {
        let range = para.range.clone();

        // If this paragraph has no RTL levels, copy as-is.
        if !bidi.levels[range.clone()]
            .iter()
            .any(unicode_bidi::Level::is_rtl)
        {
            out.push_str(&text[range]);
            continue;
        }

        let (levels, runs) = bidi.visual_runs(para, range);

        for run in runs {
            if levels[run.start].is_rtl() {
                out.extend(text[run].chars().rev().map(mirror_bracket_ascii));
            } else {
                out.push_str(&text[run]);
            }
        }
    }

    out
}

#[inline]
fn mirror_bracket_ascii(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        _ => ch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_base_direction_empty_is_neutral() {
        assert_eq!(get_base_direction(""), Direction::Neutral);
    }

    #[test]
    fn get_base_direction_pure_ltr_is_ltr() {
        assert_eq!(get_base_direction("Hello"), Direction::Ltr);
    }

    #[test]
    fn get_base_direction_pure_rtl_is_rtl() {
        assert_eq!(get_base_direction("שלום"), Direction::Rtl);
    }

    #[test]
    fn get_base_direction_neutral_only_is_neutral() {
        assert_eq!(get_base_direction("123 !?"), Direction::Neutral);
    }

    #[test]
    fn get_base_direction_mixed_ltr_first_is_ltr() {
        assert_eq!(get_base_direction("Hello שלום"), Direction::Ltr);
    }

    #[test]
    fn get_base_direction_mixed_rtl_first_is_rtl() {
        assert_eq!(get_base_direction("שלום Hello"), Direction::Rtl);
    }

    #[test]
    fn get_bidi_embedding_levels_empty_is_empty() {
        assert!(get_bidi_embedding_levels("").is_empty());
    }

    #[test]
    fn get_bidi_embedding_levels_pure_ltr_levels_zero() {
        let text = "Hello, world!";
        let levels = get_bidi_embedding_levels(text);
        assert_eq!(levels.len(), text.chars().count());
        assert!(levels.iter().all(|&l| l == 0));
    }

    #[test]
    fn get_bidi_embedding_levels_pure_rtl_hebrew_levels_one() {
        let text = "שלום";
        let levels = get_bidi_embedding_levels(text);
        assert_eq!(levels.len(), text.chars().count());
        assert!(levels.iter().all(|&l| l == 1));
    }

    #[test]
    fn get_bidi_embedding_levels_mixed_contains_rtl_levels() {
        let text = "Hello שלום";
        let levels = get_bidi_embedding_levels(text);
        assert_eq!(levels.len(), text.chars().count());
        assert!(levels.contains(&1));
        assert!(levels.contains(&0));
    }

    #[test]
    fn resolve_bidi_empty_is_neutral() {
        let info = resolve_bidi("");
        assert_eq!(info.base_direction, Direction::Neutral);
        assert!(info.levels.is_empty());
    }

    #[test]
    fn resolve_bidi_pure_ltr_levels_zero() {
        let text = "Hello, world!";
        let info = resolve_bidi(text);
        assert_eq!(info.base_direction, Direction::Ltr);
        assert_eq!(info.levels.len(), text.chars().count());
        assert!(info.levels.iter().all(|&l| l == 0));
    }

    #[test]
    fn resolve_bidi_pure_rtl_hebrew_levels_one() {
        let text = "שלום";
        let info = resolve_bidi(text);
        assert_eq!(info.base_direction, Direction::Rtl);
        assert_eq!(info.levels.len(), text.chars().count());
        assert!(info.levels.iter().all(|&l| l == 1));
    }

    #[test]
    fn resolve_bidi_numbers_are_neutral_base() {
        let text = "12345";
        let info = resolve_bidi(text);
        assert_eq!(info.base_direction, Direction::Neutral);
        assert_eq!(info.levels.len(), text.chars().count());
    }

    #[test]
    fn resolve_bidi_mixed_contains_rtl_levels() {
        let text = "Hello שלום";
        let info = resolve_bidi(text);
        assert_eq!(info.base_direction, Direction::Ltr);
        assert_eq!(info.levels.len(), text.chars().count());
        assert!(info.levels.contains(&1));
        assert!(info.levels.contains(&0));
    }

    #[test]
    fn resolve_bidi_explicit_controls_do_not_panic() {
        // RLO ... PDF
        let text = "abc\u{202E}def\u{202C}ghi";
        let info = resolve_bidi(text);
        assert_eq!(info.levels.len(), text.chars().count());
    }

    // =========================================================================
    // reorder_for_display() Tests
    // =========================================================================

    #[test]
    fn reorder_for_display_empty_is_empty() {
        assert_eq!(reorder_for_display(""), "");
    }

    #[test]
    fn reorder_for_display_pure_ltr_is_identity() {
        let text = "Hello, world!";
        assert_eq!(reorder_for_display(text), text);
    }

    #[test]
    fn reorder_for_display_pure_rtl_hebrew_reverses() {
        // Logical order "שלום" → visual order should be reversed.
        assert_eq!(reorder_for_display("שלום"), "םולש");
    }

    #[test]
    fn reorder_for_display_mixed_ltr_rtl_reorders_rtl_run() {
        assert_eq!(reorder_for_display("abc אבג"), "abc גבא");
    }

    #[test]
    fn reorder_for_display_mirrors_parentheses_in_rtl_run() {
        // Without mirroring, `unicode-bidi` would produce ".ג)ב(א".
        // With mirroring applied in RTL runs, parentheses should flip.
        assert_eq!(reorder_for_display("\u{05D0}(ב)ג."), ".ג(ב)א");
    }

    #[test]
    fn reorder_for_display_mirrors_square_brackets_in_rtl_run() {
        assert_eq!(reorder_for_display("\u{05D0}[ב]ג"), "ג[ב]א");
    }

    #[test]
    fn reorder_for_display_preserves_newlines_and_reorders_each_paragraph() {
        assert_eq!(reorder_for_display("abc\nאבג"), "abc\nגבא");
    }
}
