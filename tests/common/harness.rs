//! Conformance and benchmark test harness utilities.
//!
//! This module provides structured logging and artifact capture for E2E tests.
//! Logs are JSONL-formatted with timestamps, step IDs, and enough context
//! to debug mismatches quickly.
//!
//! # Environment Variables
//!
//! - `HARNESS_ARTIFACTS=1` - Enable artifact logging
//! - `HARNESS_ARTIFACTS_DIR` - Custom artifact directory (default: `target/test-artifacts`)
//! - `HARNESS_PRESERVE_SUCCESS=1` - Keep artifacts even for passing tests
//! - `HARNESS_LOG_LEVEL` - Log verbosity: `debug`, `info`, `warn`, `error` (default: `info`)

#![allow(dead_code)]

use opentui::input::{Event, InputParser, ParseError};
use opentui::{OptimizedBuffer, Style};
use serde::{Deserialize, Serialize};
use std::fmt::Write as FmtWrite;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Global step counter for unique step IDs across all tests.
static GLOBAL_STEP_ID: AtomicU64 = AtomicU64::new(0);

fn next_step_id() -> u64 {
    GLOBAL_STEP_ID.fetch_add(1, Ordering::SeqCst)
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| millis_to_u64(d.as_millis()))
}

fn millis_to_u64(ms: u128) -> u64 {
    u64::try_from(ms).unwrap_or(u64::MAX)
}

/// Log level for structured logging.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn from_env() -> Self {
        match std::env::var("HARNESS_LOG_LEVEL")
            .unwrap_or_default()
            .to_lowercase()
            .as_str()
        {
            "debug" => Self::Debug,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Info,
        }
    }

    const fn should_log(self, min_level: Self) -> bool {
        (self as u8) >= (min_level as u8)
    }
}

/// A single structured log entry in JSONL format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LogEntry {
    /// Monotonic step ID for ordering.
    pub step_id: u64,
    /// Unix timestamp in milliseconds.
    pub ts_ms: u64,
    /// Duration since test start.
    pub elapsed_ms: u64,
    /// Log level.
    pub level: LogLevel,
    /// Log category (e.g., "input", "render", "assert").
    pub category: String,
    /// Human-readable message.
    pub message: String,
    /// Optional cursor position (x, y).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<(u32, u32)>,
    /// Optional ANSI payload bytes (hex-encoded for readability).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ansi_hex: Option<String>,
    /// Optional arbitrary context data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

impl LogEntry {
    fn new(
        start_time: Instant,
        level: LogLevel,
        category: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            step_id: next_step_id(),
            ts_ms: unix_millis(),
            elapsed_ms: millis_to_u64(start_time.elapsed().as_millis()),
            level,
            category: category.into(),
            message: message.into(),
            cursor: None,
            ansi_hex: None,
            context: None,
        }
    }

    const fn with_cursor(mut self, x: u32, y: u32) -> Self {
        self.cursor = Some((x, y));
        self
    }

    fn with_ansi(mut self, bytes: &[u8]) -> Self {
        self.ansi_hex = Some(hex_encode(bytes));
        self
    }

    fn with_context<T: Serialize>(mut self, ctx: &T) -> Self {
        self.context = serde_json::to_value(ctx).ok();
        self
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Structured logger that writes JSONL to a file.
pub struct StructuredLogger {
    writer: Option<BufWriter<File>>,
    start_time: Instant,
    min_level: LogLevel,
    entries: Vec<LogEntry>,
}

impl StructuredLogger {
    pub fn new(log_path: &Path) -> Self {
        let writer = File::create(log_path).ok().map(BufWriter::new);
        Self {
            writer,
            start_time: Instant::now(),
            min_level: LogLevel::from_env(),
            entries: Vec::new(),
        }
    }

    pub fn disabled() -> Self {
        Self {
            writer: None,
            start_time: Instant::now(),
            min_level: LogLevel::Error,
            entries: Vec::new(),
        }
    }

    pub fn log(&mut self, entry: LogEntry) {
        if !entry.level.should_log(self.min_level) {
            return;
        }

        // Also print to stderr for immediate visibility
        eprintln!(
            "[{:06}ms] {} [{}] {}",
            entry.elapsed_ms,
            entry.level.to_string().to_uppercase(),
            entry.category,
            entry.message
        );

        if let Some(ref mut writer) = self.writer {
            if let Ok(json) = serde_json::to_string(&entry) {
                let _ = writeln!(writer, "{json}");
            }
        }

        self.entries.push(entry);
    }

    pub fn debug(&mut self, category: &str, message: impl Into<String>) {
        self.log(LogEntry::new(
            self.start_time,
            LogLevel::Debug,
            category,
            message,
        ));
    }

    pub fn info(&mut self, category: &str, message: impl Into<String>) {
        self.log(LogEntry::new(
            self.start_time,
            LogLevel::Info,
            category,
            message,
        ));
    }

    pub fn warn(&mut self, category: &str, message: impl Into<String>) {
        self.log(LogEntry::new(
            self.start_time,
            LogLevel::Warn,
            category,
            message,
        ));
    }

    pub fn error(&mut self, category: &str, message: impl Into<String>) {
        self.log(LogEntry::new(
            self.start_time,
            LogLevel::Error,
            category,
            message,
        ));
    }

    pub fn log_input(&mut self, bytes: &[u8]) {
        let entry = LogEntry::new(
            self.start_time,
            LogLevel::Debug,
            "input",
            format!("Injecting {} bytes", bytes.len()),
        )
        .with_ansi(bytes);
        self.log(entry);
    }

    pub fn log_event(&mut self, event: &Event) {
        let entry = LogEntry::new(
            self.start_time,
            LogLevel::Info,
            "event",
            format!("{event:?}"),
        );
        self.log(entry);
    }

    pub fn log_render(&mut self, cursor: Option<(u32, u32)>, ansi_bytes: &[u8]) {
        let mut entry = LogEntry::new(
            self.start_time,
            LogLevel::Debug,
            "render",
            format!("Rendered {} bytes", ansi_bytes.len()),
        )
        .with_ansi(ansi_bytes);
        if let Some((x, y)) = cursor {
            entry = entry.with_cursor(x, y);
        }
        self.log(entry);
    }

    pub fn log_assert(&mut self, passed: bool, x: u32, y: u32, expected: &str, actual: &str) {
        let level = if passed {
            LogLevel::Debug
        } else {
            LogLevel::Error
        };
        let msg = if passed {
            format!("Assert passed: expected='{expected}' actual='{actual}'")
        } else {
            format!("Assert FAILED: expected='{expected}' actual='{actual}'")
        };
        let entry = LogEntry::new(self.start_time, level, "assert", msg).with_cursor(x, y);
        self.log(entry);
    }

    pub fn flush(&mut self) {
        if let Some(ref mut writer) = self.writer {
            let _ = writer.flush();
        }
    }

    pub fn entries(&self) -> &[LogEntry] {
        &self.entries
    }
}

impl Drop for StructuredLogger {
    fn drop(&mut self) {
        self.flush();
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "debug"),
            Self::Info => write!(f, "info"),
            Self::Warn => write!(f, "warn"),
            Self::Error => write!(f, "error"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArtifactConfig {
    pub enabled: bool,
    pub preserve_on_success: bool,
}

impl Default for ArtifactConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("HARNESS_ARTIFACTS").is_ok_and(|v| v == "1"),
            preserve_on_success: std::env::var("HARNESS_PRESERVE_SUCCESS").is_ok_and(|v| v == "1"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArtifactLogger {
    suite: String,
    test: String,
    pub artifact_dir: PathBuf,
    pub config: ArtifactConfig,
    started_at: Instant,
}

impl ArtifactLogger {
    pub fn new(suite: &str, test: &str) -> Self {
        let base_dir = std::env::var("HARNESS_ARTIFACTS_DIR")
            .unwrap_or_else(|_| "target/test-artifacts".to_string());
        let artifact_dir = PathBuf::from(base_dir).join(suite).join(test);
        let config = ArtifactConfig::default();
        if config.enabled {
            fs::create_dir_all(&artifact_dir).ok();
        }
        Self {
            suite: suite.to_string(),
            test: test.to_string(),
            artifact_dir,
            config,
            started_at: Instant::now(),
        }
    }

    pub fn log_case<S: Serialize>(&self, name: &str, expected: &S, actual: &S) {
        if !self.config.enabled {
            return;
        }
        let expected_path = self.artifact_dir.join(format!("{name}.expected.json"));
        let actual_path = self.artifact_dir.join(format!("{name}.actual.json"));
        if let Ok(json) = serde_json::to_string_pretty(expected) {
            fs::write(expected_path, json).ok();
        }
        if let Ok(json) = serde_json::to_string_pretty(actual) {
            fs::write(actual_path, json).ok();
        }
    }

    pub fn log_text(&self, name: &str, expected: &str, actual: &str) {
        if !self.config.enabled {
            return;
        }
        fs::write(
            self.artifact_dir.join(format!("{name}.expected.txt")),
            expected,
        )
        .ok();
        fs::write(self.artifact_dir.join(format!("{name}.actual.txt")), actual).ok();
    }

    pub fn write_summary(&self, passed: bool, cases: &[CaseResult]) {
        if !self.config.enabled {
            return;
        }
        let failed = cases.iter().filter(|c| c.result == "fail").count();
        let total = cases.len();
        let summary = Summary {
            suite: self.suite.clone(),
            test: self.test.clone(),
            passed,
            failed,
            total,
            duration_ms: self.started_at.elapsed().as_millis(),
            cases: cases.to_vec(),
        };
        let summary_path = self.artifact_dir.join("summary.json");
        if let Ok(json) = serde_json::to_string_pretty(&summary) {
            fs::write(summary_path, json).ok();
        }

        if passed && !self.config.preserve_on_success {
            if let Ok(entries) = fs::read_dir(&self.artifact_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                        if name.contains(".expected.") || name.contains(".actual.") {
                            fs::remove_file(path).ok();
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct CaseResult {
    pub name: String,
    pub result: String,
    pub duration_ms: u128,
}

#[derive(Clone, Debug, Serialize)]
struct Summary {
    suite: String,
    test: String,
    passed: bool,
    failed: usize,
    total: usize,
    duration_ms: u128,
    cases: Vec<CaseResult>,
}

pub fn case_timer() -> Instant {
    Instant::now()
}

pub fn case_result(name: &str, passed: bool, start: Instant) -> CaseResult {
    CaseResult {
        name: name.to_string(),
        result: if passed { "pass" } else { "fail" }.to_string(),
        duration_ms: start.elapsed().as_millis(),
    }
}

pub fn ensure_dir(path: &Path) {
    fs::create_dir_all(path).ok();
}

/// Captured ANSI output snapshot.
#[derive(Clone, Debug, Serialize)]
pub struct AnsiCapture {
    /// Step ID when captured.
    pub step_id: u64,
    /// Name/label for this capture.
    pub name: String,
    /// Raw ANSI bytes.
    #[serde(skip)]
    pub bytes: Vec<u8>,
    /// Hex-encoded bytes for JSON.
    pub hex: String,
    /// Cursor position at capture time.
    pub cursor: Option<(u32, u32)>,
}

impl AnsiCapture {
    pub fn new(name: impl Into<String>, bytes: Vec<u8>, cursor: Option<(u32, u32)>) -> Self {
        let hex = hex_encode(&bytes);
        Self {
            step_id: next_step_id(),
            name: name.into(),
            bytes,
            hex,
            cursor,
        }
    }
}

/// Full E2E test harness with input injection and output capture.
pub struct E2EHarness {
    artifact_logger: ArtifactLogger,
    structured_log: StructuredLogger,
    input_buffer: Vec<u8>,
    output_buffer: OptimizedBuffer,
    parser: InputParser,
    events: Vec<(Duration, Event)>,
    start_time: Instant,
    cursor_pos: (u32, u32),
    ansi_captures: Vec<AnsiCapture>,
}

impl E2EHarness {
    /// Create a new E2E harness for a test.
    pub fn new(suite: &str, test: &str, width: u32, height: u32) -> Self {
        let artifact_logger = ArtifactLogger::new(suite, test);
        let log_path = artifact_logger.artifact_dir.join("test.jsonl");

        let structured_log = if artifact_logger.config.enabled {
            StructuredLogger::new(&log_path)
        } else {
            StructuredLogger::disabled()
        };

        Self {
            artifact_logger,
            structured_log,
            input_buffer: Vec::new(),
            output_buffer: OptimizedBuffer::new(width, height),
            parser: InputParser::new(),
            events: Vec::new(),
            start_time: Instant::now(),
            cursor_pos: (0, 0),
            ansi_captures: Vec::new(),
        }
    }

    /// Inject input bytes and parse events.
    ///
    /// Note: For bracketed paste, the parser expects chunked delivery:
    /// 1. Call `inject_input(b"\x1b[200~")` to enter paste mode
    /// 2. Call `inject_input(b"content\x1b[201~")` to get the paste event
    pub fn inject_input(&mut self, bytes: &[u8]) -> Vec<Event> {
        self.structured_log.log_input(bytes);
        self.input_buffer.extend_from_slice(bytes);

        let mut events = Vec::new();

        loop {
            if self.input_buffer.is_empty() {
                break;
            }

            match self.parser.parse(&self.input_buffer) {
                Ok((event, consumed)) => {
                    let elapsed = self.start_time.elapsed();
                    self.structured_log.log_event(&event);
                    self.events.push((elapsed, event.clone()));
                    events.push(event);
                    self.input_buffer.drain(..consumed);
                }
                Err(ParseError::Incomplete) => {
                    // Need more data - break and wait for next inject_input call
                    break;
                }
                Err(ParseError::Empty) => {
                    break;
                }
                Err(_) => {
                    // Skip unrecognized byte
                    self.input_buffer.remove(0);
                }
            }
        }
        events
    }

    /// Get the output buffer for rendering.
    pub const fn buffer_mut(&mut self) -> &mut OptimizedBuffer {
        &mut self.output_buffer
    }

    /// Get the output buffer (immutable).
    pub const fn buffer(&self) -> &OptimizedBuffer {
        &self.output_buffer
    }

    /// Dump buffer contents to artifact file.
    pub fn dump_buffer(&mut self, name: &str) {
        let mut output = String::new();
        for y in 0..self.output_buffer.height() {
            for x in 0..self.output_buffer.width() {
                if let Some(cell) = self.output_buffer.get(x, y) {
                    match &cell.content {
                        opentui::CellContent::Char(c) => output.push(*c),
                        opentui::CellContent::Grapheme(_) => output.push(' '),
                        opentui::CellContent::Empty | opentui::CellContent::Continuation => {
                            output.push(' ');
                        }
                    }
                } else {
                    output.push(' ');
                }
            }
            output.push('\n');
        }

        self.structured_log.info(
            "buffer",
            format!(
                "Buffer dump '{name}': {}x{}",
                self.output_buffer.width(),
                self.output_buffer.height()
            ),
        );

        // Also write to artifact file
        self.artifact_logger.log_text(name, &output, &output);
    }

    /// Assert cell at position has expected content.
    pub fn assert_cell(&mut self, x: u32, y: u32, expected_char: char, msg: &str) {
        let cell = self.output_buffer.get(x, y).expect("Cell should exist");
        let actual = match &cell.content {
            opentui::CellContent::Char(c) => c.to_string(),
            opentui::CellContent::Grapheme(_)
            | opentui::CellContent::Empty
            | opentui::CellContent::Continuation => " ".to_string(),
        };
        let expected = expected_char.to_string();
        let passed = actual == expected;

        self.structured_log
            .log_assert(passed, x, y, &expected, &actual);

        assert_eq!(actual, expected, "{msg} at ({x},{y})");
    }

    /// Assert cell style matches predicate.
    pub fn assert_style<F>(&mut self, x: u32, y: u32, predicate: F, msg: &str)
    where
        F: Fn(&Style) -> bool,
    {
        let cell = self.output_buffer.get(x, y).expect("Cell should exist");
        let style = Style {
            fg: Some(cell.fg),
            bg: Some(cell.bg),
            attributes: cell.attributes,
        };
        let passed = predicate(&style);

        self.structured_log
            .log_assert(passed, x, y, "style predicate", &format!("{style:?}"));

        assert!(predicate(&style), "{msg} at ({x},{y})");
    }

    /// Get all parsed events.
    pub fn events(&self) -> &[(Duration, Event)] {
        &self.events
    }

    /// Set cursor position for tracking in logs.
    pub fn set_cursor(&mut self, x: u32, y: u32) {
        self.cursor_pos = (x, y);
        self.structured_log
            .debug("cursor", format!("Cursor moved to ({x}, {y})"));
    }

    /// Get current cursor position.
    pub const fn cursor(&self) -> (u32, u32) {
        self.cursor_pos
    }

    /// Capture ANSI output bytes for artifact storage.
    pub fn capture_ansi(&mut self, name: &str, bytes: Vec<u8>) {
        self.structured_log
            .log_render(Some(self.cursor_pos), &bytes);
        let capture = AnsiCapture::new(name, bytes, Some(self.cursor_pos));
        self.ansi_captures.push(capture);
    }

    /// Capture ANSI output from a rendering callback.
    pub fn capture_render<F>(&mut self, name: &str, render_fn: F)
    where
        F: FnOnce(&mut Vec<u8>),
    {
        let mut output = Vec::new();
        render_fn(&mut output);
        self.capture_ansi(name, output);
    }

    /// Get all ANSI captures.
    pub fn ansi_captures(&self) -> &[AnsiCapture] {
        &self.ansi_captures
    }

    /// Write ANSI captures to artifact files on failure.
    fn write_ansi_artifacts(&self) {
        if !self.artifact_logger.config.enabled {
            return;
        }

        for capture in &self.ansi_captures {
            // Write raw bytes
            let raw_path = self
                .artifact_logger
                .artifact_dir
                .join(format!("{}.ansi.bin", capture.name));
            fs::write(&raw_path, &capture.bytes).ok();

            // Write hex dump
            let hex_path = self
                .artifact_logger
                .artifact_dir
                .join(format!("{}.ansi.hex", capture.name));
            fs::write(&hex_path, &capture.hex).ok();

            // Write readable escape sequence format
            let readable = ansi_to_readable(&capture.bytes);
            let readable_path = self
                .artifact_logger
                .artifact_dir
                .join(format!("{}.ansi.txt", capture.name));
            fs::write(readable_path, readable).ok();
        }
    }

    /// Access the structured logger directly.
    pub const fn log(&mut self) -> &mut StructuredLogger {
        &mut self.structured_log
    }

    /// Write test summary.
    pub fn finish(&mut self, passed: bool) {
        self.structured_log.info(
            "summary",
            format!("Test {}", if passed { "PASSED" } else { "FAILED" }),
        );

        let cases: Vec<CaseResult> = self
            .events
            .iter()
            .enumerate()
            .map(|(i, (dur, _event))| CaseResult {
                name: format!("event_{i}"),
                result: if passed { "pass" } else { "fail" }.to_string(),
                duration_ms: dur.as_millis(),
            })
            .collect();

        self.artifact_logger.write_summary(passed, &cases);

        // Write ANSI artifacts on failure
        if !passed {
            self.write_ansi_artifacts();
        }

        self.structured_log.flush();
    }
}

/// Convert ANSI bytes to a readable format for debugging.
fn ansi_to_readable(bytes: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0x1b => {
                result.push_str("ESC");
                i += 1;
            }
            0x07 => {
                result.push_str("<BEL>");
                i += 1;
            }
            0x08 => {
                result.push_str("<BS>");
                i += 1;
            }
            0x09 => {
                result.push_str("<TAB>");
                i += 1;
            }
            0x0a => {
                result.push_str("<LF>\n");
                i += 1;
            }
            0x0d => {
                result.push_str("<CR>");
                i += 1;
            }
            b if (0x20..0x7f).contains(&b) => {
                result.push(b as char);
                i += 1;
            }
            b => {
                let _ = write!(result, "<{b:02X}>");
                i += 1;
            }
        }
    }
    result
}

// ============================================================================
// Event Types and Phase Tracking
// ============================================================================

/// Event type for structured logging - provides semantic meaning to log entries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventType {
    /// Test starting - includes initial config and setup info.
    TestStart,
    /// Test phase transition (setup -> execute -> verify -> teardown).
    PhaseChange,
    /// A discrete step within a test.
    Step,
    /// An assertion being checked.
    Assertion,
    /// An error occurred.
    Error,
    /// Test finished - includes summary stats.
    TestEnd,
    /// Input was sent to the terminal.
    Input,
    /// Output was rendered.
    Render,
    /// Buffer state captured.
    BufferCapture,
    /// Timing/performance metric.
    Metric,
}

impl std::fmt::Display for EventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TestStart => write!(f, "test_start"),
            Self::PhaseChange => write!(f, "phase_change"),
            Self::Step => write!(f, "step"),
            Self::Assertion => write!(f, "assertion"),
            Self::Error => write!(f, "error"),
            Self::TestEnd => write!(f, "test_end"),
            Self::Input => write!(f, "input"),
            Self::Render => write!(f, "render"),
            Self::BufferCapture => write!(f, "buffer_capture"),
            Self::Metric => write!(f, "metric"),
        }
    }
}

/// Test phase for tracking progress through a test.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestPhase {
    /// Initial setup phase.
    #[default]
    Setup,
    /// Main test execution phase.
    Execute,
    /// Verification/assertion phase.
    Verify,
    /// Cleanup/teardown phase.
    Teardown,
}

impl std::fmt::Display for TestPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Setup => write!(f, "setup"),
            Self::Execute => write!(f, "execute"),
            Self::Verify => write!(f, "verify"),
            Self::Teardown => write!(f, "teardown"),
        }
    }
}

/// Extended log entry with event type and phase tracking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExtendedLogEntry {
    /// Monotonic step ID for ordering.
    pub step_id: u64,
    /// Unix timestamp in milliseconds.
    pub ts_ms: u64,
    /// Duration since test start.
    pub elapsed_ms: u64,
    /// Test name.
    pub test_name: String,
    /// Event type.
    pub event_type: EventType,
    /// Log level.
    pub level: LogLevel,
    /// Current test phase.
    pub phase: TestPhase,
    /// Sequence number within the phase.
    pub sequence_num: u32,
    /// Human-readable message.
    pub message: String,
    /// Optional context data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<serde_json::Value>,
}

impl ExtendedLogEntry {
    fn new(
        start_time: Instant,
        test_name: &str,
        event_type: EventType,
        level: LogLevel,
        phase: TestPhase,
        sequence_num: u32,
        message: impl Into<String>,
    ) -> Self {
        Self {
            step_id: next_step_id(),
            ts_ms: unix_millis(),
            elapsed_ms: millis_to_u64(start_time.elapsed().as_millis()),
            test_name: test_name.to_string(),
            event_type,
            level,
            phase,
            sequence_num,
            message: message.into(),
            context: None,
        }
    }

    fn with_context<T: Serialize>(mut self, ctx: &T) -> Self {
        self.context = serde_json::to_value(ctx).ok();
        self
    }
}

/// Extended structured logger with event types and phase tracking.
pub struct ExtendedLogger {
    writer: Option<BufWriter<File>>,
    start_time: Instant,
    min_level: LogLevel,
    test_name: String,
    current_phase: TestPhase,
    sequence_num: u32,
    entries: Vec<ExtendedLogEntry>,
    assertion_count: u32,
    assertion_passed: u32,
}

impl ExtendedLogger {
    /// Create a new extended logger for a test.
    pub fn new(log_path: &Path, test_name: &str) -> Self {
        let writer = File::create(log_path).ok().map(BufWriter::new);
        Self {
            writer,
            start_time: Instant::now(),
            min_level: LogLevel::from_env(),
            test_name: test_name.to_string(),
            current_phase: TestPhase::Setup,
            sequence_num: 0,
            entries: Vec::new(),
            assertion_count: 0,
            assertion_passed: 0,
        }
    }

    /// Create a disabled logger (no-op).
    pub fn disabled() -> Self {
        Self {
            writer: None,
            start_time: Instant::now(),
            min_level: LogLevel::Error,
            test_name: String::new(),
            current_phase: TestPhase::Setup,
            sequence_num: 0,
            entries: Vec::new(),
            assertion_count: 0,
            assertion_passed: 0,
        }
    }

    /// Log a test start event.
    pub fn test_start<T: Serialize>(&mut self, context: &T) {
        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::TestStart,
            LogLevel::Info,
            phase,
            seq,
            "Test started",
        )
        .with_context(context);
        self.log(entry);
    }

    /// Log a test end event.
    pub fn test_end(&mut self, passed: bool) {
        #[derive(Serialize)]
        struct TestEndContext {
            passed: bool,
            duration_ms: u64,
            assertions_total: u32,
            assertions_passed: u32,
        }

        let context = TestEndContext {
            passed,
            duration_ms: millis_to_u64(self.start_time.elapsed().as_millis()),
            assertions_total: self.assertion_count,
            assertions_passed: self.assertion_passed,
        };

        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::TestEnd,
            if passed {
                LogLevel::Info
            } else {
                LogLevel::Error
            },
            phase,
            seq,
            format!("Test {}", if passed { "PASSED" } else { "FAILED" }),
        )
        .with_context(&context);
        self.log(entry);
    }

    /// Transition to a new phase.
    pub fn set_phase(&mut self, phase: TestPhase) {
        let old_phase = self.current_phase;
        self.current_phase = phase;
        self.sequence_num = 0;

        let (start_time, test_name, _, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::PhaseChange,
            LogLevel::Debug,
            phase,
            seq,
            format!("Phase: {old_phase} -> {phase}"),
        );
        self.log(entry);
    }

    /// Log a step within the current phase.
    pub fn step(&mut self, message: impl Into<String>) {
        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Step,
            LogLevel::Info,
            phase,
            seq,
            message,
        );
        self.log(entry);
    }

    /// Log a step with context data.
    pub fn step_with_context<T: Serialize>(&mut self, message: impl Into<String>, context: &T) {
        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Step,
            LogLevel::Info,
            phase,
            seq,
            message,
        )
        .with_context(context);
        self.log(entry);
    }

    /// Log an assertion result.
    pub fn assertion(&mut self, passed: bool, expected: &str, actual: &str, message: &str) {
        self.assertion_count += 1;
        if passed {
            self.assertion_passed += 1;
        }

        #[derive(Serialize)]
        struct AssertionContext<'a> {
            expected: &'a str,
            actual: &'a str,
            passed: bool,
        }

        let context = AssertionContext {
            expected,
            actual,
            passed,
        };

        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Assertion,
            if passed {
                LogLevel::Debug
            } else {
                LogLevel::Error
            },
            phase,
            seq,
            message,
        )
        .with_context(&context);
        self.log(entry);
    }

    /// Log an error.
    pub fn error(&mut self, message: impl Into<String>) {
        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Error,
            LogLevel::Error,
            phase,
            seq,
            message,
        );
        self.log(entry);
    }

    /// Log an error with context.
    pub fn error_with_context<T: Serialize>(&mut self, message: impl Into<String>, context: &T) {
        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Error,
            LogLevel::Error,
            phase,
            seq,
            message,
        )
        .with_context(context);
        self.log(entry);
    }

    /// Log input being sent.
    pub fn input(&mut self, bytes: &[u8], description: &str) {
        #[derive(Serialize)]
        struct InputContext {
            bytes_len: usize,
            hex: String,
        }

        let context = InputContext {
            bytes_len: bytes.len(),
            hex: hex_encode(bytes),
        };

        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Input,
            LogLevel::Debug,
            phase,
            seq,
            description,
        )
        .with_context(&context);
        self.log(entry);
    }

    /// Log a timing metric.
    pub fn metric(&mut self, name: &str, value_ms: u64) {
        #[derive(Serialize)]
        struct MetricContext {
            metric_name: String,
            value_ms: u64,
        }

        let context = MetricContext {
            metric_name: name.to_string(),
            value_ms,
        };

        let (start_time, test_name, phase, seq) = self.entry_context();
        let entry = ExtendedLogEntry::new(
            start_time,
            &test_name,
            EventType::Metric,
            LogLevel::Info,
            phase,
            seq,
            format!("Metric: {name} = {value_ms}ms"),
        )
        .with_context(&context);
        self.log(entry);
    }

    fn next_sequence(&mut self) -> u32 {
        self.sequence_num += 1;
        self.sequence_num
    }

    /// Helper to get common entry fields, avoiding borrow checker issues.
    fn entry_context(&mut self) -> (Instant, String, TestPhase, u32) {
        let seq = self.next_sequence();
        (
            self.start_time,
            self.test_name.clone(),
            self.current_phase,
            seq,
        )
    }

    fn log(&mut self, entry: ExtendedLogEntry) {
        if !entry.level.should_log(self.min_level) {
            return;
        }

        // Print to stderr for immediate visibility
        eprintln!(
            "[{:06}ms] {:5} [{}] [{}] {}",
            entry.elapsed_ms,
            entry.level.to_string().to_uppercase(),
            entry.phase,
            entry.event_type,
            entry.message
        );

        // Write to file
        if let Some(ref mut writer) = self.writer {
            if let Ok(json) = serde_json::to_string(&entry) {
                let _ = writeln!(writer, "{json}");
            }
        }

        self.entries.push(entry);
    }

    /// Flush the log buffer.
    pub fn flush(&mut self) {
        if let Some(ref mut writer) = self.writer {
            let _ = writer.flush();
        }
    }

    /// Get all entries.
    pub fn entries(&self) -> &[ExtendedLogEntry] {
        &self.entries
    }
}

impl Drop for ExtendedLogger {
    fn drop(&mut self) {
        self.flush();
    }
}

// ============================================================================
// Ergonomic Macros for E2E Test Logging
// ============================================================================

/// Log a message with context using the extended logger.
///
/// # Examples
///
/// ```ignore
/// e2e_log!(harness, info, "Sending keystroke", { "key": "Enter" });
/// e2e_log!(harness, debug, "Buffer state captured");
/// ```
#[macro_export]
macro_rules! e2e_log {
    ($harness:expr, $level:ident, $msg:expr) => {
        $harness.extended_log().$level($msg)
    };
    ($harness:expr, $level:ident, $msg:expr, $ctx:expr) => {
        $harness.extended_log().step_with_context($msg, &$ctx)
    };
}

/// Assert with automatic logging to the extended logger.
///
/// # Examples
///
/// ```ignore
/// e2e_assert!(harness, cell.char() == 'X', "Cell content mismatch");
/// e2e_assert!(harness, actual == expected, "Values differ", expected, actual);
/// ```
#[macro_export]
macro_rules! e2e_assert {
    ($harness:expr, $cond:expr, $msg:expr) => {{
        let passed = $cond;
        $harness
            .extended_log()
            .assertion(passed, "true", &format!("{}", passed), $msg);
        assert!(passed, "{}", $msg);
    }};
    ($harness:expr, $cond:expr, $msg:expr, $expected:expr, $actual:expr) => {{
        let passed = $cond;
        $harness.extended_log().assertion(
            passed,
            &format!("{:?}", $expected),
            &format!("{:?}", $actual),
            $msg,
        );
        assert!(
            passed,
            "{}: expected {:?}, got {:?}",
            $msg, $expected, $actual
        );
    }};
}

/// Log a test step with optional context.
///
/// # Examples
///
/// ```ignore
/// e2e_step!(harness, "Initializing buffer");
/// e2e_step!(harness, "Sending input", { "key": "Enter", "modifiers": [] });
/// ```
#[macro_export]
macro_rules! e2e_step {
    ($harness:expr, $msg:expr) => {
        $harness.extended_log().step($msg)
    };
    ($harness:expr, $msg:expr, $ctx:expr) => {
        $harness.extended_log().step_with_context($msg, &$ctx)
    };
}

/// Set the test phase.
///
/// # Examples
///
/// ```ignore
/// e2e_phase!(harness, Execute);
/// e2e_phase!(harness, Verify);
/// ```
#[macro_export]
macro_rules! e2e_phase {
    ($harness:expr, $phase:ident) => {
        $harness
            .extended_log()
            .set_phase($crate::common::harness::TestPhase::$phase)
    };
}

// ============================================================================
// Extended E2E Harness with Phase Tracking
// ============================================================================

impl E2EHarness {
    /// Create an extended logger for this harness.
    ///
    /// Note: This creates a new logger. For most uses, prefer the built-in
    /// structured_log methods which are already integrated.
    pub fn create_extended_logger(&self) -> ExtendedLogger {
        let log_path = self.artifact_logger.artifact_dir.join("extended.jsonl");
        if self.artifact_logger.config.enabled {
            ExtendedLogger::new(
                &log_path,
                &format!(
                    "{}::{}",
                    self.artifact_logger.suite, self.artifact_logger.test
                ),
            )
        } else {
            ExtendedLogger::disabled()
        }
    }
}

// ============================================================================
// Tests for Extended Logging
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_display() {
        assert_eq!(EventType::TestStart.to_string(), "test_start");
        assert_eq!(EventType::Step.to_string(), "step");
        assert_eq!(EventType::Assertion.to_string(), "assertion");
    }

    #[test]
    fn test_test_phase_display() {
        assert_eq!(TestPhase::Setup.to_string(), "setup");
        assert_eq!(TestPhase::Execute.to_string(), "execute");
        assert_eq!(TestPhase::Verify.to_string(), "verify");
        assert_eq!(TestPhase::Teardown.to_string(), "teardown");
    }

    #[test]
    fn test_extended_logger_phases() {
        let mut logger = ExtendedLogger::disabled();
        assert_eq!(logger.current_phase, TestPhase::Setup);

        logger.set_phase(TestPhase::Execute);
        assert_eq!(logger.current_phase, TestPhase::Execute);

        logger.set_phase(TestPhase::Verify);
        assert_eq!(logger.current_phase, TestPhase::Verify);
    }

    #[test]
    fn test_extended_logger_assertions() {
        let mut logger = ExtendedLogger::disabled();

        logger.assertion(true, "expected", "actual", "Test assertion");
        assert_eq!(logger.assertion_count, 1);
        assert_eq!(logger.assertion_passed, 1);

        logger.assertion(false, "expected", "different", "Failed assertion");
        assert_eq!(logger.assertion_count, 2);
        assert_eq!(logger.assertion_passed, 1);
    }

    #[test]
    fn test_log_level_ordering() {
        assert!(LogLevel::Error.should_log(LogLevel::Debug));
        assert!(LogLevel::Warn.should_log(LogLevel::Debug));
        assert!(!LogLevel::Debug.should_log(LogLevel::Warn));
    }

    #[test]
    fn test_extended_log_entry_serialization() {
        let entry = ExtendedLogEntry {
            step_id: 1,
            ts_ms: 1234567890,
            elapsed_ms: 100,
            test_name: "test_example".to_string(),
            event_type: EventType::Step,
            level: LogLevel::Info,
            phase: TestPhase::Execute,
            sequence_num: 1,
            message: "Test message".to_string(),
            context: None,
        };

        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"event_type\":\"step\""));
        assert!(json.contains("\"phase\":\"execute\""));
        assert!(json.contains("\"level\":\"info\""));
    }
}
