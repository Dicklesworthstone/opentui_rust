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
