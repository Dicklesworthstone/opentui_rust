//! Sequence Analysis Report for E2E test debugging.
//!
//! This module provides detailed ANSI sequence analysis for test failure diagnosis:
//!
//! - **Sequence inventory**: All detected ANSI sequences with counts
//! - **Timeline view**: Sequences in order of appearance with byte offsets
//! - **Missing sequences**: Expected but not found
//! - **Unexpected sequences**: Found but not expected
//! - **Paired validation**: Ensures enter/leave, hide/show pairs match
//!
//! # Output Formats
//!
//! Reports can be generated in both human-readable text and JSON formats
//! for CI integration.
//!
//! # Usage
//!
//! ```ignore
//! use crate::common::analysis::{SequenceReport, ExpectedSequences};
//!
//! let report = SequenceReport::from_output(&pty_result.output);
//!
//! // Check paired sequences
//! let validation = report.validate_pairs();
//! assert!(validation.all_paired(), "{}", validation.to_human_readable());
//!
//! // Check expected sequences
//! let expected = ExpectedSequences::default_terminal_setup();
//! let missing = report.find_missing(&expected);
//! assert!(missing.is_empty(), "Missing sequences: {:?}", missing);
//! ```

#![allow(dead_code)]

use serde::Serialize;
use std::collections::HashMap;
use std::fmt::Write;

/// Known ANSI sequence patterns for detection.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SequenceKind {
    AltScreenEnter,
    AltScreenLeave,
    CursorHide,
    CursorShow,
    CursorPosition,
    MouseButtonEnable,
    MouseButtonDisable,
    MouseMotionEnable,
    MouseMotionDisable,
    MouseAllEnable,
    MouseAllDisable,
    MouseSgrEnable,
    MouseSgrDisable,
    SyncOutputBegin,
    SyncOutputEnd,
    BracketedPasteEnable,
    BracketedPasteDisable,
    FocusEnable,
    FocusDisable,
    SgrReset,
    SgrColor,
    EraseDisplay,
    EraseLine,
    Osc8Hyperlink,
    SetTitle,
    Unknown,
}

impl SequenceKind {
    /// Get the paired sequence kind (if applicable).
    pub const fn pair(self) -> Option<Self> {
        match self {
            Self::AltScreenEnter => Some(Self::AltScreenLeave),
            Self::AltScreenLeave => Some(Self::AltScreenEnter),
            Self::CursorHide => Some(Self::CursorShow),
            Self::CursorShow => Some(Self::CursorHide),
            Self::MouseButtonEnable => Some(Self::MouseButtonDisable),
            Self::MouseButtonDisable => Some(Self::MouseButtonEnable),
            Self::MouseMotionEnable => Some(Self::MouseMotionDisable),
            Self::MouseMotionDisable => Some(Self::MouseMotionEnable),
            Self::MouseAllEnable => Some(Self::MouseAllDisable),
            Self::MouseAllDisable => Some(Self::MouseAllEnable),
            Self::MouseSgrEnable => Some(Self::MouseSgrDisable),
            Self::MouseSgrDisable => Some(Self::MouseSgrEnable),
            Self::SyncOutputBegin => Some(Self::SyncOutputEnd),
            Self::SyncOutputEnd => Some(Self::SyncOutputBegin),
            Self::BracketedPasteEnable => Some(Self::BracketedPasteDisable),
            Self::BracketedPasteDisable => Some(Self::BracketedPasteEnable),
            Self::FocusEnable => Some(Self::FocusDisable),
            Self::FocusDisable => Some(Self::FocusEnable),
            _ => None,
        }
    }

    /// Whether this is an "opening" sequence that expects a close.
    pub const fn is_opener(self) -> bool {
        matches!(
            self,
            Self::AltScreenEnter
                | Self::CursorHide
                | Self::MouseButtonEnable
                | Self::MouseMotionEnable
                | Self::MouseAllEnable
                | Self::MouseSgrEnable
                | Self::SyncOutputBegin
                | Self::BracketedPasteEnable
                | Self::FocusEnable
        )
    }

    /// Human-readable name for the sequence.
    pub const fn display_name(self) -> &'static str {
        match self {
            Self::AltScreenEnter => "ALT_SCREEN_ENTER",
            Self::AltScreenLeave => "ALT_SCREEN_LEAVE",
            Self::CursorHide => "CURSOR_HIDE",
            Self::CursorShow => "CURSOR_SHOW",
            Self::CursorPosition => "CURSOR_POSITION",
            Self::MouseButtonEnable => "MOUSE_BUTTON_ENABLE",
            Self::MouseButtonDisable => "MOUSE_BUTTON_DISABLE",
            Self::MouseMotionEnable => "MOUSE_MOTION_ENABLE",
            Self::MouseMotionDisable => "MOUSE_MOTION_DISABLE",
            Self::MouseAllEnable => "MOUSE_ALL_ENABLE",
            Self::MouseAllDisable => "MOUSE_ALL_DISABLE",
            Self::MouseSgrEnable => "MOUSE_SGR_ENABLE",
            Self::MouseSgrDisable => "MOUSE_SGR_DISABLE",
            Self::SyncOutputBegin => "SYNC_OUTPUT_BEGIN",
            Self::SyncOutputEnd => "SYNC_OUTPUT_END",
            Self::BracketedPasteEnable => "BRACKETED_PASTE_ENABLE",
            Self::BracketedPasteDisable => "BRACKETED_PASTE_DISABLE",
            Self::FocusEnable => "FOCUS_ENABLE",
            Self::FocusDisable => "FOCUS_DISABLE",
            Self::SgrReset => "SGR_RESET",
            Self::SgrColor => "SGR_COLOR",
            Self::EraseDisplay => "ERASE_DISPLAY",
            Self::EraseLine => "ERASE_LINE",
            Self::Osc8Hyperlink => "OSC8_HYPERLINK",
            Self::SetTitle => "SET_TITLE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// A single detected sequence with its location.
#[derive(Clone, Debug, Serialize)]
pub struct DetectedSequence {
    /// The kind of sequence detected.
    pub kind: SequenceKind,
    /// Byte offset in the output where the sequence starts.
    pub offset: usize,
    /// Length of the sequence in bytes.
    pub length: usize,
    /// Raw bytes of the sequence (hex-encoded for JSON).
    pub raw_hex: String,
}

impl DetectedSequence {
    fn new(kind: SequenceKind, offset: usize, raw: &[u8]) -> Self {
        Self {
            kind,
            offset,
            length: raw.len(),
            raw_hex: raw.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "),
        }
    }
}

/// Sequence inventory with counts.
#[derive(Clone, Debug, Default, Serialize)]
pub struct SequenceInventory {
    /// Count of each sequence kind.
    pub counts: HashMap<SequenceKind, usize>,
    /// Total sequences detected.
    pub total: usize,
}

impl SequenceInventory {
    fn add(&mut self, kind: SequenceKind) {
        *self.counts.entry(kind).or_insert(0) += 1;
        self.total += 1;
    }

    /// Get count for a specific sequence kind.
    pub fn count(&self, kind: SequenceKind) -> usize {
        self.counts.get(&kind).copied().unwrap_or(0)
    }
}

/// Result of paired sequence validation.
#[derive(Clone, Debug, Serialize)]
pub struct PairValidation {
    /// Sequence pairs that are properly balanced.
    pub balanced: Vec<PairStatus>,
    /// Sequences that are unbalanced (missing pair).
    pub unbalanced: Vec<PairStatus>,
}

/// Status of a sequence pair.
#[derive(Clone, Debug, Serialize)]
pub struct PairStatus {
    /// The opener sequence kind.
    pub opener: SequenceKind,
    /// The closer sequence kind.
    pub closer: SequenceKind,
    /// Count of opener sequences.
    pub opener_count: usize,
    /// Count of closer sequences.
    pub closer_count: usize,
    /// Whether the pair is balanced.
    pub is_balanced: bool,
}

impl PairValidation {
    /// Check if all pairs are balanced.
    pub fn all_paired(&self) -> bool {
        self.unbalanced.is_empty()
    }

    /// Generate human-readable validation report.
    pub fn to_human_readable(&self) -> String {
        let mut output = String::new();
        writeln!(output, "=== Paired Sequence Validation ===").unwrap();

        if self.all_paired() {
            writeln!(output, "All sequences properly paired.").unwrap();
        } else {
            writeln!(output, "UNBALANCED PAIRS DETECTED:").unwrap();
        }

        for status in &self.unbalanced {
            writeln!(
                output,
                "  {} ({}) vs {} ({})",
                status.opener.display_name(),
                status.opener_count,
                status.closer.display_name(),
                status.closer_count
            ).unwrap();
        }

        if !self.balanced.is_empty() {
            writeln!(output, "\nBalanced pairs:").unwrap();
            for status in &self.balanced {
                writeln!(
                    output,
                    "  {} <-> {}: {} pairs",
                    status.opener.display_name(),
                    status.closer.display_name(),
                    status.opener_count
                ).unwrap();
            }
        }

        output
    }
}

/// Expected sequences for validation.
#[derive(Clone, Debug, Default)]
pub struct ExpectedSequences {
    /// Sequences that must be present.
    pub required: Vec<SequenceKind>,
    /// Sequences that must NOT be present.
    pub forbidden: Vec<SequenceKind>,
}

impl ExpectedSequences {
    /// Standard expected sequences for terminal setup.
    pub fn default_terminal_setup() -> Self {
        Self {
            required: vec![
                SequenceKind::AltScreenEnter,
                SequenceKind::AltScreenLeave,
                SequenceKind::CursorHide,
                SequenceKind::CursorShow,
            ],
            forbidden: Vec::new(),
        }
    }

    /// Expected sequences for a full demo run.
    pub fn demo_showcase_full() -> Self {
        Self {
            required: vec![
                SequenceKind::AltScreenEnter,
                SequenceKind::AltScreenLeave,
                SequenceKind::CursorHide,
                SequenceKind::CursorShow,
                SequenceKind::SyncOutputBegin,
                SequenceKind::SyncOutputEnd,
            ],
            forbidden: Vec::new(),
        }
    }

    /// Add a required sequence.
    pub fn require(mut self, kind: SequenceKind) -> Self {
        self.required.push(kind);
        self
    }

    /// Add a forbidden sequence.
    pub fn forbid(mut self, kind: SequenceKind) -> Self {
        self.forbidden.push(kind);
        self
    }
}

/// Complete sequence analysis report.
#[derive(Clone, Debug, Serialize)]
pub struct SequenceReport {
    /// Total output bytes analyzed.
    pub total_bytes: usize,
    /// Sequence inventory with counts.
    pub inventory: SequenceInventory,
    /// Timeline of detected sequences (in order).
    pub timeline: Vec<DetectedSequence>,
}

impl SequenceReport {
    /// Analyze PTY output and create a comprehensive report.
    pub fn from_output(output: &[u8]) -> Self {
        let mut inventory = SequenceInventory::default();
        let mut timeline = Vec::new();

        let mut i = 0;
        while i < output.len() {
            if output[i] == 0x1b && i + 1 < output.len() {
                if let Some((kind, length)) = Self::detect_sequence(&output[i..]) {
                    let raw = &output[i..i + length];
                    timeline.push(DetectedSequence::new(kind, i, raw));
                    inventory.add(kind);
                    i += length;
                    continue;
                }
            }
            i += 1;
        }

        Self {
            total_bytes: output.len(),
            inventory,
            timeline,
        }
    }

    /// Detect a sequence and return its kind and length.
    fn detect_sequence(data: &[u8]) -> Option<(SequenceKind, usize)> {
        if data.len() < 2 || data[0] != 0x1b {
            return None;
        }

        // Known fixed sequences (check longer patterns first).
        let patterns: &[(&[u8], SequenceKind)] = &[
            (b"\x1b[?1049h", SequenceKind::AltScreenEnter),
            (b"\x1b[?1049l", SequenceKind::AltScreenLeave),
            (b"\x1b[?25h", SequenceKind::CursorShow),
            (b"\x1b[?25l", SequenceKind::CursorHide),
            (b"\x1b[?1000h", SequenceKind::MouseButtonEnable),
            (b"\x1b[?1000l", SequenceKind::MouseButtonDisable),
            (b"\x1b[?1002h", SequenceKind::MouseMotionEnable),
            (b"\x1b[?1002l", SequenceKind::MouseMotionDisable),
            (b"\x1b[?1003h", SequenceKind::MouseAllEnable),
            (b"\x1b[?1003l", SequenceKind::MouseAllDisable),
            (b"\x1b[?1006h", SequenceKind::MouseSgrEnable),
            (b"\x1b[?1006l", SequenceKind::MouseSgrDisable),
            (b"\x1b[?2026h", SequenceKind::SyncOutputBegin),
            (b"\x1b[?2026l", SequenceKind::SyncOutputEnd),
            (b"\x1b[?2004h", SequenceKind::BracketedPasteEnable),
            (b"\x1b[?2004l", SequenceKind::BracketedPasteDisable),
            (b"\x1b[?1004h", SequenceKind::FocusEnable),
            (b"\x1b[?1004l", SequenceKind::FocusDisable),
            (b"\x1b[0m", SequenceKind::SgrReset),
            (b"\x1b[m", SequenceKind::SgrReset),
        ];

        for (pattern, kind) in patterns {
            if data.starts_with(pattern) {
                return Some((*kind, pattern.len()));
            }
        }

        // CSI sequences (ESC [ ... <final byte>).
        if data.len() >= 3 && data[1] == b'[' {
            // Find the final byte (0x40-0x7E).
            for (offset, &b) in data[2..].iter().enumerate() {
                if (0x40..=0x7E).contains(&b) {
                    let length = 3 + offset; // ESC + [ + params + final
                    let kind = match b {
                        b'H' | b'f' => SequenceKind::CursorPosition,
                        b'J' => SequenceKind::EraseDisplay,
                        b'K' => SequenceKind::EraseLine,
                        b'm' => {
                            // Check if it's a color/attribute sequence.
                            if data[2..2 + offset].contains(&b';') {
                                SequenceKind::SgrColor
                            } else {
                                SequenceKind::SgrReset
                            }
                        }
                        _ => SequenceKind::Unknown,
                    };
                    return Some((kind, length));
                }
            }
        }

        // OSC sequences (ESC ] ... BEL or ST).
        if data.len() >= 3 && data[1] == b']' {
            // Find terminator (BEL = 0x07 or ST = ESC \).
            for (offset, &b) in data[2..].iter().enumerate() {
                if b == 0x07 {
                    let length = 3 + offset;
                    let kind = if data.starts_with(b"\x1b]8;") {
                        SequenceKind::Osc8Hyperlink
                    } else if data.starts_with(b"\x1b]0;") || data.starts_with(b"\x1b]2;") {
                        SequenceKind::SetTitle
                    } else {
                        SequenceKind::Unknown
                    };
                    return Some((kind, length));
                }
                if b == 0x1b && offset + 3 < data.len() && data[offset + 3] == b'\\' {
                    let length = 4 + offset;
                    return Some((SequenceKind::Unknown, length));
                }
            }
        }

        None
    }

    /// Validate that paired sequences are balanced.
    pub fn validate_pairs(&self) -> PairValidation {
        let mut balanced = Vec::new();
        let mut unbalanced = Vec::new();

        // Define the pairs to check.
        let pairs: &[(SequenceKind, SequenceKind)] = &[
            (SequenceKind::AltScreenEnter, SequenceKind::AltScreenLeave),
            (SequenceKind::CursorHide, SequenceKind::CursorShow),
            (SequenceKind::MouseButtonEnable, SequenceKind::MouseButtonDisable),
            (SequenceKind::MouseMotionEnable, SequenceKind::MouseMotionDisable),
            (SequenceKind::MouseAllEnable, SequenceKind::MouseAllDisable),
            (SequenceKind::MouseSgrEnable, SequenceKind::MouseSgrDisable),
            (SequenceKind::SyncOutputBegin, SequenceKind::SyncOutputEnd),
            (SequenceKind::BracketedPasteEnable, SequenceKind::BracketedPasteDisable),
            (SequenceKind::FocusEnable, SequenceKind::FocusDisable),
        ];

        for &(opener, closer) in pairs {
            let opener_count = self.inventory.count(opener);
            let closer_count = self.inventory.count(closer);

            // Only report if at least one was found.
            if opener_count > 0 || closer_count > 0 {
                let is_balanced = opener_count == closer_count;
                let status = PairStatus {
                    opener,
                    closer,
                    opener_count,
                    closer_count,
                    is_balanced,
                };

                if is_balanced {
                    balanced.push(status);
                } else {
                    unbalanced.push(status);
                }
            }
        }

        PairValidation { balanced, unbalanced }
    }

    /// Find sequences that were expected but not found.
    pub fn find_missing(&self, expected: &ExpectedSequences) -> Vec<SequenceKind> {
        expected
            .required
            .iter()
            .filter(|&&kind| self.inventory.count(kind) == 0)
            .copied()
            .collect()
    }

    /// Find sequences that were forbidden but found.
    pub fn find_unexpected(&self, expected: &ExpectedSequences) -> Vec<SequenceKind> {
        expected
            .forbidden
            .iter()
            .filter(|&&kind| self.inventory.count(kind) > 0)
            .copied()
            .collect()
    }

    /// Generate human-readable report text.
    pub fn to_human_readable(&self) -> String {
        let mut output = String::new();

        writeln!(output, "=== ANSI Sequence Analysis Report ===").unwrap();
        writeln!(output, "Total bytes analyzed: {}", self.total_bytes).unwrap();
        writeln!(output, "Total sequences detected: {}", self.inventory.total).unwrap();
        writeln!(output).unwrap();

        // Sequence inventory.
        writeln!(output, "--- Sequence Inventory ---").unwrap();
        let mut counts: Vec<_> = self.inventory.counts.iter().collect();
        counts.sort_by(|a, b| b.1.cmp(a.1)); // Sort by count descending.
        for (kind, count) in counts {
            writeln!(output, "  {:30} : {}", kind.display_name(), count).unwrap();
        }
        writeln!(output).unwrap();

        // Timeline view (first N sequences).
        writeln!(output, "--- Timeline (first 20 sequences) ---").unwrap();
        for (i, seq) in self.timeline.iter().take(20).enumerate() {
            writeln!(
                output,
                "  {:3}. @{:6} {:30} [{}]",
                i + 1,
                seq.offset,
                seq.kind.display_name(),
                seq.raw_hex
            ).unwrap();
        }
        if self.timeline.len() > 20 {
            writeln!(output, "  ... and {} more", self.timeline.len() - 20).unwrap();
        }
        writeln!(output).unwrap();

        // Pair validation.
        let validation = self.validate_pairs();
        output.push_str(&validation.to_human_readable());

        output
    }

    /// Generate JSON report.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Generate compact JSON for CI.
    pub fn to_json_compact(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }
}

/// Validation result combining missing and unexpected sequences.
#[derive(Clone, Debug, Serialize)]
pub struct ExpectationResult {
    /// Sequences that were required but not found.
    pub missing: Vec<SequenceKind>,
    /// Sequences that were forbidden but found.
    pub unexpected: Vec<SequenceKind>,
    /// Overall pass/fail.
    pub passed: bool,
}

impl ExpectationResult {
    /// Create from a report and expectations.
    pub fn from_report(report: &SequenceReport, expected: &ExpectedSequences) -> Self {
        let missing = report.find_missing(expected);
        let unexpected = report.find_unexpected(expected);
        let passed = missing.is_empty() && unexpected.is_empty();
        Self { missing, unexpected, passed }
    }

    /// Generate human-readable result.
    pub fn to_human_readable(&self) -> String {
        let mut output = String::new();

        writeln!(output, "=== Expectation Check ===").unwrap();

        if self.passed {
            writeln!(output, "PASSED - All expectations met.").unwrap();
        } else {
            writeln!(output, "FAILED").unwrap();

            if !self.missing.is_empty() {
                writeln!(output, "Missing sequences:").unwrap();
                for kind in &self.missing {
                    writeln!(output, "  - {}", kind.display_name()).unwrap();
                }
            }

            if !self.unexpected.is_empty() {
                writeln!(output, "Unexpected sequences:").unwrap();
                for kind in &self.unexpected {
                    writeln!(output, "  - {}", kind.display_name()).unwrap();
                }
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_detection_alt_screen() {
        let output = b"\x1b[?1049hHello\x1b[?1049l";
        let report = SequenceReport::from_output(output);

        assert_eq!(report.inventory.count(SequenceKind::AltScreenEnter), 1);
        assert_eq!(report.inventory.count(SequenceKind::AltScreenLeave), 1);
        assert_eq!(report.timeline.len(), 2);
    }

    #[test]
    fn test_sequence_detection_cursor() {
        let output = b"\x1b[?25l\x1b[?25h";
        let report = SequenceReport::from_output(output);

        assert_eq!(report.inventory.count(SequenceKind::CursorHide), 1);
        assert_eq!(report.inventory.count(SequenceKind::CursorShow), 1);
    }

    #[test]
    fn test_pair_validation_balanced() {
        let output = b"\x1b[?1049h\x1b[?25l\x1b[?25h\x1b[?1049l";
        let report = SequenceReport::from_output(output);
        let validation = report.validate_pairs();

        assert!(validation.all_paired());
        assert!(validation.unbalanced.is_empty());
        assert_eq!(validation.balanced.len(), 2);
    }

    #[test]
    fn test_pair_validation_unbalanced() {
        let output = b"\x1b[?1049h\x1b[?25l"; // Missing closes.
        let report = SequenceReport::from_output(output);
        let validation = report.validate_pairs();

        assert!(!validation.all_paired());
        assert_eq!(validation.unbalanced.len(), 2);
    }

    #[test]
    fn test_timeline_ordering() {
        let output = b"\x1b[?1049h\x1b[2J\x1b[H\x1b[?1049l";
        let report = SequenceReport::from_output(output);

        assert!(report.timeline.len() >= 3);
        // Verify offsets are increasing.
        for window in report.timeline.windows(2) {
            assert!(window[0].offset < window[1].offset);
        }
    }

    #[test]
    fn test_missing_sequences() {
        let output = b"\x1b[?1049h\x1b[?1049l";
        let report = SequenceReport::from_output(output);
        let expected = ExpectedSequences::default_terminal_setup();
        let missing = report.find_missing(&expected);

        // Should be missing cursor hide/show.
        assert!(missing.contains(&SequenceKind::CursorHide));
        assert!(missing.contains(&SequenceKind::CursorShow));
    }

    #[test]
    fn test_expectation_result() {
        let output = b"\x1b[?1049h\x1b[?25l\x1b[?25h\x1b[?1049l";
        let report = SequenceReport::from_output(output);
        let expected = ExpectedSequences::default_terminal_setup();
        let result = ExpectationResult::from_report(&report, &expected);

        assert!(result.passed);
        assert!(result.missing.is_empty());
        assert!(result.unexpected.is_empty());
    }

    #[test]
    fn test_sync_output_detection() {
        let output = b"\x1b[?2026hHello\x1b[?2026l";
        let report = SequenceReport::from_output(output);

        assert_eq!(report.inventory.count(SequenceKind::SyncOutputBegin), 1);
        assert_eq!(report.inventory.count(SequenceKind::SyncOutputEnd), 1);
    }

    #[test]
    fn test_osc8_hyperlink_detection() {
        let output = b"\x1b]8;;https://example.com\x07Link\x1b]8;;\x07";
        let report = SequenceReport::from_output(output);

        assert_eq!(report.inventory.count(SequenceKind::Osc8Hyperlink), 2);
    }

    #[test]
    fn test_human_readable_output() {
        let output = b"\x1b[?1049h\x1b[?25l\x1b[2J\x1b[?25h\x1b[?1049l";
        let report = SequenceReport::from_output(output);
        let readable = report.to_human_readable();

        assert!(readable.contains("Sequence Inventory"));
        assert!(readable.contains("Timeline"));
        assert!(readable.contains("ALT_SCREEN_ENTER"));
    }

    #[test]
    fn test_json_output() {
        let output = b"\x1b[?1049h\x1b[?1049l";
        let report = SequenceReport::from_output(output);
        let json = report.to_json().unwrap();

        assert!(json.contains("total_bytes"));
        assert!(json.contains("inventory"));
        assert!(json.contains("timeline"));
    }

    #[test]
    fn test_sequence_kind_display_names() {
        assert_eq!(SequenceKind::AltScreenEnter.display_name(), "ALT_SCREEN_ENTER");
        assert_eq!(SequenceKind::CursorHide.display_name(), "CURSOR_HIDE");
        assert_eq!(SequenceKind::SyncOutputBegin.display_name(), "SYNC_OUTPUT_BEGIN");
    }

    #[test]
    fn test_sequence_pair_relationships() {
        assert_eq!(SequenceKind::AltScreenEnter.pair(), Some(SequenceKind::AltScreenLeave));
        assert_eq!(SequenceKind::CursorHide.pair(), Some(SequenceKind::CursorShow));
        assert_eq!(SequenceKind::EraseDisplay.pair(), None);
    }
}
