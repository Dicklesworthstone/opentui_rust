//! Display width calculation for terminal rendering.

use std::sync::atomic::{AtomicU8, Ordering};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Width calculation method for ambiguous-width characters.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WidthMethod {
    /// POSIX-like wcwidth: ambiguous width = 1.
    #[default]
    WcWidth,
    /// Unicode East Asian Width: ambiguous width = 2.
    Unicode,
}

const WIDTH_METHOD_WCWIDTH: u8 = 0;
const WIDTH_METHOD_UNICODE: u8 = 1;

static WIDTH_METHOD: AtomicU8 = AtomicU8::new(WIDTH_METHOD_WCWIDTH);

/// Set the global width method used by `display_width` helpers.
pub fn set_width_method(method: WidthMethod) {
    let value = match method {
        WidthMethod::WcWidth => WIDTH_METHOD_WCWIDTH,
        WidthMethod::Unicode => WIDTH_METHOD_UNICODE,
    };
    WIDTH_METHOD.store(value, Ordering::Relaxed);
}

/// Get the global width method.
#[must_use]
pub fn width_method() -> WidthMethod {
    match WIDTH_METHOD.load(Ordering::Relaxed) {
        WIDTH_METHOD_UNICODE => WidthMethod::Unicode,
        _ => WidthMethod::WcWidth,
    }
}

/// Get the display width of a string in terminal columns (global method).
#[must_use]
pub fn display_width(s: &str) -> usize {
    display_width_with_method(s, width_method())
}

/// Get the display width of a character in terminal columns (global method).
///
/// This includes a fast path for ASCII printable characters (0x20-0x7E)
/// which are always width 1 and are the most common case.
#[inline]
#[must_use]
pub fn display_width_char(c: char) -> usize {
    // Fast path: ASCII printable characters are always width 1
    // This covers the vast majority of terminal content
    if c.is_ascii() && (' '..='~').contains(&c) {
        return 1;
    }
    // Control characters (below space) have width 0
    if c < ' ' {
        return 0;
    }
    display_width_char_with_method(c, width_method())
}

/// Get the display width of a string in terminal columns using a specific method.
#[must_use]
pub fn display_width_with_method(s: &str, method: WidthMethod) -> usize {
    match method {
        WidthMethod::WcWidth => UnicodeWidthStr::width(s),
        WidthMethod::Unicode => UnicodeWidthStr::width_cjk(s),
    }
}

/// Get the display width of a character in terminal columns using a specific method.
#[must_use]
pub fn display_width_char_with_method(c: char, method: WidthMethod) -> usize {
    match method {
        WidthMethod::WcWidth => UnicodeWidthChar::width(c).unwrap_or(0),
        WidthMethod::Unicode => UnicodeWidthChar::width_cjk(c).unwrap_or(0),
    }
}

/// Check if a character is a zero-width character (global method).
#[must_use]
pub fn is_zero_width(c: char) -> bool {
    display_width_char(c) == 0
}

/// Check if a character is wide (takes 2 columns, global method).
#[must_use]
pub fn is_wide(c: char) -> bool {
    display_width_char(c) == 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_width() {
        assert_eq!(display_width("hello"), 5);
        assert_eq!(display_width_char('a'), 1);
    }

    #[test]
    fn test_cjk_width() {
        assert_eq!(display_width("漢字"), 4);
        assert_eq!(display_width_char('漢'), 2);
        assert!(is_wide('漢'));
    }

    #[test]
    fn test_emoji_width() {
        // Simple emoji
        assert_eq!(display_width("😀"), 2);
    }

    #[test]
    fn test_zero_width() {
        // Combining characters are zero width
        assert!(is_zero_width('\u{0301}')); // combining acute
    }

    #[test]
    fn test_width_methods() {
        // Ambiguous width character: Circled digit one (U+2460)
        // In WcWidth mode: 1, in CJK/Unicode mode: 2
        let ch = '①';
        assert_eq!(display_width_char_with_method(ch, WidthMethod::WcWidth), 1);
        assert_eq!(display_width_char_with_method(ch, WidthMethod::Unicode), 2);
    }
}
