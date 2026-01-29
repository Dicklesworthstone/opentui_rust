//! E2E Performance Regression Tests
//!
//! Automated performance regression detection for OpenTUI. These tests compare
//! actual performance against stored baselines and fail if regressions exceed
//! the threshold.
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all performance tests
//! cargo test --test e2e_perf
//!
//! # Run with verbose output
//! cargo test --test e2e_perf -- --nocapture
//!
//! # Update baselines (run with env var)
//! UPDATE_BASELINES=1 cargo test --test e2e_perf -- --nocapture
//! ```
//!
//! ## Baseline Management
//!
//! Baselines are stored in `tests/baselines/perf_baseline.json`. To update:
//! 1. Run tests with `UPDATE_BASELINES=1` to generate new timing data
//! 2. Review the suggested changes
//! 3. Manually update the baseline file with new values
//!
//! ## CI Integration
//!
//! The `.github/workflows/perf.yml` workflow runs these tests and:
//! - Alerts on >20% regression
//! - Records improvements >10%
//! - Archives results for trend analysis

#![allow(clippy::doc_markdown)]
#![allow(clippy::missing_panics_doc)]

use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Path to baseline file.
const BASELINE_PATH: &str = "tests/baselines/perf_baseline.json";

/// Default regression threshold (20%).
const DEFAULT_REGRESSION_THRESHOLD: f64 = 0.20;

/// Baseline configuration loaded from JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerfBaseline {
    version: String,
    generated_at: String,
    machine_info: MachineInfo,
    thresholds: Thresholds,
    baselines: Baselines,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MachineInfo {
    os: String,
    note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Thresholds {
    regression_threshold_percent: f64,
    improvement_threshold_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Baselines {
    rendering: RenderingBaselines,
    buffer_operations: BufferBaselines,
    text_operations: TextBaselines,
    input_processing: InputBaselines,
    e2e: E2EBaselines,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RenderingBaselines {
    full_screen_clear_100x_ms: u64,
    full_screen_text_render_100x_ms: u64,
    diff_render_10pct_changes_ms: u64,
    diff_render_50pct_changes_ms: u64,
    diff_render_90pct_changes_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BufferBaselines {
    large_buffer_create_200x60_ms: u64,
    scissor_push_pop_1000x_ms: u64,
    opacity_stack_1000x_ms: u64,
    cell_iteration_full_buffer_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TextBaselines {
    large_file_load_100kb_ms: u64,
    text_insert_1000_chars_ms: u64,
    undo_redo_100x_ms: u64,
    wrap_recalculation_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InputBaselines {
    parse_1000_keystrokes_ms: u64,
    parse_1000_mouse_events_ms: u64,
    parse_large_paste_10kb_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct E2EBaselines {
    demo_showcase_tour_max_runtime_s: u64,
    demo_showcase_tour_min_fps: u64,
    demo_showcase_headless_smoke_max_ms: u64,
}

/// Result of a performance test.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerfResult {
    name: String,
    actual_ms: u64,
    baseline_ms: u64,
    diff_percent: f64,
    status: PerfStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum PerfStatus {
    Pass,
    Regression,
    Improvement,
}

impl PerfResult {
    fn new(name: &str, actual_ms: u64, baseline_ms: u64, threshold_percent: f64) -> Self {
        let diff_percent = if baseline_ms == 0 {
            0.0
        } else {
            ((actual_ms as f64 - baseline_ms as f64) / baseline_ms as f64) * 100.0
        };

        let status = if diff_percent > threshold_percent * 100.0 {
            PerfStatus::Regression
        } else if diff_percent < -10.0 {
            // 10% improvement threshold
            PerfStatus::Improvement
        } else {
            PerfStatus::Pass
        };

        Self {
            name: name.to_string(),
            actual_ms,
            baseline_ms,
            diff_percent,
            status,
        }
    }
}

/// Collected results from all performance tests.
#[derive(Debug, Default, Serialize, Deserialize)]
struct PerfReport {
    results: Vec<PerfResult>,
    regressions: Vec<String>,
    improvements: Vec<String>,
    all_passed: bool,
}

impl PerfReport {
    fn new() -> Self {
        Self::default()
    }

    fn add(&mut self, result: PerfResult) {
        match result.status {
            PerfStatus::Regression => {
                self.regressions.push(format!(
                    "{}: {:.1}% slower ({} ms vs {} ms baseline)",
                    result.name, result.diff_percent, result.actual_ms, result.baseline_ms
                ));
            }
            PerfStatus::Improvement => {
                self.improvements.push(format!(
                    "{}: {:.1}% faster ({} ms vs {} ms baseline)",
                    result.name,
                    -result.diff_percent,
                    result.actual_ms,
                    result.baseline_ms
                ));
            }
            PerfStatus::Pass => {}
        }
        self.results.push(result);
    }

    fn finalize(&mut self) {
        self.all_passed = self.regressions.is_empty();
    }

    fn to_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Performance Test Report ===\n\n");

        if !self.regressions.is_empty() {
            summary.push_str("REGRESSIONS DETECTED:\n");
            for r in &self.regressions {
                summary.push_str(&format!("  - {r}\n"));
            }
            summary.push('\n');
        }

        if !self.improvements.is_empty() {
            summary.push_str("Improvements:\n");
            for i in &self.improvements {
                summary.push_str(&format!("  + {i}\n"));
            }
            summary.push('\n');
        }

        summary.push_str(&format!(
            "Total: {} tests, {} regressions, {} improvements\n",
            self.results.len(),
            self.regressions.len(),
            self.improvements.len()
        ));

        if self.all_passed {
            summary.push_str("\nResult: PASS\n");
        } else {
            summary.push_str("\nResult: FAIL (regressions detected)\n");
        }

        summary
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

/// Load baseline configuration.
fn load_baseline() -> Option<PerfBaseline> {
    let path = Path::new(BASELINE_PATH);
    if !path.exists() {
        eprintln!("Warning: Baseline file not found at {BASELINE_PATH}");
        return None;
    }

    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Time a closure and return duration in milliseconds.
fn time_ms<F: FnMut()>(mut f: F, iterations: u32) -> u64 {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    start.elapsed().as_millis() as u64
}

/// Check if we should update baselines.
fn should_update_baselines() -> bool {
    std::env::var("UPDATE_BASELINES").is_ok()
}

// =============================================================================
// RENDERING PERFORMANCE TESTS
// =============================================================================

#[test]
fn perf_full_screen_clear() {
    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.rendering.full_screen_clear_100x_ms)
        .unwrap_or(50);

    let mut buffer = OptimizedBuffer::new(200, 50);
    let actual = time_ms(|| buffer.clear(Rgba::BLACK), 100);

    let result = PerfResult::new("full_screen_clear_100x", actual, expected, threshold);

    println!(
        "full_screen_clear_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_full_screen_text_render() {
    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.rendering.full_screen_text_render_100x_ms)
        .unwrap_or(100);

    let mut buffer = OptimizedBuffer::new(200, 50);
    let style = Style::fg(Rgba::WHITE);
    let text = "Hello, OpenTUI! Performance test line.";

    let actual = time_ms(
        || {
            for row in 0..50 {
                buffer.draw_text(0, row, text, style);
            }
        },
        100,
    );

    let result = PerfResult::new("full_screen_text_render_100x", actual, expected, threshold);

    println!(
        "full_screen_text_render_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

// =============================================================================
// BUFFER OPERATIONS TESTS
// =============================================================================

#[test]
fn perf_large_buffer_create() {
    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.buffer_operations.large_buffer_create_200x60_ms)
        .unwrap_or(10);

    let actual = time_ms(|| drop(OptimizedBuffer::new(200, 60)), 100);

    let result = PerfResult::new("large_buffer_create_100x", actual, expected, threshold);

    println!(
        "large_buffer_create_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_scissor_push_pop() {
    use opentui::buffer::ClipRect;

    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.buffer_operations.scissor_push_pop_1000x_ms)
        .unwrap_or(20);

    let mut buffer = OptimizedBuffer::new(200, 50);

    let actual = time_ms(
        || {
            for _ in 0..100 {
                buffer.push_scissor(ClipRect::new(10, 10, 50, 30));
                buffer.pop_scissor();
            }
        },
        10,
    );

    let result = PerfResult::new("scissor_push_pop_1000x", actual, expected, threshold);

    println!(
        "scissor_push_pop_1000x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_opacity_stack() {
    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.buffer_operations.opacity_stack_1000x_ms)
        .unwrap_or(20);

    let mut buffer = OptimizedBuffer::new(200, 50);

    let actual = time_ms(
        || {
            for _ in 0..100 {
                buffer.push_opacity(0.5);
                buffer.pop_opacity();
            }
        },
        10,
    );

    let result = PerfResult::new("opacity_stack_1000x", actual, expected, threshold);

    println!(
        "opacity_stack_1000x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_cell_iteration() {
    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.buffer_operations.cell_iteration_full_buffer_ms)
        .unwrap_or(30);

    let mut buffer = OptimizedBuffer::new(200, 50);
    buffer.clear(Rgba::WHITE);
    let cell = Cell::new('X', Style::fg(Rgba::RED));

    let actual = time_ms(
        || {
            for y in 0..50 {
                for x in 0..200 {
                    buffer.set(x, y, cell);
                }
            }
        },
        100,
    );

    let result = PerfResult::new("cell_iteration_full_buffer_100x", actual, expected, threshold);

    println!(
        "cell_iteration_full_buffer_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

// =============================================================================
// TEXT OPERATIONS TESTS
// =============================================================================

#[test]
fn perf_text_insert() {
    use opentui::text::EditBuffer;

    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.text_operations.text_insert_1000_chars_ms)
        .unwrap_or(50);

    let mut edit_buffer = EditBuffer::new();

    let actual = time_ms(
        || {
            for i in 0..100 {
                let c = char::from(b'a' + (i % 26) as u8);
                edit_buffer.insert(&c.to_string());
            }
        },
        10,
    );

    let result = PerfResult::new("text_insert_1000_chars", actual, expected, threshold);

    println!(
        "text_insert_1000_chars: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_undo_redo() {
    use opentui::text::EditBuffer;

    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.text_operations.undo_redo_100x_ms)
        .unwrap_or(30);

    let mut edit_buffer = EditBuffer::new();

    // Setup: add some content to undo/redo
    // EditBuffer tracks undo automatically, no checkpoint needed
    for i in 0..50 {
        let c = char::from(b'a' + (i % 26) as u8);
        edit_buffer.insert(&c.to_string());
    }

    let actual = time_ms(
        || {
            for _ in 0..10 {
                for _ in 0..10 {
                    edit_buffer.undo();
                }
                for _ in 0..10 {
                    edit_buffer.redo();
                }
            }
        },
        1,
    );

    let result = PerfResult::new("undo_redo_100x", actual, expected, threshold);

    println!(
        "undo_redo_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

// =============================================================================
// INPUT PROCESSING TESTS
// =============================================================================

#[test]
fn perf_parse_keystrokes() {
    use opentui::input::InputParser;

    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.input_processing.parse_1000_keystrokes_ms)
        .unwrap_or(20);

    let mut parser = InputParser::new();

    // Various key sequences to parse
    let key_sequences: Vec<&[u8]> = vec![
        b"a",           // Simple char
        b"\x1b[A",      // Up arrow
        b"\x1b[B",      // Down arrow
        b"\x1b[1;5C",   // Ctrl+Right
        b"\x1bOP",      // F1
        b"\x1b[15~",    // F5
        b"\x1b[3~",     // Delete
        b"\x1b[H",      // Home
        b"\x1b[F",      // End
        b"\r",          // Enter
    ];

    let actual = time_ms(
        || {
            for seq in &key_sequences {
                let _ = parser.parse(seq);
            }
        },
        100,
    );

    let result = PerfResult::new("parse_1000_keystrokes", actual, expected, threshold);

    println!(
        "parse_1000_keystrokes: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_parse_mouse_events() {
    use opentui::input::InputParser;

    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.input_processing.parse_1000_mouse_events_ms)
        .unwrap_or(30);

    let mut parser = InputParser::new();

    // SGR mouse events (button press/release at various positions)
    let mouse_sequences: Vec<Vec<u8>> = (0..100)
        .map(|i| {
            let x = (i % 80) + 1;
            let y = (i % 24) + 1;
            format!("\x1b[<0;{x};{y}M").into_bytes()
        })
        .collect();

    let actual = time_ms(
        || {
            for seq in &mouse_sequences {
                let _ = parser.parse(seq);
            }
        },
        10,
    );

    let result = PerfResult::new("parse_1000_mouse_events", actual, expected, threshold);

    println!(
        "parse_1000_mouse_events: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

#[test]
fn perf_parse_large_paste() {
    use opentui::input::InputParser;

    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);
    let expected = baseline
        .as_ref()
        .map(|b| b.baselines.input_processing.parse_large_paste_10kb_ms)
        .unwrap_or(50);

    let mut parser = InputParser::new();

    // Simulate 10KB paste with bracketed paste mode
    let paste_content: String = (0..10240).map(|i| ('a' as u8 + (i % 26) as u8) as char).collect();
    let paste_sequence = format!("\x1b[200~{paste_content}\x1b[201~");
    let paste_bytes = paste_sequence.as_bytes();

    let actual = time_ms(
        || {
            let _ = parser.parse(paste_bytes);
        },
        10,
    );

    let result = PerfResult::new("parse_large_paste_10kb", actual, expected, threshold);

    println!(
        "parse_large_paste_10kb: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert!(
        result.status != PerfStatus::Regression,
        "REGRESSION: {} ms vs {} ms baseline ({:.1}% slower)",
        actual,
        expected,
        result.diff_percent
    );
}

// =============================================================================
// COMPREHENSIVE REGRESSION TEST
// =============================================================================

#[test]
fn perf_comprehensive_report() {
    let baseline = load_baseline();
    let threshold = baseline
        .as_ref()
        .map(|b| b.thresholds.regression_threshold_percent / 100.0)
        .unwrap_or(DEFAULT_REGRESSION_THRESHOLD);

    let mut report = PerfReport::new();

    // Rendering tests
    {
        let expected = baseline
            .as_ref()
            .map(|b| b.baselines.rendering.full_screen_clear_100x_ms)
            .unwrap_or(50);
        let mut buffer = OptimizedBuffer::new(200, 50);
        let actual = time_ms(|| buffer.clear(Rgba::BLACK), 100);
        report.add(PerfResult::new(
            "full_screen_clear_100x",
            actual,
            expected,
            threshold,
        ));
    }

    // Buffer operations
    {
        let expected = baseline
            .as_ref()
            .map(|b| b.baselines.buffer_operations.large_buffer_create_200x60_ms)
            .unwrap_or(10);
        let actual = time_ms(|| drop(OptimizedBuffer::new(200, 60)), 100);
        report.add(PerfResult::new(
            "large_buffer_create_100x",
            actual,
            expected,
            threshold,
        ));
    }

    // Blending operations
    {
        let mut buffer = OptimizedBuffer::new(200, 50);
        buffer.clear(Rgba::WHITE);
        let cell = Cell::new('X', Style::fg(Rgba::new(1.0, 0.0, 0.0, 0.5)));
        let actual = time_ms(|| buffer.set_blended(50, 25, cell), 1000);
        report.add(PerfResult::new(
            "blended_set_1000x",
            actual,
            20, // 20ms budget
            threshold,
        ));
    }

    report.finalize();

    // Print report
    println!("\n{}", report.to_summary());

    // Save JSON report for CI
    if let Ok(artifacts_dir) = std::env::var("TEST_ARTIFACTS_DIR") {
        let report_path = format!("{artifacts_dir}/perf_report.json");
        if let Err(e) = fs::write(&report_path, report.to_json()) {
            eprintln!("Warning: Failed to save perf report: {e}");
        }
    }

    // Fail on regressions
    assert!(
        report.all_passed,
        "Performance regressions detected!\n{}",
        report.to_summary()
    );
}

// =============================================================================
// BLENDING PERFORMANCE TESTS
// =============================================================================

#[test]
fn perf_alpha_blending() {
    let mut buffer = OptimizedBuffer::new(200, 50);
    buffer.clear(Rgba::WHITE);

    let semi_transparent = Rgba::new(1.0, 0.0, 0.0, 0.5);
    let cell = Cell::new('X', Style::fg(semi_transparent));

    // Warm up
    for _ in 0..100 {
        buffer.set_blended(50, 25, cell);
    }

    let start = Instant::now();
    for _ in 0..10000 {
        buffer.set_blended(50, 25, cell);
    }
    let elapsed = start.elapsed();

    println!(
        "alpha_blending_10k: {:?} ({:.2} ops/ms)",
        elapsed,
        10000.0 / elapsed.as_millis() as f64
    );

    // Should complete in reasonable time (< 100ms for 10k ops)
    assert!(
        elapsed < Duration::from_millis(100),
        "Alpha blending too slow: {elapsed:?}"
    );
}

#[test]
fn perf_diff_rendering_simulation() {
    let mut front = OptimizedBuffer::new(200, 50);
    let mut back = OptimizedBuffer::new(200, 50);

    front.clear(Rgba::BLACK);
    back.clear(Rgba::BLACK);

    // Simulate 10% changes
    let style = Style::fg(Rgba::WHITE);
    let changed_cells = (200 * 50) / 10; // 10%

    for i in 0..changed_cells {
        let x = ((i * 7) % 200) as u32;
        let y = ((i * 3) % 50) as u32;
        back.set(x, y, Cell::new('X', style));
    }

    // Count differences (simulating diff detection)
    let start = Instant::now();
    let mut diff_count = 0;
    for y in 0..50 {
        for x in 0..200 {
            let front_cell = front.get(x, y);
            let back_cell = back.get(x, y);
            if let (Some(f), Some(b)) = (front_cell, back_cell) {
                if !f.bits_eq(b) {
                    diff_count += 1;
                }
            }
        }
    }
    let elapsed = start.elapsed();

    println!(
        "diff_detection (10% changes): {:?}, found {} diffs",
        elapsed, diff_count
    );

    assert!(
        elapsed < Duration::from_millis(50),
        "Diff detection too slow: {elapsed:?}"
    );
}
