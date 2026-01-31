//! Unicode normalization helpers.

use std::cmp::Ordering;
use unicode_normalization::UnicodeNormalization;

/// Normalize `text` to NFC (canonical composition).
#[must_use]
pub fn normalize_nfc(text: &str) -> String {
    text.nfc().collect()
}

/// Normalize `text` to NFD (canonical decomposition).
#[must_use]
pub fn normalize_nfd(text: &str) -> String {
    text.nfd().collect()
}

/// Check whether `text` is already NFC normalized.
#[must_use]
pub fn is_normalized_nfc(text: &str) -> bool {
    unicode_normalization::is_nfc(text)
}

/// Compare two strings after NFC normalization.
#[must_use]
pub fn compare_normalized(a: &str, b: &str) -> Ordering {
    a.nfc().cmp(b.nfc())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_nfc_combining_to_composed() {
        let input = "e\u{0301}"; // e + combining acute
        assert_eq!(normalize_nfc(input), "é");
    }

    #[test]
    fn normalize_nfc_composed_is_unchanged() {
        assert_eq!(normalize_nfc("é"), "é");
    }

    #[test]
    fn normalize_nfd_composed_to_decomposed() {
        assert_eq!(normalize_nfd("é"), "e\u{0301}");
    }

    #[test]
    fn normalize_nfd_decomposed_is_unchanged() {
        let input = "e\u{0301}";
        assert_eq!(normalize_nfd(input), input);
    }

    #[test]
    fn is_normalized_nfc_ascii_is_true() {
        assert!(is_normalized_nfc("Hello"));
    }

    #[test]
    fn is_normalized_nfc_composed_is_true() {
        assert!(is_normalized_nfc("é"));
    }

    #[test]
    fn is_normalized_nfc_decomposed_is_false() {
        assert!(!is_normalized_nfc("e\u{0301}"));
    }

    #[test]
    fn compare_normalized_equates_visually_identical_strings() {
        let a = "café";
        let b = "cafe\u{0301}";
        assert_eq!(compare_normalized(a, b), Ordering::Equal);
    }

    #[test]
    fn compare_normalized_preserves_ordering() {
        assert_eq!(compare_normalized("a", "b"), Ordering::Less);
        assert_eq!(compare_normalized("b", "a"), Ordering::Greater);
    }
}
