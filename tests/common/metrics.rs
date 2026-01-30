//! Timing and performance metrics capture for E2E tests.
//!
//! This module provides structured timing metrics collection and reporting
//! for E2E tests, particularly for PTY-based testing of terminal applications.
//!
//! # Metrics Captured
//!
//! - **Startup time** - from spawn to first frame output
//! - **Frame render time** - per-frame duration estimates
//! - **Tour step transitions** - time between tour mode steps

#![allow(clippy::uninlined_format_args)] // Clarity over style in test code
//! - **Total runtime** - spawn to exit
//! - **Memory high-water mark** - peak memory usage (best effort)
//!
//! # Usage
//!
//! ```ignore
//! use common::metrics::{TimingMetrics, MetricThresholds};
//!
//! let result = spawn_pty(&config)?;
//! let metrics = TimingMetrics::from_pty_result(&result);
//!
//! // Check thresholds
//! let thresholds = MetricThresholds::default();
//! metrics.assert_within_thresholds(&thresholds)?;
//!
//! // Report as JSON
//! let report = metrics.to_json()?;
//! ```

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Timing metrics captured from a PTY test run.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimingMetrics {
    /// Total runtime from spawn to exit.
    pub total_runtime: Duration,

    /// Estimated startup time (time to first output).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub startup_time: Option<Duration>,

    /// First frame render time (time from spawn to first complete frame).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_frame_time: Option<Duration>,

    /// Individual tour step durations (for tour mode tests).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tour_step_durations: Vec<TourStepTiming>,

    /// Memory high-water mark in bytes (best effort, may not be available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_high_water_bytes: Option<u64>,

    /// Number of frames detected (based on sync output patterns).
    pub frame_count: u32,

    /// Average frame render time (estimated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_frame_time: Option<Duration>,

    /// Output bytes per second throughput.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_throughput_bps: Option<f64>,
}

impl TimingMetrics {
    /// Create empty metrics.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create metrics from a PTY result with basic timing.
    pub fn from_duration(total: Duration) -> Self {
        Self {
            total_runtime: total,
            ..Default::default()
        }
    }

    /// Estimate startup time by finding first ANSI output in the buffer.
    ///
    /// Looks for the first escape sequence, which indicates the application
    /// has started producing terminal output.
    pub fn estimate_startup_from_output(&mut self, output: &[u8], total: Duration) {
        // Look for first ESC character
        if let Some(first_esc_pos) = output.iter().position(|&b| b == 0x1b) {
            // Estimate startup as fraction of total time based on position
            // This is rough but gives us something to work with
            let fraction = first_esc_pos as f64 / output.len().max(1) as f64;
            // Clamp to reasonable range (startup is usually < 50% of total time)
            let fraction = fraction.min(0.5);
            self.startup_time = Some(Duration::from_secs_f64(
                total.as_secs_f64() * fraction * 0.5,
            ));
        }
    }

    /// Count frames by looking for synchronized output patterns.
    ///
    /// Uses the DECSET/DECRST 2026 (sync output) sequences as frame markers.
    pub fn count_frames_from_output(&mut self, output: &[u8]) {
        // Count SYNC_BEGIN sequences: ESC [ ? 2026 h
        let sync_begin = b"\x1b[?2026h";
        self.frame_count = output
            .windows(sync_begin.len())
            .filter(|w| *w == sync_begin)
            .count() as u32;

        // Estimate average frame time if we have frames
        if self.frame_count > 0 {
            let frame_duration = self.total_runtime.as_secs_f64() / f64::from(self.frame_count);
            self.avg_frame_time = Some(Duration::from_secs_f64(frame_duration));
        }
    }

    /// Calculate output throughput.
    pub fn calculate_throughput(&mut self, output_bytes: usize) {
        let secs = self.total_runtime.as_secs_f64();
        if secs > 0.0 {
            self.output_throughput_bps = Some(output_bytes as f64 / secs);
        }
    }

    /// Analyze output to extract tour step timings.
    ///
    /// Looks for tour step indicators in the output and estimates timing.
    pub fn extract_tour_steps(&mut self, output: &[u8]) {
        // Look for tour step patterns like "Step N:" or similar indicators
        // This is heuristic-based since we're parsing raw ANSI output
        let mut step_count = 0;
        let mut last_step_pos = 0;

        // Simple pattern: look for "Step" followed by a digit
        for (i, window) in output.windows(5).enumerate() {
            if window == b"Step " {
                if step_count > 0 && i > last_step_pos {
                    // Calculate fraction of output for this step
                    let step_fraction = (i - last_step_pos) as f64 / output.len().max(1) as f64;
                    let step_duration =
                        Duration::from_secs_f64(self.total_runtime.as_secs_f64() * step_fraction);
                    self.tour_step_durations.push(TourStepTiming {
                        step_number: step_count,
                        duration: step_duration,
                        output_bytes: i - last_step_pos,
                    });
                }
                step_count += 1;
                last_step_pos = i;
            }
        }

        // Add final step if we found any
        if step_count > 0 && last_step_pos < output.len() {
            let remaining_bytes = output.len() - last_step_pos;
            let step_fraction = remaining_bytes as f64 / output.len().max(1) as f64;
            let step_duration =
                Duration::from_secs_f64(self.total_runtime.as_secs_f64() * step_fraction);
            self.tour_step_durations.push(TourStepTiming {
                step_number: step_count,
                duration: step_duration,
                output_bytes: remaining_bytes,
            });
        }
    }

    /// Serialize metrics to JSON string.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Check if metrics are within specified thresholds.
    pub fn check_thresholds(&self, thresholds: &MetricThresholds) -> ThresholdCheckResult {
        let mut violations = Vec::new();

        if let Some(max_total) = thresholds.max_total_runtime {
            if self.total_runtime > max_total {
                violations.push(ThresholdViolation {
                    metric: "total_runtime".to_string(),
                    expected: format!("<= {:?}", max_total),
                    actual: format!("{:?}", self.total_runtime),
                });
            }
        }

        if let Some(max_startup) = thresholds.max_startup_time {
            if let Some(startup) = self.startup_time {
                if startup > max_startup {
                    violations.push(ThresholdViolation {
                        metric: "startup_time".to_string(),
                        expected: format!("<= {:?}", max_startup),
                        actual: format!("{:?}", startup),
                    });
                }
            }
        }

        if let Some(max_frame) = thresholds.max_avg_frame_time {
            if let Some(avg_frame) = self.avg_frame_time {
                if avg_frame > max_frame {
                    violations.push(ThresholdViolation {
                        metric: "avg_frame_time".to_string(),
                        expected: format!("<= {:?}", max_frame),
                        actual: format!("{:?}", avg_frame),
                    });
                }
            }
        }

        if let Some(min_fps) = thresholds.min_frame_rate {
            if let Some(avg_frame) = self.avg_frame_time {
                let actual_fps = 1.0 / avg_frame.as_secs_f64();
                if actual_fps < min_fps {
                    violations.push(ThresholdViolation {
                        metric: "frame_rate".to_string(),
                        expected: format!(">= {:.1} fps", min_fps),
                        actual: format!("{:.1} fps", actual_fps),
                    });
                }
            }
        }

        if let Some(min_throughput) = thresholds.min_throughput_bps {
            if let Some(throughput) = self.output_throughput_bps {
                if throughput < min_throughput {
                    violations.push(ThresholdViolation {
                        metric: "output_throughput".to_string(),
                        expected: format!(">= {:.0} B/s", min_throughput),
                        actual: format!("{:.0} B/s", throughput),
                    });
                }
            }
        }

        ThresholdCheckResult { violations }
    }

    /// Assert that metrics are within thresholds, panicking on violation.
    pub fn assert_within_thresholds(&self, thresholds: &MetricThresholds) {
        let result = self.check_thresholds(thresholds);
        assert!(
            result.is_ok(),
            "Timing metrics exceeded thresholds:\n{}",
            result.violations_summary()
        );
    }
}

/// Timing data for a single tour step.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TourStepTiming {
    /// Step number (1-indexed).
    pub step_number: u32,
    /// Estimated duration of this step.
    pub duration: Duration,
    /// Output bytes during this step.
    pub output_bytes: usize,
}

/// Thresholds for performance metric assertions.
#[derive(Clone, Debug, Default)]
pub struct MetricThresholds {
    /// Maximum allowed total runtime.
    pub max_total_runtime: Option<Duration>,
    /// Maximum allowed startup time.
    pub max_startup_time: Option<Duration>,
    /// Maximum allowed average frame time.
    pub max_avg_frame_time: Option<Duration>,
    /// Minimum required frame rate (fps).
    pub min_frame_rate: Option<f64>,
    /// Minimum required output throughput (bytes/second).
    pub min_throughput_bps: Option<f64>,
}

impl MetricThresholds {
    /// Create default thresholds for tour mode testing.
    pub fn tour_mode_defaults() -> Self {
        Self {
            max_total_runtime: Some(Duration::from_secs(30)),
            max_startup_time: Some(Duration::from_secs(2)),
            max_avg_frame_time: Some(Duration::from_millis(50)), // 20 fps minimum
            min_frame_rate: Some(15.0),
            min_throughput_bps: Some(10_000.0), // 10 KB/s minimum
        }
    }

    /// Create permissive thresholds for CI environments.
    pub fn ci_defaults() -> Self {
        Self {
            max_total_runtime: Some(Duration::from_secs(60)),
            max_startup_time: Some(Duration::from_secs(5)),
            max_avg_frame_time: Some(Duration::from_millis(100)), // 10 fps minimum
            min_frame_rate: Some(5.0),
            min_throughput_bps: Some(1_000.0), // 1 KB/s minimum
        }
    }

    /// Create strict thresholds for performance regression testing.
    pub fn strict() -> Self {
        Self {
            max_total_runtime: Some(Duration::from_secs(15)),
            max_startup_time: Some(Duration::from_millis(500)),
            max_avg_frame_time: Some(Duration::from_millis(20)), // 50 fps minimum
            min_frame_rate: Some(30.0),
            min_throughput_bps: Some(50_000.0), // 50 KB/s minimum
        }
    }
}

/// A single threshold violation.
#[derive(Clone, Debug)]
pub struct ThresholdViolation {
    /// Name of the metric that was violated.
    pub metric: String,
    /// Expected value description.
    pub expected: String,
    /// Actual value.
    pub actual: String,
}

/// Result of checking metrics against thresholds.
#[derive(Clone, Debug)]
pub struct ThresholdCheckResult {
    /// List of threshold violations.
    pub violations: Vec<ThresholdViolation>,
}

impl ThresholdCheckResult {
    /// Returns true if all thresholds were met.
    pub fn is_ok(&self) -> bool {
        self.violations.is_empty()
    }

    /// Generate a human-readable summary of violations.
    pub fn violations_summary(&self) -> String {
        if self.violations.is_empty() {
            return "All thresholds met.".to_string();
        }

        self.violations
            .iter()
            .map(|v| {
                format!(
                    "  - {}: expected {}, got {}",
                    v.metric, v.expected, v.actual
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Builder for creating metric thresholds incrementally.
#[derive(Clone, Debug, Default)]
pub struct MetricThresholdsBuilder {
    thresholds: MetricThresholds,
}

impl MetricThresholdsBuilder {
    /// Create a new builder with no thresholds set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum total runtime.
    pub fn max_total_runtime(mut self, duration: Duration) -> Self {
        self.thresholds.max_total_runtime = Some(duration);
        self
    }

    /// Set maximum startup time.
    pub fn max_startup_time(mut self, duration: Duration) -> Self {
        self.thresholds.max_startup_time = Some(duration);
        self
    }

    /// Set maximum average frame time.
    pub fn max_avg_frame_time(mut self, duration: Duration) -> Self {
        self.thresholds.max_avg_frame_time = Some(duration);
        self
    }

    /// Set minimum frame rate.
    pub fn min_frame_rate(mut self, fps: f64) -> Self {
        self.thresholds.min_frame_rate = Some(fps);
        self
    }

    /// Set minimum throughput.
    pub fn min_throughput_bps(mut self, bps: f64) -> Self {
        self.thresholds.min_throughput_bps = Some(bps);
        self
    }

    /// Build the thresholds.
    pub fn build(self) -> MetricThresholds {
        self.thresholds
    }
}

/// Report timing metrics to the structured logging system.
pub fn log_timing_metrics(metrics: &TimingMetrics, logger: &mut super::harness::ExtendedLogger) {
    logger.metric("total_runtime_ms", metrics.total_runtime.as_millis() as u64);

    if let Some(startup) = metrics.startup_time {
        logger.metric("startup_time_ms", startup.as_millis() as u64);
    }

    if let Some(first_frame) = metrics.first_frame_time {
        logger.metric("first_frame_time_ms", first_frame.as_millis() as u64);
    }

    logger.metric("frame_count", u64::from(metrics.frame_count));

    if let Some(avg_frame) = metrics.avg_frame_time {
        logger.metric("avg_frame_time_ms", avg_frame.as_millis() as u64);
        let fps = 1.0 / avg_frame.as_secs_f64();
        logger.metric("estimated_fps", fps as u64);
    }

    if let Some(throughput) = metrics.output_throughput_bps {
        logger.metric("output_throughput_bps", throughput as u64);
    }

    for step in &metrics.tour_step_durations {
        logger.metric(
            &format!("tour_step_{}_ms", step.step_number),
            step.duration.as_millis() as u64,
        );
    }
}

/// Summary report combining multiple test run metrics.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MetricsSummary {
    /// Number of test runs included.
    pub run_count: u32,
    /// Average total runtime across runs.
    pub avg_total_runtime: Duration,
    /// Minimum total runtime.
    pub min_total_runtime: Duration,
    /// Maximum total runtime.
    pub max_total_runtime: Duration,
    /// Average frame count.
    pub avg_frame_count: f64,
    /// Average throughput.
    pub avg_throughput_bps: Option<f64>,
}

impl MetricsSummary {
    /// Create a summary from multiple timing metrics.
    pub fn from_runs(runs: &[TimingMetrics]) -> Self {
        if runs.is_empty() {
            return Self::default();
        }

        let run_count = runs.len() as u32;
        let total_runtime_sum: Duration = runs.iter().map(|r| r.total_runtime).sum();
        let avg_total_runtime = total_runtime_sum / run_count;

        let min_total_runtime = runs
            .iter()
            .map(|r| r.total_runtime)
            .min()
            .unwrap_or_default();
        let max_total_runtime = runs
            .iter()
            .map(|r| r.total_runtime)
            .max()
            .unwrap_or_default();

        let avg_frame_count =
            runs.iter().map(|r| r.frame_count).sum::<u32>() as f64 / runs.len() as f64;

        let throughputs: Vec<f64> = runs
            .iter()
            .filter_map(|r| r.output_throughput_bps)
            .collect();
        let avg_throughput_bps = if throughputs.is_empty() {
            None
        } else {
            Some(throughputs.iter().sum::<f64>() / throughputs.len() as f64)
        };

        Self {
            run_count,
            avg_total_runtime,
            min_total_runtime,
            max_total_runtime,
            avg_frame_count,
            avg_throughput_bps,
        }
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_metrics_basic() {
        let metrics = TimingMetrics::from_duration(Duration::from_secs(5));
        assert_eq!(metrics.total_runtime, Duration::from_secs(5));
        assert!(metrics.startup_time.is_none());
    }

    #[test]
    fn test_frame_counting() {
        let mut metrics = TimingMetrics::from_duration(Duration::from_secs(1));

        // Simulate output with 3 sync begin sequences
        let output = b"\x1b[?2026hframe1\x1b[?2026ltext\x1b[?2026hframe2\x1b[?2026lmore\x1b[?2026hframe3\x1b[?2026l";
        metrics.count_frames_from_output(output);

        assert_eq!(metrics.frame_count, 3);
        assert!(metrics.avg_frame_time.is_some());
    }

    #[test]
    fn test_throughput_calculation() {
        let mut metrics = TimingMetrics::from_duration(Duration::from_secs(2));
        metrics.calculate_throughput(10_000);

        assert!(metrics.output_throughput_bps.is_some());
        let throughput = metrics.output_throughput_bps.unwrap();
        assert!((throughput - 5000.0).abs() < 0.1);
    }

    #[test]
    fn test_threshold_check_pass() {
        let metrics = TimingMetrics {
            total_runtime: Duration::from_secs(5),
            startup_time: Some(Duration::from_millis(500)),
            avg_frame_time: Some(Duration::from_millis(20)),
            ..Default::default()
        };

        let thresholds = MetricThresholds {
            max_total_runtime: Some(Duration::from_secs(10)),
            max_startup_time: Some(Duration::from_secs(1)),
            max_avg_frame_time: Some(Duration::from_millis(50)),
            ..Default::default()
        };

        let result = metrics.check_thresholds(&thresholds);
        assert!(result.is_ok());
    }

    #[test]
    fn test_threshold_check_fail() {
        let metrics = TimingMetrics {
            total_runtime: Duration::from_secs(20),
            ..Default::default()
        };

        let thresholds = MetricThresholds {
            max_total_runtime: Some(Duration::from_secs(10)),
            ..Default::default()
        };

        let result = metrics.check_thresholds(&thresholds);
        assert!(!result.is_ok());
        assert_eq!(result.violations.len(), 1);
        assert_eq!(result.violations[0].metric, "total_runtime");
    }

    #[test]
    fn test_threshold_builder() {
        let thresholds = MetricThresholdsBuilder::new()
            .max_total_runtime(Duration::from_secs(30))
            .min_frame_rate(10.0)
            .build();

        assert_eq!(thresholds.max_total_runtime, Some(Duration::from_secs(30)));
        assert_eq!(thresholds.min_frame_rate, Some(10.0));
        assert!(thresholds.max_startup_time.is_none());
    }

    #[test]
    fn test_metrics_summary() {
        let runs = vec![
            TimingMetrics {
                total_runtime: Duration::from_secs(5),
                frame_count: 100,
                output_throughput_bps: Some(10_000.0),
                ..Default::default()
            },
            TimingMetrics {
                total_runtime: Duration::from_secs(7),
                frame_count: 120,
                output_throughput_bps: Some(12_000.0),
                ..Default::default()
            },
        ];

        let summary = MetricsSummary::from_runs(&runs);

        assert_eq!(summary.run_count, 2);
        assert_eq!(summary.min_total_runtime, Duration::from_secs(5));
        assert_eq!(summary.max_total_runtime, Duration::from_secs(7));
        assert!((summary.avg_frame_count - 110.0).abs() < 0.1);
    }

    #[test]
    fn test_json_serialization() {
        let metrics = TimingMetrics {
            total_runtime: Duration::from_secs(5),
            frame_count: 100,
            ..Default::default()
        };

        let json = metrics.to_json().unwrap();
        assert!(json.contains("\"total_runtime\""));
        // Pretty JSON has spaces after colons
        assert!(json.contains("\"frame_count\": 100"));
    }

    #[test]
    fn test_preset_thresholds() {
        let tour = MetricThresholds::tour_mode_defaults();
        assert!(tour.max_total_runtime.is_some());
        assert!(tour.min_frame_rate.is_some());

        let ci = MetricThresholds::ci_defaults();
        assert!(ci.max_total_runtime.unwrap() > tour.max_total_runtime.unwrap());

        let strict = MetricThresholds::strict();
        assert!(strict.max_total_runtime.unwrap() < tour.max_total_runtime.unwrap());
    }
}
