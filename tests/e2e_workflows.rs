//! E2E tests for complex input workflows.
//!
//! Tests complex input sequences representing real user workflows:
//! - Text editing workflows
//! - Navigation workflows

#![allow(clippy::uninlined_format_args)] // Clarity over style in test code
//! - Selection workflows
//! - Undo/redo sequences

mod common;

use common::harness::E2EHarness;
use common::input_sim::{InputSequence, sequence_to_ansi};
use opentui::input::{InputParser, KeyCode, KeyModifiers, MouseButton};

// ============================================================================
// Text Editing Workflow Tests
// ============================================================================

/// Test typing → delete → undo workflow.
#[test]
fn test_e2e_workflow_type_delete_undo() {
    let mut harness = E2EHarness::new("workflows", "type_delete_undo", 80, 24);
    harness
        .log()
        .info("init", "Testing type → delete → undo workflow");

    let mut parser = InputParser::new();

    // Build workflow: type "hello", delete last char, undo
    let seq = InputSequence::type_text("hello")
        .key(KeyCode::Backspace)
        .ctrl_key(KeyCode::Char('z')); // Undo

    let ansi = sequence_to_ansi(&seq);
    harness
        .log()
        .info("workflow", format!("Sequence has {} events", seq.len()));

    // Parse and count events
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

    // Verify event count: 5 chars + backspace + Ctrl+Z = 7
    assert_eq!(events.len(), 7);
    harness
        .log()
        .info("verify", format!("Parsed {} events", events.len()));

    harness.finish(true);
    eprintln!("[TEST] PASS: Type → delete → undo workflow works");
}

/// Test navigation workflow: arrows + home/end.
#[test]
fn test_e2e_workflow_navigation() {
    let mut harness = E2EHarness::new("workflows", "navigation", 80, 24);
    harness.log().info("init", "Testing navigation workflow");

    let mut parser = InputParser::new();

    // Navigate: right, right, down, home, end
    let seq = InputSequence::new()
        .key(KeyCode::Right)
        .key(KeyCode::Right)
        .key(KeyCode::Down)
        .key(KeyCode::Home)
        .key(KeyCode::End);

    let ansi = sequence_to_ansi(&seq);

    let mut count = 0;
    let mut offset = 0;
    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((event, consumed)) => {
                let key = event.key().expect("Should be key event");
                assert!(
                    key.code.is_navigation() || matches!(key.code, KeyCode::Home | KeyCode::End)
                );
                count += 1;
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    assert_eq!(count, 5);
    harness
        .log()
        .info("verify", "Navigation workflow parsed correctly");

    harness.finish(true);
    eprintln!("[TEST] PASS: Navigation workflow works");
}

/// Test selection workflow: Shift+arrows.
#[test]
fn test_e2e_workflow_selection() {
    let mut harness = E2EHarness::new("workflows", "selection", 80, 24);
    harness.log().info("init", "Testing selection workflow");

    let mut parser = InputParser::new();

    // Select with Shift: Shift+Right x3, Shift+Down
    let seq = InputSequence::new()
        .shift_key(KeyCode::Right)
        .shift_key(KeyCode::Right)
        .shift_key(KeyCode::Right)
        .shift_key(KeyCode::Down);

    let ansi = sequence_to_ansi(&seq);

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

    // Verify all events have Shift modifier
    for event in &events {
        let key = event.key().expect("Should be key event");
        assert!(key.shift(), "All events should have Shift modifier");
    }

    assert_eq!(events.len(), 4);
    harness
        .log()
        .info("verify", "Selection workflow with Shift modifier works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Selection workflow works");
}

/// Test copy-paste workflow: select → Ctrl+C → move → Ctrl+V.
#[test]
fn test_e2e_workflow_copy_paste() {
    let mut harness = E2EHarness::new("workflows", "copy_paste", 80, 24);
    harness.log().info("init", "Testing copy-paste workflow");

    let mut parser = InputParser::new();

    // Workflow: Select (Shift+End), Copy (Ctrl+C), End, Paste (Ctrl+V)
    let seq = InputSequence::new()
        .key_with_mods(KeyCode::End, KeyModifiers::SHIFT) // Select to end
        .ctrl_key(KeyCode::Char('c')) // Copy
        .key(KeyCode::End) // Move to end
        .ctrl_key(KeyCode::Char('v')); // Paste

    let ansi = sequence_to_ansi(&seq);
    harness
        .log()
        .info("workflow", format!("Sequence: {} bytes", ansi.len()));

    let mut count = 0;
    let mut offset = 0;
    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((event, consumed)) => {
                let key = event.key().expect("Should be key event");
                harness.log().info(
                    "event",
                    format!("{}: {:?} mods:{:?}", count, key.code, key.modifiers),
                );
                count += 1;
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    assert_eq!(count, 4);
    harness.finish(true);
    eprintln!("[TEST] PASS: Copy-paste workflow works");
}

/// Test undo/redo sequence: multiple undos then redo.
#[test]
fn test_e2e_workflow_undo_redo() {
    let mut harness = E2EHarness::new("workflows", "undo_redo", 80, 24);
    harness.log().info("init", "Testing undo/redo workflow");

    let mut parser = InputParser::new();

    // Workflow: Undo x3, Redo x2
    let seq = InputSequence::new()
        .ctrl_key(KeyCode::Char('z'))
        .ctrl_key(KeyCode::Char('z'))
        .ctrl_key(KeyCode::Char('z'))
        .ctrl_key(KeyCode::Char('y')) // Redo (Ctrl+Y)
        .ctrl_key(KeyCode::Char('y'));

    let ansi = sequence_to_ansi(&seq);

    let mut count = 0;
    let mut offset = 0;
    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((event, consumed)) => {
                let key = event.key().expect("Should be key event");
                assert!(key.ctrl(), "All should have Ctrl modifier");
                count += 1;
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    assert_eq!(count, 5);
    harness
        .log()
        .info("verify", "Undo/redo workflow parsed correctly");

    harness.finish(true);
    eprintln!("[TEST] PASS: Undo/redo workflow works");
}

// ============================================================================
// Mouse Workflow Tests
// ============================================================================

/// Test scroll → click → drag selection workflow.
#[test]
fn test_e2e_workflow_scroll_click_drag() {
    let mut harness = E2EHarness::new("workflows", "scroll_click_drag", 80, 24);
    harness
        .log()
        .info("init", "Testing scroll → click → drag workflow");

    let mut parser = InputParser::new();

    // Workflow: scroll down x2, click at (10, 15), drag to (30, 15)
    let seq = InputSequence::new()
        .scroll_down(40, 12)
        .scroll_down(40, 12)
        .left_click(10, 15)
        .drag((10, 15), (30, 15));

    let ansi = sequence_to_ansi(&seq);
    harness
        .log()
        .info("workflow", format!("Sequence: {} events", seq.len()));

    let mut mouse_events = 0;
    let mut offset = 0;
    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((event, consumed)) => {
                if event.is_mouse() {
                    mouse_events += 1;
                }
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    // 2 scrolls + 2 clicks (press/release) + 7 drag events (press + 5 moves + release)
    assert!(
        mouse_events >= 10,
        "Should have multiple mouse events: got {}",
        mouse_events
    );
    harness
        .log()
        .info("verify", format!("Parsed {} mouse events", mouse_events));

    harness.finish(true);
    eprintln!("[TEST] PASS: Scroll → click → drag workflow works");
}

/// Test right-click context menu workflow.
#[test]
fn test_e2e_workflow_context_menu() {
    let mut harness = E2EHarness::new("workflows", "context_menu", 80, 24);
    harness
        .log()
        .info("init", "Testing right-click context menu workflow");

    let mut parser = InputParser::new();

    // Workflow: right-click → arrow down x2 → Enter (select option)
    let seq = InputSequence::new()
        .right_click(20, 10)
        .key(KeyCode::Down)
        .key(KeyCode::Down)
        .key(KeyCode::Enter);

    let ansi = sequence_to_ansi(&seq);

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

    // 2 mouse (press/release) + 3 keys
    assert_eq!(events.len(), 5);

    // Verify right-click
    let first_mouse = events[0].mouse().expect("First should be mouse");
    assert_eq!(first_mouse.button, MouseButton::Right);

    // Verify Enter at end (Enter = 0x0D = Ctrl+M in ANSI)
    let last_key = events[4].key().expect("Last should be key");
    // Enter key generates '\r' which is parsed as Ctrl+M
    assert!(
        last_key.code == KeyCode::Enter || (last_key.code == KeyCode::Char('m') && last_key.ctrl()),
        "Last key should be Enter or Ctrl+M (both are 0x0D)"
    );

    harness
        .log()
        .info("verify", "Context menu workflow parsed correctly");

    harness.finish(true);
    eprintln!("[TEST] PASS: Context menu workflow works");
}

// ============================================================================
// Focus Workflow Tests
// ============================================================================

/// Test focus change workflow: focus lost → focus gained.
#[test]
fn test_e2e_workflow_focus_change() {
    let mut harness = E2EHarness::new("workflows", "focus_change", 80, 24);
    harness.log().info("init", "Testing focus change workflow");

    let mut parser = InputParser::new();

    // Workflow: Focus lost, some time passes, focus gained, then typing
    let seq = InputSequence::new()
        .focus_lost()
        .focus_gained()
        .key(KeyCode::Char('a'));

    let ansi = sequence_to_ansi(&seq);

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

    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], opentui::input::Event::FocusLost));
    assert!(matches!(events[1], opentui::input::Event::FocusGained));
    assert!(events[2].is_key());

    harness.log().info("verify", "Focus change workflow works");

    harness.finish(true);
    eprintln!("[TEST] PASS: Focus change workflow works");
}

// ============================================================================
// Search Workflow Tests
// ============================================================================

/// Test search workflow: Ctrl+F → type query → Enter → navigate results.
#[test]
fn test_e2e_workflow_search() {
    let mut harness = E2EHarness::new("workflows", "search", 80, 24);
    harness.log().info("init", "Testing search workflow");

    let mut parser = InputParser::new();

    // Workflow: Open search (Ctrl+F), type "test", Enter, next result (F3), prev (Shift+F3)
    let seq = InputSequence::new()
        .ctrl_key(KeyCode::Char('f'))
        .then(InputSequence::type_text("test"))
        .key(KeyCode::Enter)
        .key(KeyCode::F(3)) // Next result
        .shift_key(KeyCode::F(3)); // Prev result

    let ansi = sequence_to_ansi(&seq);
    harness
        .log()
        .info("workflow", format!("Sequence: {} events", seq.len()));

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

    // Ctrl+F + 4 chars + Enter + F3 + Shift+F3 = 8
    assert_eq!(count, 8);
    harness
        .log()
        .info("verify", format!("Search workflow: {} events", count));

    harness.finish(true);
    eprintln!("[TEST] PASS: Search workflow works");
}

// ============================================================================
// Resize Workflow Tests
// ============================================================================

/// Test resize workflow: resize → redraw verification.
#[test]
fn test_e2e_workflow_resize() {
    let mut harness = E2EHarness::new("workflows", "resize", 80, 24);
    harness.log().info("init", "Testing resize workflow");

    let mut parser = InputParser::new();

    // Workflow: Resize to 120x50, then type
    let seq = InputSequence::new().resize(120, 50).key(KeyCode::Char('x'));

    let ansi = sequence_to_ansi(&seq);

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

    assert_eq!(events.len(), 2);

    // Verify resize event
    let resize = events[0].resize().expect("First should be resize");
    assert_eq!(resize.width, 120);
    assert_eq!(resize.height, 50);

    harness.log().info(
        "verify",
        format!("Resize to {}x{}", resize.width, resize.height),
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: Resize workflow works");
}

// ============================================================================
// Complex Combined Workflows
// ============================================================================

/// Test file editing workflow: navigate → select → cut → paste.
#[test]
fn test_e2e_workflow_file_editing() {
    let mut harness = E2EHarness::new("workflows", "file_editing", 80, 24);
    harness.log().info("init", "Testing file editing workflow");

    let mut parser = InputParser::new();

    // Simulate editing: Go to line start, select word (Ctrl+Shift+Right), cut (Ctrl+X), paste (Ctrl+V)
    let seq = InputSequence::new()
        .key(KeyCode::Home) // Go to start
        .key_with_mods(KeyCode::Right, KeyModifiers::CTRL | KeyModifiers::SHIFT) // Select word
        .ctrl_key(KeyCode::Char('x')) // Cut
        .key(KeyCode::End) // Go to end
        .ctrl_key(KeyCode::Char('v')); // Paste

    let ansi = sequence_to_ansi(&seq);
    harness
        .log()
        .info("workflow", format!("File editing: {} events", seq.len()));

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

    assert_eq!(count, 5);
    harness.log().info("verify", "File editing workflow works");

    harness.finish(true);
    eprintln!("[TEST] PASS: File editing workflow works");
}

/// Test rapid burst input workflow (stress test).
#[test]
fn test_e2e_workflow_rapid_burst() {
    let mut harness = E2EHarness::new("workflows", "rapid_burst", 80, 24);
    harness
        .log()
        .info("init", "Testing rapid burst input (stress test)");

    let mut parser = InputParser::new();

    // Generate rapid burst: 50 keys, 10 clicks, 5 scrolls
    let mut seq = InputSequence::type_text(&"x".repeat(50)).stress_mode();
    for i in 0..10 {
        seq = seq.left_click(i * 5, 10);
    }
    for _ in 0..5 {
        seq = seq.scroll_down(40, 12);
    }

    let ansi = sequence_to_ansi(&seq);
    harness
        .log()
        .info("stress", format!("Burst: {} bytes", ansi.len()));

    let mut key_count = 0;
    let mut mouse_count = 0;
    let mut offset = 0;

    while offset < ansi.len() {
        match parser.parse(&ansi[offset..]) {
            Ok((event, consumed)) => {
                if event.is_key() {
                    key_count += 1;
                } else if event.is_mouse() {
                    mouse_count += 1;
                }
                offset += consumed;
            }
            Err(_) => break,
        }
    }

    harness.log().info(
        "verify",
        format!("Parsed: {} keys, {} mouse events", key_count, mouse_count),
    );

    assert_eq!(key_count, 50, "Should parse all key events");
    // Each click = press + release = 2, plus 5 scrolls = 25
    assert!(mouse_count >= 20, "Should parse mouse events");

    harness.finish(true);
    eprintln!("[TEST] PASS: Rapid burst workflow works");
}
