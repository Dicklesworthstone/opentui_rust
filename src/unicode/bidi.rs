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

/// Resolve bidirectional embedding levels for `text` (UAX #9).
///
/// The returned `levels` are per `char` (Unicode scalar value), not per byte.
#[must_use]
pub fn resolve_bidi(text: &str) -> BidiInfo {
    if text.is_empty() {
        return BidiInfo {
            base_direction: Direction::Neutral,
            levels: Vec::new(),
        };
    }

    let base_direction = detect_base_direction(text);
    let bidi = UnicodeBidiInfo::new(text, None);

    let mut levels = Vec::with_capacity(text.chars().count());
    for (byte_idx, _) in text.char_indices() {
        // `unicode-bidi` stores one level per byte; the level is repeated for all
        // bytes in a multi-byte code point. Using the starting byte index yields
        // a stable per-`char` level without additional lookups.
        levels.push(bidi.levels[byte_idx].number());
    }

    BidiInfo {
        base_direction,
        levels,
    }
}

fn detect_base_direction(text: &str) -> Direction {
    for ch in text.chars() {
        match unicode_bidi::bidi_class(ch) {
            BidiClass::L => return Direction::Ltr,
            BidiClass::R | BidiClass::AL => return Direction::Rtl,
            _ => {}
        }
    }
    Direction::Neutral
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(info.levels.iter().any(|&l| l == 1));
        assert!(info.levels.iter().any(|&l| l == 0));
    }

    #[test]
    fn resolve_bidi_explicit_controls_do_not_panic() {
        // RLO ... PDF
        let text = "abc\u{202E}def\u{202C}ghi";
        let info = resolve_bidi(text);
        assert_eq!(info.levels.len(), text.chars().count());
    }
}
