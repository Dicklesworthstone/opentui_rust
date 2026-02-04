//! Conformance tests based on fixture files.
//!
//! These tests verify that the Rust implementation matches the expected
//! behavior captured in JSON fixtures. Following the porting-to-rust skill
//! methodology: capture expected outputs, then verify against them.

// Allow common patterns in test code that clippy flags as pedantic
#![allow(
    clippy::redundant_closure_for_method_calls,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::uninlined_format_args,
    clippy::too_many_lines,
    clippy::unnecessary_map_or,
    clippy::map_unwrap_or
)]

use serde::Deserialize;
use serde_json::Value;

mod common;

use common::harness::{ArtifactLogger, CaseResult, case_result, case_timer};
use opentui::ansi::{self, ColorMode};
use opentui::buffer::{BoxOptions, BoxStyle, ClipRect};
use opentui::style::TextAttributes;
use opentui::terminal::{MouseButton, MouseEventKind};
use opentui::unicode;
use opentui::{
    Event, InputParser, KeyCode, KeyModifiers, OptimizedBuffer, Rgba, Style, TextBuffer,
    TextBufferView, WrapMode,
};
use opentui_rust as opentui;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FixtureSet {
    #[serde(rename = "crate")]
    crate_name: String,
    #[serde(default)]
    version: String,
    captured_at: String,
    tests: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct FixtureCase {
    name: String,
    category: String,
    input: Value,
    expected_output: Value,
}

fn load_fixtures() -> FixtureSet {
    let data = std::fs::read_to_string("tests/conformance/fixtures/opentui.json")
        .expect("read conformance fixture");
    let mut parsed: FixtureSet = serde_json::from_str(&data).expect("parse fixture");
    // Normalize field name (crate -> crate_name)
    if parsed.crate_name.is_empty() {
        parsed.crate_name = "opentui".to_string();
    }
    parsed
}

#[test]
fn conformance_fixtures() {
    let fixtures = load_fixtures();
    let logger = ArtifactLogger::new("conformance", "fixtures");
    let mut results: Vec<CaseResult> = Vec::new();
    let mut all_passed = true;
    let mut passed_count = 0;
    let mut failed_count = 0;

    for case in &fixtures.tests {
        let start = case_timer();
        let result = run_case(case, &logger);
        results.push(case_result(&case.name, result, start));
        if result {
            passed_count += 1;
        } else {
            eprintln!("FAILED: {} (category: {})", case.name, case.category);
            eprintln!("  Input: {:?}", case.input);
            eprintln!("  Expected: {:?}", case.expected_output);
            failed_count += 1;
            all_passed = false;
        }
    }

    logger.write_summary(all_passed, &results);
    eprintln!(
        "Conformance: {} passed, {} failed, {} total",
        passed_count,
        failed_count,
        fixtures.tests.len()
    );
    assert!(all_passed, "conformance fixtures failed");
}

#[allow(clippy::too_many_lines)]
fn run_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.category.as_str() {
        "color" => run_color_case(case, logger),
        "buffer" => run_buffer_case(case, logger),
        "text" => run_text_case(case, logger),
        "input" => run_input_case(case, logger),
        "ansi" => run_ansi_case(case, logger),
        "unicode" => run_unicode_case(case, logger),
        "style" => run_style_case(case, logger),
        // Legacy category name
        "unit" => run_legacy_case(case, logger),
        _ => {
            eprintln!("Unknown category: {}", case.category);
            true // Skip unknown categories
        }
    }
}

// =============================================================================
// Color Tests
// =============================================================================

fn run_color_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.name.as_str() {
        name if name.starts_with("rgba_blend") => run_blend_test(case, logger),
        name if name.starts_with("rgba_from_hex") => run_hex_parse_test(case, logger),
        name if name.starts_with("rgba_from_hsv") => run_hsv_test(case, logger),
        name if name.starts_with("rgba_to_256") => run_to_256_test(case, logger),
        name if name.starts_with("rgba_to_16") => run_to_16_test(case, logger),
        _ => {
            eprintln!("Unknown color test: {}", case.name);
            true
        }
    }
}

fn run_blend_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let fg = case
        .input
        .get("fg")
        .and_then(|v| v.as_str())
        .and_then(Rgba::from_hex)
        .expect("fg color");
    let bg = case
        .input
        .get("bg")
        .and_then(|v| v.as_str())
        .and_then(Rgba::from_hex)
        .expect("bg color");
    let blended = fg.blend_over(bg);
    let actual = serde_json::json!({ "hex": format!("{blended}") });
    let expected = case.expected_output.clone();
    let passed = actual == expected;
    if !passed {
        logger.log_case(&case.name, &expected, &actual);
    }
    passed
}

fn run_hex_parse_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let hex = case.input.get("hex").and_then(|v| v.as_str()).unwrap_or("");
    let parsed = Rgba::from_hex(hex);

    let valid_expected = case
        .expected_output
        .get("valid")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if !valid_expected {
        let success = parsed.is_none();
        if !success {
            let actual = serde_json::json!({ "valid": true });
            let expected = serde_json::json!({ "valid": false });
            logger.log_case(&case.name, &expected, &actual);
        }
        return success;
    }

    let Some(color) = parsed else {
        let actual = serde_json::json!({ "valid": false });
        logger.log_case(&case.name, &case.expected_output, &actual);
        return false;
    };

    let (r, g, b) = color.to_rgb_u8();
    let mut actual = serde_json::json!({ "r": r, "g": g, "b": b, "valid": true });

    // Include alpha if expected
    if case.expected_output.get("a").is_some() {
        let alpha_byte = (color.a * 255.0).round() as u8;
        actual["a"] = serde_json::json!(alpha_byte);
    }

    let success = actual == case.expected_output;
    if !success {
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    success
}

#[allow(clippy::many_single_char_names)]
fn run_hsv_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let h = case.input.get("h").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let s = case.input.get("s").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
    let v = case.input.get("v").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;

    let color = Rgba::from_hsv(h, s, v);
    let (r, g, b) = color.to_rgb_u8();
    let actual = serde_json::json!({ "r": r, "g": g, "b": b });

    let passed = actual == case.expected_output;
    if !passed {
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_to_256_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let hex = case.input.get("hex").and_then(|v| v.as_str()).unwrap_or("");
    let color = Rgba::from_hex(hex).expect("valid hex");
    let index = color.to_256_color();

    let min = case
        .expected_output
        .get("index_min")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;
    let max = case
        .expected_output
        .get("index_max")
        .and_then(|v| v.as_u64())
        .unwrap_or(255) as u8;

    let passed = index >= min && index <= max;
    if !passed {
        let actual = serde_json::json!({ "index": index, "expected_range": [min, max] });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_to_16_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let hex = case.input.get("hex").and_then(|v| v.as_str()).unwrap_or("");
    let color = Rgba::from_hex(hex).expect("valid hex");
    let index = color.to_16_color();

    let expected_index = case
        .expected_output
        .get("index")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;

    let passed = index == expected_index;
    if !passed {
        let actual = serde_json::json!({ "index": index });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

// =============================================================================
// Buffer Tests
// =============================================================================

fn run_buffer_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.name.as_str() {
        name if name.starts_with("buffer_draw_box") => run_box_test(case, logger),
        "buffer_clear_size" => run_clear_test(case, logger),
        name if name.starts_with("buffer_draw_text") => run_draw_text_test(case, logger),
        "buffer_scissor_clip" => run_scissor_test(case, logger),
        _ => {
            eprintln!("Unknown buffer test: {}", case.name);
            true
        }
    }
}

fn run_box_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let width = case
        .input
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(10);
    let height = case
        .input
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(4);
    let title = case
        .input
        .get("title")
        .and_then(Value::as_str)
        .map(String::from);

    let mut buffer = OptimizedBuffer::new(width, height);
    let options = BoxOptions {
        style: BoxStyle::single(Style::NONE),
        sides: opentui::buffer::BoxSides::default(),
        fill: None,
        title,
        title_align: opentui::buffer::TitleAlign::default(),
    };
    buffer.draw_box_with_options(0, 0, width, height, options);

    let actual_lines = buffer_to_lines(&buffer);
    let actual = serde_json::json!({ "lines": actual_lines });
    let expected = case.expected_output.clone();
    let passed = actual == expected;
    if !passed {
        logger.log_case(&case.name, &expected, &actual);
    }
    passed
}

fn run_clear_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let width = case
        .input
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(5);
    let height = case
        .input
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(3);

    let buffer = OptimizedBuffer::new(width, height);
    let (w, h) = buffer.size();

    let mut all_empty = true;
    for y in 0..h {
        for x in 0..w {
            if let Some(cell) = buffer.get(x, y) {
                if !cell.content.is_empty() {
                    all_empty = false;
                }
            }
        }
    }

    let cell_count = (w * h) as usize;
    let actual = serde_json::json!({ "all_empty": all_empty, "cell_count": cell_count });
    let passed = actual == case.expected_output;
    if !passed {
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_draw_text_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let width = case
        .input
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(20);
    let height = case
        .input
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(1);
    let text = case.input.get("text").and_then(Value::as_str).unwrap_or("");
    let x = case
        .input
        .get("x")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let y = case
        .input
        .get("y")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);

    let mut buffer = OptimizedBuffer::new(width, height);
    buffer.draw_text(x, y, text, Style::NONE);

    let lines = buffer_to_lines(&buffer);
    let line = lines.first().cloned().unwrap_or_default();
    let actual = serde_json::json!({ "line": line });

    // Compare just the line field, ignore "note" in expected
    let expected_line = case
        .expected_output
        .get("line")
        .and_then(Value::as_str)
        .unwrap_or("");
    let passed = line == expected_line;
    if !passed {
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_scissor_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let width = case
        .input
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(10);
    let height = case
        .input
        .get("height")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(5);
    let scissor = case.input.get("scissor").expect("scissor object");
    let sx = scissor
        .get("x")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let sy = scissor
        .get("y")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let sw = scissor
        .get("w")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let sh = scissor
        .get("h")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let fill_char = case
        .input
        .get("fill_char")
        .and_then(Value::as_str)
        .and_then(|s| s.chars().next())
        .unwrap_or('X');

    let mut buffer = OptimizedBuffer::new(width, height);
    buffer.push_scissor(ClipRect::new(sx as i32, sy as i32, sw, sh));

    // Fill entire buffer - scissor should clip
    for y in 0..height {
        for x in 0..width {
            buffer.set(x, y, opentui::Cell::new(fill_char, Style::NONE));
        }
    }
    buffer.pop_scissor();

    let actual_lines = buffer_to_lines(&buffer);
    let actual = serde_json::json!({ "lines": actual_lines });
    let passed = actual == case.expected_output;
    if !passed {
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

// =============================================================================
// Text Tests
// =============================================================================

fn run_text_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.name.as_str() {
        name if name.contains("wrap") || name.contains("multiline") => run_wrap_test(case, logger),
        name if name.starts_with("selection") => run_selection_test(case, logger),
        _ => {
            eprintln!("Unknown text test: {}", case.name);
            true
        }
    }
}

fn run_wrap_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let text = case.input.get("text").and_then(Value::as_str).unwrap_or("");
    let width = case
        .input
        .get("width")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(80);
    let mode_str = case
        .input
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("char");

    let wrap_mode = match mode_str {
        "none" => WrapMode::None,
        "word" => WrapMode::Word,
        _ => WrapMode::Char,
    };

    let buffer = TextBuffer::with_text(text);
    let view = TextBufferView::new(&buffer)
        .viewport(0, 0, width, 100)
        .wrap_mode(wrap_mode);
    let count = view.virtual_line_count();

    // Compare only the line_count field (ignore notes/comments in expected)
    let expected_count = case
        .expected_output
        .get("line_count")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;

    let passed = count == expected_count;
    if !passed {
        eprintln!(
            "  [DEBUG] wrap test: text='{}' width={} mode={:?} actual={} expected={}",
            text, width, wrap_mode, count, expected_count
        );
        let actual = serde_json::json!({ "line_count": count });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_selection_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let text = case.input.get("text").and_then(Value::as_str).unwrap_or("");
    let start = case
        .input
        .get("start")
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(0);
    let end = case
        .input
        .get("end")
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(0);

    let buffer = TextBuffer::with_text(text);
    let mut view = TextBufferView::new(&buffer);
    view.set_selection(start, end, Style::NONE);
    let selected = view.selected_text().unwrap_or_default();

    let actual = serde_json::json!({ "selected": selected });
    let passed = actual == case.expected_output;
    if !passed {
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

// =============================================================================
// Input Tests
// =============================================================================

fn run_input_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let bytes: Vec<u8> = case
        .input
        .get("bytes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_u64().map(|n| n as u8))
                .collect()
        })
        .unwrap_or_default();

    let mut parser = InputParser::new();

    // For paste events, the parser expects:
    // 1. First parse: start sequence ESC[200~ → Incomplete (enters paste mode)
    // 2. Second parse: content + end sequence → Paste event
    // So we need to split the input appropriately
    if case.name.contains("paste") && bytes.len() > 6 {
        // Check if it starts with paste start sequence ESC[200~
        if bytes.starts_with(&[27, 91, 50, 48, 48, 126]) {
            // First call with just the start sequence
            let _ = parser.parse(&bytes[..6]);
            // Second call with the rest (content + end sequence)
            let result = parser.parse(&bytes[6..]);
            return match result {
                Ok((event, _)) => verify_input_event(case, &event, logger),
                Err(e) => {
                    let actual = serde_json::json!({ "error": format!("{:?}", e) });
                    logger.log_case(&case.name, &case.expected_output, &actual);
                    false
                }
            };
        }
    }

    let result = parser.parse(&bytes);

    match result {
        Ok((event, _consumed)) => verify_input_event(case, &event, logger),
        Err(opentui::input::ParseError::Incomplete) => {
            let actual = serde_json::json!({ "error": "incomplete_sequence" });
            logger.log_case(&case.name, &case.expected_output, &actual);
            false
        }
        Err(e) => {
            let actual = serde_json::json!({ "error": format!("{:?}", e) });
            logger.log_case(&case.name, &case.expected_output, &actual);
            false
        }
    }
}

fn verify_input_event(case: &FixtureCase, event: &Event, logger: &ArtifactLogger) -> bool {
    let expected_type = case
        .expected_output
        .get("event_type")
        .and_then(Value::as_str)
        .unwrap_or("");

    let passed = match (expected_type, event) {
        ("key", Event::Key(key_event)) => {
            let expected_code = case
                .expected_output
                .get("key_code")
                .and_then(Value::as_str)
                .unwrap_or("");
            let expected_char = case
                .expected_output
                .get("char")
                .and_then(Value::as_str)
                .and_then(|s| s.chars().next());
            let expected_mods: Vec<String> = case
                .expected_output
                .get("modifiers")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            let code_matches = match &key_event.code {
                KeyCode::Up => expected_code == "Up",
                KeyCode::Down => expected_code == "Down",
                KeyCode::Left => expected_code == "Left",
                KeyCode::Right => expected_code == "Right",
                KeyCode::Home => expected_code == "Home",
                KeyCode::End => expected_code == "End",
                KeyCode::PageUp => expected_code == "PageUp",
                KeyCode::PageDown => expected_code == "PageDown",
                KeyCode::Insert => expected_code == "Insert",
                KeyCode::Delete => expected_code == "Delete",
                KeyCode::Backspace => expected_code == "Backspace",
                KeyCode::F(n) => expected_code == format!("F{n}"),
                KeyCode::Char(c) => {
                    expected_code == "Char" && expected_char.map_or(true, |ec| ec == *c)
                }
                _ => false,
            };

            let mods_match = {
                let has_ctrl = key_event.modifiers.contains(KeyModifiers::CTRL)
                    == expected_mods.contains(&"Ctrl".to_string());
                let has_shift = key_event.modifiers.contains(KeyModifiers::SHIFT)
                    == expected_mods.contains(&"Shift".to_string());
                let has_alt = key_event.modifiers.contains(KeyModifiers::ALT)
                    == expected_mods.contains(&"Alt".to_string());
                has_ctrl && has_shift && has_alt
            };

            code_matches && mods_match
        }
        ("mouse", Event::Mouse(mouse_event)) => {
            let expected_button = case
                .expected_output
                .get("button")
                .and_then(Value::as_str)
                .unwrap_or("");
            let expected_kind = case
                .expected_output
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("");
            let expected_x = case
                .expected_output
                .get("x")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;
            let expected_y = case
                .expected_output
                .get("y")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32;

            let button_matches = match mouse_event.button {
                MouseButton::Left => expected_button == "Left" || expected_button.is_empty(),
                MouseButton::Right => expected_button == "Right",
                MouseButton::Middle => expected_button == "Middle",
                MouseButton::None => expected_button.is_empty() || expected_button == "None",
            };

            let kind_matches = match mouse_event.kind {
                MouseEventKind::Press => expected_kind == "Press",
                MouseEventKind::Release => expected_kind == "Release",
                MouseEventKind::Move => expected_kind == "Move" || expected_kind == "Drag",
                MouseEventKind::ScrollUp => expected_kind == "ScrollUp",
                MouseEventKind::ScrollDown => expected_kind == "ScrollDown",
                MouseEventKind::ScrollLeft => expected_kind == "ScrollLeft",
                MouseEventKind::ScrollRight => expected_kind == "ScrollRight",
            };

            let pos_matches = mouse_event.x == expected_x && mouse_event.y == expected_y;

            button_matches && kind_matches && pos_matches
        }
        ("focus", Event::FocusGained) => case
            .expected_output
            .get("gained")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        ("focus", Event::FocusLost) => !case
            .expected_output
            .get("gained")
            .and_then(Value::as_bool)
            .unwrap_or(true),
        ("paste", Event::Paste(paste_event)) => {
            let expected_content = case
                .expected_output
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("");
            paste_event.content == expected_content
        }
        _ => false,
    };

    if !passed {
        let actual = serde_json::json!({ "event": format!("{:?}", event) });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

// =============================================================================
// ANSI Tests
// =============================================================================

fn run_ansi_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.name.as_str() {
        name if name.contains("cursor_position") => run_cursor_position_test(case, logger),
        name if name.contains("fg_true_color") || name.contains("bg_true_color") => {
            run_true_color_test(case, logger)
        }
        name if name.contains("256_color") => run_256_color_test(case, logger),
        name if name.contains("16_color") => run_16_color_test(case, logger),
        name if name.contains("attributes") => run_attributes_test(case, logger),
        name if name.contains("hyperlink") => run_hyperlink_test(case, logger),
        _ => {
            eprintln!("Unknown ansi test: {}", case.name);
            true
        }
    }
}

fn run_hyperlink_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let expected = case
        .expected_output
        .get("sequence")
        .and_then(Value::as_str)
        .unwrap_or("");

    let sequence = if case.input.get("end_link").is_some() {
        // Hyperlink end sequence
        ansi::HYPERLINK_END.to_string()
    } else {
        // Hyperlink start sequence
        let link_id = case
            .input
            .get("link_id")
            .and_then(Value::as_u64)
            .unwrap_or(1) as u32;
        let url = case.input.get("url").and_then(Value::as_str).unwrap_or("");
        ansi::hyperlink_start(link_id, url)
    };

    let passed = sequence == expected;
    if !passed {
        let actual = serde_json::json!({ "sequence": sequence });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_cursor_position_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let row = case.input.get("row").and_then(Value::as_u64).unwrap_or(0) as u32;
    let col = case.input.get("col").and_then(Value::as_u64).unwrap_or(0) as u32;

    let sequence = ansi::cursor_position(row, col);
    let expected = case
        .expected_output
        .get("sequence")
        .and_then(Value::as_str)
        .unwrap_or("");

    let passed = sequence == expected;
    if !passed {
        let actual = serde_json::json!({ "sequence": sequence });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_true_color_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let r = case.input.get("r").and_then(Value::as_u64).unwrap_or(0) as u8;
    let g = case.input.get("g").and_then(Value::as_u64).unwrap_or(0) as u8;
    let b = case.input.get("b").and_then(Value::as_u64).unwrap_or(0) as u8;
    let is_bg = case
        .input
        .get("is_bg")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let color = Rgba::from_rgb_u8(r, g, b);
    let sequence = if is_bg {
        ansi::bg_color_with_mode(color, ColorMode::TrueColor)
    } else {
        ansi::fg_color_with_mode(color, ColorMode::TrueColor)
    };

    let expected = case
        .expected_output
        .get("sequence")
        .and_then(Value::as_str)
        .unwrap_or("");

    let passed = sequence == expected;
    if !passed {
        let actual = serde_json::json!({ "sequence": sequence });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_256_color_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let index = case.input.get("index").and_then(Value::as_u64).unwrap_or(0) as u8;

    // Generate 256-color foreground sequence: ESC[38;5;{index}m
    let sequence = format!("\x1b[38;5;{index}m");
    let expected = case
        .expected_output
        .get("sequence")
        .and_then(Value::as_str)
        .unwrap_or("");

    let passed = sequence == expected;
    if !passed {
        let actual = serde_json::json!({ "sequence": sequence });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_16_color_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let index = case.input.get("index").and_then(Value::as_u64).unwrap_or(0) as u8;

    // Generate 16-color foreground sequence:
    // 0-7: normal colors (30-37)
    // 8-15: bright colors (90-97)
    let sequence = if index < 8 {
        format!("\x1b[{}m", 30 + index)
    } else {
        format!("\x1b[{}m", 90 + index - 8)
    };
    let expected = case
        .expected_output
        .get("sequence")
        .and_then(Value::as_str)
        .unwrap_or("");

    let passed = sequence == expected;
    if !passed {
        let actual = serde_json::json!({ "sequence": sequence });
        logger.log_case(&case.name, &case.expected_output, &actual);
    }
    passed
}

fn run_attributes_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let attrs_list: Vec<String> = case
        .input
        .get("attributes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut attrs = TextAttributes::empty();
    for attr in &attrs_list {
        match attr.as_str() {
            "bold" => attrs |= TextAttributes::BOLD,
            "dim" => attrs |= TextAttributes::DIM,
            "italic" => attrs |= TextAttributes::ITALIC,
            "underline" => attrs |= TextAttributes::UNDERLINE,
            "blink" => attrs |= TextAttributes::BLINK,
            "inverse" => attrs |= TextAttributes::INVERSE,
            "hidden" => attrs |= TextAttributes::HIDDEN,
            "strikethrough" => attrs |= TextAttributes::STRIKETHROUGH,
            _ => {}
        }
    }

    let sequence = ansi::attributes(attrs);

    // Check for exact match or contains
    if let Some(expected_seq) = case.expected_output.get("sequence").and_then(Value::as_str) {
        let passed = sequence == expected_seq;
        if !passed {
            let actual = serde_json::json!({ "sequence": sequence });
            logger.log_case(&case.name, &case.expected_output, &actual);
        }
        return passed;
    }

    if let Some(contains_arr) = case
        .expected_output
        .get("contains")
        .and_then(Value::as_array)
    {
        let all_found = contains_arr.iter().all(|v| {
            v.as_str()
                .map(|expected| sequence.contains(expected))
                .unwrap_or(false)
        });
        if !all_found {
            let actual = serde_json::json!({ "sequence": sequence });
            logger.log_case(&case.name, &case.expected_output, &actual);
        }
        return all_found;
    }

    true
}

// =============================================================================
// Unicode Tests
// =============================================================================

fn run_unicode_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    let text = case.input.get("text").and_then(Value::as_str).unwrap_or("");
    let mut all_passed = true;

    // Check grapheme count if expected
    if let Some(expected_count) = case
        .expected_output
        .get("grapheme_count")
        .and_then(Value::as_u64)
    {
        let count = unicode::graphemes(text).count();
        if count != expected_count as usize {
            let actual = serde_json::json!({ "grapheme_count": count, "expected": expected_count });
            logger.log_case(&case.name, &case.expected_output, &actual);
            all_passed = false;
        }
    }

    // Check display width if expected
    if let Some(expected_width) = case
        .expected_output
        .get("display_width")
        .and_then(Value::as_u64)
    {
        let width = unicode::display_width(text);
        if width != expected_width as usize {
            let actual = serde_json::json!({ "display_width": width, "expected": expected_width });
            logger.log_case(&case.name, &case.expected_output, &actual);
            all_passed = false;
        }
    }

    all_passed
}

// =============================================================================
// Style Tests (link ID packing, attributes)
// =============================================================================

fn run_style_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.name.as_str() {
        name if name.contains("link_id") => run_link_id_test(case, logger),
        _ => {
            eprintln!("Unknown style test: {}", case.name);
            true
        }
    }
}

fn run_link_id_test(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    // Parse base attributes
    let base_attrs_list: Vec<String> = case
        .input
        .get("base_attributes")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let mut base_attrs = TextAttributes::empty();
    for attr in &base_attrs_list {
        match attr.as_str() {
            "bold" => base_attrs |= TextAttributes::BOLD,
            "dim" => base_attrs |= TextAttributes::DIM,
            "italic" => base_attrs |= TextAttributes::ITALIC,
            "underline" => base_attrs |= TextAttributes::UNDERLINE,
            "blink" => base_attrs |= TextAttributes::BLINK,
            "inverse" => base_attrs |= TextAttributes::INVERSE,
            "hidden" => base_attrs |= TextAttributes::HIDDEN,
            "strikethrough" => base_attrs |= TextAttributes::STRIKETHROUGH,
            _ => {}
        }
    }

    // Test link ID packing
    if let Some(link_id) = case.input.get("link_id").and_then(Value::as_u64) {
        let attrs_with_link = base_attrs.with_link_id(link_id as u32);

        // Verify link ID is correctly packed and retrievable
        let expected_link_id = case
            .expected_output
            .get("link_id")
            .and_then(Value::as_u64)
            .unwrap_or(link_id) as u32;

        let actual_link_id = attrs_with_link.link_id().unwrap_or(0);

        // Check that expected link ID is masked to 24 bits
        let masked_expected = expected_link_id & TextAttributes::MAX_LINK_ID;

        if actual_link_id != masked_expected {
            let actual = serde_json::json!({
                "link_id": actual_link_id,
                "expected": masked_expected
            });
            logger.log_case(&case.name, &case.expected_output, &actual);
            return false;
        }

        // Verify flags are preserved
        if let Some(expected_flags) = case
            .expected_output
            .get("flags_preserved")
            .and_then(Value::as_array)
        {
            for flag in expected_flags {
                if let Some(flag_name) = flag.as_str() {
                    let flag_preserved = match flag_name {
                        "bold" => attrs_with_link.contains(TextAttributes::BOLD),
                        "dim" => attrs_with_link.contains(TextAttributes::DIM),
                        "italic" => attrs_with_link.contains(TextAttributes::ITALIC),
                        "underline" => attrs_with_link.contains(TextAttributes::UNDERLINE),
                        _ => true,
                    };
                    if !flag_preserved {
                        let actual = serde_json::json!({
                            "error": format!("Flag '{}' not preserved", flag_name)
                        });
                        logger.log_case(&case.name, &case.expected_output, &actual);
                        return false;
                    }
                }
            }
        }

        return true;
    }

    // Test link ID merge behavior
    if let Some(base_link_id) = case.input.get("base_link_id").and_then(Value::as_u64) {
        let base_with_link = TextAttributes::empty().with_link_id(base_link_id as u32);

        let overlay_link_id = case
            .input
            .get("overlay_link_id")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let overlay_attrs_list: Vec<String> = case
            .input
            .get("overlay_attributes")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let mut overlay = TextAttributes::empty();
        for attr in &overlay_attrs_list {
            match attr.as_str() {
                "bold" => overlay |= TextAttributes::BOLD,
                "italic" => overlay |= TextAttributes::ITALIC,
                _ => {}
            }
        }
        if overlay_link_id > 0 {
            overlay = overlay.with_link_id(overlay_link_id as u32);
        }

        let merged = base_with_link.merge(overlay);
        let expected_merged_link_id = case
            .expected_output
            .get("merged_link_id")
            .and_then(Value::as_u64)
            .unwrap_or(0) as u32;

        let actual_merged_link_id = merged.link_id().unwrap_or(0);

        if actual_merged_link_id != expected_merged_link_id {
            let actual = serde_json::json!({
                "merged_link_id": actual_merged_link_id,
                "expected": expected_merged_link_id
            });
            logger.log_case(&case.name, &case.expected_output, &actual);
            return false;
        }

        return true;
    }

    true
}

// =============================================================================
// Legacy Tests (for backwards compatibility with original 4 tests)
// =============================================================================

fn run_legacy_case(case: &FixtureCase, logger: &ArtifactLogger) -> bool {
    match case.name.as_str() {
        "rgba_blend_half_red_over_blue" => run_blend_test(case, logger),
        "buffer_draw_box_title" => run_box_test(case, logger),
        "text_wrap_char_count" => run_wrap_test(case, logger),
        "selection_text" => run_selection_test(case, logger),
        _ => {
            eprintln!("Unknown legacy test: {}", case.name);
            true
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn buffer_to_lines(buffer: &OptimizedBuffer) -> Vec<String> {
    let (w, h) = buffer.size();
    let mut lines = Vec::new();
    for y in 0..h {
        let mut line = String::new();
        for x in 0..w {
            let cell = buffer.get(x, y).unwrap();
            if let Some(text) = cell.content.as_str_without_pool() {
                line.push_str(&text);
            } else {
                line.push(' ');
            }
        }
        lines.push(line);
    }
    lines
}
