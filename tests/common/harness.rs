//! Conformance and benchmark test harness utilities.

#![allow(dead_code)]

use opentui::input::{Event, InputParser, ParseError};
use opentui::{OptimizedBuffer, Style};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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
    artifact_dir: PathBuf,
    config: ArtifactConfig,
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

/// Full E2E test harness with input injection and output capture.
pub struct E2EHarness {
    artifact_logger: ArtifactLogger,
    input_buffer: Vec<u8>,
    output_buffer: OptimizedBuffer,
    parser: InputParser,
    events: Vec<(Duration, Event)>,
    start_time: Instant,
}

impl E2EHarness {
    /// Create a new E2E harness for a test.
    pub fn new(suite: &str, test: &str, width: u32, height: u32) -> Self {
        Self {
            artifact_logger: ArtifactLogger::new(suite, test),
            input_buffer: Vec::new(),
            output_buffer: OptimizedBuffer::new(width, height),
            parser: InputParser::new(),
            events: Vec::new(),
            start_time: Instant::now(),
        }
    }

    /// Inject input bytes and parse events.
    ///
    /// Note: For bracketed paste, the parser expects chunked delivery:
    /// 1. Call `inject_input(b"\x1b[200~")` to enter paste mode
    /// 2. Call `inject_input(b"content\x1b[201~")` to get the paste event
    pub fn inject_input(&mut self, bytes: &[u8]) -> Vec<Event> {
        eprintln!(
            "[HARNESS] Injecting {} bytes: {:02x?}",
            bytes.len(),
            bytes
        );
        self.input_buffer.extend_from_slice(bytes);

        let mut events = Vec::new();

        loop {
            if self.input_buffer.is_empty() {
                break;
            }

            match self.parser.parse(&self.input_buffer) {
                Ok((event, consumed)) => {
                    let elapsed = self.start_time.elapsed();
                    eprintln!("[HARNESS] {:?} Parsed event: {:?}", elapsed, event);
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
    pub fn buffer_mut(&mut self) -> &mut OptimizedBuffer {
        &mut self.output_buffer
    }

    /// Get the output buffer (immutable).
    pub fn buffer(&self) -> &OptimizedBuffer {
        &self.output_buffer
    }

    /// Dump buffer contents to artifact file.
    pub fn dump_buffer(&self, name: &str) {
        let mut output = String::new();
        for y in 0..self.output_buffer.height() {
            for x in 0..self.output_buffer.width() {
                if let Some(cell) = self.output_buffer.get(x, y) {
                    match &cell.content {
                        opentui::CellContent::Char(c) => output.push(*c),
                        opentui::CellContent::Grapheme(g) => output.push_str(g),
                        opentui::CellContent::Empty | opentui::CellContent::Continuation => {
                            output.push(' ')
                        }
                    }
                } else {
                    output.push(' ');
                }
            }
            output.push('\n');
        }

        eprintln!("[HARNESS] Buffer dump '{name}':\n{output}");

        // Also write to artifact file
        self.artifact_logger.log_text(name, &output, &output);
    }

    /// Assert cell at position has expected content.
    pub fn assert_cell(&self, x: u32, y: u32, expected_char: char, msg: &str) {
        let cell = self.output_buffer.get(x, y).expect("Cell should exist");
        let actual = match &cell.content {
            opentui::CellContent::Char(c) => c.to_string(),
            opentui::CellContent::Grapheme(g) => g.to_string(),
            opentui::CellContent::Empty | opentui::CellContent::Continuation => " ".to_string(),
        };
        let expected = expected_char.to_string();

        eprintln!(
            "[HARNESS] assert_cell({x},{y}) expected='{expected}' actual='{actual}'"
        );

        assert_eq!(actual, expected, "{msg} at ({x},{y})");
    }

    /// Assert cell style matches predicate.
    pub fn assert_style<F>(&self, x: u32, y: u32, predicate: F, msg: &str)
    where
        F: Fn(&Style) -> bool,
    {
        let cell = self.output_buffer.get(x, y).expect("Cell should exist");
        let style = Style {
            fg: Some(cell.fg),
            bg: Some(cell.bg),
            attributes: cell.attributes,
            link_id: cell.link_id,
        };
        eprintln!("[HARNESS] assert_style({x},{y}) style={:?}", style);

        assert!(predicate(&style), "{msg} at ({x},{y})");
    }

    /// Get all parsed events.
    pub fn events(&self) -> &[(Duration, Event)] {
        &self.events
    }

    /// Write test summary.
    pub fn finish(&self, passed: bool) {
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
    }
}
