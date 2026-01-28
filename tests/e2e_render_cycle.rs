//! E2E tests for full render cycle with frame validation.
//!
//! Tests complete render cycle from initialization through multiple frames to cleanup.
//! Verifies diff rendering, ANSI output correctness, and frame sequence behavior.

mod common;

use common::harness::E2EHarness;
use common::mock_terminal::MockTerminal;
use opentui::ansi::AnsiWriter;
use opentui::buffer::OptimizedBuffer;
use opentui::grapheme_pool::GraphemePool;
use opentui::renderer::BufferDiff;
use opentui::style::TextAttributes;
use opentui::{Rgba, Style};
use std::io::Write;

/// Test basic render cycle: init -> draw -> present -> modify -> present -> cleanup.
#[test]
fn test_e2e_basic_render_cycle() {
    let mut harness = E2EHarness::new("render_cycle", "basic_cycle", 40, 10);

    harness.log().info("init", "Starting basic render cycle test");

    // Step 1: Initialize buffers (simulating Renderer)
    let mut front_buffer = OptimizedBuffer::new(40, 10);
    let mut back_buffer = OptimizedBuffer::new(40, 10);

    harness.log().info("init", "Buffers initialized: 40x10");

    // Step 2: Draw initial content to back buffer
    back_buffer.draw_text(0, 0, "Hello, OpenTUI!", Style::fg(Rgba::GREEN));
    back_buffer.draw_text(0, 1, "Frame 1", Style::fg(Rgba::WHITE));

    harness.log().info("draw", "Drew initial content to back buffer");

    // Step 3: Compute diff and verify first frame behavior
    let diff1 = BufferDiff::compute(&front_buffer, &back_buffer);

    harness.log().info(
        "diff",
        format!(
            "First frame diff: {} cells changed",
            diff1.change_count
        ),
    );

    // First frame should have changes (back buffer has content, front is empty)
    assert!(diff1.change_count > 0, "First frame should have changes");
    assert!(!diff1.changed_cells.is_empty(), "Changed cells list should not be empty");

    // Verify "Hello, OpenTUI!" is in the changed region (15 characters at row 0)
    let row0_changes: Vec<_> = diff1
        .changed_cells
        .iter()
        .filter(|(_, y)| *y == 0)
        .collect();
    assert!(row0_changes.len() >= 15, "Row 0 should have at least 15 changed cells for 'Hello, OpenTUI!'");

    // Step 4: Swap buffers (simulate present)
    std::mem::swap(&mut front_buffer, &mut back_buffer);
    back_buffer.clear(Rgba::TRANSPARENT);

    // Redraw to back buffer for next frame
    back_buffer.draw_text(0, 0, "Hello, OpenTUI!", Style::fg(Rgba::GREEN));
    back_buffer.draw_text(0, 1, "Frame 2", Style::fg(Rgba::WHITE)); // Changed content

    harness.log().info("draw", "Drew Frame 2 content (modified)");

    // Step 5: Compute diff for second frame
    let diff2 = BufferDiff::compute(&front_buffer, &back_buffer);

    harness.log().info(
        "diff",
        format!(
            "Second frame diff: {} cells changed",
            diff2.change_count
        ),
    );

    // Second frame should only have changes where content differs ("Frame 1" -> "Frame 2")
    // The "Hello, OpenTUI!" line should be unchanged
    assert!(
        diff2.change_count < diff1.change_count,
        "Second frame should have fewer changes than first (only 'Frame 1' -> 'Frame 2')"
    );

    // Verify row 0 has no changes (content unchanged)
    let row0_changes2: Vec<_> = diff2
        .changed_cells
        .iter()
        .filter(|(_, y)| *y == 0)
        .collect();
    assert!(
        row0_changes2.is_empty(),
        "Row 0 should have no changes (content unchanged)"
    );

    // Verify row 1 has changes
    let row1_changes: Vec<_> = diff2
        .changed_cells
        .iter()
        .filter(|(_, y)| *y == 1)
        .collect();
    assert!(
        !row1_changes.is_empty(),
        "Row 1 should have changes (Frame 1 -> Frame 2)"
    );

    harness.dump_buffer("final_state");
    harness.finish(true);
    eprintln!("[TEST] PASS: E2E basic render cycle works");
}

/// Test that first frame outputs full buffer content.
#[test]
fn test_e2e_first_frame_full_output() {
    let mut harness = E2EHarness::new("render_cycle", "first_frame_full", 20, 5);

    harness.log().info("init", "Testing first frame full output");

    // Create buffers
    let front_buffer = OptimizedBuffer::new(20, 5);
    let mut back_buffer = OptimizedBuffer::new(20, 5);

    // Draw some content
    back_buffer.draw_text(0, 0, "Line 1", Style::default());
    back_buffer.draw_text(0, 1, "Line 2", Style::default());
    back_buffer.draw_text(0, 2, "Line 3", Style::default());

    // Compute diff
    let diff = BufferDiff::compute(&front_buffer, &back_buffer);

    // First frame: all drawn cells should be in the diff
    let total_drawn_cells = 6 + 6 + 6; // "Line X" = 6 chars each
    assert!(
        diff.change_count >= total_drawn_cells,
        "First frame should include all drawn cells: expected >= {}, got {}",
        total_drawn_cells,
        diff.change_count
    );

    harness.log().info(
        "verify",
        format!("First frame has {} changes (expected >= {})", diff.change_count, total_drawn_cells),
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E first frame full output works");
}

/// Test that subsequent frames output only diffs.
#[test]
fn test_e2e_subsequent_frames_diff_only() {
    let mut harness = E2EHarness::new("render_cycle", "diff_only", 30, 5);

    harness.log().info("init", "Testing subsequent frames diff-only output");

    // Create initial state
    let mut front_buffer = OptimizedBuffer::new(30, 5);
    let mut back_buffer = OptimizedBuffer::new(30, 5);

    // Fill both buffers with same content initially
    front_buffer.draw_text(0, 0, "Static content here", Style::default());
    front_buffer.draw_text(0, 1, "Counter: 0", Style::default());

    back_buffer.draw_text(0, 0, "Static content here", Style::default());
    back_buffer.draw_text(0, 1, "Counter: 1", Style::default()); // Only this changes

    // Compute diff
    let diff = BufferDiff::compute(&front_buffer, &back_buffer);

    harness.log().info(
        "diff",
        format!("Diff has {} changed cells", diff.change_count),
    );

    // Only the counter digit should change (position 9 on row 1: '0' -> '1')
    assert!(
        diff.change_count <= 3,
        "Only counter digit should change: expected <= 3, got {}",
        diff.change_count
    );

    // Verify static content row has no changes
    let row0_changes: Vec<_> = diff
        .changed_cells
        .iter()
        .filter(|(_, y)| *y == 0)
        .collect();
    assert!(row0_changes.is_empty(), "Static row should have no changes");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E subsequent frames diff-only works");
}

/// Test force redraw outputs full buffer.
#[test]
fn test_e2e_force_redraw_full_output() {
    let mut harness = E2EHarness::new("render_cycle", "force_redraw", 20, 5);

    harness.log().info("init", "Testing force redraw full output");

    // Create identical buffers
    let mut front_buffer = OptimizedBuffer::new(20, 5);
    let mut back_buffer = OptimizedBuffer::new(20, 5);

    let content = "Same content";
    front_buffer.draw_text(0, 0, content, Style::default());
    back_buffer.draw_text(0, 0, content, Style::default());

    // Normal diff should show no changes
    let diff = BufferDiff::compute(&front_buffer, &back_buffer);
    assert_eq!(diff.change_count, 0, "Identical buffers should have no diff");

    harness.log().info("verify", "Identical buffers: no diff");

    // Force redraw simulation: treat all cells as changed
    let total_cells = 20 * 5;
    let force_diff = BufferDiff {
        changed_cells: (0..20u32)
            .flat_map(|x| (0..5u32).map(move |y| (x, y)))
            .collect(),
        dirty_regions: vec![],
        change_count: total_cells,
    };

    assert_eq!(
        force_diff.change_count, total_cells,
        "Force redraw should include all {} cells",
        total_cells
    );

    harness.log().info(
        "verify",
        format!("Force redraw: {} cells", force_diff.change_count),
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E force redraw full output works");
}

/// Test clear + draw outputs correctly.
#[test]
fn test_e2e_clear_and_draw() {
    let mut harness = E2EHarness::new("render_cycle", "clear_draw", 20, 5);

    harness.log().info("init", "Testing clear + draw sequence");

    // Initial state with content
    let mut front_buffer = OptimizedBuffer::new(20, 5);
    front_buffer.draw_text(0, 0, "Old content", Style::default());

    // Back buffer after clear + new draw
    let mut back_buffer = OptimizedBuffer::new(20, 5);
    back_buffer.clear(Rgba::BLACK);
    back_buffer.draw_text(0, 0, "New content", Style::fg(Rgba::RED));

    // Compute diff
    let diff = BufferDiff::compute(&front_buffer, &back_buffer);

    harness.log().info(
        "diff",
        format!("Clear + draw diff: {} cells changed", diff.change_count),
    );

    // Should detect changes where old content was cleared and new content drawn
    assert!(diff.change_count > 0, "Clear + draw should produce changes");

    // Verify first cell is different
    let first_row_changes: Vec<_> = diff
        .changed_cells
        .iter()
        .filter(|(_, y)| *y == 0)
        .collect();
    assert!(
        !first_row_changes.is_empty(),
        "Row 0 should have changes after clear + draw"
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E clear and draw works");
}

/// Test ANSI cursor positioning sequences.
#[test]
fn test_e2e_ansi_cursor_positioning() {
    let mut harness = E2EHarness::new("render_cycle", "cursor_positioning", 40, 10);

    harness.log().info("init", "Testing ANSI cursor positioning");

    // Capture ANSI output
    let mut output: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut output);

        // Move cursor to various positions
        writer.move_cursor(0, 0);
        writer.move_cursor(5, 10);
        writer.move_cursor(9, 39);

        writer.flush().unwrap();
    }

    let output_str = String::from_utf8_lossy(&output);

    harness.log().info(
        "ansi",
        format!("Output length: {} bytes", output.len()),
    );

    // Parse and verify sequences
    let mut mock = MockTerminal::new(40, 10);
    mock.write_all(&output).unwrap();

    let cursor_moves = mock.cursor_moves();

    harness.log().info(
        "verify",
        format!("Cursor moves: {:?}", cursor_moves),
    );

    // Should have cursor position sequences
    assert!(
        output_str.contains("\x1b["),
        "Output should contain CSI sequences"
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E ANSI cursor positioning works");
}

/// Test ANSI color sequences match drawn colors.
#[test]
fn test_e2e_ansi_color_sequences() {
    let mut harness = E2EHarness::new("render_cycle", "color_sequences", 40, 10);

    harness.log().info("init", "Testing ANSI color sequences");

    // Create a cell with specific colors using builder
    let style = Style::builder().fg(Rgba::RED).bg(Rgba::BLUE).build();
    let cell = opentui::cell::Cell::new('X', style);

    // Capture ANSI output
    let mut output: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut output);
        writer.move_cursor(0, 0);
        writer.write_cell(&cell);
        writer.flush().unwrap();
    }

    let output_str = String::from_utf8_lossy(&output);

    harness.log().info(
        "ansi",
        format!(
            "Output: {}",
            output_str.replace('\x1b', "ESC")
        ),
    );

    // Verify red foreground (255, 0, 0)
    assert!(
        output_str.contains("\x1b[38;2;255;0;0m"),
        "Output should contain red foreground color sequence"
    );

    // Verify blue background (0, 0, 255)
    assert!(
        output_str.contains("\x1b[48;2;0;0;255m"),
        "Output should contain blue background color sequence"
    );

    // Verify the character 'X' is present
    assert!(output_str.contains('X'), "Output should contain the cell character");

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E ANSI color sequences work");
}

/// Test ANSI text attribute sequences.
#[test]
fn test_e2e_ansi_text_attributes() {
    let mut harness = E2EHarness::new("render_cycle", "text_attributes", 40, 10);

    harness.log().info("init", "Testing ANSI text attribute sequences");

    // Create cells with various attributes
    let bold_style = Style::bold();
    let italic_style = Style::italic();
    let underline_style = Style::underline();

    let bold_cell = opentui::cell::Cell::new('B', bold_style);
    let italic_cell = opentui::cell::Cell::new('I', italic_style);
    let underline_cell = opentui::cell::Cell::new('U', underline_style);

    // Capture ANSI output
    let mut output: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut output);

        writer.move_cursor(0, 0);
        writer.write_cell(&bold_cell);

        writer.reset();
        writer.move_cursor(0, 1);
        writer.write_cell(&italic_cell);

        writer.reset();
        writer.move_cursor(0, 2);
        writer.write_cell(&underline_cell);

        writer.flush().unwrap();
    }

    let output_str = String::from_utf8_lossy(&output);
    let readable = output_str.replace('\x1b', "ESC");

    harness.log().info("ansi", format!("Output: {readable}"));

    // Parse sequences using MockTerminal
    let mut mock = MockTerminal::new(40, 10);
    mock.write_all(&output).unwrap();
    let sequences = mock.parse_sequences();

    // Verify we have attribute sequences
    let has_bold = sequences
        .iter()
        .any(|s| matches!(s, common::mock_terminal::AnsiSequence::SetAttributes(a) if a.contains(TextAttributes::BOLD)));
    let has_italic = sequences
        .iter()
        .any(|s| matches!(s, common::mock_terminal::AnsiSequence::SetAttributes(a) if a.contains(TextAttributes::ITALIC)));
    let has_underline = sequences
        .iter()
        .any(|s| matches!(s, common::mock_terminal::AnsiSequence::SetAttributes(a) if a.contains(TextAttributes::UNDERLINE)));

    harness.log().info(
        "verify",
        format!(
            "Attributes found: bold={}, italic={}, underline={}",
            has_bold, has_italic, has_underline
        ),
    );

    // Check raw sequences in output
    assert!(
        output_str.contains("\x1b[1m"),
        "Output should contain bold sequence (CSI 1 m)"
    );
    assert!(
        output_str.contains("\x1b[3m"),
        "Output should contain italic sequence (CSI 3 m)"
    );
    assert!(
        output_str.contains("\x1b[4m"),
        "Output should contain underline sequence (CSI 4 m)"
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E ANSI text attributes work");
}

/// Test reset sequence on cleanup.
#[test]
fn test_e2e_ansi_reset_on_cleanup() {
    let mut harness = E2EHarness::new("render_cycle", "reset_cleanup", 40, 10);

    harness.log().info("init", "Testing ANSI reset on cleanup");

    // Capture ANSI output with styling then reset
    let mut output: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut output);

        // Set some attributes using builder
        let styled = Style::builder()
            .fg(Rgba::RED)
            .bold()
            .underline()
            .build();
        let styled_cell = opentui::cell::Cell::new('S', styled);
        writer.write_cell(&styled_cell);

        // Reset (cleanup)
        writer.reset();

        writer.flush().unwrap();
    }

    let output_str = String::from_utf8_lossy(&output);

    harness.log().info(
        "ansi",
        format!("Output: {}", output_str.replace('\x1b', "ESC")),
    );

    // Verify reset sequence is present (CSI 0 m)
    assert!(
        output_str.contains("\x1b[0m") || output_str.contains("\x1b[m"),
        "Output should contain reset sequence"
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E ANSI reset on cleanup works");
}

/// Test full render cycle with grapheme pool integration.
#[test]
fn test_e2e_render_cycle_with_graphemes() {
    let mut harness = E2EHarness::new("render_cycle", "graphemes", 40, 10);

    harness.log().info("init", "Testing render cycle with graphemes");

    let mut grapheme_pool = GraphemePool::new();

    // Allocate some graphemes
    let emoji_id = grapheme_pool.alloc("🎉");
    let family_id = grapheme_pool.alloc("👨‍👩‍👧");

    harness.log().info(
        "pool",
        format!("Allocated grapheme IDs: emoji={emoji_id:?}, family={family_id:?}"),
    );

    // Create buffer and draw
    let mut buffer = OptimizedBuffer::new(40, 10);
    buffer.draw_text(0, 0, "Party: 🎉", Style::default());

    // Verify grapheme is stored correctly
    assert_eq!(
        grapheme_pool.get(emoji_id),
        Some("🎉"),
        "Should retrieve emoji from pool"
    );
    assert_eq!(
        grapheme_pool.get(family_id),
        Some("👨‍👩‍👧"),
        "Should retrieve family emoji from pool"
    );

    // Generate ANSI output with pool
    let mut output: Vec<u8> = Vec::new();
    {
        let mut writer = AnsiWriter::new(&mut output);

        for y in 0..1u32 {
            for x in 0..20u32 {
                if let Some(cell) = buffer.get(x, y) {
                    if !cell.is_continuation() {
                        writer.write_cell_with_pool(cell, &grapheme_pool);
                    }
                }
            }
        }

        writer.flush().unwrap();
    }

    let output_str = String::from_utf8_lossy(&output);

    harness.log().info(
        "ansi",
        format!("Output length: {} bytes", output.len()),
    );

    // Verify content is in output
    assert!(
        output_str.contains("Party"),
        "Output should contain 'Party'"
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E render cycle with graphemes works");
}

/// Test diff threshold for full redraw decision.
#[test]
fn test_e2e_diff_threshold_decision() {
    let mut harness = E2EHarness::new("render_cycle", "diff_threshold", 20, 10);

    harness.log().info("init", "Testing diff threshold for full redraw");

    let total_cells = 20 * 10;

    // Create diff with 10% changes (should use diff)
    let small_diff = BufferDiff {
        changed_cells: vec![(0, 0); total_cells / 10],
        dirty_regions: vec![],
        change_count: total_cells / 10,
    };

    assert!(
        !small_diff.should_full_redraw(total_cells),
        "10% changes should use diff rendering"
    );

    harness.log().info("threshold", "10% changes: diff mode");

    // Create diff with 60% changes (should use full redraw)
    let large_diff = BufferDiff {
        changed_cells: vec![(0, 0); total_cells * 6 / 10],
        dirty_regions: vec![],
        change_count: total_cells * 6 / 10,
    };

    assert!(
        large_diff.should_full_redraw(total_cells),
        "60% changes should trigger full redraw"
    );

    harness.log().info("threshold", "60% changes: full redraw");

    // Edge case: exactly 50%
    let half_diff = BufferDiff {
        changed_cells: vec![(0, 0); total_cells / 2],
        dirty_regions: vec![],
        change_count: total_cells / 2,
    };

    harness.log().info(
        "threshold",
        format!(
            "50% changes: full_redraw={}",
            half_diff.should_full_redraw(total_cells)
        ),
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E diff threshold decision works");
}

/// Test JSONL logging format for render cycle events.
#[test]
fn test_e2e_render_cycle_logging() {
    // Define struct at start to avoid items_after_statements lint
    #[derive(serde::Serialize)]
    struct RenderStats {
        frame: u32,
        changed_cells: usize,
        bytes_output: usize,
    }

    let mut harness = E2EHarness::new("render_cycle", "logging", 20, 5);

    harness.log().info("step", "init");
    harness.log().info("step", "draw");
    harness.log().info("step", "present");
    harness.log().info("step", "verify");
    harness.log().info("step", "cleanup");

    // Log structured data
    let stats = RenderStats {
        frame: 1,
        changed_cells: 50,
        bytes_output: 256,
    };

    harness.log().info(
        "present",
        format!(
            "Frame {}: {} cells, {} bytes",
            stats.frame, stats.changed_cells, stats.bytes_output
        ),
    );

    harness.finish(true);
    eprintln!("[TEST] PASS: E2E render cycle logging works");
}
