//! PTY (pseudo-terminal) harness for E2E testing.
//!
//! Spawns `demo_showcase` under a real pseudo-terminal to capture actual
//! ANSI output sequences emitted during terminal I/O.

// PTY operations require unsafe libc FFI calls
#![allow(
    dead_code,
    unsafe_code,
    clippy::format_push_string,
    clippy::uninlined_format_args
)]

use std::collections::HashMap;
use std::ffi::CString;
use std::io::{self, Read};
use std::os::fd::FromRawFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// Environment variables for deterministic terminal behavior.
pub const DEFAULT_ENV: &[(&str, &str)] = &[
    ("TERM", "xterm-kitty"),
    ("TERM_PROGRAM", "kitty"),
    ("COLORTERM", "truecolor"),
    ("LANG", "C.UTF-8"),
    ("LC_ALL", "C.UTF-8"),
    // Disable any user config that could affect behavior
    ("NO_COLOR", ""),
    ("FORCE_COLOR", ""),
];

/// Result of running a command under PTY.
#[derive(Clone, Debug)]
pub struct PtyResult {
    /// Exit status code (None if process didn't exit cleanly).
    pub exit_code: Option<i32>,
    /// Captured stdout/stderr via PTY.
    pub output: Vec<u8>,
    /// Total runtime.
    pub duration: Duration,
    /// Environment variables used.
    pub env: HashMap<String, String>,
    /// Command that was run.
    pub command: Vec<String>,
    /// Timing metrics computed from the run.
    pub timing: super::metrics::TimingMetrics,
}

impl PtyResult {
    /// Get the timing metrics for this run.
    pub fn metrics(&self) -> &super::metrics::TimingMetrics {
        &self.timing
    }

    /// Check timing metrics against thresholds.
    ///
    /// Returns a result indicating whether all thresholds were met.
    pub fn check_timing_thresholds(
        &self,
        thresholds: &super::metrics::MetricThresholds,
    ) -> super::metrics::ThresholdCheckResult {
        self.timing.check_thresholds(thresholds)
    }

    /// Assert that timing metrics are within thresholds.
    ///
    /// Panics with a detailed message if any threshold is exceeded.
    pub fn assert_timing_within_thresholds(&self, thresholds: &super::metrics::MetricThresholds) {
        self.timing.assert_within_thresholds(thresholds);
    }

    /// Generate a JSON report of timing metrics.
    pub fn timing_report_json(&self) -> Result<String, serde_json::Error> {
        self.timing.to_json()
    }

    /// Generate a comprehensive sequence analysis report.
    ///
    /// This provides detailed analysis including:
    /// - Sequence inventory with counts
    /// - Timeline of sequences with byte offsets
    /// - Paired sequence validation
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = spawn_pty(&config)?;
    /// let report = result.sequence_report();
    ///
    /// // Validate paired sequences.
    /// let validation = report.validate_pairs();
    /// assert!(validation.all_paired(), "{}", validation.to_human_readable());
    ///
    /// // Check expectations.
    /// let expected = ExpectedSequences::default_terminal_setup();
    /// let missing = report.find_missing(&expected);
    /// assert!(missing.is_empty());
    /// ```
    pub fn sequence_report(&self) -> super::analysis::SequenceReport {
        super::analysis::SequenceReport::from_output(&self.output)
    }

    /// Check if output contains a byte sequence.
    pub fn contains_sequence(&self, seq: &[u8]) -> bool {
        self.output.windows(seq.len()).any(|window| window == seq)
    }

    /// Check if output contains an ANSI CSI sequence like `ESC [ ? <n> h`.
    pub fn contains_csi_private_set(&self, n: u16) -> bool {
        let seq = format!("\x1b[?{n}h");
        self.contains_sequence(seq.as_bytes())
    }

    /// Check if output contains an ANSI CSI sequence like `ESC [ ? <n> l`.
    pub fn contains_csi_private_reset(&self, n: u16) -> bool {
        let seq = format!("\x1b[?{n}l");
        self.contains_sequence(seq.as_bytes())
    }

    /// Check if output contains OSC 8 hyperlink start.
    pub fn contains_osc8_hyperlink(&self) -> bool {
        // OSC 8 format: ESC ] 8 ; params ; url BEL (or ST)
        // We check for the minimal prefix
        self.contains_sequence(b"\x1b]8;")
    }

    /// Count occurrences of a sequence.
    pub fn count_sequence(&self, seq: &[u8]) -> usize {
        if seq.is_empty() {
            return 0;
        }
        self.output
            .windows(seq.len())
            .filter(|window| *window == seq)
            .count()
    }

    /// Convert output to readable format for debugging.
    pub fn output_readable(&self) -> String {
        use std::fmt::Write;
        let mut result = String::new();
        for &b in &self.output {
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

    /// Get hex dump of output.
    pub fn output_hex(&self) -> String {
        self.output
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Configuration for PTY spawn.
#[derive(Clone, Debug)]
pub struct PtyConfig {
    /// Path to the binary to run.
    pub binary: PathBuf,
    /// Arguments to pass.
    pub args: Vec<String>,
    /// Environment overrides (added to `DEFAULT_ENV`).
    pub env_overrides: HashMap<String, String>,
    /// Maximum time to wait for process to exit.
    pub timeout: Duration,
    /// Terminal size (columns, rows).
    pub size: (u16, u16),
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            binary: PathBuf::from("target/debug/demo_showcase"),
            args: Vec::new(),
            env_overrides: HashMap::new(),
            timeout: Duration::from_secs(60),
            size: (80, 24),
        }
    }
}

impl PtyConfig {
    /// Create config for `demo_showcase` with tour mode.
    pub fn demo_showcase_tour() -> Self {
        Self {
            binary: PathBuf::from("target/debug/demo_showcase"),
            args: vec![
                "--tour".to_string(),
                "--exit-after-tour".to_string(),
                "--max-frames".to_string(),
                "600".to_string(),
                "--fps".to_string(),
                "30".to_string(),
            ],
            timeout: Duration::from_secs(30),
            ..Default::default()
        }
    }

    /// Add an argument.
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Set environment variable.
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env_overrides.insert(key.into(), value.into());
        self
    }

    /// Set timeout.
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set terminal size.
    pub const fn size(mut self, cols: u16, rows: u16) -> Self {
        self.size = (cols, rows);
        self
    }
}

/// Spawn a process under a PTY and capture output.
///
/// # Safety
///
/// Uses libc fork/exec which is inherently unsafe. This function should only
/// be called in test code where the child process is trusted.
///
/// # Errors
///
/// Returns an error if PTY creation, fork, or exec fails.
#[cfg(unix)]
#[allow(clippy::too_many_lines)]
pub fn spawn_pty(config: &PtyConfig) -> io::Result<PtyResult> {
    let start = Instant::now();

    // Build environment
    let mut env: HashMap<String, String> = DEFAULT_ENV
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    for (k, v) in &config.env_overrides {
        env.insert(k.clone(), v.clone());
    }

    // Build command for logging
    let command: Vec<String> = std::iter::once(config.binary.to_string_lossy().into_owned())
        .chain(config.args.iter().cloned())
        .collect();

    // Open PTY
    let mut master_fd: libc::c_int = 0;
    let mut slave_fd: libc::c_int = 0;

    // SAFETY: openpty is a standard POSIX function
    let ret = unsafe {
        libc::openpty(
            std::ptr::from_mut(&mut master_fd),
            std::ptr::from_mut(&mut slave_fd),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ret != 0 {
        return Err(io::Error::last_os_error());
    }

    // Set terminal size
    let winsize = libc::winsize {
        ws_row: config.size.1,
        ws_col: config.size.0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: ioctl with TIOCSWINSZ is safe on a valid fd
    unsafe {
        libc::ioctl(slave_fd, libc::TIOCSWINSZ, &winsize);
    }

    // Fork
    // SAFETY: fork is a standard POSIX function
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }

    if pid == 0 {
        // Child process
        // SAFETY: These are standard POSIX operations in a forked child
        unsafe {
            // Create new session
            libc::setsid();

            // Set controlling terminal
            libc::ioctl(slave_fd, libc::TIOCSCTTY, 0);

            // Redirect stdin/stdout/stderr to slave
            libc::dup2(slave_fd, 0);
            libc::dup2(slave_fd, 1);
            libc::dup2(slave_fd, 2);

            // Close original fds
            if slave_fd > 2 {
                libc::close(slave_fd);
            }
            libc::close(master_fd);

            // Set environment
            for (key, value) in &env {
                let key_c = CString::new(key.as_str()).unwrap();
                let value_c = CString::new(value.as_str()).unwrap();
                libc::setenv(key_c.as_ptr(), value_c.as_ptr(), 1);
            }

            // Exec
            let binary_c = CString::new(config.binary.to_string_lossy().as_ref()).unwrap();
            let mut args_c: Vec<CString> = vec![binary_c.clone()];
            for arg in &config.args {
                args_c.push(CString::new(arg.as_str()).unwrap());
            }
            let args_ptrs: Vec<*const libc::c_char> = args_c
                .iter()
                .map(|s| s.as_ptr())
                .chain(std::iter::once(std::ptr::null()))
                .collect();

            libc::execvp(binary_c.as_ptr(), args_ptrs.as_ptr());

            // If exec fails, exit
            libc::_exit(127);
        }
    }

    // Parent process
    // SAFETY: close is safe on a valid fd
    unsafe {
        libc::close(slave_fd);
    }

    // Set master to non-blocking for timeout handling
    // SAFETY: fcntl is safe on a valid fd
    unsafe {
        let flags = libc::fcntl(master_fd, libc::F_GETFL);
        libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    // Read output with timeout
    let mut output = Vec::new();
    let mut buf = [0u8; 4096];
    let deadline = Instant::now() + config.timeout;

    // SAFETY: File::from_raw_fd takes ownership of the fd
    let mut master = unsafe { std::fs::File::from_raw_fd(master_fd) };

    loop {
        if Instant::now() > deadline {
            // Timeout - kill the child
            // SAFETY: kill is safe with a valid pid
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            break;
        }

        // Check if child has exited
        let mut status: libc::c_int = 0;
        // SAFETY: waitpid with WNOHANG is safe
        let wait_result =
            unsafe { libc::waitpid(pid, std::ptr::from_mut(&mut status), libc::WNOHANG) };
        if wait_result == pid {
            // Child exited - drain remaining output
            while let Ok(n) = master.read(&mut buf) {
                if n == 0 {
                    break;
                }
                output.extend_from_slice(&buf[..n]);
            }
            let exit_code = if libc::WIFEXITED(status) {
                Some(libc::WEXITSTATUS(status))
            } else {
                None
            };
            let duration = start.elapsed();
            let timing = compute_timing_metrics(&output, duration);
            return Ok(PtyResult {
                exit_code,
                output,
                duration,
                env,
                command,
                timing,
            });
        }

        // Try to read
        match master.read(&mut buf) {
            Ok(0) => {
                // EOF
                std::thread::sleep(Duration::from_millis(10));
            }
            Ok(n) => {
                output.extend_from_slice(&buf[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(e) => {
                // Other error - might be normal on process exit
                if e.raw_os_error() == Some(libc::EIO) {
                    // EIO is expected when slave closes
                    std::thread::sleep(Duration::from_millis(50));
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Timeout case - wait for child
    let mut status: libc::c_int = 0;
    // SAFETY: waitpid is safe
    unsafe {
        libc::waitpid(pid, std::ptr::from_mut(&mut status), 0);
    }

    let duration = start.elapsed();
    let timing = compute_timing_metrics(&output, duration);
    Ok(PtyResult {
        exit_code: None,
        output,
        duration,
        env,
        command,
        timing,
    })
}

/// Compute timing metrics from PTY output.
fn compute_timing_metrics(
    output: &[u8],
    total_duration: Duration,
) -> super::metrics::TimingMetrics {
    let mut metrics = super::metrics::TimingMetrics::from_duration(total_duration);
    metrics.estimate_startup_from_output(output, total_duration);
    metrics.count_frames_from_output(output);
    metrics.calculate_throughput(output.len());
    metrics.extract_tour_steps(output);
    metrics
}

/// ANSI sequence constants for assertions.
pub mod sequences {
    /// Enter alternate screen buffer (DECSET).
    pub const ALT_SCREEN_ENTER: &[u8] = b"\x1b[?1049h";
    /// Leave alternate screen buffer (DECRST).
    pub const ALT_SCREEN_LEAVE: &[u8] = b"\x1b[?1049l";
    /// Hide cursor (DECTCEM reset).
    pub const CURSOR_HIDE: &[u8] = b"\x1b[?25l";
    /// Show cursor (DECTCEM set).
    pub const CURSOR_SHOW: &[u8] = b"\x1b[?25h";
    /// Enable mouse tracking (X10).
    pub const MOUSE_X10_ENABLE: &[u8] = b"\x1b[?9h";
    /// Enable mouse button tracking.
    pub const MOUSE_BUTTON_ENABLE: &[u8] = b"\x1b[?1000h";
    /// Enable mouse motion tracking.
    pub const MOUSE_MOTION_ENABLE: &[u8] = b"\x1b[?1002h";
    /// Enable mouse all motion tracking.
    pub const MOUSE_ALL_ENABLE: &[u8] = b"\x1b[?1003h";
    /// Enable SGR mouse format.
    pub const MOUSE_SGR_ENABLE: &[u8] = b"\x1b[?1006h";
    /// Disable mouse button tracking.
    pub const MOUSE_BUTTON_DISABLE: &[u8] = b"\x1b[?1000l";
    /// Disable mouse motion tracking.
    pub const MOUSE_MOTION_DISABLE: &[u8] = b"\x1b[?1002l";
    /// Disable SGR mouse format.
    pub const MOUSE_SGR_DISABLE: &[u8] = b"\x1b[?1006l";
    /// Enable synchronized output (begin).
    pub const SYNC_OUTPUT_BEGIN: &[u8] = b"\x1b[?2026h";
    /// Disable synchronized output (end).
    pub const SYNC_OUTPUT_END: &[u8] = b"\x1b[?2026l";
    /// Enable bracketed paste mode.
    pub const BRACKETED_PASTE_ENABLE: &[u8] = b"\x1b[?2004h";
    /// Disable bracketed paste mode.
    pub const BRACKETED_PASTE_DISABLE: &[u8] = b"\x1b[?2004l";
    /// Enable focus reporting.
    pub const FOCUS_ENABLE: &[u8] = b"\x1b[?1004h";
    /// Disable focus reporting.
    pub const FOCUS_DISABLE: &[u8] = b"\x1b[?1004l";
    /// OSC 8 hyperlink prefix.
    pub const OSC8_PREFIX: &[u8] = b"\x1b]8;";
}

/// Log PTY result to artifacts using the new artifact system.
pub fn log_pty_result(result: &PtyResult, test_name: &str) {
    use super::analysis::SequenceReport;
    use super::artifacts::{ArtifactManager, SequenceAnalysis, output_to_readable};

    eprintln!("=== PTY Test: {test_name} ===");
    eprintln!("Command: {:?}", result.command);
    eprintln!("Exit code: {:?}", result.exit_code);
    eprintln!("Duration: {:?}", result.duration);
    eprintln!("Output bytes: {}", result.output.len());

    // Log environment
    eprintln!("Environment:");
    for (k, v) in &result.env {
        if !v.is_empty() {
            eprintln!("  {k}={v}");
        }
    }

    // Use artifact manager for structured artifact storage
    let artifacts = ArtifactManager::new(test_name);

    if artifacts.is_enabled() {
        eprintln!(
            "Artifacts directory: {}",
            artifacts.artifact_dir().display()
        );

        // Save raw output
        artifacts.save_raw_output(&result.output);

        // Save decoded/readable output
        let readable = output_to_readable(&result.output);
        artifacts.save_decoded_output(&readable);

        // Save sequence analysis (legacy format for backwards compatibility)
        let analysis = SequenceAnalysis::from_output(&result.output);
        artifacts.save_sequence_analysis(&analysis);

        // Save comprehensive sequence report (new format)
        let report = SequenceReport::from_output(&result.output);
        if let Ok(json) = report.to_json() {
            artifacts.save_text("sequence_report.json", &json);
        }
        artifacts.save_text("sequence_report.txt", &report.to_human_readable());

        // Save pair validation results
        let validation = report.validate_pairs();
        artifacts.save_text("pair_validation.txt", &validation.to_human_readable());

        // Save test info
        let info = format!(
            "Command: {:?}\nExit code: {:?}\nDuration: {:?}\nOutput bytes: {}\n\nEnvironment:\n{}",
            result.command,
            result.exit_code,
            result.duration,
            result.output.len(),
            result
                .env
                .iter()
                .map(|(k, v)| format!("  {k}={v}"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        artifacts.save_text("info.txt", &info);

        // Also save hex dump for detailed analysis
        artifacts.save_text("output.hex", &result.output_hex());

        // Save timing metrics
        if let Ok(timing_json) = result.timing.to_json() {
            artifacts.save_text("timing_metrics.json", &timing_json);
        }

        // Save human-readable timing summary
        let timing_summary = format_timing_summary(&result.timing);
        artifacts.save_text("timing_summary.txt", &timing_summary);
    }
}

/// Format timing metrics as human-readable summary.
fn format_timing_summary(metrics: &super::metrics::TimingMetrics) -> String {
    let mut summary = String::new();
    summary.push_str("=== Timing Metrics Summary ===\n\n");

    summary.push_str(&format!("Total Runtime: {:?}\n", metrics.total_runtime));

    if let Some(startup) = metrics.startup_time {
        summary.push_str(&format!("Startup Time: {:?}\n", startup));
    }

    if let Some(first_frame) = metrics.first_frame_time {
        summary.push_str(&format!("First Frame: {:?}\n", first_frame));
    }

    summary.push_str(&format!("Frame Count: {}\n", metrics.frame_count));

    if let Some(avg_frame) = metrics.avg_frame_time {
        let fps = 1.0 / avg_frame.as_secs_f64();
        summary.push_str(&format!(
            "Avg Frame Time: {:?} ({:.1} fps)\n",
            avg_frame, fps
        ));
    }

    if let Some(throughput) = metrics.output_throughput_bps {
        summary.push_str(&format!("Output Throughput: {:.0} bytes/sec\n", throughput));
    }

    if !metrics.tour_step_durations.is_empty() {
        summary.push_str("\nTour Step Durations:\n");
        for step in &metrics.tour_step_durations {
            summary.push_str(&format!(
                "  Step {}: {:?} ({} bytes)\n",
                step.step_number, step.duration, step.output_bytes
            ));
        }
    }

    summary
}
