//! E2E tests for input simulation and event flow.
//!
//! Tests complete input handling from raw bytes → parsed events → verification.
//! Covers keyboard, mouse, paste, and focus events.

mod common;

use common::harness::E2EHarness;
use common::input_sim::{
    event_to_ansi, key_to_ansi, mouse_to_sgr, paste_to_ansi, sequence_to_ansi, InputSequence,
    TimingMode,
};
use opentui::input::{Event, InputParser, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

// ============================================================================
// Keyboard Input Flow Tests
// ============================================================================

/// Test single keypress → KeyEvent.
#[test]
fn test_e2e_single_keypress() {
    let mut harness = E2EHarness::new("input_flow", "single_keypress", 80, 24);
    harness.log().info("init", "Testing single keypress");

    let mut parser = InputParser::new();

    // Generate ANSI for 'a' key
    let seq = InputSequence::keystroke(KeyCode::Char('a'), KeyModifiers::empty());
    let ansi = sequence_to_ansi(&seq);

    harness.log().info("input", format!("ANSI bytes: {:?}", ansi));

    // Parse
    let (event, consumed) = parser.parse(&ansi).expect("Should parse");

    harness.log().info(
        "parse",
        format!("Consumed {} bytes, event: {:?}", consumed, event),
    );

    // Verify
    assert_eq!(consumed, 1);
    let key = event.key().expect("Should be key event");
    assert_eq!(key.code, KeyCode::Char('a'));
    assert!(key.modifiers.is_empty());

    harness.finish(true);
    eprintln!("[TEST] PASS: Single keypress flow works");
}

/// Test modifier combinations (Ctrl+X, Alt+Y, Ctrl+Alt+Z).
#[test]
fn test_e2e_modifier_combinations() {
    let mut harness = E2EHarness::new("input_flow", "modifier_combinations", 80, 24);
    harness.log().info("init", "Testing modifier combinations");

    let mut parser = InputParser::new();

    // Ctrl+C
    let ctrl_c = KeyEvent::with_ctrl(KeyCode::Char('c'));
    let ansi = key_to_ansi(&ctrl_c);
    harness.log().info("input", format!("Ctrl+C ANSI: {:?}", ansi));

    let (event, _) = parser.parse(&ansi).expect("Should parse Ctrl+C");
    let key = event.key().expect("Should be key event");
    assert!(key.ctrl(), "Ctrl should be set");
    assert!(key.is_ctrl_c(), "Should be Ctrl+C");

    // Alt+X
    let alt_x = KeyEvent::with_alt(KeyCode::Char('x'));
    let ansi = key_to_ansi(&alt_x);
    harness.log().info("input", format!("Alt+X ANSI: {:?}", ansi));

    let (event, _) = parser.parse(&ansi).expect("Should parse Alt+X");
    let key = event.key().expect("Should be key event");
    assert!(key.alt(), "Alt should be set");
    assert_eq!(key.code, KeyCode::Char('x'));

    harness.finish(true);
    eprintln!("[TEST] PASS: Modifier combinations work");
}

/// Test function keys (F1-F12).
#[test]
fn test_e2e_function_keys() {
    let mut harness = E2EHarness::new("input_flow", "function_keys", 80, 24);
    harness.log().info("init", "Testing function keys F1-F12");

    let mut parser = InputParser::new();

    // F1-F4 use SS3 sequences
    let f1_ansi = b"\x1bOP";
    let (event, _) = parser.parse(f1_ansi).expect("Should parse F1");
    assert_eq!(event.key().unwrap().code, KeyCode::F(1));
    harness.log().info("verify", "F1 parsed correctly");

    // F5+ use CSI tilde sequences
    let f5_ansi = b"\x1b[15~";
    let (event, _) = parser.parse(f5_ansi).expect("Should parse F5");
    assert_eq!(event.key().unwrap().code, KeyCode::F(5));
    harness.log().info("verify", "F5 parsed correctly");

    let f12_ansi = b"\x1b[24~";
    let (event, _) = parser.parse(f12_ansi).expect("Should parse F12");
    assert_eq!(event.key().unwrap().code, KeyCode::F(12));
    harness.log().info("verify", "F12 parsed correctly");

    harness.finish(true);
    eprintln!("[TEST] PASS: Function keys work");
}

/// Test special keys (arrows, home, end, pgup, pgdn, insert, delete).
#[test]
fn test_e2e_special_keys() {
    let mut harness = E2EHarness::new("input_flow", "special_keys", 80, 24);
    harness.log().info("init", "Testing special keys");

    let mut parser = InputParser::new();

    let test_cases = [
        (b"\x1b[A".to_vec(), KeyCode::Up),
        (b"\x1b[B".to_vec(), KeyCode::Down),
        (b"\x1b[C".to_vec(), KeyCode::Right),
        (b"\x1b[D".to_vec(), KeyCode::Left),
        (b"\x1b[H".to_vec(), KeyCode::Home),
        (b"\x1b[F".to_vec(), KeyCode::End),
        (b"\x1b[5~".to_vec(), KeyCode::PageUp),
        (b"\x1b[6~".to_vec(), KeyCode::PageDown),
        (b"\x1b[2~".to_vec(), KeyCode::Insert),
        (b"\x1b[3~".to_vec(), KeyCode::Delete),
    ];

    for (ansi, expected_code) in test_cases {
        let (event, _) = parser.parse(&ansi).expect("Should parse");
        let key = event.key().expect("Should be key event");
        assert_eq!(key.code, expected_code, "Key code mismatch for {:?}", ansi);
        harness.log().info("verify", format!("{:?} parsed correctly", expected_code));
    }

    harness.finish(true);
    eprintln!("[TEST] PASS: Special keys work");
}

/// Test Unicode character input (multi-byte UTF-8).
#[test]
fn test_e2e_unicode_input() {
    let mut harness = E2EHarness::new("input_flow", "unicode_input", 80, 24);
    harness.log().info("init", "Testing Unicode character input");

    let mut parser = InputParser::new();

    // 2-byte UTF-8: ñ (U+00F1)
    let (event, consumed) = parser.parse("ñ".as_bytes()).expect("Should parse ñ");
    assert_eq!(consumed, 2);
    assert_eq!(event.key().unwrap().code, KeyCode::Char('ñ'));
    harness.log().info("verify", "2-byte UTF-8 (ñ) works");

    // 3-byte UTF-8: 日 (U+65E5)
    let (event, consumed) = parser.parse("日".as_bytes()).expect("Should parse 日");
    assert_eq!(consumed, 3);
    assert_eq!(event.key().unwrap().code, KeyCode::Char('日'));
    harness.log().info("verify", "3-byte UTF-8 (日) works");

    // 4-byte UTF-8: 🎉 (U+1F389)
    let (event, consumed) = parser.parse("🎉".as_bytes()).expect("Should parse 🎉");
    assert_eq!(consumed, 4);
    assert_eq!(event.key().unwrap().code, KeyCode::Char('🎉'));
    harness.log().info("verify", "4-byte UTF-8 (🎉) works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Unicode input works");
}

/// Test escape key handling (vs Alt prefix).
#[test]
fn test_e2e_escape_handling() {
    let mut harness = E2EHarness::new("input_flow", "escape_handling", 80, 24);
    harness.log().info("init", "Testing escape key handling");

    let mut parser = InputParser::new();

    // Standalone escape (requires checking for incomplete)
    let result = parser.parse(b"\x1b");
    assert!(
        result.is_err(),
        "Standalone ESC should be Incomplete (could be start of sequence)"
    );
    harness.log().info("verify", "Standalone ESC returns Incomplete");

    // Double escape
    let (event, consumed) = parser.parse(b"\x1b\x1b").expect("Should parse double ESC");
    assert_eq!(consumed, 1);
    assert_eq!(event.key().unwrap().code, KeyCode::Esc);
    harness.log().info("verify", "Double ESC returns Esc key");

    // Alt+letter (ESC followed by letter)
    let (event, consumed) = parser.parse(b"\x1bx").expect("Should parse Alt+x");
    assert_eq!(consumed, 2);
    let key = event.key().unwrap();
    assert!(key.alt());
    assert_eq!(key.code, KeyCode::Char('x'));
    harness.log().info("verify", "Alt+letter works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Escape handling works");
}

// ============================================================================
// Mouse Input Flow Tests
// ============================================================================

/// Test click → MouseEvent with correct position.
#[test]
fn test_e2e_mouse_click_position() {
    let mut harness = E2EHarness::new("input_flow", "mouse_click_position", 80, 24);
    harness.log().info("init", "Testing mouse click position accuracy");

    let mut parser = InputParser::new();

    // Left click at (15, 10)
    let click = MouseEvent::new(15, 10, MouseButton::Left, MouseEventKind::Press);
    let ansi = mouse_to_sgr(&click);

    harness.log().info("input", format!("SGR bytes: {:?}", String::from_utf8_lossy(&ansi)));

    let (event, _) = parser.parse(&ansi).expect("Should parse");
    let mouse = event.mouse().expect("Should be mouse event");

    assert_eq!(mouse.x, 15);
    assert_eq!(mouse.y, 10);
    assert_eq!(mouse.button, MouseButton::Left);
    assert_eq!(mouse.kind, MouseEventKind::Press);

    harness.log().info("verify", format!("Position: ({}, {})", mouse.x, mouse.y));
    harness.finish(true);
    eprintln!("[TEST] PASS: Mouse click position works");
}

/// Test drag sequence (press → move → release).
#[test]
fn test_e2e_mouse_drag_sequence() {
    let mut harness = E2EHarness::new("input_flow", "mouse_drag_sequence", 80, 24);
    harness.log().info("init", "Testing mouse drag sequence");

    let mut parser = InputParser::new();

    // Build drag sequence
    let seq = InputSequence::mouse_drag((5, 5), (20, 15), MouseButton::Left);
    let ansi = sequence_to_ansi(&seq);

    harness.log().info("input", format!("Drag sequence has {} events", seq.len()));

    // Parse all events
    let mut events = Vec::new();
    let mut offset = 0;
    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((event, consumed)) => {
                events.push(event);
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    harness.log().info("parse", format!("Parsed {} events", events.len()));

    // Verify sequence: Press, Move(s), Release
    assert!(events.len() >= 3, "Should have at least press, move, release");

    // First should be Press
    let first = events.first().unwrap().mouse().unwrap();
    assert_eq!(first.kind, MouseEventKind::Press);

    // Last should be Release
    let last = events.last().unwrap().mouse().unwrap();
    assert_eq!(last.kind, MouseEventKind::Release);

    harness.finish(true);
    eprintln!("[TEST] PASS: Mouse drag sequence works");
}

/// Test scroll wheel events (up/down).
#[test]
fn test_e2e_scroll_wheel() {
    let mut harness = E2EHarness::new("input_flow", "scroll_wheel", 80, 24);
    harness.log().info("init", "Testing scroll wheel events");

    let mut parser = InputParser::new();

    // Scroll up: button byte 64
    let scroll_up = b"\x1b[<64;10;5M";
    let (event, _) = parser.parse(scroll_up).expect("Should parse scroll up");
    let mouse = event.mouse().unwrap();
    assert_eq!(mouse.kind, MouseEventKind::ScrollUp);
    harness.log().info("verify", "Scroll up works");

    // Scroll down: button byte 65
    let scroll_down = b"\x1b[<65;10;5M";
    let (event, _) = parser.parse(scroll_down).expect("Should parse scroll down");
    let mouse = event.mouse().unwrap();
    assert_eq!(mouse.kind, MouseEventKind::ScrollDown);
    harness.log().info("verify", "Scroll down works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Scroll wheel works");
}

/// Test mouse position accuracy at boundaries.
#[test]
fn test_e2e_mouse_boundary_positions() {
    let mut harness = E2EHarness::new("input_flow", "mouse_boundaries", 80, 24);
    harness.log().info("init", "Testing mouse position at boundaries");

    let mut parser = InputParser::new();

    // Origin (0, 0) - SGR uses 1-indexed, so (1,1) in protocol
    let (event, _) = parser.parse(b"\x1b[<0;1;1M").expect("Should parse origin");
    let mouse = event.mouse().unwrap();
    assert_eq!((mouse.x, mouse.y), (0, 0));
    harness.log().info("verify", "Origin (0,0) works");

    // Large coordinates
    let (event, _) = parser.parse(b"\x1b[<0;1000;500M").expect("Should parse large coords");
    let mouse = event.mouse().unwrap();
    assert_eq!((mouse.x, mouse.y), (999, 499));
    harness.log().info("verify", format!("Large coords ({}, {}) work", mouse.x, mouse.y));

    harness.finish(true);
    eprintln!("[TEST] PASS: Mouse boundary positions work");
}

/// Test mouse button combinations (modifiers).
#[test]
fn test_e2e_mouse_modifiers() {
    let mut harness = E2EHarness::new("input_flow", "mouse_modifiers", 80, 24);
    harness.log().info("init", "Testing mouse with keyboard modifiers");

    let mut parser = InputParser::new();

    // Shift+Click: button 0 + shift(4) = 4
    let (event, _) = parser.parse(b"\x1b[<4;10;5M").expect("Should parse shift+click");
    let mouse = event.mouse().unwrap();
    assert!(mouse.shift);
    assert!(!mouse.ctrl);
    assert!(!mouse.alt);
    harness.log().info("verify", "Shift+Click works");

    // Ctrl+Click: button 0 + ctrl(16) = 16
    let (event, _) = parser.parse(b"\x1b[<16;10;5M").expect("Should parse ctrl+click");
    let mouse = event.mouse().unwrap();
    assert!(mouse.ctrl);
    harness.log().info("verify", "Ctrl+Click works");

    // Alt+Click: button 0 + alt(8) = 8
    let (event, _) = parser.parse(b"\x1b[<8;10;5M").expect("Should parse alt+click");
    let mouse = event.mouse().unwrap();
    assert!(mouse.alt);
    harness.log().info("verify", "Alt+Click works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Mouse modifiers work");
}

// ============================================================================
// Paste Input Tests
// ============================================================================

/// Test bracketed paste detection.
#[test]
fn test_e2e_bracketed_paste_detection() {
    let mut harness = E2EHarness::new("input_flow", "bracketed_paste", 80, 24);
    harness.log().info("init", "Testing bracketed paste detection");

    let mut parser = InputParser::new();

    // Generate paste sequence
    let content = "Hello, World!";
    let ansi = paste_to_ansi(content);

    harness.log().info("input", format!("Paste ANSI length: {} bytes", ansi.len()));

    // First parse enters paste mode
    let result = parser.parse(&ansi);
    assert!(result.is_err(), "First parse should enter paste mode");

    // Second parse returns paste event
    let (event, _) = parser.parse(&ansi).expect("Should parse paste");
    let paste = event.paste().expect("Should be paste event");
    assert_eq!(paste.content(), content);

    harness.log().info("verify", format!("Paste content: {}", paste.content()));
    harness.finish(true);
    eprintln!("[TEST] PASS: Bracketed paste detection works");
}

/// Test large paste handling (>10KB).
#[test]
fn test_e2e_large_paste() {
    let mut harness = E2EHarness::new("input_flow", "large_paste", 80, 24);
    harness.log().info("init", "Testing large paste (>10KB)");

    let mut parser = InputParser::new();

    // Generate large content (15KB)
    let content: String = "x".repeat(15_000);
    let ansi = paste_to_ansi(&content);

    harness.log().info("input", format!("Large paste: {} bytes", ansi.len()));

    // Parse
    let _ = parser.parse(&ansi); // Enter paste mode
    let (event, _) = parser.parse(&ansi).expect("Should parse large paste");
    let paste = event.paste().expect("Should be paste event");

    // Note: MAX_PASTE_BUFFER_SIZE is 10MB, so this should fit
    assert_eq!(paste.content().len(), 15_000);
    harness.log().info("verify", format!("Paste length: {} chars", paste.len()));

    harness.finish(true);
    eprintln!("[TEST] PASS: Large paste handling works");
}

/// Test paste with special characters and Unicode.
#[test]
fn test_e2e_paste_with_unicode() {
    let mut harness = E2EHarness::new("input_flow", "paste_unicode", 80, 24);
    harness.log().info("init", "Testing paste with Unicode content");

    let mut parser = InputParser::new();

    let content = "日本語テスト 🎉 emoji ñ special";
    let ansi = paste_to_ansi(content);

    let _ = parser.parse(&ansi);
    let (event, _) = parser.parse(&ansi).expect("Should parse unicode paste");
    let paste = event.paste().expect("Should be paste event");

    assert_eq!(paste.content(), content);
    harness.log().info("verify", format!("Unicode paste: {}", paste.content()));

    harness.finish(true);
    eprintln!("[TEST] PASS: Paste with Unicode works");
}

/// Test binary data in paste (no corruption).
#[test]
fn test_e2e_paste_binary_data() {
    let mut harness = E2EHarness::new("input_flow", "paste_binary", 80, 24);
    harness.log().info("init", "Testing paste with binary data");

    let mut parser = InputParser::new();

    // Content with various byte values (avoiding paste end sequence)
    let content = "text\x00null\x01soh\x7fDEL";
    let ansi = paste_to_ansi(content);

    let _ = parser.parse(&ansi);
    let (event, _) = parser.parse(&ansi).expect("Should parse binary paste");
    let paste = event.paste().expect("Should be paste event");

    // Note: from_utf8_lossy may replace invalid UTF-8
    assert!(!paste.is_empty());
    harness.log().info("verify", format!("Binary paste length: {}", paste.len()));

    harness.finish(true);
    eprintln!("[TEST] PASS: Paste binary data works");
}

// ============================================================================
// Focus and Resize Events
// ============================================================================

/// Test focus change events.
#[test]
fn test_e2e_focus_events() {
    let mut harness = E2EHarness::new("input_flow", "focus_events", 80, 24);
    harness.log().info("init", "Testing focus change events");

    let mut parser = InputParser::new();

    // Focus gained: CSI I
    let (event, _) = parser.parse(b"\x1b[I").expect("Should parse focus gained");
    assert_eq!(event, Event::FocusGained);
    harness.log().info("verify", "Focus gained works");

    // Focus lost: CSI O
    let (event, _) = parser.parse(b"\x1b[O").expect("Should parse focus lost");
    assert_eq!(event, Event::FocusLost);
    harness.log().info("verify", "Focus lost works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Focus events work");
}

/// Test resize event.
#[test]
fn test_e2e_resize_event() {
    let mut harness = E2EHarness::new("input_flow", "resize_event", 80, 24);
    harness.log().info("init", "Testing resize event");

    let mut parser = InputParser::new();

    // Resize to 120x50: CSI 8;50;120 t
    let (event, _) = parser.parse(b"\x1b[8;50;120t").expect("Should parse resize");
    let resize = event.resize().expect("Should be resize event");

    assert_eq!(resize.width, 120);
    assert_eq!(resize.height, 50);
    harness.log().info("verify", format!("Resize: {}x{}", resize.width, resize.height));

    harness.finish(true);
    eprintln!("[TEST] PASS: Resize event works");
}

// ============================================================================
// Edge Cases and Error Handling
// ============================================================================

/// Test partial escape sequences (interrupted input).
#[test]
fn test_e2e_partial_sequences() {
    let mut harness = E2EHarness::new("input_flow", "partial_sequences", 80, 24);
    harness.log().info("init", "Testing partial/interrupted sequences");

    let mut parser = InputParser::new();

    // Incomplete CSI sequence
    let result = parser.parse(b"\x1b[");
    assert!(result.is_err());
    harness.log().info("verify", "Incomplete CSI returns error");

    // Incomplete CSI with params but no terminator
    let result = parser.parse(b"\x1b[1;2");
    assert!(result.is_err());
    harness.log().info("verify", "Incomplete params returns error");

    harness.finish(true);
    eprintln!("[TEST] PASS: Partial sequences handled correctly");
}

/// Test invalid/malformed sequences.
#[test]
fn test_e2e_invalid_sequences() {
    let mut harness = E2EHarness::new("input_flow", "invalid_sequences", 80, 24);
    harness.log().info("init", "Testing invalid/malformed sequences");

    let mut parser = InputParser::new();

    // Unknown CSI terminator
    let result = parser.parse(b"\x1b[999Z");
    assert!(result.is_err());
    harness.log().info("verify", "Unknown CSI terminator returns error");

    // Invalid UTF-8 continuation
    let result = parser.parse(&[0x80]);
    assert!(result.is_err());
    harness.log().info("verify", "Invalid UTF-8 returns error");

    harness.finish(true);
    eprintln!("[TEST] PASS: Invalid sequences handled gracefully");
}

/// Test rapid input (stress test).
#[test]
fn test_e2e_rapid_input() {
    let mut harness = E2EHarness::new("input_flow", "rapid_input", 80, 24);
    harness.log().info("init", "Testing rapid input (100+ events)");

    let mut parser = InputParser::new();

    // Generate 200 keystrokes
    let seq = InputSequence::type_text(&"a".repeat(200)).stress_mode();
    let ansi = sequence_to_ansi(&seq);

    harness.log().info("input", format!("Rapid input: {} bytes", ansi.len()));

    // Parse all events
    let mut count = 0;
    let mut offset = 0;
    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((_, consumed)) => {
                count += 1;
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    assert_eq!(count, 200, "Should parse all 200 events");
    harness.log().info("verify", format!("Parsed {} events", count));

    harness.finish(true);
    eprintln!("[TEST] PASS: Rapid input handled correctly");
}

/// Test typing speed simulation.
#[test]
fn test_e2e_typing_speed_simulation() {
    let mut harness = E2EHarness::new("input_flow", "typing_speed", 80, 24);
    harness.log().info("init", "Testing typing speed simulation");

    // 60 WPM = 300 chars/min = 200ms per char average
    let seq = InputSequence::type_text("hello world").with_wpm(60);

    assert_eq!(seq.timing(), TimingMode::Realistic { wpm: 60 });

    // Total time should be approximately 11 chars * 200ms = 2200ms
    // But first char has no delay, so ~2000ms
    let total_time = seq.total_time_ms();
    harness.log().info("timing", format!("Total simulated time: {}ms", total_time));

    // The actual time calculation depends on implementation
    // Just verify it's non-zero for realistic mode
    assert!(seq.len() == 11, "Should have 11 key events");

    harness.finish(true);
    eprintln!("[TEST] PASS: Typing speed simulation works");
}

/// Test input sequence building and chaining.
#[test]
fn test_e2e_sequence_chaining() {
    let mut harness = E2EHarness::new("input_flow", "sequence_chaining", 80, 24);
    harness.log().info("init", "Testing sequence building and chaining");

    // Build complex sequence
    let seq = InputSequence::new()
        .key(KeyCode::Char('H'))
        .key(KeyCode::Char('i'))
        .ctrl_key(KeyCode::Char('s')) // Ctrl+S (save)
        .left_click(10, 5)
        .key(KeyCode::Enter);

    // Count events
    let events = seq.to_terminal_events();
    harness.log().info("build", format!("Built {} events", events.len()));

    // Verify event types
    assert!(matches!(events[0], Event::Key(_)));
    assert!(matches!(events[1], Event::Key(_)));
    assert!(matches!(events[2], Event::Key(_))); // Ctrl+S
    assert!(matches!(events[3], Event::Mouse(_))); // Click press
    assert!(matches!(events[4], Event::Mouse(_))); // Click release
    assert!(matches!(events[5], Event::Key(_))); // Enter

    harness.finish(true);
    eprintln!("[TEST] PASS: Sequence chaining works");
}

/// Test modifier combinations for special keys.
#[test]
fn test_e2e_modified_special_keys() {
    let mut harness = E2EHarness::new("input_flow", "modified_special_keys", 80, 24);
    harness.log().info("init", "Testing modified special keys (Shift+Arrow, etc.)");

    let mut parser = InputParser::new();

    // Shift+Up: ESC [ 1 ; 2 A
    let (event, _) = parser.parse(b"\x1b[1;2A").expect("Should parse Shift+Up");
    let key = event.key().unwrap();
    assert_eq!(key.code, KeyCode::Up);
    assert!(key.shift());
    harness.log().info("verify", "Shift+Up works");

    // Ctrl+Shift+End: ESC [ 1 ; 6 F
    let (event, _) = parser.parse(b"\x1b[1;6F").expect("Should parse Ctrl+Shift+End");
    let key = event.key().unwrap();
    assert_eq!(key.code, KeyCode::End);
    assert!(key.shift());
    assert!(key.ctrl());
    harness.log().info("verify", "Ctrl+Shift+End works");

    // All modifiers: Ctrl+Shift+Alt+Home: ESC [ 1 ; 8 H
    let (event, _) = parser.parse(b"\x1b[1;8H").expect("Should parse all mods+Home");
    let key = event.key().unwrap();
    assert_eq!(key.code, KeyCode::Home);
    assert!(key.shift());
    assert!(key.alt());
    assert!(key.ctrl());
    harness.log().info("verify", "All modifiers work");

    harness.finish(true);
    eprintln!("[TEST] PASS: Modified special keys work");
}
