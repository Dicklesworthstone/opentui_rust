//! Golden file tests for rendering output.
//!
//! These tests verify that rendering produces expected ANSI output by comparing
//! against golden files. Run with `GOLDEN_UPDATE=1` or `BLESS=1` to update files.
//!
//! # Test Categories
//!
//! 1. **Basic Rendering** (5 cases): Empty buffers, single chars, boxes
//! 2. **Color Rendering** (5 cases): `TrueColor`, 256-color, 16-color, attributes
//! 3. **Complex Rendering** (5 cases): Alpha blending, scissor clipping
//! 4. **Unicode** (5 cases): Emoji, ZWJ, combining marks, mixed width
//! 5. **Demo Showcase** (5 cases): Tour screens, overlays, panels

mod common;

use common::golden::{GoldenMetadata, GoldenResult, compare_golden, current_date};
use opentui::OptimizedBuffer;
use opentui::ansi::AnsiWriter;
use opentui::buffer::{BoxStyle, ClipRect};
use opentui::cell::Cell;
use opentui::color::Rgba;
use opentui::grapheme_pool::GraphemePool;
use opentui::style::Style;
use opentui_rust as opentui;

// Color constants that aren't in the standard library
const CYAN: Rgba = Rgba::rgb(0.0, 1.0, 1.0);
const MAGENTA: Rgba = Rgba::rgb(1.0, 0.0, 1.0);
const YELLOW: Rgba = Rgba::rgb(1.0, 1.0, 0.0);

/// Render a buffer to ANSI bytes.
fn render_buffer_to_ansi(buffer: &OptimizedBuffer, pool: &GraphemePool) -> Vec<u8> {
    let mut output = Vec::with_capacity(8192);
    let mut writer = AnsiWriter::new(&mut output);

    let (width, height) = buffer.size();
    for y in 0..height {
        writer.move_cursor(y, 0);
        for x in 0..width {
            if let Some(cell) = buffer.get(x, y) {
                if !cell.is_continuation() {
                    writer.write_cell_with_pool(cell, pool);
                }
            }
        }
    }

    writer.reset();
    let _ = writer.flush();

    output
}

/// Create metadata for a test.
fn test_metadata(name: &str, width: u32, height: u32) -> GoldenMetadata {
    GoldenMetadata {
        name: name.to_string(),
        generated: current_date(),
        terminal: "xterm-256color".to_string(),
        size: (width, height),
        extra: vec![],
    }
}

/// Assert a golden file match.
fn assert_golden(name: &str, output: &[u8], width: u32, height: u32) {
    let metadata = test_metadata(name, width, height);
    match compare_golden(name, output, &metadata) {
        GoldenResult::Match => {}
        GoldenResult::Updated { path } => {
            eprintln!("Updated golden file: {}", path.display());
        }
        GoldenResult::NotFound { path } => {
            eprintln!("Created new golden file: {}", path.display());
        }
        GoldenResult::Mismatch { diff_summary, .. } => {
            unreachable!("Golden file mismatch for '{name}':\n{diff_summary}");
        }
    }
}

// ============================================================================
// Basic Rendering (5 cases)
// ============================================================================

#[test]
fn test_golden_empty_buffer_80x24() {
    let buffer = OptimizedBuffer::new(80, 24);
    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("empty_buffer_80x24", &output, 80, 24);
}

#[test]
fn test_golden_single_char_center() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);
    buffer.set(40, 12, Cell::new('X', Style::fg(Rgba::WHITE)));
    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("single_char_center", &output, 80, 24);
}

#[test]
fn test_golden_full_screen_text() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Fill with a pattern
    for y in 0..24 {
        for x in 0..80 {
            let ch = if (x + y) % 2 == 0 { '#' } else { '.' };
            buffer.set(x, y, Cell::new(ch, Style::fg(Rgba::WHITE)));
        }
    }

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("full_screen_text", &output, 80, 24);
}

#[test]
fn test_golden_box_single_line() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);
    buffer.draw_box(10, 5, 30, 10, BoxStyle::single(Style::fg(Rgba::WHITE)));

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("box_single_line", &output, 80, 24);
}

#[test]
fn test_golden_box_double_line() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);
    buffer.draw_box(10, 5, 30, 10, BoxStyle::double(Style::fg(CYAN)));

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("box_double_line", &output, 80, 24);
}

// ============================================================================
// Color Rendering (5 cases)
// ============================================================================

#[test]
fn test_golden_truecolor_gradient() {
    let mut buffer = OptimizedBuffer::new(80, 24);

    // Create a horizontal RGB gradient
    for x in 0..80 {
        let r = f32::from(u8::try_from(x).expect("x fits in u8")) / 79.0;
        for y in 0..24 {
            let g = f32::from(u8::try_from(y).expect("y fits in u8")) / 23.0;
            let b = 0.5;
            let color = Rgba::new(r, g, b, 1.0);
            buffer.set(x, y, Cell::new(' ', Style::bg(color)));
        }
    }

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("truecolor_gradient", &output, 80, 24);
}

#[test]
fn test_golden_color256_palette() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Display 256 color palette (16x16 grid)
    for i in 0u8..=255 {
        let x = u32::from((i % 16) * 4);
        let y = u32::from(i / 16);

        // Convert 256 color index to approximate RGB
        let color = color_256_to_rgba(i);
        buffer.fill_rect(x, y, 4, 1, color);
    }

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("color256_palette", &output, 80, 24);
}

#[test]
fn test_golden_color16_palette() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Standard 16 ANSI colors
    let colors = [
        Rgba::from_rgb_u8(0, 0, 0),       // Black
        Rgba::from_rgb_u8(128, 0, 0),     // Red
        Rgba::from_rgb_u8(0, 128, 0),     // Green
        Rgba::from_rgb_u8(128, 128, 0),   // Yellow
        Rgba::from_rgb_u8(0, 0, 128),     // Blue
        Rgba::from_rgb_u8(128, 0, 128),   // Magenta
        Rgba::from_rgb_u8(0, 128, 128),   // Cyan
        Rgba::from_rgb_u8(192, 192, 192), // White
        Rgba::from_rgb_u8(128, 128, 128), // Bright Black
        Rgba::from_rgb_u8(255, 0, 0),     // Bright Red
        Rgba::from_rgb_u8(0, 255, 0),     // Bright Green
        Rgba::from_rgb_u8(255, 255, 0),   // Bright Yellow
        Rgba::from_rgb_u8(0, 0, 255),     // Bright Blue
        Rgba::from_rgb_u8(255, 0, 255),   // Bright Magenta
        Rgba::from_rgb_u8(0, 255, 255),   // Bright Cyan
        Rgba::from_rgb_u8(255, 255, 255), // Bright White
    ];

    for (i, &color) in colors.iter().enumerate() {
        let i = u32::try_from(i).expect("palette index fits u32");
        let x = (i % 8) * 10;
        let y = (i / 8) * 3;
        buffer.fill_rect(x, y, 10, 3, color);
    }

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("color16_palette", &output, 80, 24);
}

#[test]
fn test_golden_bold_colors() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Draw text with bold attribute
    let bold_style = Style::fg(Rgba::WHITE).with_bold();
    buffer.draw_text(10, 5, "Bold White Text", bold_style);

    let bold_red = Style::fg(Rgba::RED).with_bold();
    buffer.draw_text(10, 7, "Bold Red Text", bold_red);

    let bold_green = Style::fg(Rgba::GREEN).with_bold();
    buffer.draw_text(10, 9, "Bold Green Text", bold_green);

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("bold_colors", &output, 80, 24);
}

#[test]
fn test_golden_dim_colors() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Draw text with dim attribute
    let dim_style = Style::fg(Rgba::WHITE).with_attributes(opentui::TextAttributes::DIM);
    buffer.draw_text(10, 5, "Dim White Text", dim_style);

    let dim_red = Style::fg(Rgba::RED).with_attributes(opentui::TextAttributes::DIM);
    buffer.draw_text(10, 7, "Dim Red Text", dim_red);

    let dim_green = Style::fg(Rgba::GREEN).with_attributes(opentui::TextAttributes::DIM);
    buffer.draw_text(10, 9, "Dim Green Text", dim_green);

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("dim_colors", &output, 80, 24);
}

// ============================================================================
// Complex Rendering (5 cases)
// ============================================================================

#[test]
fn test_golden_alpha_blend_50() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLUE);

    // Draw a base layer
    buffer.fill_rect(10, 5, 30, 10, Rgba::RED);

    // Overlay with 50% opacity
    buffer.push_opacity(0.5);
    buffer.fill_rect(20, 8, 30, 10, Rgba::GREEN);
    buffer.pop_opacity();

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("alpha_blend_50", &output, 80, 24);
}

#[test]
fn test_golden_scissor_clipped() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Push scissor to restrict to a region
    buffer.push_scissor(ClipRect::new(10, 5, 40, 14));

    // Draw something larger than the scissor region
    buffer.fill_rect(0, 0, 80, 24, Rgba::RED);
    buffer.draw_text(
        5,
        10,
        "This text should be clipped!",
        Style::fg(Rgba::WHITE),
    );

    buffer.pop_scissor();

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("scissor_clipped", &output, 80, 24);
}

#[test]
fn test_golden_nested_scissor() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    // Outer scissor
    buffer.push_scissor(ClipRect::new(5, 3, 70, 18));
    buffer.fill_rect(0, 0, 80, 24, Rgba::BLUE);

    // Inner scissor (should be intersection)
    buffer.push_scissor(ClipRect::new(20, 8, 40, 8));
    buffer.fill_rect(0, 0, 80, 24, Rgba::GREEN);

    buffer.pop_scissor();
    buffer.pop_scissor();

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("nested_scissor", &output, 80, 24);
}

#[test]
fn test_golden_opacity_stack() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::WHITE);

    // Stack multiple opacity levels
    buffer.push_opacity(0.8);
    buffer.fill_rect(10, 5, 60, 14, Rgba::RED);

    buffer.push_opacity(0.6);
    buffer.fill_rect(20, 8, 40, 8, Rgba::GREEN);

    buffer.push_opacity(0.4);
    buffer.fill_rect(30, 10, 20, 4, Rgba::BLUE);

    buffer.pop_opacity();
    buffer.pop_opacity();
    buffer.pop_opacity();

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("opacity_stack", &output, 80, 24);
}

#[test]
fn test_golden_wide_chars_cjk() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    let pool = GraphemePool::new();

    // CJK characters (each takes 2 cells)
    buffer.draw_text(5, 5, "Hello: ", Style::fg(Rgba::WHITE));

    // Draw CJK text - these are wide characters
    let cjk_text = "\u{4E2D}\u{6587}\u{5B57}\u{7B26}"; // 中文字符
    buffer.draw_text(12, 5, cjk_text, Style::fg(YELLOW));

    // Japanese hiragana
    let jp_text = "\u{3053}\u{3093}\u{306B}\u{3061}\u{306F}"; // こんにちは
    buffer.draw_text(5, 7, jp_text, Style::fg(CYAN));

    // Korean
    let kr_text = "\u{D55C}\u{AE00}"; // 한글
    buffer.draw_text(5, 9, kr_text, Style::fg(MAGENTA));

    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("wide_chars_cjk", &output, 80, 24);
}

// ============================================================================
// Unicode (5 cases)
// ============================================================================

#[test]
fn test_golden_emoji_basic() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    let pool = GraphemePool::new();

    // Basic emoji (single codepoints, typically 2 cells wide)
    buffer.draw_text(5, 5, "Smileys: ", Style::fg(Rgba::WHITE));
    buffer.draw_text(14, 5, "\u{1F600}\u{1F601}\u{1F602}", Style::NONE); // 😀😁😂

    buffer.draw_text(5, 7, "Animals: ", Style::fg(Rgba::WHITE));
    buffer.draw_text(14, 7, "\u{1F436}\u{1F431}\u{1F42D}", Style::NONE); // 🐶🐱🐭

    buffer.draw_text(5, 9, "Foods: ", Style::fg(Rgba::WHITE));
    buffer.draw_text(12, 9, "\u{1F34E}\u{1F34F}\u{1F34A}", Style::NONE); // 🍎🍏🍊

    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("emoji_basic", &output, 80, 24);
}

#[test]
fn test_golden_emoji_zwj() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    let pool = GraphemePool::new();

    // ZWJ (Zero Width Joiner) sequences
    buffer.draw_text(5, 5, "Family: ", Style::fg(Rgba::WHITE));
    // Family: man, woman, girl, boy joined with ZWJ
    buffer.draw_text(
        13,
        5,
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        Style::NONE,
    );

    buffer.draw_text(5, 7, "Flags: ", Style::fg(Rgba::WHITE));
    // Regional indicator sequences for US flag
    buffer.draw_text(12, 7, "\u{1F1FA}\u{1F1F8}", Style::NONE);

    buffer.draw_text(5, 9, "Profession: ", Style::fg(Rgba::WHITE));
    // Woman technologist (woman + ZWJ + laptop)
    buffer.draw_text(17, 9, "\u{1F469}\u{200D}\u{1F4BB}", Style::NONE);

    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("emoji_zwj", &output, 80, 24);
}

#[test]
fn test_golden_combining_marks() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    let pool = GraphemePool::new();

    // Combining diacritical marks
    buffer.draw_text(5, 5, "Accents: ", Style::fg(Rgba::WHITE));
    // e with acute, a with grave, n with tilde
    buffer.draw_text(14, 5, "e\u{0301} a\u{0300} n\u{0303}", Style::fg(YELLOW));

    buffer.draw_text(5, 7, "Multi: ", Style::fg(Rgba::WHITE));
    // a with multiple combining marks
    buffer.draw_text(12, 7, "a\u{0301}\u{0302}\u{0303}", Style::fg(CYAN));

    buffer.draw_text(5, 9, "Zalgo: ", Style::fg(Rgba::WHITE));
    // Text with many combining marks (zalgo-style)
    buffer.draw_text(
        12,
        9,
        "H\u{0336}\u{0335}E\u{0337}\u{0338}L\u{0336}P",
        Style::fg(Rgba::RED),
    );

    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("combining_marks", &output, 80, 24);
}

#[test]
fn test_golden_rtl_text() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    let pool = GraphemePool::new();

    // RTL text (Arabic and Hebrew)
    buffer.draw_text(5, 5, "Arabic: ", Style::fg(Rgba::WHITE));
    buffer.draw_text(
        13,
        5,
        "\u{0645}\u{0631}\u{062D}\u{0628}\u{0627}",
        Style::fg(YELLOW),
    ); // مرحبا

    buffer.draw_text(5, 7, "Hebrew: ", Style::fg(Rgba::WHITE));
    buffer.draw_text(13, 7, "\u{05E9}\u{05DC}\u{05D5}\u{05DD}", Style::fg(CYAN)); // שלום

    buffer.draw_text(5, 9, "Mixed: ", Style::fg(Rgba::WHITE));
    buffer.draw_text(
        12,
        9,
        "Hello \u{05E9}\u{05DC}\u{05D5}\u{05DD} World",
        Style::fg(Rgba::GREEN),
    );

    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("rtl_text", &output, 80, 24);
}

#[test]
fn test_golden_mixed_width() {
    let mut buffer = OptimizedBuffer::new(80, 24);
    buffer.clear(Rgba::BLACK);

    let pool = GraphemePool::new();

    // Mixed ASCII and wide characters on same line
    buffer.draw_text(
        5,
        5,
        "ASCII and \u{4E2D}\u{6587} mixed",
        Style::fg(Rgba::WHITE),
    );
    buffer.draw_text(
        5,
        7,
        "1234\u{3042}\u{3044}5678\u{3046}\u{3048}90",
        Style::fg(YELLOW),
    );
    buffer.draw_text(5, 9, "Tab\u{2192}ulation \u{2190}Arrow", Style::fg(CYAN));

    // Math symbols and special characters
    buffer.draw_text(
        5,
        11,
        "\u{221E} \u{2211} \u{220F} \u{222B}",
        Style::fg(MAGENTA),
    ); // ∞ ∑ ∏ ∫

    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("mixed_width", &output, 80, 24);
}

// ============================================================================
// Demo Showcase (5 cases)
// ============================================================================

#[test]
fn test_golden_tour_screen_1() {
    let mut buffer = OptimizedBuffer::new(120, 40);
    let dark_bg = Rgba::from_rgb_u8(32, 32, 48); // Dark blue-gray
    buffer.clear(dark_bg);

    // Title bar
    let title_bg = Rgba::from_rgb_u8(64, 64, 96);
    buffer.fill_rect(0, 0, 120, 1, title_bg);
    buffer.draw_text(
        2,
        0,
        " [H] Help  [/] Palette  [T] Tour  ",
        Style::fg(Rgba::WHITE).with_bold(),
    );
    buffer.draw_text(100, 0, " demo_showcase ", Style::fg(CYAN));

    // Main content area with welcome message
    buffer.draw_text(
        45,
        10,
        "Welcome to OpenTUI",
        Style::fg(Rgba::WHITE).with_bold(),
    );
    buffer.draw_text(
        35,
        12,
        "A terminal UI rendering engine in Rust",
        Style::fg(Rgba::from_rgb_u8(180, 180, 200)),
    );

    // Feature list
    buffer.draw_text(
        40,
        16,
        "\u{2022} Porter-Duff alpha blending",
        Style::fg(Rgba::GREEN),
    );
    buffer.draw_text(40, 17, "\u{2022} Scissor clipping", Style::fg(Rgba::GREEN));
    buffer.draw_text(
        40,
        18,
        "\u{2022} Double-buffered rendering",
        Style::fg(Rgba::GREEN),
    );
    buffer.draw_text(
        40,
        19,
        "\u{2022} Unicode & emoji support",
        Style::fg(Rgba::GREEN),
    );

    // Status bar
    buffer.fill_rect(0, 39, 120, 1, title_bg);
    buffer.draw_text(
        2,
        39,
        " Step 1/10  Press Space to continue ",
        Style::fg(Rgba::WHITE),
    );

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("tour_screen_1", &output, 120, 40);
}

#[test]
fn test_golden_tour_screen_5() {
    let mut buffer = OptimizedBuffer::new(120, 40);
    let dark_bg = Rgba::from_rgb_u8(32, 32, 48);
    buffer.clear(dark_bg);

    // Title bar
    let title_bg = Rgba::from_rgb_u8(64, 64, 96);
    buffer.fill_rect(0, 0, 120, 1, title_bg);
    buffer.draw_text(
        2,
        0,
        " [H] Help  [/] Palette  [T] Tour  ",
        Style::fg(Rgba::WHITE).with_bold(),
    );

    // Demo: Alpha blending showcase
    buffer.draw_text(
        10,
        3,
        "Alpha Blending Demo",
        Style::fg(Rgba::WHITE).with_bold(),
    );

    // Base layers
    buffer.fill_rect(15, 6, 30, 15, Rgba::RED);
    buffer.fill_rect(35, 8, 30, 15, Rgba::new(0.0, 0.0, 1.0, 0.7)); // Semi-transparent blue
    buffer.fill_rect(55, 10, 30, 15, Rgba::new(0.0, 1.0, 0.0, 0.5)); // More transparent green

    // Labels
    buffer.draw_text(20, 22, "Red (100%)", Style::fg(Rgba::WHITE));
    buffer.draw_text(38, 24, "Blue (70%)", Style::fg(Rgba::WHITE));
    buffer.draw_text(58, 26, "Green (50%)", Style::fg(Rgba::WHITE));

    // Status
    buffer.fill_rect(0, 39, 120, 1, title_bg);
    buffer.draw_text(
        2,
        39,
        " Step 5/10  Porter-Duff 'over' compositing ",
        Style::fg(Rgba::WHITE),
    );

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("tour_screen_5", &output, 120, 40);
}

#[test]
fn test_golden_help_overlay() {
    let mut buffer = OptimizedBuffer::new(120, 40);

    // Background content (simulated main UI)
    let dark_bg = Rgba::from_rgb_u8(32, 32, 48);
    buffer.clear(dark_bg);
    buffer.draw_text(10, 5, "Main UI Content Here", Style::fg(Rgba::WHITE));

    // Semi-transparent overlay
    buffer.push_opacity(0.85);
    let overlay_bg = Rgba::from_rgb_u8(16, 16, 32);
    buffer.fill_rect(20, 5, 80, 30, overlay_bg);
    buffer.pop_opacity();

    // Help box
    buffer.draw_box(20, 5, 80, 30, BoxStyle::double(Style::fg(CYAN)));
    buffer.draw_text(50, 6, " Help ", Style::fg(CYAN).with_bold());

    // Help content
    buffer.draw_text(
        25,
        9,
        "Keyboard Shortcuts:",
        Style::fg(Rgba::WHITE).with_bold(),
    );
    buffer.draw_text(25, 11, "  H       Toggle this help", Style::fg(Rgba::WHITE));
    buffer.draw_text(
        25,
        12,
        "  /       Open command palette",
        Style::fg(Rgba::WHITE),
    );
    buffer.draw_text(25, 13, "  T       Start/stop tour", Style::fg(Rgba::WHITE));
    buffer.draw_text(
        25,
        14,
        "  D       Toggle debug panel",
        Style::fg(Rgba::WHITE),
    );
    buffer.draw_text(25, 15, "  Q       Quit application", Style::fg(Rgba::WHITE));
    buffer.draw_text(25, 17, "Mouse:", Style::fg(Rgba::WHITE).with_bold());
    buffer.draw_text(25, 19, "  Click   Select item", Style::fg(Rgba::WHITE));
    buffer.draw_text(25, 20, "  Scroll  Navigate lists", Style::fg(Rgba::WHITE));

    buffer.draw_text(
        45,
        32,
        "Press H or Esc to close",
        Style::fg(Rgba::from_rgb_u8(128, 128, 160)),
    );

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("help_overlay", &output, 120, 40);
}

#[test]
fn test_golden_debug_panel() {
    let mut buffer = OptimizedBuffer::new(120, 40);
    let dark_bg = Rgba::from_rgb_u8(32, 32, 48);
    buffer.clear(dark_bg);

    // Main content area
    buffer.draw_text(10, 5, "Application Content", Style::fg(Rgba::WHITE));

    // Debug panel on the right side
    let panel_bg = Rgba::from_rgb_u8(24, 24, 36);
    buffer.fill_rect(85, 0, 35, 40, panel_bg);
    buffer.draw_text(87, 1, "Debug Panel", Style::fg(YELLOW).with_bold());

    // Stats
    buffer.draw_text(87, 3, "FPS: 60.0", Style::fg(Rgba::GREEN));
    buffer.draw_text(87, 4, "Frame: 12345", Style::fg(Rgba::WHITE));
    buffer.draw_text(87, 5, "Dirty: 142 cells", Style::fg(Rgba::WHITE));

    buffer.draw_text(87, 7, "Buffer", Style::fg(CYAN).with_bold());
    buffer.draw_text(87, 8, "  Size: 120x40", Style::fg(Rgba::WHITE));
    buffer.draw_text(87, 9, "  Cells: 4800", Style::fg(Rgba::WHITE));

    buffer.draw_text(87, 11, "Memory", Style::fg(CYAN).with_bold());
    buffer.draw_text(87, 12, "  Cells: 48.0 KB", Style::fg(Rgba::WHITE));
    buffer.draw_text(87, 13, "  Graphemes: 128", Style::fg(Rgba::WHITE));
    buffer.draw_text(87, 14, "  Links: 5", Style::fg(Rgba::WHITE));

    buffer.draw_text(87, 16, "Input", Style::fg(CYAN).with_bold());
    buffer.draw_text(87, 17, "  Events: 2341", Style::fg(Rgba::WHITE));
    buffer.draw_text(87, 18, "  Keys: 1823", Style::fg(Rgba::WHITE));
    buffer.draw_text(87, 19, "  Mouse: 518", Style::fg(Rgba::WHITE));

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("debug_panel", &output, 120, 40);
}

#[test]
fn test_golden_command_palette() {
    let mut buffer = OptimizedBuffer::new(120, 40);
    let dark_bg = Rgba::from_rgb_u8(32, 32, 48);
    buffer.clear(dark_bg);

    // Background content (dimmed)
    buffer.push_opacity(0.3);
    buffer.draw_text(10, 5, "Main Application", Style::fg(Rgba::WHITE));
    buffer.draw_text(10, 7, "Some content here", Style::fg(Rgba::WHITE));
    buffer.pop_opacity();

    // Command palette overlay
    let palette_bg = Rgba::from_rgb_u8(40, 40, 60);
    buffer.fill_rect(25, 8, 70, 20, palette_bg);
    buffer.draw_box(25, 8, 70, 20, BoxStyle::rounded(Style::fg(CYAN)));

    // Search input
    let input_bg = Rgba::from_rgb_u8(60, 60, 80);
    buffer.fill_rect(27, 10, 66, 1, input_bg);
    buffer.draw_text(28, 10, "> theme", Style::fg(Rgba::WHITE));
    buffer.set(35, 10, Cell::new('\u{2588}', Style::fg(Rgba::WHITE))); // Cursor

    // Results
    buffer.draw_text(28, 12, "\u{2192} Switch Theme", Style::fg(CYAN));
    buffer.draw_text(28, 13, "   Theme: Dark", Style::fg(Rgba::WHITE));
    buffer.draw_text(28, 14, "   Theme: Light", Style::fg(Rgba::WHITE));
    buffer.draw_text(28, 15, "   Theme: Monokai", Style::fg(Rgba::WHITE));
    buffer.draw_text(28, 16, "   Theme: Dracula", Style::fg(Rgba::WHITE));

    // Hint
    buffer.draw_text(
        35,
        26,
        "Type to search, Enter to select, Esc to cancel",
        Style::fg(Rgba::from_rgb_u8(100, 100, 140)),
    );

    let pool = GraphemePool::new();
    let output = render_buffer_to_ansi(&buffer, &pool);
    assert_golden("command_palette", &output, 120, 40);
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Convert 256-color index to approximate RGBA.
fn color_256_to_rgba(index: u8) -> Rgba {
    if index < 16 {
        // Standard colors
        let colors: [(u8, u8, u8); 16] = [
            (0, 0, 0),
            (128, 0, 0),
            (0, 128, 0),
            (128, 128, 0),
            (0, 0, 128),
            (128, 0, 128),
            (0, 128, 128),
            (192, 192, 192),
            (128, 128, 128),
            (255, 0, 0),
            (0, 255, 0),
            (255, 255, 0),
            (0, 0, 255),
            (255, 0, 255),
            (0, 255, 255),
            (255, 255, 255),
        ];
        let (r, g, b) = colors[index as usize];
        Rgba::from_rgb_u8(r, g, b)
    } else if index < 232 {
        // 6x6x6 color cube
        let i = index - 16;
        let r = (i / 36) % 6;
        let g = (i / 6) % 6;
        let b = i % 6;
        let to_255 = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
        Rgba::from_rgb_u8(to_255(r), to_255(g), to_255(b))
    } else {
        // Grayscale ramp
        let gray = 8 + (index - 232) * 10;
        Rgba::from_rgb_u8(gray, gray, gray)
    }
}
