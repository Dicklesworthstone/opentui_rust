//! Artifact management for E2E test debugging.
//!
//! When `HARNESS_ARTIFACTS=1`, this module captures test outputs for debugging:
//! - Raw PTY output
//! - Decoded/readable output
//! - ANSI sequence analysis
//! - Buffer screenshots
//!
//! # Environment Variables
//!
//! - `HARNESS_ARTIFACTS=1` - Enable artifact capture
//! - `HARNESS_ARTIFACTS_DIR` - Custom artifact directory (default: `target/e2e-artifacts`)
//! - `HARNESS_PRESERVE_SUCCESS=1` - Keep artifacts even for passing tests

#![allow(dead_code)]

use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

/// Global run timestamp for consistent directory naming across all tests in a run.
static RUN_TIMESTAMP: OnceLock<String> = OnceLock::new();

fn get_run_timestamp() -> &'static str {
    RUN_TIMESTAMP.get_or_init(|| {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let secs = now.as_secs();
        // Format: YYYY-MM-DDTHHMMSS
        let datetime = chrono_lite_format(secs);
        datetime
    })
}

/// Simple timestamp formatter without chrono dependency.
fn chrono_lite_format(unix_secs: u64) -> String {
    // Convert unix timestamp to date components
    // Days since Unix epoch
    let days = unix_secs / 86400;
    let time_of_day = unix_secs % 86400;

    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Calculate year, month, day from days since 1970-01-01
    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}{:02}{:02}",
        year, month, day, hours, minutes, seconds
    )
}

/// Convert days since Unix epoch to year, month, day.
fn days_to_ymd(days: u64) -> (u32, u32, u32) {
    // Simplified algorithm - good enough for recent dates
    let mut remaining = days as i64;
    let mut year = 1970u32;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let days_in_months: [i64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for days_in_month in days_in_months {
        if remaining < days_in_month {
            break;
        }
        remaining -= days_in_month;
        month += 1;
    }

    let day = (remaining + 1) as u32;
    (year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Configuration for artifact capture.
#[derive(Clone, Debug)]
pub struct ArtifactConfig {
    /// Whether artifact capture is enabled.
    pub enabled: bool,
    /// Base directory for artifacts.
    pub base_dir: PathBuf,
    /// Preserve artifacts even for successful tests.
    pub preserve_on_success: bool,
}

impl Default for ArtifactConfig {
    fn default() -> Self {
        let enabled = std::env::var("HARNESS_ARTIFACTS").is_ok_and(|v| v == "1");
        let base_dir = std::env::var("HARNESS_ARTIFACTS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("target/e2e-artifacts"));
        let preserve_on_success = std::env::var("HARNESS_PRESERVE_SUCCESS").is_ok_and(|v| v == "1");

        Self {
            enabled,
            base_dir,
            preserve_on_success,
        }
    }
}

/// Manages artifact storage for a single test run.
#[derive(Debug)]
pub struct ArtifactManager {
    config: ArtifactConfig,
    test_name: String,
    artifact_dir: PathBuf,
}

impl ArtifactManager {
    /// Create a new artifact manager for a test.
    pub fn new(test_name: &str) -> Self {
        let config = ArtifactConfig::default();
        let timestamp = get_run_timestamp();
        let artifact_dir = config.base_dir.join(timestamp).join(test_name);

        if config.enabled {
            fs::create_dir_all(&artifact_dir).ok();
        }

        Self {
            config,
            test_name: test_name.to_string(),
            artifact_dir,
        }
    }

    /// Create with custom config.
    pub fn with_config(test_name: &str, config: ArtifactConfig) -> Self {
        let timestamp = get_run_timestamp();
        let artifact_dir = config.base_dir.join(timestamp).join(test_name);

        if config.enabled {
            fs::create_dir_all(&artifact_dir).ok();
        }

        Self {
            config,
            test_name: test_name.to_string(),
            artifact_dir,
        }
    }

    /// Check if artifacts are enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the artifact directory path.
    pub fn artifact_dir(&self) -> &Path {
        &self.artifact_dir
    }

    /// Save raw PTY output.
    pub fn save_raw_output(&self, data: &[u8]) {
        if !self.config.enabled {
            return;
        }
        let path = self
            .artifact_dir
            .join(format!("{}.raw.bin", self.test_name));
        fs::write(&path, data).ok();
        eprintln!("  Artifact: {}", path.display());
    }

    /// Save decoded/readable output.
    pub fn save_decoded_output(&self, readable: &str) {
        if !self.config.enabled {
            return;
        }
        let path = self
            .artifact_dir
            .join(format!("{}.decoded.txt", self.test_name));
        fs::write(&path, readable).ok();
        eprintln!("  Artifact: {}", path.display());
    }

    /// Save ANSI sequence analysis as JSON.
    pub fn save_sequence_analysis(&self, analysis: &SequenceAnalysis) {
        if !self.config.enabled {
            return;
        }
        let path = self
            .artifact_dir
            .join(format!("{}.sequences.json", self.test_name));
        if let Ok(json) = serde_json::to_string_pretty(analysis) {
            fs::write(&path, json).ok();
            eprintln!("  Artifact: {}", path.display());
        }
    }

    /// Save a text-based "screenshot" of buffer state.
    pub fn save_screenshot(&self, screenshot: &str) {
        if !self.config.enabled {
            return;
        }
        let path = self
            .artifact_dir
            .join(format!("{}.screenshot.txt", self.test_name));
        fs::write(&path, screenshot).ok();
        eprintln!("  Artifact: {}", path.display());
    }

    /// Save arbitrary text artifact.
    pub fn save_text(&self, name: &str, content: &str) {
        if !self.config.enabled {
            return;
        }
        let path = self.artifact_dir.join(name);
        fs::write(&path, content).ok();
    }

    /// Save arbitrary binary artifact.
    pub fn save_binary(&self, name: &str, data: &[u8]) {
        if !self.config.enabled {
            return;
        }
        let path = self.artifact_dir.join(name);
        fs::write(&path, data).ok();
    }

    /// Save JSON artifact.
    pub fn save_json<T: Serialize>(&self, name: &str, data: &T) {
        if !self.config.enabled {
            return;
        }
        let path = self.artifact_dir.join(name);
        if let Ok(json) = serde_json::to_string_pretty(data) {
            fs::write(&path, json).ok();
        }
    }

    /// Create a JSONL writer for streaming log output.
    pub fn create_jsonl_writer(&self, name: &str) -> Option<JsonlWriter> {
        if !self.config.enabled {
            return None;
        }
        let path = self.artifact_dir.join(name);
        File::create(&path).ok().map(|f| JsonlWriter {
            writer: BufWriter::new(f),
        })
    }

    /// Clean up artifacts if test passed and preserve_on_success is false.
    pub fn cleanup_on_success(&self) {
        if self.config.preserve_on_success || !self.config.enabled {
            return;
        }
        // Remove the test-specific directory
        fs::remove_dir_all(&self.artifact_dir).ok();
    }
}

/// JSONL (JSON Lines) writer for streaming log output.
pub struct JsonlWriter {
    writer: BufWriter<File>,
}

impl JsonlWriter {
    /// Write a single JSON line.
    pub fn write<T: Serialize>(&mut self, entry: &T) {
        if let Ok(json) = serde_json::to_string(entry) {
            let _ = writeln!(self.writer, "{json}");
        }
    }

    /// Flush the writer.
    pub fn flush(&mut self) {
        let _ = self.writer.flush();
    }
}

impl Drop for JsonlWriter {
    fn drop(&mut self) {
        self.flush();
    }
}

/// Analysis of ANSI sequences found in PTY output.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SequenceAnalysis {
    /// Total output bytes.
    pub total_bytes: usize,
    /// Number of ESC sequences found.
    pub esc_sequences: usize,
    /// Number of CSI (Control Sequence Introducer) sequences.
    pub csi_sequences: usize,
    /// Number of OSC (Operating System Command) sequences.
    pub osc_sequences: usize,
    /// Detected terminal mode changes.
    pub mode_changes: Vec<ModeChange>,
    /// Detected cursor operations.
    pub cursor_ops: Vec<String>,
    /// Detected color sequences.
    pub color_ops: usize,
    /// Detected text attributes (bold, italic, etc.).
    pub text_attrs: usize,
    /// OSC 8 hyperlinks found.
    pub hyperlinks: usize,
    /// Summary of sequences by type.
    pub sequence_summary: SequenceSummary,
}

/// Terminal mode change event.
#[derive(Clone, Debug, Serialize)]
pub struct ModeChange {
    pub mode: String,
    pub enabled: bool,
    pub offset: usize,
}

/// Summary counts of sequence types.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SequenceSummary {
    pub alt_screen_enter: usize,
    pub alt_screen_leave: usize,
    pub cursor_hide: usize,
    pub cursor_show: usize,
    pub cursor_position: usize,
    pub mouse_enable: usize,
    pub mouse_disable: usize,
    pub sync_output_begin: usize,
    pub sync_output_end: usize,
    pub bracketed_paste_enable: usize,
    pub bracketed_paste_disable: usize,
    pub focus_enable: usize,
    pub focus_disable: usize,
    pub sgr_reset: usize,
    pub erase_display: usize,
    pub erase_line: usize,
}

impl SequenceAnalysis {
    /// Analyze PTY output and extract sequence information.
    pub fn from_output(output: &[u8]) -> Self {
        let mut analysis = Self {
            total_bytes: output.len(),
            ..Default::default()
        };

        let mut i = 0;
        while i < output.len() {
            if output[i] == 0x1b {
                // ESC sequence
                analysis.esc_sequences += 1;

                if i + 1 < output.len() {
                    match output[i + 1] {
                        b'[' => {
                            // CSI sequence
                            analysis.csi_sequences += 1;
                            analysis.analyze_csi(output, i);
                        }
                        b']' => {
                            // OSC sequence
                            analysis.osc_sequences += 1;
                            if output[i..].starts_with(b"\x1b]8;") {
                                analysis.hyperlinks += 1;
                            }
                        }
                        _ => {}
                    }
                }
            }
            i += 1;
        }

        analysis
    }

    fn analyze_csi(&mut self, output: &[u8], offset: usize) {
        let remaining = &output[offset..];

        // Private mode set/reset (starts_with is correct for prefix matching)
        if remaining.starts_with(b"\x1b[?") {
            self.analyze_private_mode(remaining, offset);
            return;
        }

        // Find the final byte of this CSI sequence.
        // CSI format: ESC [ <params 0x30-0x3F>* <intermediates 0x20-0x2F>* <final 0x40-0x7E>
        // Skip ESC and '[' (first 2 bytes), then find the first byte in the final range.
        let Some(&final_byte) = remaining
            .iter()
            .skip(2)
            .find(|&&b| (0x40..=0x7E).contains(&b))
        else {
            return; // Incomplete or malformed CSI sequence
        };

        match final_byte {
            b'H' | b'f' => {
                self.sequence_summary.cursor_position += 1;
                self.cursor_ops.push(format!("CUP at offset {offset}"));
            }
            b'm' => {
                if remaining.starts_with(b"\x1b[0m") || remaining.starts_with(b"\x1b[m") {
                    self.sequence_summary.sgr_reset += 1;
                }
                // Count color/attribute sequences by checking params for semicolons.
                // Params are between ESC[ (pos 2) and the final 'm' byte.
                if let Some(m_pos) = remaining.iter().position(|&b| b == b'm') {
                    if remaining[2..m_pos].contains(&b';') {
                        self.color_ops += 1;
                    }
                }
                self.text_attrs += 1;
            }
            b'J' => {
                self.sequence_summary.erase_display += 1;
            }
            b'K' => {
                self.sequence_summary.erase_line += 1;
            }
            _ => {}
        }
    }

    fn analyze_private_mode(&mut self, seq: &[u8], offset: usize) {
        // Check common private modes
        let patterns: &[(&[u8], &str, bool, fn(&mut SequenceSummary))] = &[
            (b"\x1b[?1049h", "alt_screen", true, |s| {
                s.alt_screen_enter += 1
            }),
            (b"\x1b[?1049l", "alt_screen", false, |s| {
                s.alt_screen_leave += 1;
            }),
            (b"\x1b[?25h", "cursor_visible", true, |s| s.cursor_show += 1),
            (b"\x1b[?25l", "cursor_visible", false, |s| {
                s.cursor_hide += 1
            }),
            (b"\x1b[?1000h", "mouse_button", true, |s| {
                s.mouse_enable += 1
            }),
            (b"\x1b[?1000l", "mouse_button", false, |s| {
                s.mouse_disable += 1;
            }),
            (b"\x1b[?1002h", "mouse_motion", true, |s| {
                s.mouse_enable += 1
            }),
            (b"\x1b[?1002l", "mouse_motion", false, |s| {
                s.mouse_disable += 1;
            }),
            (b"\x1b[?1003h", "mouse_all", true, |s| s.mouse_enable += 1),
            (b"\x1b[?1003l", "mouse_all", false, |s| s.mouse_disable += 1),
            (b"\x1b[?2026h", "sync_output", true, |s| {
                s.sync_output_begin += 1;
            }),
            (b"\x1b[?2026l", "sync_output", false, |s| {
                s.sync_output_end += 1;
            }),
            (b"\x1b[?2004h", "bracketed_paste", true, |s| {
                s.bracketed_paste_enable += 1;
            }),
            (b"\x1b[?2004l", "bracketed_paste", false, |s| {
                s.bracketed_paste_disable += 1;
            }),
            (b"\x1b[?1004h", "focus_reporting", true, |s| {
                s.focus_enable += 1;
            }),
            (b"\x1b[?1004l", "focus_reporting", false, |s| {
                s.focus_disable += 1;
            }),
        ];

        for (pattern, mode, enabled, update_fn) in patterns {
            if seq.starts_with(pattern) {
                self.mode_changes.push(ModeChange {
                    mode: (*mode).to_string(),
                    enabled: *enabled,
                    offset,
                });
                update_fn(&mut self.sequence_summary);
                return;
            }
        }
    }
}

/// Convert raw output to readable text format.
pub fn output_to_readable(output: &[u8]) -> String {
    use std::fmt::Write;
    let mut result = String::new();
    for &b in output {
        match b {
            0x1b => result.push_str("ESC"),
            0x07 => result.push_str("<BEL>"),
            0x08 => result.push_str("<BS>"),
            0x09 => result.push_str("<TAB>"),
            0x0a => result.push_str("<LF>\n"),
            0x0d => result.push_str("<CR>"),
            b if (0x20..0x7f).contains(&b) => result.push(b as char),
            b => {
                let _ = write!(result, "<{b:02X}>");
            }
        }
    }
    result
}

/// Convert raw output to hex dump.
pub fn output_to_hex(output: &[u8]) -> String {
    output
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono_lite_format() {
        // 2024-01-15 12:30:45 UTC = 1705322445
        let ts = chrono_lite_format(1705322445);
        assert!(ts.starts_with("2024-01-15T"));
    }

    #[test]
    fn test_days_to_ymd() {
        // 2024-01-01 is 19723 days since 1970-01-01
        let (year, month, day) = days_to_ymd(19723);
        assert_eq!(year, 2024);
        assert_eq!(month, 1);
        assert_eq!(day, 1);
    }

    #[test]
    fn test_sequence_analysis_basic() {
        let output = b"\x1b[?1049h\x1b[?25l\x1b[2JHello\x1b[?25h\x1b[?1049l";
        let analysis = SequenceAnalysis::from_output(output);

        assert_eq!(analysis.sequence_summary.alt_screen_enter, 1);
        assert_eq!(analysis.sequence_summary.alt_screen_leave, 1);
        assert_eq!(analysis.sequence_summary.cursor_hide, 1);
        assert_eq!(analysis.sequence_summary.cursor_show, 1);
    }

    #[test]
    fn test_sequence_analysis_hyperlinks() {
        let output = b"\x1b]8;;https://example.com\x07Link\x1b]8;;\x07";
        let analysis = SequenceAnalysis::from_output(output);
        assert_eq!(analysis.hyperlinks, 2);
    }

    #[test]
    fn test_output_to_readable() {
        let output = b"\x1b[H\x1bOA";
        let readable = output_to_readable(output);
        assert!(readable.contains("ESC"));
    }

    #[test]
    fn test_artifact_config_default() {
        let config = ArtifactConfig::default();
        // Without env vars, should be disabled
        assert!(!config.enabled);
        assert_eq!(config.base_dir, PathBuf::from("target/e2e-artifacts"));
    }
}
