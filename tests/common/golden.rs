//! Golden file testing utilities.
//!
//! This module provides utilities for comparing actual output to golden files,
//! with support for updating golden files via environment variables.
//!
//! # Environment Variables
//!
//! - `GOLDEN_UPDATE=1` - Update golden files instead of comparing
//! - `BLESS=1` - Alias for GOLDEN_UPDATE (insta-style)
//!

#![allow(clippy::format_push_string)] // Test code - clarity over micro-optimization
//! # Golden File Format
//!
//! ```text
//! # Golden file: test_name
//! # Generated: 2026-01-28
//! # Terminal: xterm-256color
//! # Size: 80x24
//! ---
//! <raw ANSI output>
//! ```

#![allow(dead_code)]

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// Golden file metadata.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GoldenMetadata {
    /// Test name.
    pub name: String,
    /// Generation date (YYYY-MM-DD).
    pub generated: String,
    /// Terminal type.
    pub terminal: String,
    /// Terminal size (width, height).
    pub size: (u32, u32),
    /// Optional additional metadata.
    pub extra: Vec<(String, String)>,
}

/// A loaded golden file.
#[derive(Clone, Debug)]
pub struct GoldenFile {
    /// File path.
    pub path: PathBuf,
    /// Metadata from header.
    pub metadata: GoldenMetadata,
    /// Raw content bytes.
    pub content: Vec<u8>,
}

/// Result of comparing actual output to golden file.
#[derive(Clone, Debug)]
pub enum GoldenResult {
    /// Output matches golden file.
    Match,
    /// Output differs from golden file.
    Mismatch {
        expected: Vec<u8>,
        actual: Vec<u8>,
        diff_summary: String,
    },
    /// Golden file not found (first run).
    NotFound { path: PathBuf },
    /// Golden file updated (when GOLDEN_UPDATE=1).
    Updated { path: PathBuf },
}

impl GoldenResult {
    /// Returns true if the result is a match or update.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Match | Self::Updated { .. })
    }

    /// Returns true if the result is a mismatch.
    #[must_use]
    pub fn is_mismatch(&self) -> bool {
        matches!(self, Self::Mismatch { .. })
    }
}

/// Check if golden file update mode is enabled.
#[must_use]
pub fn should_update() -> bool {
    std::env::var("GOLDEN_UPDATE").is_ok_and(|v| v == "1")
        || std::env::var("BLESS").is_ok_and(|v| v == "1")
}

/// Get the golden files directory.
#[must_use]
pub fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

/// Load a golden file by name.
pub fn load_golden(name: &str) -> Result<GoldenFile, std::io::Error> {
    let path = golden_dir().join(format!("{name}.golden"));
    load_golden_from_path(&path)
}

/// Load a golden file from a specific path.
pub fn load_golden_from_path(path: &Path) -> Result<GoldenFile, std::io::Error> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut metadata = GoldenMetadata::default();
    let mut header_lines = Vec::new();
    let mut line = String::new();

    // Parse header lines (starting with #)
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if trimmed.starts_with('#') {
            header_lines.push(trimmed.to_string());
        }
    }

    // Parse metadata from header
    for header in &header_lines {
        if let Some(rest) = header.strip_prefix("# Golden file: ") {
            metadata.name = rest.trim().to_string();
        } else if let Some(rest) = header.strip_prefix("# Generated: ") {
            metadata.generated = rest.trim().to_string();
        } else if let Some(rest) = header.strip_prefix("# Terminal: ") {
            metadata.terminal = rest.trim().to_string();
        } else if let Some(rest) = header.strip_prefix("# Size: ") {
            if let Some((w, h)) = rest.trim().split_once('x') {
                if let (Ok(width), Ok(height)) = (w.parse(), h.parse()) {
                    metadata.size = (width, height);
                }
            }
        } else if let Some(rest) = header.strip_prefix("# ") {
            if let Some((key, value)) = rest.split_once(": ") {
                metadata.extra.push((key.to_string(), value.to_string()));
            }
        }
    }

    // Read remaining content
    let mut content = Vec::new();
    std::io::Read::read_to_end(&mut reader, &mut content)?;

    Ok(GoldenFile {
        path: path.to_path_buf(),
        metadata,
        content,
    })
}

/// Save a golden file.
pub fn save_golden(
    name: &str,
    metadata: &GoldenMetadata,
    content: &[u8],
) -> Result<PathBuf, std::io::Error> {
    let dir = golden_dir();
    fs::create_dir_all(&dir)?;

    let path = dir.join(format!("{name}.golden"));
    let mut file = File::create(&path)?;

    // Write header
    writeln!(file, "# Golden file: {}", metadata.name)?;
    writeln!(file, "# Generated: {}", metadata.generated)?;
    writeln!(file, "# Terminal: {}", metadata.terminal)?;
    writeln!(file, "# Size: {}x{}", metadata.size.0, metadata.size.1)?;
    for (key, value) in &metadata.extra {
        writeln!(file, "# {key}: {value}")?;
    }
    writeln!(file, "---")?;

    // Write content
    file.write_all(content)?;

    Ok(path)
}

/// Compare actual output to a golden file.
pub fn compare_golden(name: &str, actual: &[u8], metadata: &GoldenMetadata) -> GoldenResult {
    let path = golden_dir().join(format!("{name}.golden"));

    // Update mode
    if should_update() {
        match save_golden(name, metadata, actual) {
            Ok(path) => return GoldenResult::Updated { path },
            Err(e) => {
                eprintln!("Failed to update golden file: {e}");
            }
        }
    }

    // Load and compare
    match load_golden(name) {
        Ok(golden) => {
            if golden.content == actual {
                GoldenResult::Match
            } else {
                let diff_summary = compute_diff_summary(&golden.content, actual);
                GoldenResult::Mismatch {
                    expected: golden.content,
                    actual: actual.to_vec(),
                    diff_summary,
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // First run - create the golden file
            if let Err(e) = save_golden(name, metadata, actual) {
                eprintln!("Failed to create golden file: {e}");
            }
            GoldenResult::NotFound { path }
        }
        Err(e) => unreachable!("Failed to load golden file {name}: {e}"),
    }
}

/// Compute a human-readable diff summary.
fn compute_diff_summary(expected: &[u8], actual: &[u8]) -> String {
    let mut summary = String::new();

    summary.push_str(&format!(
        "Length: expected {} bytes, actual {} bytes\n",
        expected.len(),
        actual.len()
    ));

    // Find first difference
    let first_diff = expected.iter().zip(actual.iter()).position(|(a, b)| a != b);

    if let Some(pos) = first_diff {
        summary.push_str(&format!("First difference at byte {pos}\n"));

        // Show context around first difference
        let start = pos.saturating_sub(20);
        let end_exp = (pos + 20).min(expected.len());
        let end_act = (pos + 20).min(actual.len());

        summary.push_str("Expected: ");
        summary.push_str(&escape_bytes(&expected[start..end_exp]));
        summary.push('\n');

        summary.push_str("Actual:   ");
        summary.push_str(&escape_bytes(&actual[start..end_act]));
        summary.push('\n');
    } else if expected.len() != actual.len() {
        // Content matches up to the shorter length
        let min_len = expected.len().min(actual.len());
        summary.push_str(&format!("Content matches for first {min_len} bytes\n"));

        if expected.len() > actual.len() {
            summary.push_str("Expected has extra bytes: ");
            // Show up to 40 bytes of the extra content
            let end = (min_len + 40).min(expected.len());
            summary.push_str(&escape_bytes(&expected[min_len..end]));
        } else {
            summary.push_str("Actual has extra bytes: ");
            // Show up to 40 bytes of the extra content
            let end = (min_len + 40).min(actual.len());
            summary.push_str(&escape_bytes(&actual[min_len..end]));
        }
        summary.push('\n');
    }

    summary
}

/// Escape bytes for display.
fn escape_bytes(bytes: &[u8]) -> String {
    let mut result = String::new();
    for &b in bytes {
        match b {
            0x1b => result.push_str("\\e"),
            0x07 => result.push_str("\\a"),
            0x08 => result.push_str("\\b"),
            0x09 => result.push_str("\\t"),
            0x0a => result.push_str("\\n"),
            0x0d => result.push_str("\\r"),
            b if (0x20..0x7f).contains(&b) => result.push(b as char),
            b => result.push_str(&format!("\\x{b:02x}")),
        }
    }
    result
}

/// Get the current date in YYYY-MM-DD format.
#[must_use]
pub fn current_date() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();

    // Simple date calculation
    let days = secs / 86400;
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}")
}

fn days_to_ymd(days: u64) -> (u32, u32, u32) {
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

/// Macro for asserting golden file matches.
///
/// # Examples
///
/// ```ignore
/// assert_golden!("test_name", actual_output, 80, 24);
/// ```
#[macro_export]
macro_rules! assert_golden {
    ($name:expr, $actual:expr, $width:expr, $height:expr) => {{
        use $crate::common::golden::{GoldenMetadata, GoldenResult, compare_golden, current_date};

        let metadata = GoldenMetadata {
            name: $name.to_string(),
            generated: current_date(),
            terminal: "xterm-256color".to_string(),
            size: ($width, $height),
            extra: vec![],
        };

        match compare_golden($name, $actual, &metadata) {
            GoldenResult::Match => {}
            GoldenResult::Updated { path } => {
                eprintln!("Updated golden file: {}", path.display());
            }
            GoldenResult::NotFound { path } => {
                eprintln!("Created new golden file: {}", path.display());
            }
            GoldenResult::Mismatch { diff_summary, .. } => {
                unreachable!("Golden file mismatch for '{}':\n{}", $name, diff_summary);
            }
        }
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_date_format() {
        let date = current_date();
        // Should be YYYY-MM-DD format
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }

    #[test]
    fn test_escape_bytes() {
        let bytes = b"\x1b[31mHello\x1b[0m\n";
        let escaped = escape_bytes(bytes);
        assert!(escaped.contains("\\e"));
        assert!(escaped.contains("Hello"));
        assert!(escaped.contains("\\n"));
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
    fn test_golden_metadata_default() {
        let meta = GoldenMetadata::default();
        assert!(meta.name.is_empty());
        assert_eq!(meta.size, (0, 0));
    }
}
