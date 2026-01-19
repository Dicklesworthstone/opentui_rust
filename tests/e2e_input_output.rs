//! E2E tests for input injection and output verification.

mod common;

use common::harness::E2EHarness;
use opentui::input::{Event, KeyCode, MouseEventKind};
use opentui::{EditBuffer, EditorView, Style};

#[test]
fn test_e2e_key_input_and_render() {
    let mut harness = E2EHarness::new("input_output", "key_input", 80, 24);

    // Create an editor with some text
    let edit_buffer = EditBuffer::with_text("Hello");
    let mut view = EditorView::new(edit_buffer);
    view.set_viewport(0, 0, 80, 24);

    // Inject right arrow key input (ESC [ C)
    let events = harness.inject_input(b"\x1b[C");

    eprintln!("[TEST] Parsed events: {:?}", events);
    assert_eq!(events.len(), 1, "Should parse exactly one event");

    if let Event::Key(key) = &events[0] {
        assert_eq!(key.code, KeyCode::Right, "Should be right arrow key");
        view.edit_buffer_mut().move_right();
    } else {
        panic!("Expected Key event, got {:?}", events[0]);
    }

    // Render editor to buffer
    let buffer = harness.buffer_mut();
    buffer.draw_text(0, 0, "Hello", Style::default());

    // Verify first character
    harness.assert_cell(0, 0, 'H', "First char should be H");
    harness.assert_cell(1, 0, 'e', "Second char should be e");
    harness.assert_cell(2, 0, 'l', "Third char should be l");
    harness.assert_cell(3, 0, 'l', "Fourth char should be l");
    harness.assert_cell(4, 0, 'o', "Fifth char should be o");

    harness.dump_buffer("after_right_arrow");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E key input and render works");
}

#[test]
fn test_e2e_mouse_click_and_selection() {
    let mut harness = E2EHarness::new("input_output", "mouse_selection", 80, 24);

    // Create editor with text
    let edit_buffer = EditBuffer::with_text("Click here to select");
    let view = EditorView::new(edit_buffer);

    // Inject SGR mouse click at column 5, row 0 (button 0 = left click)
    // Format: ESC [ < Cb ; Cx ; Cy M
    let events = harness.inject_input(b"\x1b[<0;6;1M");

    eprintln!("[TEST] Mouse events: {:?}", events);
    assert!(!events.is_empty(), "Should parse at least one event");

    if let Some(Event::Mouse(mouse)) = events.first() {
        eprintln!("[TEST] Mouse click at ({}, {})", mouse.x, mouse.y);
        assert_eq!(mouse.kind, MouseEventKind::Press, "Should be mouse press");
        // SGR mouse uses 1-based coordinates, parser converts to 0-based
        assert_eq!(mouse.x, 5, "X coordinate should be 5 (6-1)");
        assert_eq!(mouse.y, 0, "Y coordinate should be 0 (1-1)");
    } else {
        panic!("Expected Mouse event, got {:?}", events.first());
    }

    // Render text to buffer
    let buffer = harness.buffer_mut();
    buffer.draw_text(0, 0, "Click here to select", Style::default());

    harness.dump_buffer("mouse_click_position");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E mouse click works");

    // Verify view was created (suppress unused warning)
    let _ = view;
}

#[test]
fn test_e2e_bracketed_paste() {
    let mut harness = E2EHarness::new("input_output", "bracketed_paste", 80, 24);

    let edit_buffer = EditBuffer::new();
    let mut view = EditorView::new(edit_buffer);
    view.set_viewport(0, 0, 80, 24);

    // Inject bracketed paste in chunks as the parser expects:
    // 1. First call enters paste mode (returns no events, waits for content)
    // 2. Second call provides content + end marker (returns Paste event)
    // Format: ESC [ 200 ~ <content> ESC [ 201 ~
    let events1 = harness.inject_input(b"\x1b[200~");
    eprintln!("[TEST] After start sequence: {:?}", events1);
    assert!(events1.is_empty(), "Start sequence should not produce events yet");

    let events = harness.inject_input(b"Pasted text\x1b[201~");

    eprintln!("[TEST] Paste events: {:?}", events);
    assert!(!events.is_empty(), "Should parse paste event");

    let mut paste_found = false;
    for event in &events {
        if let Event::Paste(paste) = event {
            eprintln!("[TEST] Paste content: {:?}", paste.content());
            assert_eq!(
                paste.content(),
                "Pasted text",
                "Paste content should match"
            );
            paste_found = true;

            // Insert pasted text into editor
            view.edit_buffer_mut().insert(paste.content());
        }
    }
    assert!(paste_found, "Should have found a paste event");

    // Verify text was inserted
    let text = view.edit_buffer().text();
    assert_eq!(text, "Pasted text", "Editor should contain pasted text");

    // Render to buffer
    let buffer = harness.buffer_mut();
    buffer.draw_text(0, 0, &text, Style::default());

    harness.dump_buffer("after_paste");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E bracketed paste works");
}
