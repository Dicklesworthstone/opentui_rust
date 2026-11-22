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

#![allow(clippy::format_push_string, clippy::uninlined_format_args)] // Test code - clarity over micro-optimization
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
#![allow(clippy::struct_field_names)] // Field names match JSON baseline format

use opentui::{Cell, OptimizedBuffer, Rgba, Style};
use opentui_rust as opentui;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Path to baseline file.
const BASELINE_PATH: &str = "tests/baselines/perf_baseline.json";

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
    InvalidBaseline,
}

impl PerfStatus {
    const fn is_passing(self) -> bool {
        matches!(self, Self::Pass | Self::Improvement)
    }
}

impl PerfResult {
    fn new(name: &str, actual_ms: u64, baseline_ms: u64, threshold_percent: f64) -> Self {
        if baseline_ms == 0 || !threshold_percent.is_finite() || threshold_percent < 0.0 {
            return Self {
                name: name.to_string(),
                actual_ms,
                baseline_ms,
                diff_percent: 0.0,
                status: PerfStatus::InvalidBaseline,
            };
        }

        // `u64 -> f64` is potentially lossy, but these values are milliseconds measured in tests
        // and are expected to be well within the precise integer range of `f64`.
        #[allow(clippy::cast_precision_loss)]
        let diff_percent = ((actual_ms as f64 - baseline_ms as f64) / baseline_ms as f64) * 100.0;

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
    invalid_baselines: Vec<String>,
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
            PerfStatus::InvalidBaseline => {
                self.invalid_baselines.push(format!(
                    "{}: invalid baseline ({} ms actual vs {} ms baseline)",
                    result.name, result.actual_ms, result.baseline_ms
                ));
            }
            PerfStatus::Improvement => {
                self.improvements.push(format!(
                    "{}: {:.1}% faster ({} ms vs {} ms baseline)",
                    result.name, -result.diff_percent, result.actual_ms, result.baseline_ms
                ));
            }
            PerfStatus::Pass => {}
        }
        self.results.push(result);
    }

    fn finalize(&mut self) {
        self.all_passed = self.regressions.is_empty() && self.invalid_baselines.is_empty();
    }

    fn to_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("=== Performance Test Report ===\n\n");

        if !self.invalid_baselines.is_empty() {
            summary.push_str("INVALID BASELINES DETECTED:\n");
            for b in &self.invalid_baselines {
                summary.push_str(&format!("  ! {b}\n"));
            }
            summary.push('\n');
        }

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
            "Total: {} tests, {} regressions, {} invalid baselines, {} improvements\n",
            self.results.len(),
            self.regressions.len(),
            self.invalid_baselines.len(),
            self.improvements.len()
        ));

        if self.all_passed {
            summary.push_str("\nResult: PASS\n");
        } else {
            summary.push_str("\nResult: FAIL (regressions or invalid baselines detected)\n");
        }

        summary
    }

    fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

impl PerfBaseline {
    fn validate(&self) -> Result<(), String> {
        if !self.thresholds.regression_threshold_percent.is_finite()
            || self.thresholds.regression_threshold_percent < 0.0
        {
            return Err(format!(
                "Invalid regression_threshold_percent: {}",
                self.thresholds.regression_threshold_percent
            ));
        }
        if !self.thresholds.improvement_threshold_percent.is_finite()
            || self.thresholds.improvement_threshold_percent < 0.0
        {
            return Err(format!(
                "Invalid improvement_threshold_percent: {}",
                self.thresholds.improvement_threshold_percent
            ));
        }

        for (field, value) in self.required_nonzero_fields() {
            if value == 0 {
                return Err(format!("Baseline field '{field}' must be > 0, got 0"));
            }
        }

        Ok(())
    }

    const fn required_nonzero_fields(&self) -> [(&'static str, u64); 19] {
        let rend = &self.baselines.rendering;
        let buf = &self.baselines.buffer_operations;
        let txt = &self.baselines.text_operations;
        let inp = &self.baselines.input_processing;
        let e2e = &self.baselines.e2e;
        [
            (
                "rendering.full_screen_clear_100x_ms",
                rend.full_screen_clear_100x_ms,
            ),
            (
                "rendering.full_screen_text_render_100x_ms",
                rend.full_screen_text_render_100x_ms,
            ),
            (
                "rendering.diff_render_10pct_changes_ms",
                rend.diff_render_10pct_changes_ms,
            ),
            (
                "rendering.diff_render_50pct_changes_ms",
                rend.diff_render_50pct_changes_ms,
            ),
            (
                "rendering.diff_render_90pct_changes_ms",
                rend.diff_render_90pct_changes_ms,
            ),
            (
                "buffer_operations.large_buffer_create_200x60_ms",
                buf.large_buffer_create_200x60_ms,
            ),
            (
                "buffer_operations.scissor_push_pop_1000x_ms",
                buf.scissor_push_pop_1000x_ms,
            ),
            (
                "buffer_operations.opacity_stack_1000x_ms",
                buf.opacity_stack_1000x_ms,
            ),
            (
                "buffer_operations.cell_iteration_full_buffer_ms",
                buf.cell_iteration_full_buffer_ms,
            ),
            (
                "text_operations.large_file_load_100kb_ms",
                txt.large_file_load_100kb_ms,
            ),
            (
                "text_operations.text_insert_1000_chars_ms",
                txt.text_insert_1000_chars_ms,
            ),
            ("text_operations.undo_redo_100x_ms", txt.undo_redo_100x_ms),
            (
                "text_operations.wrap_recalculation_ms",
                txt.wrap_recalculation_ms,
            ),
            (
                "input_processing.parse_1000_keystrokes_ms",
                inp.parse_1000_keystrokes_ms,
            ),
            (
                "input_processing.parse_1000_mouse_events_ms",
                inp.parse_1000_mouse_events_ms,
            ),
            (
                "input_processing.parse_large_paste_10kb_ms",
                inp.parse_large_paste_10kb_ms,
            ),
            (
                "e2e.demo_showcase_tour_max_runtime_s",
                e2e.demo_showcase_tour_max_runtime_s,
            ),
            (
                "e2e.demo_showcase_tour_min_fps",
                e2e.demo_showcase_tour_min_fps,
            ),
            (
                "e2e.demo_showcase_headless_smoke_max_ms",
                e2e.demo_showcase_headless_smoke_max_ms,
            ),
        ]
    }
}

/// Load and validate baseline configuration from an explicit path.
fn load_baseline_from_path(path: &Path) -> Result<PerfBaseline, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read baseline file '{}': {e}", path.display()))?;
    let baseline: PerfBaseline = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse baseline file '{}': {e}", path.display()))?;
    baseline.validate()?;
    Ok(baseline)
}

/// Load baseline configuration from the canonical repository path.
fn load_baseline() -> Result<PerfBaseline, String> {
    load_baseline_from_path(Path::new(BASELINE_PATH))
}

/// Time a closure and return duration in milliseconds.
fn time_ms<F: FnMut()>(mut f: F, iterations: u32) -> u64 {
    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    u64::try_from(start.elapsed().as_millis()).expect("elapsed ms fits u64")
}

/// Check if we should update baselines.
fn should_update_baselines() -> bool {
    std::env::var("UPDATE_BASELINES").is_ok()
}

fn regression_threshold(baseline: &PerfBaseline) -> f64 {
    baseline.thresholds.regression_threshold_percent / 100.0
}

fn assert_perf_passing(result: &PerfResult) {
    assert!(
        result.status.is_passing(),
        "{}: {} ms vs {} ms baseline ({:.1}% diff, status={:?})",
        result.name,
        result.actual_ms,
        result.baseline_ms,
        result.diff_percent,
        result.status
    );
}

// =============================================================================
// RENDERING PERFORMANCE TESTS
// =============================================================================

#[test]
fn perf_full_screen_clear() {
    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline.baselines.rendering.full_screen_clear_100x_ms;

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

    assert_perf_passing(&result);
}

#[test]
fn perf_full_screen_text_render() {
    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline.baselines.rendering.full_screen_text_render_100x_ms;

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

    assert_perf_passing(&result);
}

// =============================================================================
// BUFFER OPERATIONS TESTS
// =============================================================================

#[test]
fn perf_large_buffer_create() {
    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline
        .baselines
        .buffer_operations
        .large_buffer_create_200x60_ms;

    let actual = time_ms(|| drop(OptimizedBuffer::new(200, 60)), 100);

    let result = PerfResult::new("large_buffer_create_100x", actual, expected, threshold);

    println!(
        "large_buffer_create_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert_perf_passing(&result);
}

#[test]
fn perf_scissor_push_pop() {
    use opentui::buffer::ClipRect;

    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline
        .baselines
        .buffer_operations
        .scissor_push_pop_1000x_ms;

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

    assert_perf_passing(&result);
}

#[test]
fn perf_opacity_stack() {
    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline.baselines.buffer_operations.opacity_stack_1000x_ms;

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

    assert_perf_passing(&result);
}

#[test]
fn perf_cell_iteration() {
    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline
        .baselines
        .buffer_operations
        .cell_iteration_full_buffer_ms;

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

    let result = PerfResult::new(
        "cell_iteration_full_buffer_100x",
        actual,
        expected,
        threshold,
    );

    println!(
        "cell_iteration_full_buffer_100x: {} ms (baseline: {} ms, diff: {:.1}%)",
        actual, expected, result.diff_percent
    );

    if should_update_baselines() {
        println!("  Suggested baseline: {actual}");
    }

    assert_perf_passing(&result);
}

// =============================================================================
// TEXT OPERATIONS TESTS
// =============================================================================

#[test]
fn perf_text_insert() {
    use opentui::text::EditBuffer;

    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline.baselines.text_operations.text_insert_1000_chars_ms;

    let mut edit_buffer = EditBuffer::new();

    let actual = time_ms(
        || {
            for i in 0u8..100 {
                let c = char::from(b'a' + (i % 26));
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

    assert_perf_passing(&result);
}

#[test]
fn perf_undo_redo() {
    use opentui::text::EditBuffer;

    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline.baselines.text_operations.undo_redo_100x_ms;

    let mut edit_buffer = EditBuffer::new();

    // Setup: add some content to undo/redo
    // EditBuffer tracks undo automatically, no checkpoint needed
    for i in 0u8..50 {
        let c = char::from(b'a' + (i % 26));
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

    assert_perf_passing(&result);
}

// =============================================================================
// INPUT PROCESSING TESTS
// =============================================================================

#[test]
fn perf_parse_keystrokes() {
    use opentui::input::InputParser;

    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline.baselines.input_processing.parse_1000_keystrokes_ms;

    let mut parser = InputParser::new();

    // Various key sequences to parse
    let key_sequences: Vec<&[u8]> = vec![
        b"a",         // Simple char
        b"\x1b[A",    // Up arrow
        b"\x1b[B",    // Down arrow
        b"\x1b[1;5C", // Ctrl+Right
        b"\x1bOP",    // F1
        b"\x1b[15~",  // F5
        b"\x1b[3~",   // Delete
        b"\x1b[H",    // Home
        b"\x1b[F",    // End
        b"\r",        // Enter
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

    assert_perf_passing(&result);
}

#[test]
fn perf_parse_mouse_events() {
    use opentui::input::InputParser;

    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline
        .baselines
        .input_processing
        .parse_1000_mouse_events_ms;

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

    assert_perf_passing(&result);
}

#[test]
fn perf_parse_large_paste() {
    use opentui::input::InputParser;

    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);
    let expected = baseline
        .baselines
        .input_processing
        .parse_large_paste_10kb_ms;

    let mut parser = InputParser::new();

    // Simulate 10KB paste with bracketed paste mode
    let paste_content: String = (0usize..10240)
        .map(|i| {
            let offset = u8::try_from(i % 26).expect("alphabet index fits u8");
            char::from(b'a' + offset)
        })
        .collect();
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

    assert_perf_passing(&result);
}

// =============================================================================
// COMPREHENSIVE REGRESSION TEST
// =============================================================================

#[test]
fn perf_comprehensive_report() {
    let baseline = load_baseline().expect("valid performance baseline file");
    let threshold = regression_threshold(&baseline);

    let mut report = PerfReport::new();

    // Rendering tests
    {
        let expected = baseline.baselines.rendering.full_screen_clear_100x_ms;
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
            .baselines
            .buffer_operations
            .large_buffer_create_200x60_ms;
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

    let ops_per_ms = 10000.0 / (elapsed.as_secs_f64() * 1000.0);
    println!(
        "alpha_blending_10k: {:?} ({:.2} ops/ms)",
        elapsed, ops_per_ms
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
    let changed_cells = (200u32 * 50) / 10; // 10%

    for i in 0..changed_cells {
        let x = (i * 7) % 200;
        let y = (i * 3) % 50;
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

// =============================================================================
// HARNESS INTEGRITY & BASELINE VALIDATION TESTS
// =============================================================================

#[test]
fn perf_zero_baseline_is_invalid_not_pass() {
    let result = PerfResult::new("zero_baseline_case", 100, 0, 0.20);
    assert_eq!(result.status, PerfStatus::InvalidBaseline);
    assert!(!result.status.is_passing());
}

#[test]
fn perf_invalid_threshold_is_rejected() {
    let nan_res = PerfResult::new("nan_threshold", 10, 50, f64::NAN);
    assert_eq!(nan_res.status, PerfStatus::InvalidBaseline);
    assert!(!nan_res.status.is_passing());

    let neg_res = PerfResult::new("neg_threshold", 10, 50, -0.10);
    assert_eq!(neg_res.status, PerfStatus::InvalidBaseline);
    assert!(!neg_res.status.is_passing());
}

#[test]
fn perf_missing_baseline_file_errors() {
    let missing = Path::new("tests/baselines/does_not_exist_perf_baseline.json");
    let err = load_baseline_from_path(missing).expect_err("missing baseline file must fail");
    assert!(err.contains("Failed to read baseline file"), "got: {err}");
}

#[test]
fn perf_malformed_baseline_file_errors() {
    let temp = tempfile::NamedTempFile::new().expect("create temp baseline file");
    fs::write(temp.path(), "{ not valid json").expect("write malformed json");

    let err = load_baseline_from_path(temp.path()).expect_err("malformed baseline JSON must fail");
    assert!(err.contains("Failed to parse baseline file"), "got: {err}");
}

#[test]
fn perf_zero_value_in_baseline_json_errors() {
    let raw = fs::read_to_string(BASELINE_PATH).expect("read canonical baseline json");
    let zeroed = raw.replace(
        "\"full_screen_clear_100x_ms\": 50",
        "\"full_screen_clear_100x_ms\": 0",
    );
    assert_ne!(raw, zeroed, "baseline replacement should mutate field");

    let temp = tempfile::NamedTempFile::new().expect("create temp baseline file");
    fs::write(temp.path(), zeroed).expect("write zeroed baseline");

    let err =
        load_baseline_from_path(temp.path()).expect_err("zero baseline field in JSON must fail");
    assert!(err.contains("must be > 0"), "got: {err}");
}

#[test]
fn perf_report_fails_on_regression_and_invalid_baseline() {
    let mut regressed_report = PerfReport::new();
    regressed_report.add(PerfResult::new("ok_op", 50, 50, 0.20));
    regressed_report.add(PerfResult::new("slow_op", 100, 50, 0.20));
    regressed_report.finalize();
    assert!(!regressed_report.all_passed);
    assert_eq!(regressed_report.regressions.len(), 1);
    assert!(regressed_report.to_summary().contains("Result: FAIL"));

    let mut invalid_report = PerfReport::new();
    invalid_report.add(PerfResult::new("ok_op", 50, 50, 0.20));
    invalid_report.add(PerfResult::new("bad_baseline", 10, 0, 0.20));
    invalid_report.finalize();
    assert!(!invalid_report.all_passed);
    assert_eq!(invalid_report.invalid_baselines.len(), 1);
    assert!(
        invalid_report
            .to_summary()
            .contains("INVALID BASELINES DETECTED")
    );
    assert!(invalid_report.to_summary().contains("Result: FAIL"));
}

#[test]
fn perf_result_classifies_pass_improvement_and_regression() {
    let pass = PerfResult::new("within_budget", 55, 50, 0.20);
    assert_eq!(pass.status, PerfStatus::Pass);
    assert!(pass.status.is_passing());

    let improved = PerfResult::new("much_faster", 30, 50, 0.20);
    assert_eq!(improved.status, PerfStatus::Improvement);
    assert!(improved.status.is_passing());

    let regressed = PerfResult::new("too_slow", 65, 50, 0.20);
    assert_eq!(regressed.status, PerfStatus::Regression);
    assert!(!regressed.status.is_passing());
}
