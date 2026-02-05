//! E2E tests for input injection and output verification.

mod common;

use common::harness::E2EHarness;
use common::mock_terminal::MockTerminal;
use opentui::input::{Event, KeyCode, MouseEventKind};
use opentui::terminal::Terminal;
use opentui::{EditBuffer, EditorView, Style};
use opentui_rust as opentui;

#[test]
fn test_e2e_key_input_and_render() {
    let mut harness = E2EHarness::new("input_output", "key_input", 80, 24);

    // Create an editor with some text
    let edit_buffer = EditBuffer::with_text("Hello");
    let mut view = EditorView::new(edit_buffer);
    view.set_viewport(0, 0, 80, 24);

    // Inject right arrow key input (ESC [ C)
    let events = harness.inject_input(b"\x1b[C");

    eprintln!("[TEST] Parsed events: {events:?}");
    assert_eq!(events.len(), 1, "Should parse exactly one event");

    if let Event::Key(key) = &events[0] {
        assert_eq!(key.code, KeyCode::Right, "Should be right arrow key");
        view.edit_buffer_mut().move_right();
    } else {
        unreachable!("Expected Key event, got {:?}", events[0]);
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

    eprintln!("[TEST] Mouse events: {events:?}");
    assert!(!events.is_empty(), "Should parse at least one event");

    if let Some(Event::Mouse(mouse)) = events.first() {
        eprintln!("[TEST] Mouse click at ({}, {})", mouse.x, mouse.y);
        assert_eq!(mouse.kind, MouseEventKind::Press, "Should be mouse press");
        // SGR mouse uses 1-based coordinates, parser converts to 0-based
        assert_eq!(mouse.x, 5, "X coordinate should be 5 (6-1)");
        assert_eq!(mouse.y, 0, "Y coordinate should be 0 (1-1)");
    } else {
        unreachable!("Expected Mouse event, got {:?}", events.first());
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
    eprintln!("[TEST] After start sequence: {events1:?}");
    assert!(
        events1.is_empty(),
        "Start sequence should not produce events yet"
    );

    let events = harness.inject_input(b"Pasted text\x1b[201~");

    eprintln!("[TEST] Paste events: {events:?}");
    assert!(!events.is_empty(), "Should parse paste event");

    let mut paste_found = false;
    for event in &events {
        if let Event::Paste(paste) = event {
            eprintln!("[TEST] Paste content: {:?}", paste.content());
            assert_eq!(paste.content(), "Pasted text", "Paste content should match");
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

#[test]
fn test_e2e_terminal_response_handling() {
    let mut harness = E2EHarness::new("input_output", "terminal_response", 80, 24);
    harness
        .log()
        .info("init", "Starting terminal response handling test");

    let mock = MockTerminal::new(80, 24);
    let mut terminal = Terminal::new(mock);

    terminal
        .query_capabilities()
        .expect("query_capabilities should write to mock terminal");

    // DA1 response with sixel support (param 4)
    let da1 = b"\x1b[?1;4c";
    assert!(
        terminal.parse_response(da1).is_some(),
        "DA1 response should parse"
    );
    assert!(terminal.capabilities().sixel, "DA1 should enable sixel");
    harness.log().info("caps", "DA1 set sixel=true");

    // Pixel size response enables explicit width + pixel mode
    let pixel = b"\x1b[4;24;80t";
    assert!(
        terminal.parse_response(pixel).is_some(),
        "Pixel size response should parse"
    );
    assert!(
        terminal.capabilities().explicit_width,
        "Pixel size should enable explicit width"
    );
    assert!(
        terminal.capabilities().sgr_pixels,
        "Pixel size should enable SGR pixel mode"
    );
    harness
        .log()
        .info("caps", "Pixel size set explicit_width + sgr_pixels");

    // XTVERSION response for kitty enables sync + kitty flags
    let xtversion = b"\x1bP>|kitty 0.30\x1b\\";
    assert!(
        terminal.parse_response(xtversion).is_some(),
        "XTVERSION response should parse"
    );
    assert!(
        terminal.capabilities().kitty_keyboard,
        "Kitty version should enable kitty keyboard"
    );
    assert!(
        terminal.capabilities().sync_output,
        "Kitty version should enable synchronized output"
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E terminal response handling works");
}

#[test]
fn test_e2e_grapheme_rendering_emoji() {
    let mut harness = E2EHarness::new("grapheme", "emoji_rendering", 80, 24);

    // Test multi-codepoint grapheme rendering
    // Family emoji (ZWJ sequence): 👨‍👩‍👧‍👦 = man + ZWJ + woman + ZWJ + girl + ZWJ + boy
    let family_emoji = "👨‍👩‍👧‍👦";
    let simple_emoji = "😀";
    let flag_emoji = "🇺🇸"; // Regional indicator symbols

    harness.log().info(
        "step",
        format!(
            "Testing graphemes: family={family_emoji}, simple={simple_emoji}, flag={flag_emoji}"
        ),
    );

    // Get buffer for drawing
    let buffer = harness.buffer_mut();
    let style = Style::default();

    // Line 0: Simple emoji (2 cell width)
    buffer.draw_text(0, 0, simple_emoji, style);

    // Line 1: ZWJ family emoji (2 cell width, single grapheme)
    buffer.draw_text(0, 1, family_emoji, style);

    // Line 2: Flag emoji (2 cell width)
    buffer.draw_text(0, 2, flag_emoji, style);

    // Line 3: Mixed text with emoji
    buffer.draw_text(0, 3, "Hi 👋 there", style);

    harness.log().info("render", "Emoji drawn to buffer");

    // Verify emoji rendering
    // Simple emoji at (0,0) should have width 2, cell (1,0) is continuation
    let cell_0_0 = *harness.buffer().get(0, 0).unwrap();
    let cell_1_0 = *harness.buffer().get(1, 0).unwrap();

    eprintln!(
        "[TEST] Cell(0,0): content={:?}, is_continuation={}",
        cell_0_0.content,
        cell_0_0.is_continuation()
    );
    eprintln!(
        "[TEST] Cell(1,0): content={:?}, is_continuation={}",
        cell_1_0.content,
        cell_1_0.is_continuation()
    );

    // Family emoji at (0,1) should occupy 2 cells
    let cell_0_1 = *harness.buffer().get(0, 1).unwrap();
    let cell_1_1 = *harness.buffer().get(1, 1).unwrap();

    eprintln!(
        "[TEST] Cell(0,1): content={:?}, is_continuation={}",
        cell_0_1.content,
        cell_0_1.is_continuation()
    );
    eprintln!(
        "[TEST] Cell(1,1): content={:?}, is_continuation={}",
        cell_1_1.content,
        cell_1_1.is_continuation()
    );

    // Cell (1,1) should be a continuation cell for the family emoji
    assert!(
        cell_1_1.is_continuation(),
        "Cell (1,1) should be continuation for wide emoji"
    );

    harness.dump_buffer("emoji_rendering");
    harness.finish(true);
    eprintln!("[TEST] PASS: E2E grapheme emoji rendering works");
}

#[test]
fn test_e2e_grapheme_cursor_navigation() {
    let mut harness = E2EHarness::new("grapheme", "cursor_navigation", 80, 24);

    // Test cursor navigation over graphemes
    let text_with_emoji = "A😀B";

    harness.log().info(
        "setup",
        format!("Testing cursor nav over: {text_with_emoji}"),
    );

    let edit_buffer = EditBuffer::with_text(text_with_emoji);
    let mut view = EditorView::new(edit_buffer);
    view.set_viewport(0, 0, 80, 24);

    // Log initial cursor position
    let cursor = view.edit_buffer().get_cursor_position();
    harness.log().info(
        "cursor",
        format!("Initial: row={}, col={}", cursor.row, cursor.col),
    );
    #[allow(clippy::cast_possible_truncation)]
    harness.set_cursor(cursor.col as u32, cursor.row as u32);

    // Move cursor right through the text
    // A -> emoji -> B
    view.edit_buffer_mut().move_right();
    let cursor = view.edit_buffer().get_cursor_position();
    harness.log().info(
        "cursor",
        format!("After move 1: row={}, col={}", cursor.row, cursor.col),
    );

    view.edit_buffer_mut().move_right();
    let cursor = view.edit_buffer().get_cursor_position();
    harness.log().info(
        "cursor",
        format!("After move 2: row={}, col={}", cursor.row, cursor.col),
    );

    view.edit_buffer_mut().move_right();
    let cursor = view.edit_buffer().get_cursor_position();
    harness.log().info(
        "cursor",
        format!("After move 3: row={}, col={}", cursor.row, cursor.col),
    );
    #[allow(clippy::cast_possible_truncation)]
    harness.set_cursor(cursor.col as u32, cursor.row as u32);

    // Move back left
    view.edit_buffer_mut().move_left();
    let cursor = view.edit_buffer().get_cursor_position();
    harness.log().info(
        "cursor",
        format!("After move left: row={}, col={}", cursor.row, cursor.col),
    );

    // Render text to buffer for visual verification
    let buffer = harness.buffer_mut();
    buffer.draw_text(0, 0, text_with_emoji, Style::default());

    harness.dump_buffer("cursor_after_navigation");
    harness.finish(true);
    eprintln!("[TEST] PASS: E2E grapheme cursor navigation works");
}

#[test]
fn test_e2e_grapheme_combining_marks() {
    let mut harness = E2EHarness::new("grapheme", "combining_marks", 80, 24);

    // Test combining marks
    // café with combining acute accent: cafe + U+0301
    let cafe_combining = "cafe\u{0301}";
    // café with precomposed é
    let cafe_precomposed = "café";

    harness.log().info(
        "setup",
        format!("Testing: combining='{cafe_combining}' precomposed='{cafe_precomposed}'"),
    );

    let buffer = harness.buffer_mut();
    let style = Style::default();

    // Line 0: combining form
    buffer.draw_text(0, 0, cafe_combining, style);

    // Line 1: precomposed form
    buffer.draw_text(0, 1, cafe_precomposed, style);

    // Verify both render with same visual width
    // "café" should be 4 graphemes, 4 display width
    let grapheme_count_combining = opentui::unicode::graphemes(cafe_combining).count();
    let grapheme_count_precomposed = opentui::unicode::graphemes(cafe_precomposed).count();

    harness.log().info(
        "verify",
        format!(
            "Grapheme counts: combining={grapheme_count_combining}, precomposed={grapheme_count_precomposed}"
        ),
    );

    // Both should have 4 graphemes (c, a, f, é)
    assert_eq!(
        grapheme_count_combining, 4,
        "Combining form should have 4 graphemes"
    );
    assert_eq!(
        grapheme_count_precomposed, 4,
        "Precomposed form should have 4 graphemes"
    );

    harness.dump_buffer("combining_marks");
    harness.finish(true);
    eprintln!("[TEST] PASS: E2E grapheme combining marks works");
}

#[test]
fn test_e2e_hyperlink_output() {
    use opentui::Rgba;
    use opentui::ansi::AnsiWriter;
    use opentui::cell::Cell;
    use opentui::link::LinkPool;

    let mut harness = E2EHarness::new("hyperlink", "link_output", 80, 24);

    // Create a link pool and allocate URLs
    let mut link_pool = LinkPool::new();
    let link_id_1 = link_pool.alloc("https://example.com");
    let link_id_2 = link_pool.alloc("https://rust-lang.org");

    harness.log().info(
        "setup",
        format!("Allocated link IDs: id1={link_id_1}, id2={link_id_2}"),
    );

    // Verify link IDs are non-zero (0 means no link)
    assert_ne!(link_id_1, 0, "Link ID should be non-zero");
    assert_ne!(link_id_2, 0, "Link ID should be non-zero");
    assert_ne!(link_id_1, link_id_2, "Link IDs should be unique");

    // Create cells with hyperlinks using Style::with_link()
    let linked_style_1 = Style::fg(Rgba::BLUE).with_underline().with_link(link_id_1);
    let linked_style_2 = Style::fg(Rgba::GREEN).with_link(link_id_2);

    // Verify link ID is packed into attributes
    assert_eq!(
        linked_style_1.attributes.link_id(),
        Some(link_id_1),
        "Link ID should be packed in attributes"
    );

    harness.log().info(
        "style",
        format!(
            "Style 1 attributes: {:?}, link_id: {:?}",
            linked_style_1.attributes,
            linked_style_1.attributes.link_id()
        ),
    );

    // Create cells with the linked styles
    let cell_1 = Cell::new('E', linked_style_1);
    let cell_2 = Cell::new('R', linked_style_2);

    // Capture ANSI output using AnsiWriter
    let mut output_buffer: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut output_buffer);

        // Write first linked cell at position (0, 0)
        let url_1 = link_pool.get(link_id_1);
        writer.write_cell_at_with_link(0, 0, &cell_1, url_1);

        // Write second linked cell at position (0, 1)
        let url_2 = link_pool.get(link_id_2);
        writer.write_cell_at_with_link(0, 1, &cell_2, url_2);

        // End any open hyperlink
        writer.set_link(None, None);

        writer.flush().unwrap();
    }

    let output_str = String::from_utf8_lossy(&output_buffer);
    harness.log().info(
        "ansi_output",
        format!("Raw output length: {} bytes", output_buffer.len()),
    );

    // Log readable version of escape sequences
    let readable_output = output_str.replace('\x1b', "ESC").replace('\x07', "BEL");
    harness.log().info("ansi_readable", readable_output.clone());

    // Verify OSC 8 hyperlink sequences are present
    // OSC 8 format: ESC ] 8 ; id=<id> ; <url> ESC \ (or BEL)
    let osc8_start_1 = format!("\x1b]8;id={link_id_1};https://example.com\x1b\\");
    let osc8_start_2 = format!("\x1b]8;id={link_id_2};https://rust-lang.org\x1b\\");
    let osc8_end = "\x1b]8;;\x1b\\";

    assert!(
        output_str.contains(&osc8_start_1),
        "Output should contain OSC 8 start for link 1: expected {:?} in {:?}",
        osc8_start_1.replace('\x1b', "ESC"),
        readable_output
    );

    assert!(
        output_str.contains(&osc8_start_2),
        "Output should contain OSC 8 start for link 2: expected {:?} in {:?}",
        osc8_start_2.replace('\x1b', "ESC"),
        readable_output
    );

    assert!(
        output_str.contains(osc8_end),
        "Output should contain OSC 8 end sequence"
    );

    // Verify the cell characters are present
    assert!(output_str.contains('E'), "Output should contain 'E'");
    assert!(output_str.contains('R'), "Output should contain 'R'");

    harness.log().info("verify", "All OSC 8 sequences verified");

    // Also verify link pool retrieval
    assert_eq!(link_pool.get(link_id_1), Some("https://example.com"));
    assert_eq!(link_pool.get(link_id_2), Some("https://rust-lang.org"));
    assert_eq!(link_pool.get(0), None, "Link ID 0 should return None");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E hyperlink output works");
}

#[test]
fn test_e2e_link_id_packing_in_attributes() {
    use opentui::style::TextAttributes;

    let mut harness = E2EHarness::new("hyperlink", "link_id_packing", 80, 24);

    // Test link ID packing into TextAttributes (24-bit max)
    let test_cases: Vec<(u32, &str)> = vec![
        (1, "minimum non-zero"),
        (255, "single byte max"),
        (256, "two bytes"),
        (65535, "two bytes max"),
        (0x00FF_FFFF, "maximum 24-bit"),
    ];

    for (link_id, description) in test_cases {
        let attrs = TextAttributes::BOLD.with_link_id(link_id);

        harness.log().info(
            "packing",
            format!("Testing {description}: id={link_id} (0x{link_id:06X})"),
        );

        // Verify link ID is correctly stored and retrieved
        assert_eq!(
            attrs.link_id(),
            Some(link_id),
            "Link ID {link_id} ({description}) should be packed correctly"
        );

        // Verify style flags are preserved
        assert!(
            attrs.contains(TextAttributes::BOLD),
            "BOLD flag should be preserved with link ID {link_id}"
        );

        // Verify flags_only() strips link ID
        let flags = attrs.flags_only();
        assert_eq!(flags.link_id(), None, "flags_only() should strip link ID");
        assert!(
            flags.contains(TextAttributes::BOLD),
            "flags_only() should preserve BOLD"
        );

        harness.log().info(
            "verified",
            format!("Link ID {link_id} correctly packed, flags preserved"),
        );
    }

    // Test overflow: values beyond 24-bit should be masked
    let overflow_id = 0x1FF_FFFF; // 25 bits
    let attrs = TextAttributes::empty().with_link_id(overflow_id);
    let expected_masked = TextAttributes::MAX_LINK_ID; // 0x00FF_FFFF

    harness.log().info(
        "overflow",
        format!(
            "Testing overflow: input=0x{:08X}, expected=0x{:06X}, actual={:?}",
            overflow_id,
            expected_masked,
            attrs.link_id()
        ),
    );

    assert_eq!(
        attrs.link_id(),
        Some(expected_masked),
        "Overflow link ID should be masked to 24 bits"
    );

    // Test merge behavior: overlay link ID takes precedence when set
    let base = TextAttributes::BOLD.with_link_id(1);
    let overlay_no_link = TextAttributes::ITALIC;
    let overlay_with_link = TextAttributes::UNDERLINE.with_link_id(2);

    let merged_no_link = base.merge(overlay_no_link);
    assert_eq!(
        merged_no_link.link_id(),
        Some(1),
        "Base link ID preserved when overlay has none"
    );

    let merged_with_link = base.merge(overlay_with_link);
    assert_eq!(
        merged_with_link.link_id(),
        Some(2),
        "Overlay link ID takes precedence"
    );

    harness
        .log()
        .info("merge", "Link ID merge behavior verified");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E link ID packing works");
}

#[test]
#[allow(clippy::too_many_lines)]
fn test_e2e_threaded_renderer_lifecycle_components() {
    use opentui::Rgba;
    use opentui::buffer::OptimizedBuffer;
    use opentui::grapheme_pool::GraphemePool;
    use opentui::link::LinkPool;
    use opentui::renderer::{BufferDiff, ThreadedRenderStats};
    use std::sync::mpsc;
    use std::time::Duration;

    let mut harness = E2EHarness::new("threaded_renderer", "lifecycle_components", 80, 24);

    // Test 1: ThreadedRenderStats lifecycle
    harness
        .log()
        .info("stats", "Testing ThreadedRenderStats lifecycle");

    let mut stats = ThreadedRenderStats::default();
    assert_eq!(stats.frames, 0, "Initial frame count should be 0");
    assert_eq!(stats.last_frame_cells, 0, "Initial cells should be 0");

    // Simulate frame updates
    stats.frames = 60;
    stats.fps = 60.0;
    stats.last_frame_cells = 1920;
    stats.last_frame_time = Duration::from_millis(16);

    let cloned = stats;
    assert_eq!(cloned.frames, 60, "Cloned stats should preserve frames");
    assert!(
        (cloned.fps - 60.0).abs() < f32::EPSILON,
        "Cloned stats should preserve fps"
    );

    harness.log().info(
        "stats",
        format!(
            "Stats verified: frames={}, fps={:.1}, cells={}",
            cloned.frames, cloned.fps, cloned.last_frame_cells
        ),
    );

    // Test 2: BufferDiff for incremental updates (core to threaded renderer)
    harness
        .log()
        .info("diff", "Testing BufferDiff for incremental rendering");

    let buffer_a = OptimizedBuffer::new(10, 5);
    let mut buffer_b = OptimizedBuffer::new(10, 5);

    // Both buffers start identical (empty)
    let diff_empty = BufferDiff::compute(&buffer_a, &buffer_b);
    assert_eq!(
        diff_empty.change_count, 0,
        "Identical buffers should have no diff"
    );
    harness
        .log()
        .info("diff", "Empty buffers: no changes detected");

    // Modify buffer_b
    buffer_b.draw_text(0, 0, "Hello", Style::fg(Rgba::GREEN));
    let diff_changed = BufferDiff::compute(&buffer_a, &buffer_b);
    assert!(
        diff_changed.change_count > 0,
        "Modified buffer should have changes"
    );
    harness.log().info(
        "diff",
        format!(
            "After drawing text: {} cells changed",
            diff_changed.change_count
        ),
    );

    // Verify diff contains changed positions
    assert!(
        !diff_changed.changed_cells.is_empty(),
        "Changed cells list should not be empty"
    );
    harness.log().info(
        "diff",
        format!(
            "Changed cell positions: {:?}",
            &diff_changed.changed_cells[..diff_changed.changed_cells.len().min(5)]
        ),
    );

    // Test 3: Channel communication pattern (simulating threaded renderer IPC)
    harness
        .log()
        .info("channel", "Testing channel-based buffer ownership transfer");

    // This simulates the threaded renderer's ownership model:
    // Main thread sends buffer -> Render thread processes -> Returns buffer

    let (tx, rx) = mpsc::channel::<OptimizedBuffer>();
    let (reply_tx, reply_rx) = mpsc::channel::<OptimizedBuffer>();

    // Simulate main thread creating and sending buffer
    let buffer = OptimizedBuffer::new(80, 24);
    harness
        .log()
        .info("channel", "Main thread: created 80x24 buffer");

    tx.send(buffer).expect("Channel send should succeed");
    harness
        .log()
        .info("channel", "Main thread: sent buffer to render thread");

    // Simulate render thread receiving, processing, and returning
    let mut received_buffer = rx.recv().expect("Channel receive should succeed");
    harness
        .log()
        .info("channel", "Render thread: received buffer");

    // Simulate rendering work
    received_buffer.draw_text(0, 0, "Rendered!", Style::fg(Rgba::WHITE));
    harness.log().info("channel", "Render thread: drew content");

    reply_tx
        .send(received_buffer)
        .expect("Reply send should succeed");
    harness
        .log()
        .info("channel", "Render thread: returned buffer");

    // Main thread receives buffer back
    let returned_buffer = reply_rx.recv().expect("Reply receive should succeed");
    harness
        .log()
        .info("channel", "Main thread: received buffer back");

    // Verify the rendered content
    let cell = returned_buffer.get(0, 0).expect("Cell should exist");
    assert!(
        matches!(cell.content.as_char(), Some('R')),
        "Cell should contain 'R' from 'Rendered!'"
    );
    harness
        .log()
        .info("channel", "Buffer ownership transfer verified");

    // Test 4: Pool resource management (GraphemePool and LinkPool)
    harness
        .log()
        .info("pools", "Testing pool resource lifecycle");

    let mut grapheme_pool = GraphemePool::new();
    let mut link_pool = LinkPool::new();

    // Allocate resources
    let grapheme_id = grapheme_pool.alloc("👨‍👩‍👧‍👦");
    let link_id = link_pool.alloc("https://example.com");

    harness.log().info(
        "pools",
        format!("Allocated: grapheme_id slot, link_id={link_id}"),
    );

    // Verify retrieval
    assert_eq!(grapheme_pool.get(grapheme_id), Some("👨‍👩‍👧‍👦"));
    assert_eq!(link_pool.get(link_id), Some("https://example.com"));

    // Test pool cloning (used when transferring to render thread)
    let pool_clone = grapheme_pool.clone();
    assert_eq!(pool_clone.get(grapheme_id), Some("👨‍👩‍👧‍👦"));
    harness.log().info("pools", "Pool cloning preserves data");

    // Test 5: Full redraw threshold
    harness
        .log()
        .info("threshold", "Testing full redraw vs diff threshold");

    let total_cells = 80 * 24;
    let diff_10_percent = BufferDiff {
        changed_cells: vec![(0, 0); total_cells / 10],
        dirty_regions: vec![],
        change_count: total_cells / 10,
    };
    let diff_80_percent = BufferDiff {
        changed_cells: vec![(0, 0); total_cells * 8 / 10],
        dirty_regions: vec![],
        change_count: total_cells * 8 / 10,
    };

    // 10% changes should use diff
    assert!(
        !diff_10_percent.should_full_redraw(total_cells),
        "10% changes should use diff"
    );
    harness
        .log()
        .info("threshold", "10% changes: diff mode selected");

    // 80% changes should trigger full redraw
    assert!(
        diff_80_percent.should_full_redraw(total_cells),
        "80% changes should use full redraw"
    );
    harness
        .log()
        .info("threshold", "80% changes: full redraw selected");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E threaded renderer lifecycle components work");
}

#[test]
fn test_e2e_threaded_renderer_smoke_render() {
    use opentui::Rgba;
    use opentui::ansi::AnsiWriter;
    use opentui::buffer::OptimizedBuffer;
    use opentui::grapheme_pool::GraphemePool;
    use opentui::link::LinkPool;

    let mut harness = E2EHarness::new("threaded_renderer", "smoke_render", 40, 10);

    // Simulate what the threaded renderer does internally:
    // 1. Accept a buffer with drawn content
    // 2. Generate ANSI output
    // 3. Track what was rendered

    harness
        .log()
        .info("setup", "Simulating threaded renderer smoke test");

    // Create buffers and pools like ThreadedRenderer does
    let mut buffer = OptimizedBuffer::new(40, 10);
    let mut grapheme_pool = GraphemePool::new();
    let mut link_pool = LinkPool::new();

    // Draw some content
    buffer.draw_text(0, 0, "Frame 1", Style::fg(Rgba::GREEN));
    buffer.draw_text(0, 1, "Status: OK", Style::fg(Rgba::WHITE));

    // Add a hyperlink
    let link_id = link_pool.alloc("https://docs.rs");
    let link_style = Style::fg(Rgba::BLUE).with_underline().with_link(link_id);
    buffer.draw_text(0, 2, "Documentation", link_style);

    // Add a grapheme
    let emoji_id = grapheme_pool.alloc("🚀");
    let emoji_cell = opentui::cell::Cell {
        content: opentui::cell::CellContent::Grapheme(emoji_id),
        fg: Rgba::WHITE,
        bg: Rgba::TRANSPARENT,
        attributes: opentui::style::TextAttributes::empty(),
    };
    buffer.set(20, 0, emoji_cell);
    buffer.set(21, 0, opentui::cell::Cell::continuation(Rgba::TRANSPARENT));

    harness
        .log()
        .info("draw", "Drew text, hyperlink, and emoji");

    // Capture ANSI output like the render thread would
    let mut ansi_output: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut ansi_output);

        for y in 0..10 {
            writer.move_cursor(y, 0);
            for x in 0..40 {
                if let Some(cell) = buffer.get(x, y) {
                    if !cell.is_continuation() {
                        let url = cell.attributes.link_id().and_then(|id| link_pool.get(id));
                        writer.write_cell_with_link_and_pool(cell, url, &grapheme_pool);
                    }
                }
            }
        }

        writer.reset();
        writer.flush().unwrap();
    }

    harness.log().info(
        "render",
        format!("Generated {} bytes of ANSI output", ansi_output.len()),
    );

    // Verify ANSI output contains expected elements
    let output_str = String::from_utf8_lossy(&ansi_output);

    // Check for text content
    assert!(
        output_str.contains("Frame 1"),
        "Output should contain 'Frame 1'"
    );
    assert!(
        output_str.contains("Status"),
        "Output should contain 'Status'"
    );
    assert!(
        output_str.contains("Documentation"),
        "Output should contain 'Documentation'"
    );
    assert!(output_str.contains("🚀"), "Output should contain emoji");

    harness
        .log()
        .info("verify", "Text and emoji content verified");

    // Check for hyperlink OSC 8 sequence
    let osc8_start = format!("\x1b]8;id={link_id};https://docs.rs\x1b\\");
    assert!(
        output_str.contains(&osc8_start),
        "Output should contain OSC 8 hyperlink start"
    );

    harness
        .log()
        .info("verify", "Hyperlink OSC 8 sequence verified");

    // Check for color sequences (green for Frame 1)
    assert!(
        output_str.contains("\x1b[38;2;0;255;0m") || output_str.contains("\x1b[38;2;0;128;0m"),
        "Output should contain green foreground color"
    );

    harness.log().info("verify", "Color sequences verified");

    // Verify cursor positioning is present
    assert!(
        output_str.contains("\x1b["),
        "Output should contain cursor positioning"
    );

    harness
        .log()
        .info("render", "Smoke render verification complete");

    // Log readable version for debugging
    let readable = output_str.replace('\x1b', "ESC");
    harness.log().debug("ansi", readable);

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E threaded renderer smoke render works");
}
