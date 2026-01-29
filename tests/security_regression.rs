//! Security Regression Tests
//!
//! This module ensures that security fixes cannot regress. All tests here
//! verify critical security properties that must never be violated.
//!
//! Security areas covered:
//! - Escape sequence injection prevention (bd-er8q, bd-27qz)
//! - Paste buffer overflow protection (bd-nkgh)
//! - OSC 8 URL escaping (hyperlink injection prevention)

use opentui::ansi::escape_url_for_osc8;
use opentui::input::{InputParser, ParseError};
use opentui::terminal::Terminal;

// =============================================================================
// Escape Sequence Injection Tests (bd-er8q)
// =============================================================================

/// Test that set_title() filters C0 control characters.
#[test]
fn security_title_filters_c0_controls() {
    let mut output = Vec::new();
    {
        let mut term = Terminal::new(&mut output);
        // ESC (0x1B) - the critical injection vector
        term.set_title("Hello\x1bWorld").unwrap();
    }
    let s = String::from_utf8_lossy(&output);

    // ESC should be filtered out, not present in title content
    // The only ESC should be the OSC sequence markers
    let title_start = s.find("\x1b]0;").expect("Should have OSC prefix");
    let title_end = s.find("\x1b\\").expect("Should have ST terminator");
    let title_content = &s[title_start + 4..title_end];

    assert!(
        !title_content.contains('\x1b'),
        "Title content should not contain ESC: {title_content:?}"
    );
}

/// Test that set_title() filters BEL (0x07) which could terminate OSC early.
#[test]
fn security_title_filters_bel() {
    let mut output = Vec::new();
    {
        let mut term = Terminal::new(&mut output);
        term.set_title("Hello\x07World").unwrap();
    }
    let s = String::from_utf8_lossy(&output);

    let title_start = s.find("\x1b]0;").unwrap();
    let title_end = s.find("\x1b\\").unwrap();
    let title_content = &s[title_start + 4..title_end];

    assert!(
        !title_content.contains('\x07'),
        "Title content should not contain BEL"
    );
}

/// Test that set_title() filters C1 control characters (U+0080-U+009F).
/// These are critical because they can inject terminal commands:
/// - U+009B (CSI) - equivalent to ESC [
/// - U+009C (ST) - String Terminator
/// - U+009D (OSC) - equivalent to ESC ]
#[test]
fn security_title_filters_c1_controls() {
    let mut output = Vec::new();
    {
        let mut term = Terminal::new(&mut output);
        // CSI (U+009B) - could inject commands like "clear screen"
        term.set_title("Hello\u{009B}2JWorld").unwrap();
    }
    let s = String::from_utf8_lossy(&output);

    let title_start = s.find("\x1b]0;").unwrap();
    let title_end = s.find("\x1b\\").unwrap();
    let title_content = &s[title_start + 4..title_end];

    assert!(
        !title_content.contains('\u{009B}'),
        "Title should not contain CSI (U+009B)"
    );
}

/// Test that set_title() filters OSC control (U+009D).
#[test]
fn security_title_filters_osc_c1() {
    let mut output = Vec::new();
    {
        let mut term = Terminal::new(&mut output);
        // OSC (U+009D) - could start new OSC sequence
        term.set_title("Hello\u{009D}0;EvilTitle\u{009C}World")
            .unwrap();
    }
    let s = String::from_utf8_lossy(&output);

    let title_start = s.find("\x1b]0;").unwrap();
    let title_end = s.find("\x1b\\").unwrap();
    let title_content = &s[title_start + 4..title_end];

    assert!(
        !title_content.contains('\u{009D}'),
        "Title should not contain OSC (U+009D)"
    );
    assert!(
        !title_content.contains('\u{009C}'),
        "Title should not contain ST (U+009C)"
    );
}

/// Test that set_title() preserves normal Unicode (non-control) characters.
#[test]
fn security_title_preserves_normal_unicode() {
    let mut output = Vec::new();
    {
        let mut term = Terminal::new(&mut output);
        term.set_title("日本語タイトル 🎉 Émojis").unwrap();
    }
    let s = String::from_utf8_lossy(&output);

    assert!(s.contains("日本語タイトル"), "Japanese should be preserved");
    assert!(s.contains("🎉"), "Emoji should be preserved");
    assert!(s.contains("Émojis"), "Accented chars should be preserved");
}

// =============================================================================
// OSC 8 Hyperlink Injection Tests (bd-27qz)
// =============================================================================

/// Test that escape_url_for_osc8() escapes ESC character.
#[test]
fn security_osc8_escapes_esc() {
    let malicious = "http://evil.com/\x1b]0;Pwned\x1b\\";
    let escaped = escape_url_for_osc8(malicious);

    assert!(!escaped.contains('\x1b'), "ESC should be percent-encoded");
    assert!(escaped.contains("%1B"), "ESC should become %1B");
}

/// Test that escape_url_for_osc8() escapes BEL character.
#[test]
fn security_osc8_escapes_bel() {
    let malicious = "http://evil.com/\x07";
    let escaped = escape_url_for_osc8(malicious);

    assert!(!escaped.contains('\x07'), "BEL should be percent-encoded");
    assert!(escaped.contains("%07"), "BEL should become %07");
}

/// Test that escape_url_for_osc8() escapes C1 controls.
#[test]
fn security_osc8_escapes_c1_controls() {
    // CSI (U+009B) - could inject terminal commands
    let url_with_csi = "http://evil.com/\u{009B}2J";
    let escaped = escape_url_for_osc8(url_with_csi);
    assert!(
        !escaped.contains('\u{009B}'),
        "CSI should be percent-encoded"
    );

    // ST (U+009C) - could terminate OSC early
    let url_with_st = "http://evil.com/\u{009C}";
    let escaped = escape_url_for_osc8(url_with_st);
    assert!(
        !escaped.contains('\u{009C}'),
        "ST should be percent-encoded"
    );

    // OSC (U+009D) - could start new command
    let url_with_osc = "http://evil.com/\u{009D}";
    let escaped = escape_url_for_osc8(url_with_osc);
    assert!(
        !escaped.contains('\u{009D}'),
        "OSC should be percent-encoded"
    );
}

/// Test that escape_url_for_osc8() handles a realistic injection attempt.
#[test]
fn security_osc8_injection_attempt() {
    // Attacker tries to:
    // 1. End the current OSC 8 with ST (ESC \)
    // 2. Clear the screen with CSI 2 J
    // 3. Start a new fake title
    let malicious = "http://x\x1b\\\x1b[2J\x1b]0;Pwned";
    let escaped = escape_url_for_osc8(malicious);

    // Count ESC characters - should be zero (all escaped)
    let esc_count = escaped.chars().filter(|&c| c == '\x1b').count();
    assert_eq!(esc_count, 0, "All ESC should be percent-encoded");

    // The escaped URL should be safe to include in OSC 8
    assert!(escaped.contains("%1B"), "ESC should be %1B");
}

/// Test that escape_url_for_osc8() preserves valid URL characters.
#[test]
fn security_osc8_preserves_valid_urls() {
    let valid_url = "https://example.com/path?query=value&other=123#anchor";
    let escaped = escape_url_for_osc8(valid_url);
    assert_eq!(escaped, valid_url, "Valid URL should not be modified");
}

/// Test that escape_url_for_osc8() preserves Unicode in URLs.
#[test]
fn security_osc8_preserves_unicode() {
    let unicode_url = "https://example.com/路径/файл?q=日本語";
    let escaped = escape_url_for_osc8(unicode_url);
    assert_eq!(escaped, unicode_url, "Unicode should be preserved");
}

// =============================================================================
// Paste Buffer Overflow Tests (bd-nkgh)
// =============================================================================

/// Maximum paste buffer size (must match the constant in parser.rs).
const MAX_PASTE_SIZE: usize = 10 * 1024 * 1024; // 10 MB

/// Test that paste overflow returns an error instead of silently truncating.
#[test]
fn security_paste_overflow_returns_error() {
    let mut parser = InputParser::new();

    // Start bracketed paste
    let start_paste = b"\x1b[200~";
    let result = parser.parse(start_paste);
    assert!(result.is_err()); // Incomplete, waiting for more data

    // Send data that exceeds the limit
    let large_chunk = vec![b'x'; MAX_PASTE_SIZE + 100];
    let result = parser.parse(&large_chunk);

    assert!(
        matches!(result, Err(ParseError::PasteBufferOverflow)),
        "Should return PasteBufferOverflow error, got: {result:?}"
    );
}

/// Test that paste overflow resets parser state for next paste.
#[test]
fn security_paste_overflow_resets_state() {
    let mut parser = InputParser::new();

    // Trigger overflow
    let _ = parser.parse(b"\x1b[200~");
    let large_chunk = vec![b'x'; MAX_PASTE_SIZE + 100];
    let overflow_result = parser.parse(&large_chunk);
    assert!(matches!(
        overflow_result,
        Err(ParseError::PasteBufferOverflow)
    ));

    // After overflow, parser should be reset. Start a new paste.
    // First, we need to enter paste mode again
    let start_result = parser.parse(b"\x1b[200~");
    assert!(
        matches!(start_result, Err(ParseError::Incomplete)),
        "Should enter paste mode: {start_result:?}"
    );

    // Then send content with end sequence
    let content_result = parser.parse(b"hello\x1b[201~");
    assert!(
        content_result.is_ok(),
        "Parser should accept normal paste after overflow: {content_result:?}"
    );
}

/// Test that incremental paste accumulation respects the limit.
#[test]
fn security_paste_incremental_overflow() {
    let mut parser = InputParser::new();

    // Start paste
    let _ = parser.parse(b"\x1b[200~");

    // Send chunks that together exceed the limit
    let chunk_size = MAX_PASTE_SIZE / 4;
    let chunk = vec![b'a'; chunk_size];

    // First 4 chunks should be fine (total = MAX_PASTE_SIZE)
    for i in 0..4 {
        let result = parser.parse(&chunk);
        assert!(
            matches!(result, Err(ParseError::Incomplete)),
            "Chunk {i} should return Incomplete"
        );
    }

    // 5th chunk should trigger overflow
    let result = parser.parse(&chunk);
    assert!(
        matches!(result, Err(ParseError::PasteBufferOverflow)),
        "5th chunk should trigger overflow: {result:?}"
    );
}

/// Test that paste exactly at limit succeeds.
#[test]
fn security_paste_at_limit_succeeds() {
    let mut parser = InputParser::new();

    // Enter paste mode first
    let _ = parser.parse(b"\x1b[200~");

    // Create paste content exactly at limit with end sequence
    let content = vec![b'x'; MAX_PASTE_SIZE];
    let mut paste_data = content;
    paste_data.extend_from_slice(b"\x1b[201~");

    let result = parser.parse(&paste_data);
    assert!(
        result.is_ok(),
        "Paste exactly at limit should succeed: {result:?}"
    );
}

// =============================================================================
// Combined Security Scenarios
// =============================================================================

/// Test that a multi-vector attack is properly defended.
#[test]
fn security_combined_attack_vectors() {
    // This tests a hypothetical attacker who:
    // 1. Tries C1 injection in title
    // 2. Tries ESC injection in hyperlink URL
    // 3. Tries to overflow paste buffer

    // 1. Title with C1 injection attempt
    let mut output = Vec::new();
    {
        let mut term = Terminal::new(&mut output);
        term.set_title("Safe\u{009B}[2J\u{009D}Evil").unwrap();
    }
    let title_output = String::from_utf8_lossy(&output);
    assert!(
        !title_output.contains('\u{009B}'),
        "C1 injection in title blocked"
    );

    // 2. Hyperlink with ESC injection attempt
    let malicious_url = "http://x\x1b]0;Pwned\x1b\\click";
    let escaped = escape_url_for_osc8(malicious_url);
    assert!(!escaped.contains('\x1b'), "ESC injection in URL blocked");

    // 3. Paste overflow attempt
    let mut parser = InputParser::new();
    let _ = parser.parse(b"\x1b[200~");
    let overflow = vec![b'x'; MAX_PASTE_SIZE + 1];
    let result = parser.parse(&overflow);
    assert!(
        matches!(result, Err(ParseError::PasteBufferOverflow)),
        "Paste overflow blocked"
    );
}
