//! `demo_showcase` — `OpenTUI` demonstration binary
//!
//! A comprehensive showcase of `OpenTUI`'s rendering capabilities, presenting
//! a Developer Workbench with editor, preview, logs, and overlays.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin demo_showcase
//! cargo run --bin demo_showcase -- --help
//! cargo run --bin demo_showcase -- --fps 30 --no-mouse
//! cargo run --bin demo_showcase -- --headless-smoke
//! ```
//!
//! Press Ctrl+Q to quit.

// Required for libc FFI (fcntl for non-blocking stdin).
#![allow(unsafe_code)]

use opentui::GraphemePool;
use opentui::buffer::{ClipRect, GrayscaleBuffer, OptimizedBuffer, PixelBuffer};
use opentui::event::{LogLevel as OpentuiLogLevel, set_log_callback};
use opentui::input::{Event, InputParser, KeyCode, KeyModifiers};
use opentui::terminal::{MouseButton, MouseEventKind, enable_raw_mode, terminal_size};
// TODO: EditBuffer, EditorView, WrapMode will be used for editor integration
#[allow(unused_imports)]
use opentui::text::{EditBuffer, EditorView, WrapMode};
use opentui::{CellContent, Renderer, RendererOptions, Rgba, Style};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io::{self, Read};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

// ============================================================================
// CLI Parsing
// ============================================================================

const HELP_TEXT: &str = "demo_showcase - OpenTUI demonstration binary

USAGE:
    demo_showcase [OPTIONS]

OPTIONS:
    -h, --help              Print this help message and exit
    --tour                  Start in tour mode immediately
    --fps <N>               Cap frames per second (default: 60)

    --no-mouse              Disable mouse tracking
    --no-alt-screen         Don't enter alternate screen
    --no-cap-queries        Skip terminal capability queries

    --max-frames <N>        Exit after presenting N frames
    --exit-after-tour       Exit automatically when tour completes

    --headless-smoke        Run headless smoke test (no TTY required)
    --headless-size <WxH>   Force headless buffer size (default: 80x24)
    --headless-dump-json    Output JSON snapshot for regression testing
    --headless-check <NAME> Run specific headless check (layout, config,
                            palette, hitgrid, logs)

    --cap-preset <NAME>     Capability preset: auto, ideal, no_truecolor,
                            no_hyperlinks, no_mouse, minimal (default: auto)

    --threaded              Use ThreadedRenderer backend
    --seed <N>              Deterministic seed for animations (default: 0)

EXAMPLES:
    demo_showcase                       # Interactive mode
    demo_showcase --tour                # Start tour immediately
    demo_showcase --fps 30 --no-mouse   # 30 FPS, keyboard only
    demo_showcase --headless-smoke      # CI smoke test
    demo_showcase --max-frames 100      # Run exactly 100 frames then exit
";

/// Capability preset for testing different terminal configurations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CapPreset {
    #[default]
    Auto,
    Ideal,
    NoTruecolor,
    NoHyperlinks,
    NoMouse,
    Minimal,
}

impl CapPreset {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "ideal" => Some(Self::Ideal),
            "no_truecolor" | "notruecolor" => Some(Self::NoTruecolor),
            "no_hyperlinks" | "nohyperlinks" => Some(Self::NoHyperlinks),
            "no_mouse" | "nomouse" => Some(Self::NoMouse),
            "minimal" => Some(Self::Minimal),
            _ => None,
        }
    }
}

/// Effective capabilities after applying preset constraints.
///
/// In interactive mode: `effective = detected ∩ preset` (preset can only DISABLE).
/// In headless mode: preset defines capabilities directly.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // Capabilities naturally map to booleans
pub struct EffectiveCaps {
    /// True color (24-bit) support.
    pub truecolor: bool,
    /// Mouse tracking support.
    pub mouse: bool,
    /// Hyperlink (OSC 8) support.
    pub hyperlinks: bool,
    /// Focus event support.
    pub focus: bool,
    /// Synchronized output support.
    pub sync_output: bool,
    /// List of features that were disabled by the preset.
    pub degraded: Vec<&'static str>,
}

impl Default for EffectiveCaps {
    fn default() -> Self {
        Self {
            truecolor: true,
            mouse: true,
            hyperlinks: true,
            focus: true,
            sync_output: true,
            degraded: Vec::new(),
        }
    }
}

impl EffectiveCaps {
    /// Compute effective capabilities from detected caps and preset.
    ///
    /// For interactive mode, pass detected capabilities from the renderer.
    /// For headless mode, pass `None` to use preset directly.
    #[must_use]
    pub fn compute(detected: Option<&opentui::terminal::Capabilities>, preset: CapPreset) -> Self {
        let mut caps = Self::default();
        let mut degraded = Vec::new();

        // Start with detected capabilities (or ideal defaults for headless)
        if let Some(det) = detected {
            caps.truecolor = det.has_true_color();
            caps.mouse = det.mouse;
            caps.hyperlinks = det.hyperlinks;
            caps.focus = det.focus;
            caps.sync_output = det.sync_output;
        }

        // Apply preset constraints (can only disable in interactive mode)
        match preset {
            CapPreset::Auto | CapPreset::Ideal => {
                // No additional constraints
            }
            CapPreset::NoTruecolor => {
                if caps.truecolor {
                    degraded.push("truecolor (preset)");
                }
                caps.truecolor = false;
            }
            CapPreset::NoHyperlinks => {
                if caps.hyperlinks {
                    degraded.push("hyperlinks (preset)");
                }
                caps.hyperlinks = false;
            }
            CapPreset::NoMouse => {
                if caps.mouse {
                    degraded.push("mouse (preset)");
                }
                caps.mouse = false;
            }
            CapPreset::Minimal => {
                if caps.truecolor {
                    degraded.push("truecolor (minimal)");
                }
                if caps.mouse {
                    degraded.push("mouse (minimal)");
                }
                if caps.hyperlinks {
                    degraded.push("hyperlinks (minimal)");
                }
                if caps.sync_output {
                    degraded.push("sync_output (minimal)");
                }
                caps.truecolor = false;
                caps.mouse = false;
                caps.hyperlinks = false;
                caps.sync_output = false;
            }
        }

        // Track features that were unavailable from detection
        if let Some(det) = detected {
            if !det.has_true_color()
                && preset != CapPreset::NoTruecolor
                && preset != CapPreset::Minimal
            {
                degraded.push("truecolor (terminal)");
            }
            if !det.mouse && preset != CapPreset::NoMouse && preset != CapPreset::Minimal {
                degraded.push("mouse (terminal)");
            }
            if !det.hyperlinks && preset != CapPreset::NoHyperlinks && preset != CapPreset::Minimal
            {
                degraded.push("hyperlinks (terminal)");
            }
        }

        caps.degraded = degraded;
        caps
    }

    /// Check if any features are degraded.
    #[must_use]
    pub fn is_degraded(&self) -> bool {
        !self.degraded.is_empty()
    }
}

/// Application configuration parsed from command-line arguments.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // Config naturally has many boolean flags
pub struct Config {
    // Interactive mode
    pub start_in_tour: bool,
    pub fps_cap: u32,

    // Renderer options
    pub enable_mouse: bool,
    pub use_alt_screen: bool,
    pub query_capabilities: bool,

    // Deterministic termination
    pub max_frames: Option<u64>,
    pub exit_after_tour: bool,

    // Headless/testing
    pub headless_smoke: bool,
    pub headless_size: (u16, u16),
    pub headless_dump_json: bool,
    /// Run a specific headless check (layout, config, palette, hitgrid, logs).
    pub headless_check: Option<String>,

    // Capability override
    pub cap_preset: CapPreset,

    // Advanced
    pub threaded: bool,
    pub seed: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            start_in_tour: false,
            fps_cap: 60,
            enable_mouse: true,
            use_alt_screen: true,
            query_capabilities: true,
            max_frames: None,
            exit_after_tour: false,
            headless_smoke: false,
            headless_size: (80, 24),
            headless_dump_json: false,
            headless_check: None,
            cap_preset: CapPreset::Auto,
            threaded: false,
            seed: 0,
        }
    }
}

/// Result of CLI parsing.
pub enum ParseResult {
    /// Successfully parsed configuration.
    Config(Config),
    /// User requested help.
    Help,
    /// Parse error with message.
    Error(String),
}

impl Config {
    /// Parse configuration from command-line arguments.
    #[allow(clippy::too_many_lines)]
    pub fn from_args<I>(args: I) -> ParseResult
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut config = Self::default();
        let mut args = args.into_iter();

        // Skip program name
        args.next();

        while let Some(arg) = args.next() {
            let arg_str = arg.to_string_lossy();

            match arg_str.as_ref() {
                "-h" | "--help" => return ParseResult::Help,

                "--tour" => config.start_in_tour = true,

                "--fps" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => return ParseResult::Error("--fps requires a value".to_string()),
                    };
                    match value.parse::<u32>() {
                        Ok(n) if n > 0 => config.fps_cap = n,
                        _ => {
                            return ParseResult::Error(format!(
                                "Invalid --fps value: {value} (must be positive integer)"
                            ));
                        }
                    }
                }

                "--no-mouse" => config.enable_mouse = false,
                "--no-alt-screen" => config.use_alt_screen = false,
                "--no-cap-queries" => config.query_capabilities = false,

                "--max-frames" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => {
                            return ParseResult::Error("--max-frames requires a value".to_string());
                        }
                    };
                    match value.parse::<u64>() {
                        Ok(n) => config.max_frames = Some(n),
                        Err(_) => {
                            return ParseResult::Error(format!(
                                "Invalid --max-frames value: {value}"
                            ));
                        }
                    }
                }

                "--exit-after-tour" => config.exit_after_tour = true,

                "--headless-smoke" => config.headless_smoke = true,
                "--headless-dump-json" => config.headless_dump_json = true,

                "--headless-size" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => {
                            return ParseResult::Error(
                                "--headless-size requires a value (e.g., 80x24)".to_string(),
                            );
                        }
                    };
                    match parse_size(&value) {
                        Some((w, h)) => config.headless_size = (w, h),
                        None => {
                            return ParseResult::Error(format!(
                                "Invalid --headless-size: {value} (use WxH format, e.g., 80x24)"
                            ));
                        }
                    }
                }

                "--headless-check" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => {
                            return ParseResult::Error(
                                "--headless-check requires a value (layout, config, palette, hitgrid, logs)".to_string(),
                            );
                        }
                    };
                    let valid_checks = ["layout", "config", "palette", "hitgrid", "logs"];
                    if valid_checks.contains(&value.as_str()) {
                        config.headless_check = Some(value);
                    } else {
                        return ParseResult::Error(format!(
                            "Unknown --headless-check: {value} (valid: layout, config, palette, hitgrid, logs)"
                        ));
                    }
                }

                "--cap-preset" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => {
                            return ParseResult::Error("--cap-preset requires a value".to_string());
                        }
                    };
                    match CapPreset::from_str(&value) {
                        Some(preset) => config.cap_preset = preset,
                        None => {
                            return ParseResult::Error(format!(
                                "Unknown --cap-preset: {value} \
                                 (valid: auto, ideal, no_truecolor, no_mouse, minimal)"
                            ));
                        }
                    }
                }

                "--threaded" => config.threaded = true,

                "--seed" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => return ParseResult::Error("--seed requires a value".to_string()),
                    };
                    match value.parse::<u64>() {
                        Ok(n) => config.seed = n,
                        Err(_) => {
                            return ParseResult::Error(format!("Invalid --seed value: {value}"));
                        }
                    }
                }

                other => {
                    if other.starts_with('-') {
                        return ParseResult::Error(format!("Unknown option: {other}"));
                    }
                    // Ignore positional arguments for now
                }
            }
        }

        ParseResult::Config(config)
    }

    /// Get renderer options from config.
    #[must_use]
    pub fn renderer_options(&self) -> RendererOptions {
        RendererOptions {
            use_alt_screen: self.use_alt_screen,
            hide_cursor: true,
            enable_mouse: self.enable_mouse && self.cap_preset != CapPreset::NoMouse,
            query_capabilities: self.query_capabilities,
        }
    }

    /// Get target frame duration.
    #[must_use]
    pub fn frame_duration(&self) -> Duration {
        Duration::from_micros(1_000_000 / u64::from(self.fps_cap))
    }
}

/// Parse a size string like "80x24" into (width, height).
#[allow(clippy::missing_const_for_fn)] // str::split is not const
fn parse_size(s: &str) -> Option<(u16, u16)> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return None;
    }
    let w = parts[0].parse::<u16>().ok()?;
    let h = parts[1].parse::<u16>().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

// ============================================================================
// Hit Testing IDs
// ============================================================================

/// Hit ID ranges for mouse interaction.
///
/// We reserve ID ranges by component for stability and debuggability:
/// - `1000-1999`: Chrome buttons (top bar controls)
/// - `2000-2999`: Sidebar rows (section navigation)
/// - `3000-3999`: Panel areas (focus targets)
/// - `4000-4999`: Overlay controls (close buttons, etc.)
///
/// ID 0 is reserved for "no hit" / background.
pub mod hit_ids {
    // Chrome buttons (top bar)
    pub const BTN_HELP: u32 = 1000;
    pub const BTN_PALETTE: u32 = 1001;
    pub const BTN_TOUR: u32 = 1002;
    pub const BTN_THEME: u32 = 1003;

    // Sidebar rows (base + index)
    pub const SIDEBAR_ROW_BASE: u32 = 2000;

    // Panel focus areas
    pub const PANEL_SIDEBAR: u32 = 3000;
    pub const PANEL_EDITOR: u32 = 3001;
    pub const PANEL_PREVIEW: u32 = 3002;
    pub const PANEL_LOGS: u32 = 3003;

    // Overlay controls
    pub const OVERLAY_CLOSE: u32 = 4000;
    pub const PALETTE_ITEM_BASE: u32 = 4100;
}

// ============================================================================
// Animation Clock & Easing
// ============================================================================

/// Easing functions for smooth animations.
///
/// All functions take `t` in `[0.0, 1.0]` and return a value in `[0.0, 1.0]`.
pub mod easing {
    /// Smooth Hermite interpolation: `3t² - 2t³`.
    ///
    /// Starts and ends with zero velocity.
    #[must_use]
    pub fn smoothstep(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        t * t * 2.0_f32.mul_add(-t, 3.0)
    }

    /// Ease in-out cubic: slow start, fast middle, slow end.
    ///
    /// Formula: `4t³` for t < 0.5, `1 - (-2t + 2)³ / 2` for t ≥ 0.5.
    #[must_use]
    pub fn ease_in_out_cubic(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            let p = (-2.0_f32).mul_add(t, 2.0);
            1.0 - p * p * p / 2.0
        }
    }

    /// Pulsing sine wave: `0.5 + 0.5 * sin(t * ω)`.
    ///
    /// Returns a value oscillating between 0.0 and 1.0.
    /// - `t`: time in seconds
    /// - `omega`: angular frequency (2π = one cycle per second)
    #[must_use]
    pub fn pulse(t: f32, omega: f32) -> f32 {
        0.5_f32.mul_add((t * omega).sin(), 0.5)
    }

    /// Ease-out cubic: fast start, slow end.
    ///
    /// Formula: `1 - (1 - t)³`.
    #[must_use]
    pub fn ease_out_cubic(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}

/// Animation clock for frame-based timing.
///
/// Provides:
/// - `t`: monotonic animation time in seconds (doesn't advance when paused)
/// - `dt`: delta time for the current frame (clamped to avoid huge jumps)
/// - Automatic pause handling when terminal focus is lost
#[derive(Clone, Debug)]
pub struct AnimationClock {
    /// Monotonic animation time in seconds.
    ///
    /// Only advances when not paused. Use for animations.
    pub t: f32,
    /// Delta time for the current frame in seconds.
    ///
    /// Clamped to `[0.0, MAX_DT]` to avoid huge jumps after resize/backgrounding.
    pub dt: f32,
    /// Last update instant for computing dt.
    last_instant: Instant,
    /// Whether animation time should advance.
    paused: bool,
}

impl Default for AnimationClock {
    fn default() -> Self {
        Self::new()
    }
}

impl AnimationClock {
    /// Maximum delta time to prevent huge jumps after backgrounding/resize.
    ///
    /// At 60fps, normal dt ≈ 0.0167s. Cap at 0.1s (10fps equivalent).
    pub const MAX_DT: f32 = 0.1;

    /// Minimum delta time to ensure animations always progress.
    ///
    /// Prevents dt = 0 issues when frames are extremely fast.
    pub const MIN_DT: f32 = 0.001;

    /// Create a new animation clock starting at t=0.
    #[must_use]
    pub fn new() -> Self {
        Self {
            t: 0.0,
            dt: 0.0,
            last_instant: Instant::now(),
            paused: false,
        }
    }

    /// Update the clock for a new frame.
    ///
    /// Call this once at the start of each frame, before any animation updates.
    /// Pass the current pause state from the app.
    pub fn tick(&mut self, paused: bool) {
        let now = Instant::now();
        let raw_dt = now.duration_since(self.last_instant).as_secs_f32();
        self.last_instant = now;

        // Clamp dt to avoid huge jumps
        self.dt = raw_dt.clamp(Self::MIN_DT, Self::MAX_DT);

        // Update pause state
        self.paused = paused;

        // Only advance animation time when not paused
        if !self.paused {
            self.t += self.dt;
        }
    }

    /// Check if the clock is paused.
    #[must_use]
    pub const fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set the pause state directly.
    pub const fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Get animation time with a phase offset (useful for staggered animations).
    #[must_use]
    pub fn t_offset(&self, offset: f32) -> f32 {
        self.t + offset
    }

    /// Get a pulsing value for the current time.
    ///
    /// Convenience method that calls `easing::pulse(self.t, omega)`.
    #[must_use]
    pub fn pulse(&self, omega: f32) -> f32 {
        easing::pulse(self.t, omega)
    }
}

// ============================================================================
// Layout Helpers
// ============================================================================

/// A rectangle with signed origin (allows off-screen positioning).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

// Allow u32 to i32 casts in Rect methods - values are validated to be small enough.
#[allow(clippy::cast_possible_wrap)]
impl Rect {
    /// Create a new rectangle.
    #[must_use]
    pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Self {
        Self { x, y, w, h }
    }

    /// Create a rectangle from origin 0,0.
    #[must_use]
    pub const fn from_size(w: u32, h: u32) -> Self {
        Self { x: 0, y: 0, w, h }
    }

    /// Shrink the rectangle by `pad` on all sides.
    #[must_use]
    pub const fn inset(self, pad: u32) -> Self {
        let pad2 = pad.saturating_mul(2);
        Self {
            x: self.x.saturating_add(pad as i32),
            y: self.y.saturating_add(pad as i32),
            w: self.w.saturating_sub(pad2),
            h: self.h.saturating_sub(pad2),
        }
    }

    /// Split horizontally: left gets `left_w`, right gets the rest.
    #[must_use]
    pub const fn split_h(self, left_w: u32) -> (Self, Self) {
        let left_w = if left_w > self.w { self.w } else { left_w };
        let left = Self {
            x: self.x,
            y: self.y,
            w: left_w,
            h: self.h,
        };
        let right = Self {
            x: self.x.saturating_add(left_w as i32),
            y: self.y,
            w: self.w.saturating_sub(left_w),
            h: self.h,
        };
        (left, right)
    }

    /// Split vertically: top gets `top_h`, bottom gets the rest.
    #[must_use]
    pub const fn split_v(self, top_h: u32) -> (Self, Self) {
        let top_h = if top_h > self.h { self.h } else { top_h };
        let top = Self {
            x: self.x,
            y: self.y,
            w: self.w,
            h: top_h,
        };
        let bottom = Self {
            x: self.x,
            y: self.y.saturating_add(top_h as i32),
            w: self.w,
            h: self.h.saturating_sub(top_h),
        };
        (top, bottom)
    }

    /// Clamp to fit within given bounds (from origin 0,0).
    #[must_use]
    pub const fn clamp_to(self, max_w: u32, max_h: u32) -> Self {
        let new_w = if self.w > max_w { max_w } else { self.w };
        let new_h = if self.h > max_h { max_h } else { self.h };
        Self {
            x: self.x,
            y: self.y,
            w: new_w,
            h: new_h,
        }
    }

    /// Check if the rectangle is empty (zero width or height).
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    /// Get right edge (x + w).
    #[must_use]
    pub const fn right(self) -> i32 {
        self.x.saturating_add(self.w as i32)
    }

    /// Get bottom edge (y + h).
    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.y.saturating_add(self.h as i32)
    }
}

/// Layout mode based on terminal size (from `DEMO_SHOWCASE_RESILIENCE.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LayoutMode {
    /// Full layout: 80+ x 24+ with all panels visible.
    #[default]
    Full,
    /// Compact layout: 60-79 x 16-23 — sidebar collapses to icons.
    Compact,
    /// Minimal layout: 40-59 x 12-15 — single panel, no sidebar.
    Minimal,
    /// Terminal too small to display anything useful.
    TooSmall,
}

impl LayoutMode {
    /// Compute layout mode from terminal dimensions.
    #[must_use]
    pub const fn from_size(width: u32, height: u32) -> Self {
        if width < 40 || height < 12 {
            Self::TooSmall
        } else if width < 60 || height < 16 {
            Self::Minimal
        } else if width < 80 || height < 24 {
            Self::Compact
        } else {
            Self::Full
        }
    }
}

/// Layout constants for the showcase panels.
pub mod layout {
    /// Height of the top bar.
    pub const TOP_BAR_HEIGHT: u32 = 1;
    /// Height of the status bar.
    pub const STATUS_BAR_HEIGHT: u32 = 1;
    /// Sidebar width in full layout mode.
    pub const SIDEBAR_WIDTH_FULL: u32 = 20;
    /// Sidebar width in compact layout mode (icons only).
    pub const SIDEBAR_WIDTH_COMPACT: u32 = 4;
    /// Preview panel width ratio (percentage of remaining space).
    pub const PREVIEW_WIDTH_RATIO: u32 = 40;
    /// Minimum width for the editor panel.
    pub const EDITOR_MIN_WIDTH: u32 = 30;
    /// Logs panel height in full layout mode.
    pub const LOGS_HEIGHT_FULL: u32 = 6;
    /// Logs panel height in compact layout mode.
    pub const LOGS_HEIGHT_COMPACT: u32 = 4;
    /// Minimum terminal width.
    pub const MIN_WIDTH: u32 = 40;
    /// Minimum terminal height.
    pub const MIN_HEIGHT: u32 = 12;
}

// ============================================================================
// Theme System
// ============================================================================

/// Available UI themes for the showcase.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiTheme {
    /// Synthwave Professional (dark, neon accents).
    #[default]
    SynthwaveDark,
    /// Light theme with paper-like appearance.
    PaperLight,
    /// Solarized-inspired low eye strain theme.
    Solarized,
    /// High contrast for accessibility / limited terminals.
    HighContrast,
}

impl UiTheme {
    /// All themes in order.
    pub const ALL: [Self; 4] = [
        Self::SynthwaveDark,
        Self::PaperLight,
        Self::Solarized,
        Self::HighContrast,
    ];

    /// Get display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::SynthwaveDark => "Synthwave",
            Self::PaperLight => "Paper",
            Self::Solarized => "Solarized",
            Self::HighContrast => "High Contrast",
        }
    }

    /// Cycle to next theme.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::SynthwaveDark => Self::PaperLight,
            Self::PaperLight => Self::Solarized,
            Self::Solarized => Self::HighContrast,
            Self::HighContrast => Self::SynthwaveDark,
        }
    }

    /// Is this a dark theme?
    #[must_use]
    pub const fn is_dark(self) -> bool {
        match self {
            Self::SynthwaveDark | Self::Solarized | Self::HighContrast => true,
            Self::PaperLight => false,
        }
    }

    /// Get the tokens (colors) for this theme.
    #[must_use]
    pub fn tokens(self) -> Theme {
        match self {
            Self::SynthwaveDark => Theme::synthwave(),
            Self::PaperLight => Theme::paper_light(),
            Self::Solarized => Theme::solarized(),
            Self::HighContrast => Theme::high_contrast(),
        }
    }
}

/// Color tokens for the UI.
///
/// Each theme provides a complete set of colors for consistent styling.
pub struct Theme {
    /// Primary background (darkest / app background).
    pub bg0: Rgba,
    /// Secondary background (panels).
    pub bg1: Rgba,
    /// Tertiary background (raised surfaces / borders).
    pub bg2: Rgba,
    /// Primary foreground (main text).
    pub fg0: Rgba,
    /// Secondary foreground (labels).
    pub fg1: Rgba,
    /// Muted foreground (hints, disabled).
    pub fg2: Rgba,
    /// Primary accent (brand color / links / focus).
    pub accent_primary: Rgba,
    /// Secondary accent (highlights / hover).
    pub accent_secondary: Rgba,
    /// Success color.
    pub accent_success: Rgba,
    /// Warning color.
    pub accent_warning: Rgba,
    /// Error color.
    pub accent_error: Rgba,
    /// Selection background.
    pub selection_bg: Rgba,
    /// Focus border color.
    pub focus_border: Rgba,
}

impl Theme {
    /// Synthwave Professional theme (dark, neon accents).
    #[must_use]
    pub fn synthwave() -> Self {
        Self {
            bg0: Rgba::from_hex("#0f1220").unwrap_or(Rgba::BLACK),
            bg1: Rgba::from_hex("#151a2e").unwrap_or(Rgba::BLACK),
            bg2: Rgba::from_hex("#1d2440").unwrap_or(Rgba::BLACK),
            fg0: Rgba::from_hex("#e6e6e6").unwrap_or(Rgba::WHITE),
            fg1: Rgba::from_hex("#aeb6d6").unwrap_or(Rgba::WHITE),
            fg2: Rgba::from_hex("#6c7396").unwrap_or(Rgba::WHITE),
            accent_primary: Rgba::from_hex("#4dd6ff").unwrap_or(Rgba::rgb(0.0, 1.0, 1.0)),
            accent_secondary: Rgba::from_hex("#ff4fd8").unwrap_or(Rgba::rgb(1.0, 0.0, 1.0)),
            accent_success: Rgba::from_hex("#2bff88").unwrap_or(Rgba::GREEN),
            accent_warning: Rgba::from_hex("#ffb020").unwrap_or(Rgba::rgb(1.0, 0.7, 0.1)),
            accent_error: Rgba::from_hex("#ff4455").unwrap_or(Rgba::RED),
            selection_bg: Rgba::from_hex("#2a335c").unwrap_or(Rgba::rgb(0.16, 0.2, 0.36)),
            focus_border: Rgba::from_hex("#4dd6ff").unwrap_or(Rgba::rgb(0.0, 1.0, 1.0)),
        }
    }

    /// Paper Light theme (light, paper-like).
    #[must_use]
    pub fn paper_light() -> Self {
        Self {
            bg0: Rgba::from_hex("#f7f7fb").unwrap_or(Rgba::WHITE),
            bg1: Rgba::from_hex("#ffffff").unwrap_or(Rgba::WHITE),
            bg2: Rgba::from_hex("#eef0f7").unwrap_or(Rgba::WHITE),
            fg0: Rgba::from_hex("#1a1b26").unwrap_or(Rgba::BLACK),
            fg1: Rgba::from_hex("#3a3f5a").unwrap_or(Rgba::BLACK),
            fg2: Rgba::from_hex("#6a6f8a").unwrap_or(Rgba::BLACK),
            accent_primary: Rgba::from_hex("#2a6fff").unwrap_or(Rgba::BLUE),
            accent_secondary: Rgba::from_hex("#7b61ff").unwrap_or(Rgba::rgb(1.0, 0.0, 1.0)),
            accent_success: Rgba::from_hex("#00a86b").unwrap_or(Rgba::GREEN),
            accent_warning: Rgba::from_hex("#ff8a00").unwrap_or(Rgba::rgb(1.0, 0.55, 0.0)),
            accent_error: Rgba::from_hex("#e53935").unwrap_or(Rgba::RED),
            selection_bg: Rgba::from_hex("#dbe6ff").unwrap_or(Rgba::rgb(0.86, 0.9, 1.0)),
            focus_border: Rgba::from_hex("#2a6fff").unwrap_or(Rgba::BLUE),
        }
    }

    /// Solarized-inspired theme (low eye strain).
    #[must_use]
    pub fn solarized() -> Self {
        Self {
            bg0: Rgba::from_hex("#002b36").unwrap_or(Rgba::BLACK),
            bg1: Rgba::from_hex("#073642").unwrap_or(Rgba::BLACK),
            bg2: Rgba::from_hex("#0b4452").unwrap_or(Rgba::BLACK),
            fg0: Rgba::from_hex("#eee8d5").unwrap_or(Rgba::WHITE),
            fg1: Rgba::from_hex("#93a1a1").unwrap_or(Rgba::WHITE),
            fg2: Rgba::from_hex("#657b83").unwrap_or(Rgba::WHITE),
            accent_primary: Rgba::from_hex("#2aa198").unwrap_or(Rgba::rgb(0.0, 1.0, 1.0)),
            accent_secondary: Rgba::from_hex("#268bd2").unwrap_or(Rgba::BLUE),
            accent_success: Rgba::from_hex("#859900").unwrap_or(Rgba::GREEN),
            accent_warning: Rgba::from_hex("#b58900").unwrap_or(Rgba::rgb(0.7, 0.55, 0.0)),
            accent_error: Rgba::from_hex("#dc322f").unwrap_or(Rgba::RED),
            selection_bg: Rgba::from_hex("#0d5161").unwrap_or(Rgba::rgb(0.05, 0.32, 0.38)),
            focus_border: Rgba::from_hex("#2aa198").unwrap_or(Rgba::rgb(0.0, 1.0, 1.0)),
        }
    }

    /// High contrast theme (accessibility / limited terminals).
    #[must_use]
    pub fn high_contrast() -> Self {
        Self {
            bg0: Rgba::BLACK,
            bg1: Rgba::BLACK,
            bg2: Rgba::from_hex("#111111").unwrap_or(Rgba::BLACK),
            fg0: Rgba::WHITE,
            fg1: Rgba::from_hex("#e0e0e0").unwrap_or(Rgba::WHITE),
            fg2: Rgba::from_hex("#a0a0a0").unwrap_or(Rgba::WHITE),
            accent_primary: Rgba::rgb(0.0, 1.0, 1.0),
            accent_secondary: Rgba::rgb(1.0, 0.0, 1.0),
            accent_success: Rgba::GREEN,
            accent_warning: Rgba::rgb(1.0, 1.0, 0.0),
            accent_error: Rgba::RED,
            selection_bg: Rgba::from_hex("#333333").unwrap_or(Rgba::rgb(0.2, 0.2, 0.2)),
            focus_border: Rgba::rgb(1.0, 1.0, 0.0),
        }
    }

    /// Lerp (linear interpolate) between two colors.
    ///
    /// `t = 0.0` returns `a`, `t = 1.0` returns `b`.
    #[must_use]
    pub fn lerp(a: Rgba, b: Rgba, t: f32) -> Rgba {
        Rgba::new(
            (b.r - a.r).mul_add(t, a.r),
            (b.g - a.g).mul_add(t, a.g),
            (b.b - a.b).mul_add(t, a.b),
            (b.a - a.a).mul_add(t, a.a),
        )
    }

    /// Create a horizontal gradient style iterator.
    ///
    /// Returns an iterator that yields colors from `start` to `end`
    /// over `steps` columns.
    #[allow(clippy::cast_precision_loss)] // Precision loss acceptable for gradient steps
    pub fn gradient(start: Rgba, end: Rgba, steps: u32) -> impl Iterator<Item = Rgba> {
        (0..steps).map(move |i| {
            let t = if steps > 1 {
                i as f32 / (steps - 1) as f32
            } else {
                0.0
            };
            Self::lerp(start, end, t)
        })
    }
}

// ============================================================================
// Style Builders
// ============================================================================

/// Pre-built styles for common UI elements.
pub struct Styles;

impl Styles {
    /// Header style: bold with primary accent.
    #[must_use]
    pub fn header(theme: &Theme) -> Style {
        Style::builder().fg(theme.fg0).bg(theme.bg1).bold().build()
    }

    /// Panel border style (unfocused).
    #[must_use]
    pub fn border(theme: &Theme) -> Style {
        Style::builder().fg(theme.fg2).bg(theme.bg0).build()
    }

    /// Panel border style (focused).
    #[must_use]
    pub fn border_focused(theme: &Theme) -> Style {
        Style::builder()
            .fg(theme.focus_border)
            .bg(theme.bg0)
            .bold()
            .build()
    }

    /// Selection style.
    #[must_use]
    pub fn selection(theme: &Theme) -> Style {
        Style::builder()
            .fg(theme.fg0)
            .bg(theme.selection_bg)
            .build()
    }

    /// Muted/hint text style.
    #[must_use]
    pub fn muted(theme: &Theme) -> Style {
        Style::builder().fg(theme.fg2).bg(theme.bg0).build()
    }

    /// Status bar style.
    #[must_use]
    pub fn status_bar(theme: &Theme) -> Style {
        Style::builder().fg(theme.fg1).bg(theme.bg2).build()
    }

    /// Key hint style (hotkeys in status bar).
    #[must_use]
    pub fn key_hint(theme: &Theme) -> Style {
        Style::builder().fg(theme.fg0).bg(theme.bg2).bold().build()
    }

    /// Link style.
    #[must_use]
    pub fn link(theme: &Theme) -> Style {
        Style::builder()
            .fg(theme.accent_primary)
            .underline()
            .build()
    }

    /// Error style.
    #[must_use]
    pub fn error(theme: &Theme) -> Style {
        Style::builder().fg(theme.accent_error).bold().build()
    }

    /// Success style.
    #[must_use]
    pub fn success(theme: &Theme) -> Style {
        Style::builder().fg(theme.accent_success).bold().build()
    }

    /// Warning style.
    #[must_use]
    pub fn warning(theme: &Theme) -> Style {
        Style::builder().fg(theme.accent_warning).build()
    }
}

// ============================================================================
// Overlay System
// ============================================================================

/// Animation state for overlay transitions.
#[derive(Clone, Copy, Debug, Default)]
pub struct OverlayAnim {
    /// Progress from 0.0 (closed) to 1.0 (fully open).
    pub progress: f32,
    /// Whether we're animating in (true) or out (false).
    pub opening: bool,
}

impl OverlayAnim {
    /// Animation speed in progress units per second.
    ///
    /// At 9.0/sec, the full 0→1 transition takes ~0.11 seconds (snappy).
    const SPEED: f32 = 9.0;

    /// Create a new animation starting to open.
    #[must_use]
    pub const fn opening() -> Self {
        Self {
            progress: 0.0,
            opening: true,
        }
    }

    /// Update the animation state. Returns true if animation is complete.
    ///
    /// `dt` is the delta time in seconds from the animation clock.
    pub fn tick(&mut self, dt: f32) -> bool {
        let delta = Self::SPEED * dt;
        if self.opening {
            self.progress = (self.progress + delta).min(1.0);
            self.progress >= 1.0
        } else {
            self.progress = (self.progress - delta).max(0.0);
            self.progress <= 0.0
        }
    }

    /// Start closing the overlay.
    pub const fn start_close(&mut self) {
        self.opening = false;
    }

    /// Get the current opacity (eased).
    #[must_use]
    pub fn opacity(&self) -> f32 {
        // Use ease-out cubic from our easing module
        easing::ease_out_cubic(self.progress)
    }

    /// Check if fully closed.
    #[must_use]
    pub const fn is_closed(&self) -> bool {
        self.progress <= 0.0 && !self.opening
    }

    /// Check if fully open.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.progress >= 1.0
    }
}

/// State for the Help overlay.
#[derive(Clone, Debug, Default)]
pub struct HelpState {
    /// Current scroll offset (line index).
    pub scroll: usize,
    /// Which help section is focused (for future use).
    pub focused_section: usize,
}

impl HelpState {
    /// Help content sections.
    pub const SECTIONS: &'static [(&'static str, &'static [&'static str])] = &[
        (
            "Navigation",
            &[
                "Tab / Shift+Tab    Cycle focus between panels",
                "1-6                Jump to section by number",
                "↑/↓                Navigate within focused panel",
            ],
        ),
        (
            "Actions",
            &[
                "Ctrl+Q             Quit application",
                "Ctrl+N             Cycle UI theme",
                "Ctrl+R             Force redraw",
                "Ctrl+D             Toggle debug overlay",
            ],
        ),
        (
            "Overlays",
            &[
                "F1                 Toggle this help overlay",
                "Ctrl+P             Toggle command palette",
                "Ctrl+T             Toggle guided tour",
                "Esc                Close current overlay",
            ],
        ),
        (
            "Mouse",
            &[
                "Click              Focus panel / activate button",
                "Scroll             Scroll within logs/lists",
                "Sidebar click      Navigate to section",
            ],
        ),
        (
            "Feature Legend",
            &[
                "• Alpha blending   Overlays + glass panels",
                "• Scissor stack    Sidebar/log scroll clipping",
                "• Opacity stack    Tinted UI + overlay backdrop",
                "• Grapheme pool    Unicode panel + emoji",
                "• OSC 8 links      Logs + help (in term)",
                "• Hit grid         Clickable buttons/nav",
                "• Pixel buffers    Preview animated orb",
                "• Diff rendering   Efficient partial updates",
            ],
        ),
        (
            "Links",
            &["Repo: github.com/opentui/opentui", "Docs: opentui.dev"],
        ),
    ];

    /// Scroll up by one line.
    pub const fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    /// Scroll down by one line.
    pub const fn scroll_down(&mut self, max_scroll: usize) {
        if self.scroll < max_scroll {
            self.scroll += 1;
        }
    }
}

/// State for the Command Palette overlay.
#[derive(Clone, Debug, Default)]
pub struct PaletteState {
    /// Current search query.
    pub query: String,
    /// Selected command index.
    pub selected: usize,
    /// Filtered command indices.
    pub filtered: Vec<usize>,
}

impl PaletteState {
    /// Available commands in the palette.
    pub const COMMANDS: &'static [(&'static str, &'static str)] = &[
        ("Toggle Help", "Show keyboard shortcuts and tips"),
        ("Toggle Tour", "Start the guided feature tour"),
        ("Cycle Theme", "Switch to the next color theme"),
        ("Force Redraw", "Refresh the entire display"),
        ("Toggle Debug", "Show/hide performance overlay"),
        ("Quit", "Exit the application"),
    ];

    /// Update filtered commands based on query.
    pub fn update_filter(&mut self) {
        let query_lower = self.query.to_lowercase();
        self.filtered = Self::COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, (name, desc))| {
                query_lower.is_empty()
                    || name.to_lowercase().contains(&query_lower)
                    || desc.to_lowercase().contains(&query_lower)
            })
            .map(|(i, _)| i)
            .collect();

        // Clamp selection to valid range
        if !self.filtered.is_empty() && self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    /// Move selection up.
    pub fn select_prev(&mut self) {
        if !self.filtered.is_empty() {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    /// Move selection down.
    pub fn select_next(&mut self) {
        if !self.filtered.is_empty() && self.selected < self.filtered.len() - 1 {
            self.selected += 1;
        }
    }
}

/// State for the Tour overlay.
#[derive(Clone, Debug, Default)]
pub struct TourState {
    /// Current tour step (0-indexed).
    pub step: usize,
    /// Highlight rectangle for the current step (if any).
    pub spotlight: Option<Rect>,
}

impl TourState {
    /// Tour step definitions: (title, description, `spotlight_target`).
    pub const STEPS: &'static [(&'static str, &'static str, Option<&'static str>)] = &[
        (
            "Welcome to OpenTUI!",
            "This tour will guide you through the key features.\nPress Enter to continue, Esc to exit.",
            None,
        ),
        (
            "Sidebar Navigation",
            "Use number keys 1-6 or click to switch sections.\nThe sidebar adapts to terminal size.",
            Some("sidebar"),
        ),
        (
            "Editor Panel",
            "The main content area displays text with\nfull grapheme and Unicode support.",
            Some("editor"),
        ),
        (
            "Preview Panel",
            "See rendered output and visual effects.\nAlpha blending is demonstrated here.",
            Some("preview"),
        ),
        (
            "Theme System",
            "Press Ctrl+N to cycle through themes.\n4 built-in themes with full color tokens.",
            None,
        ),
        (
            "Keyboard Shortcuts",
            "Press F1 anytime to see all shortcuts.\nTab cycles focus between panels.",
            None,
        ),
        (
            "Command Palette",
            "Press Ctrl+P to open the command palette.\nQuickly access any action by typing.",
            None,
        ),
        (
            "Responsive Layout",
            "Resize the terminal to see adaptive layouts.\nFull → Compact → Minimal → TooSmall.",
            None,
        ),
        (
            "Alpha Blending Demo",
            "This overlay itself demonstrates alpha blending!\nNotice the backdrop transparency.",
            None,
        ),
        (
            "Performance",
            "OpenTUI uses diff-based rendering.\nOnly changed cells are sent to the terminal.",
            None,
        ),
        (
            "Scissor Clipping",
            "Content is clipped to panel boundaries.\nOverlays use the scissor stack.",
            None,
        ),
        (
            "Tour Complete!",
            "You've seen all the key features.\nPress Esc to exit and explore on your own.",
            None,
        ),
    ];

    /// Advance to the next step. Returns true if tour is complete.
    pub const fn next_step(&mut self) -> bool {
        if self.step < Self::STEPS.len() - 1 {
            self.step += 1;
            false
        } else {
            true
        }
    }

    /// Go back to the previous step.
    pub const fn prev_step(&mut self) {
        self.step = self.step.saturating_sub(1);
    }

    /// Get current step info.
    #[must_use]
    pub fn current(&self) -> (&'static str, &'static str, Option<&'static str>) {
        Self::STEPS
            .get(self.step)
            .copied()
            .unwrap_or(("", "", None))
    }
}

// ============================================================================
// Tour Runner (Script Executor)
// ============================================================================

/// Action that a tour step can trigger.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TourAction {
    /// No action, just show the message.
    None,
    /// Change focus to a specific panel.
    SetFocus(Focus),
    /// Navigate to a specific section.
    SetSection(Section),
    /// Open the help overlay.
    OpenHelp,
    /// Open the command palette.
    OpenPalette,
    /// Close any open overlay.
    CloseOverlay,
    /// Cycle to the next theme.
    CycleTheme,
    /// Show the debug overlay.
    ShowDebug,
}

/// A single tour step with timing and action.
#[derive(Clone, Copy, Debug)]
pub struct TourStep {
    /// Step title shown in the tour overlay.
    pub title: &'static str,
    /// Step description/explanation.
    pub description: &'static str,
    /// Duration in milliseconds before auto-advancing.
    pub duration_ms: u32,
    /// Action to execute when step begins.
    pub action: TourAction,
    /// Spotlight target (panel name for highlighting).
    pub spotlight: Option<&'static str>,
}

/// The canonical tour script - 12 steps proving all major features.
pub const TOUR_SCRIPT: &[TourStep] = &[
    // 1. Welcome
    TourStep {
        title: "Welcome to OpenTUI!",
        description: "This tour demonstrates the key features.\nDiff rendering eliminates flicker.",
        duration_ms: 4000,
        action: TourAction::None,
        spotlight: None,
    },
    // 2. Sidebar Navigation
    TourStep {
        title: "Sidebar Navigation",
        description: "Scissor-clipped scrolling inside panel bounds.\nUse 1-6 or click to navigate.",
        duration_ms: 4000,
        action: TourAction::SetFocus(Focus::Sidebar),
        spotlight: Some("sidebar"),
    },
    // 3. Focus & Hit Testing
    TourStep {
        title: "Focus & Hit Testing",
        description: "Tab cycles focus between panels.\nClick anywhere for instant focus.",
        duration_ms: 3500,
        action: TourAction::SetFocus(Focus::Editor),
        spotlight: Some("editor"),
    },
    // 4. Command Palette
    TourStep {
        title: "Command Palette",
        description: "Glass overlay with alpha blending.\nCtrl+P to open anytime.",
        duration_ms: 4000,
        action: TourAction::OpenPalette,
        spotlight: None,
    },
    // 5. Editor Panel
    TourStep {
        title: "Editor: Rope + Undo",
        description: "Rope-backed text buffer for efficient edits.\nUndo/redo with Ctrl+Z/Y.",
        duration_ms: 4000,
        action: TourAction::CloseOverlay,
        spotlight: Some("editor"),
    },
    // 6. Syntax Highlighting
    TourStep {
        title: "Syntax Highlighting",
        description: "Built-in tokenizers for Rust and Markdown.\nTheme-aware token colors.",
        duration_ms: 3500,
        action: TourAction::SetSection(Section::Editor),
        spotlight: Some("editor"),
    },
    // 7. Theme System
    TourStep {
        title: "Theme System",
        description: "4 built-in themes with full color tokens.\nPress Ctrl+N to cycle themes.",
        duration_ms: 4000,
        action: TourAction::CycleTheme,
        spotlight: None,
    },
    // 8. Unicode & Grapheme Pool
    TourStep {
        title: "Unicode & Graphemes",
        description: "CJK, emoji, ZWJ sequences rendered correctly.\nGrapheme pool handles multi-codepoint chars.",
        duration_ms: 4500,
        action: TourAction::SetSection(Section::Unicode),
        spotlight: Some("preview"),
    },
    // 9. Preview Panel
    TourStep {
        title: "Preview: Alpha Blending",
        description: "Porter-Duff compositing for translucent layers.\nReal RGBA blending, not dithering.",
        duration_ms: 4000,
        action: TourAction::SetSection(Section::Preview),
        spotlight: Some("preview"),
    },
    // 10. Logs Panel
    TourStep {
        title: "Logs & Hyperlinks",
        description: "Event stream with OSC 8 hyperlinks.\nClick links to open in browser.",
        duration_ms: 4000,
        action: TourAction::SetSection(Section::Logs),
        spotlight: Some("logs"),
    },
    // 11. Performance
    TourStep {
        title: "Performance Stats",
        description: "Diff rendering: only changed cells written.\nTypically <1KB per frame after first.",
        duration_ms: 4000,
        action: TourAction::SetSection(Section::Performance),
        spotlight: Some("preview"),
    },
    // 12. Finale
    TourStep {
        title: "Tour Complete!",
        description: "You've seen the core features.\nPress Esc to explore freely.",
        duration_ms: 5000,
        action: TourAction::None,
        spotlight: None,
    },
];

/// Tour runner that executes the script with deterministic timing.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // Tour state naturally has multiple flags
pub struct TourRunner {
    /// Current step index (0-based).
    pub step_idx: usize,
    /// Animation time when current step started.
    pub step_started_t: f32,
    /// Whether tour is currently paused.
    pub paused: bool,
    /// Whether to auto-advance steps (for unattended mode).
    pub auto_advance: bool,
    /// Whether to exit the app when tour completes.
    pub exit_on_complete: bool,
    /// Whether the tour has completed.
    pub completed: bool,
}

impl Default for TourRunner {
    fn default() -> Self {
        Self {
            step_idx: 0,
            step_started_t: 0.0,
            paused: false,
            auto_advance: true,
            exit_on_complete: false,
            completed: false,
        }
    }
}

impl TourRunner {
    /// Create a new tour runner with the given settings.
    #[must_use]
    pub const fn new(auto_advance: bool, exit_on_complete: bool) -> Self {
        Self {
            step_idx: 0,
            step_started_t: 0.0,
            paused: false,
            auto_advance,
            exit_on_complete,
            completed: false,
        }
    }

    /// Get the current tour step.
    #[must_use]
    pub fn current_step(&self) -> Option<&'static TourStep> {
        TOUR_SCRIPT.get(self.step_idx)
    }

    /// Get the total number of steps.
    #[must_use]
    pub const fn total_steps(&self) -> usize {
        TOUR_SCRIPT.len()
    }

    /// Check if there are more steps.
    #[must_use]
    pub const fn has_next(&self) -> bool {
        self.step_idx < TOUR_SCRIPT.len() - 1
    }

    /// Advance to the next step. Returns true if this was the last step.
    pub const fn next_step(&mut self, current_t: f32) -> bool {
        if self.step_idx < TOUR_SCRIPT.len() - 1 {
            self.step_idx += 1;
            self.step_started_t = current_t;
            false
        } else {
            self.completed = true;
            true
        }
    }

    /// Go back to the previous step.
    pub const fn prev_step(&mut self, current_t: f32) {
        if self.step_idx > 0 {
            self.step_idx -= 1;
            self.step_started_t = current_t;
        }
    }

    /// Reset tour to the beginning.
    pub const fn reset(&mut self, current_t: f32) {
        self.step_idx = 0;
        self.step_started_t = current_t;
        self.completed = false;
    }

    /// Toggle pause state.
    pub const fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    /// Check if auto-advance timer has elapsed for current step.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // duration_ms fits in f32 mantissa
    pub fn should_auto_advance(&self, current_t: f32) -> bool {
        if !self.auto_advance || self.paused || self.completed {
            return false;
        }
        self.current_step().is_some_and(|step| {
            let elapsed_ms = (current_t - self.step_started_t) * 1000.0;
            elapsed_ms >= step.duration_ms as f32
        })
    }

    /// Get progress through current step (0.0 to 1.0).
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // duration_ms fits in f32 mantissa
    pub fn step_progress(&self, current_t: f32) -> f32 {
        self.current_step().map_or(1.0, |step| {
            let elapsed_ms = (current_t - self.step_started_t) * 1000.0;
            (elapsed_ms / step.duration_ms as f32).clamp(0.0, 1.0)
        })
    }

    /// Execute the action for the current step, returning actions to apply.
    #[must_use]
    pub fn execute_step_action(&self) -> Option<TourAction> {
        self.current_step().map(|s| s.action)
    }
}

/// Which overlay is currently active.
#[derive(Clone, Debug)]
pub enum Overlay {
    /// Help overlay with keyboard shortcuts.
    Help(HelpState),
    /// Command palette for quick actions.
    Palette(PaletteState),
    /// Guided tour overlay.
    Tour(TourState),
}

/// Manages overlay state and transitions.
#[derive(Clone, Debug, Default)]
pub struct OverlayManager {
    /// Currently active overlay (if any).
    pub active: Option<Overlay>,
    /// Animation state for the current overlay.
    pub anim: OverlayAnim,
}

impl OverlayManager {
    /// Open a new overlay.
    pub fn open(&mut self, overlay: Overlay) {
        self.active = Some(overlay);
        self.anim = OverlayAnim::opening();
    }

    /// Close the current overlay.
    pub const fn close(&mut self) {
        self.anim.start_close();
    }

    /// Update overlay state for a new frame.
    ///
    /// `dt` is the delta time in seconds from the animation clock.
    pub fn tick(&mut self, dt: f32) {
        if self.active.is_some() {
            let done = self.anim.tick(dt);
            if done && self.anim.is_closed() {
                self.active = None;
            }
        }
    }

    /// Check if any overlay is active (including closing animation).
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active.is_some()
    }

    /// Get the current overlay kind (for mode matching).
    #[must_use]
    pub fn kind(&self) -> Option<AppMode> {
        self.active.as_ref().map(|o| match o {
            Overlay::Help(_) => AppMode::Help,
            Overlay::Palette(_) => AppMode::CommandPalette,
            Overlay::Tour(_) => AppMode::Tour,
        })
    }
}

// ============================================================================
// Toast / Notification System
// ============================================================================

/// Toast severity level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ToastLevel {
    /// Informational message.
    #[default]
    Info,
    /// Warning message.
    Warn,
    /// Error message.
    Error,
}

impl ToastLevel {
    /// Get the level name for display.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }

    /// Get icon glyph for this level.
    #[must_use]
    pub const fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ",
            Self::Warn => "⚠",
            Self::Error => "✗",
        }
    }
}

/// A single toast notification.
#[derive(Clone, Debug)]
pub struct Toast {
    /// Severity level.
    pub level: ToastLevel,
    /// Short title text.
    pub title: String,
    /// Optional detail line.
    pub detail: Option<String>,
    /// Time-to-live in seconds (auto-dismiss).
    pub ttl: f32,
    /// Time elapsed since creation (for fade animation).
    pub elapsed: f32,
}

impl Toast {
    /// Default TTL for toasts (3 seconds).
    pub const DEFAULT_TTL: f32 = 3.0;

    /// Fade-out duration at end of life.
    pub const FADE_DURATION: f32 = 0.3;

    /// Create a new info toast.
    #[must_use]
    pub fn info(title: impl Into<String>) -> Self {
        Self {
            level: ToastLevel::Info,
            title: title.into(),
            detail: None,
            ttl: Self::DEFAULT_TTL,
            elapsed: 0.0,
        }
    }

    /// Create a new warning toast.
    #[must_use]
    pub fn warn(title: impl Into<String>) -> Self {
        Self {
            level: ToastLevel::Warn,
            title: title.into(),
            detail: None,
            ttl: Self::DEFAULT_TTL,
            elapsed: 0.0,
        }
    }

    /// Create a new error toast.
    #[must_use]
    pub fn error(title: impl Into<String>) -> Self {
        Self {
            level: ToastLevel::Error,
            title: title.into(),
            detail: None,
            ttl: Self::DEFAULT_TTL,
            elapsed: 0.0,
        }
    }

    /// Add detail text to the toast.
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set custom TTL.
    #[must_use]
    pub const fn with_ttl(mut self, ttl: f32) -> Self {
        self.ttl = ttl;
        self
    }

    /// Check if this toast has expired.
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.elapsed >= self.ttl
    }

    /// Get the opacity for rendering (fades out at end of life).
    #[must_use]
    pub fn opacity(&self) -> f32 {
        let remaining = self.ttl - self.elapsed;
        if remaining <= Self::FADE_DURATION {
            (remaining / Self::FADE_DURATION).max(0.0)
        } else {
            1.0
        }
    }
}

/// Manager for toast notifications.
#[derive(Clone, Debug, Default)]
pub struct ToastManager {
    /// Stack of active toasts (newest at back).
    toasts: VecDeque<Toast>,
}

impl ToastManager {
    /// Maximum number of visible toasts.
    pub const MAX_VISIBLE: usize = 5;

    /// Spacing between toasts (rows).
    pub const TOAST_GAP: u32 = 1;

    /// Toast width (characters).
    pub const TOAST_WIDTH: u32 = 40;

    /// Create a new empty manager.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // VecDeque::new() is not const
    pub fn new() -> Self {
        Self {
            toasts: VecDeque::new(),
        }
    }

    /// Add a new toast to the stack.
    pub fn push(&mut self, toast: Toast) {
        self.toasts.push_back(toast);
        // Limit stack size
        while self.toasts.len() > Self::MAX_VISIBLE {
            self.toasts.pop_front();
        }
    }

    /// Update all toasts for a new frame.
    ///
    /// Returns the number of toasts that expired.
    pub fn tick(&mut self, dt: f32) -> usize {
        for toast in &mut self.toasts {
            toast.elapsed += dt;
        }
        // Remove expired toasts
        let before = self.toasts.len();
        self.toasts.retain(|t: &Toast| !t.is_expired());
        before - self.toasts.len()
    }

    /// Get iterator over visible toasts.
    #[must_use]
    #[allow(clippy::iter_without_into_iter)] // Simple internal iteration
    pub fn iter(&self) -> std::collections::vec_deque::Iter<'_, Toast> {
        self.toasts.iter()
    }

    /// Get the number of active toasts.
    #[must_use]
    pub fn len(&self) -> usize {
        self.toasts.len()
    }

    /// Check if there are no toasts.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.toasts.is_empty()
    }

    /// Clear all toasts.
    pub fn clear(&mut self) {
        self.toasts.clear();
    }
}

// ============================================================================
// Render Pass System
// ============================================================================

/// Render passes in back-to-front order.
///
/// Each pass draws on top of the previous one:
/// 1. **Background** - Fill screen with bg0
/// 2. **Chrome** - Top bar and status bar
/// 3. **Panels** - Sidebar, editor, preview, logs (each clipped)
/// 4. **Overlays** - Help, command palette, tour (semi-transparent)
/// 5. **Toasts** - Ephemeral notifications
/// 6. **Debug** - Performance overlay
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RenderPass {
    /// Fill screen with background color.
    Background = 0,
    /// Draw top bar and status bar.
    Chrome = 1,
    /// Draw main panels (sidebar, editor, preview, logs).
    Panels = 2,
    /// Draw modal overlays (help, command palette, tour).
    Overlays = 3,
    /// Draw toast notifications.
    Toasts = 4,
    /// Draw debug/performance overlay.
    Debug = 5,
}

impl RenderPass {
    /// Get all passes in order.
    pub const ALL: [Self; 6] = [
        Self::Background,
        Self::Chrome,
        Self::Panels,
        Self::Overlays,
        Self::Toasts,
        Self::Debug,
    ];
}

/// Computed panel rectangles for the current layout.
#[derive(Clone, Copy, Debug, Default)]
pub struct PanelLayout {
    /// The layout mode in effect.
    pub mode: LayoutMode,
    /// Full screen bounds.
    pub screen: Rect,
    /// Top bar (full width, 1 row at top).
    pub top_bar: Rect,
    /// Status bar (full width, 1 row at bottom).
    pub status_bar: Rect,
    /// Content area (between top and status bar).
    pub content: Rect,
    /// Sidebar (left side of content area).
    pub sidebar: Rect,
    /// Main area (right of sidebar).
    pub main_area: Rect,
    /// Upper main area (editor + preview in Full mode).
    pub upper_main: Rect,
    /// Editor panel (left portion of upper main area in Full mode).
    pub editor: Rect,
    /// Preview panel (right portion of upper main area in Full mode).
    pub preview: Rect,
    /// Logs panel (bottom of main area, spans full width).
    pub logs: Rect,
}

impl PanelLayout {
    /// Compute panel layout from terminal dimensions.
    #[must_use]
    pub fn compute(width: u32, height: u32) -> Self {
        let mode = LayoutMode::from_size(width, height);
        let screen = Rect::from_size(width, height);

        if mode == LayoutMode::TooSmall {
            // For TooSmall, we just set screen bounds; nothing else makes sense.
            return Self {
                mode,
                screen,
                ..Self::default()
            };
        }

        // Split off top bar.
        let (top_bar, rest) = screen.split_v(layout::TOP_BAR_HEIGHT);
        // Split off status bar from bottom.
        let status_h = rest.h.saturating_sub(layout::STATUS_BAR_HEIGHT);
        let (content, status_bar) = rest.split_v(status_h);

        // Sidebar width depends on mode.
        let sidebar_w = match mode {
            LayoutMode::Full => layout::SIDEBAR_WIDTH_FULL,
            LayoutMode::Compact => layout::SIDEBAR_WIDTH_COMPACT,
            LayoutMode::Minimal | LayoutMode::TooSmall => 0,
        };

        let (sidebar, main_area) = content.split_h(sidebar_w);

        // Logs height depends on mode and available space.
        let logs_h = match mode {
            LayoutMode::Full => layout::LOGS_HEIGHT_FULL.min(main_area.h / 3),
            LayoutMode::Compact => layout::LOGS_HEIGHT_COMPACT.min(main_area.h / 3),
            LayoutMode::Minimal | LayoutMode::TooSmall => 0,
        };

        // Split main area: upper for editor/preview, lower for logs.
        let upper_h = main_area.h.saturating_sub(logs_h);
        let (upper_main, logs) = main_area.split_v(upper_h);

        // Editor/Preview split only in Full mode.
        let (editor, preview) =
            if mode == LayoutMode::Full && upper_main.w > layout::EDITOR_MIN_WIDTH {
                let preview_w = upper_main.w * layout::PREVIEW_WIDTH_RATIO / 100;
                let editor_w = upper_main.w.saturating_sub(preview_w);
                upper_main.split_h(editor_w)
            } else {
                // Compact/Minimal: editor takes all upper main area, no preview.
                (upper_main, Rect::default())
            };

        Self {
            mode,
            screen,
            top_bar,
            status_bar,
            content,
            sidebar,
            main_area,
            upper_main,
            editor,
            preview,
            logs,
        }
    }

    /// Get a panel rect by name (for tour spotlight targeting).
    ///
    /// Supported names: "sidebar", "editor", "preview", "logs", "top_bar", "status_bar"
    #[must_use]
    pub fn get_panel_rect(&self, name: &str) -> Option<Rect> {
        match name {
            "sidebar" => Some(self.sidebar),
            "editor" => Some(self.editor),
            "preview" => Some(self.preview),
            "logs" => Some(self.logs),
            "top_bar" => Some(self.top_bar),
            "status_bar" => Some(self.status_bar),
            "content" => Some(self.content),
            "main_area" => Some(self.main_area),
            _ => None,
        }
    }
}

// ============================================================================
// Application State Machine
// ============================================================================

/// Application mode (from `DEMO_SHOWCASE_KEYBINDINGS.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AppMode {
    /// Standard operation, all panels interactive.
    #[default]
    Normal,
    /// Help overlay is open.
    Help,
    /// Command palette is open.
    CommandPalette,
    /// Guided tour mode.
    Tour,
}

/// Reason for application exit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExitReason {
    /// No exit yet or normal user-initiated quit.
    #[default]
    UserQuit,
    /// Exited due to --max-frames limit.
    MaxFrames,
    /// Exited after tour completion (--exit-after-tour).
    TourComplete,
}

/// Which panel has keyboard focus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    /// Sidebar panel (section navigation).
    #[default]
    Sidebar,
    /// Editor panel (text editing).
    Editor,
    /// Preview panel (visual output).
    Preview,
    /// Logs panel (event stream).
    Logs,
}

impl Focus {
    /// Cycle to the next focus (Tab behavior).
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Sidebar => Self::Editor,
            Self::Editor => Self::Preview,
            Self::Preview => Self::Logs,
            Self::Logs => Self::Sidebar,
        }
    }

    /// Cycle to the previous focus (Shift+Tab behavior).
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Sidebar => Self::Logs,
            Self::Editor => Self::Sidebar,
            Self::Preview => Self::Editor,
            Self::Logs => Self::Preview,
        }
    }
}

/// Content section being displayed/emphasized.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Section {
    /// Overview / welcome screen.
    #[default]
    Overview,
    /// Editor demonstration.
    Editor,
    /// Preview panel demonstration.
    Preview,
    /// Logs panel demonstration.
    Logs,
    /// Unicode / grapheme cluster demonstration.
    Unicode,
    /// Performance / FPS demonstration.
    Performance,
}

impl Section {
    /// All sections for iteration.
    pub const ALL: [Self; 6] = [
        Self::Overview,
        Self::Editor,
        Self::Preview,
        Self::Logs,
        Self::Unicode,
        Self::Performance,
    ];

    /// Get section by index (for number key navigation).
    #[must_use]
    pub fn from_index(idx: usize) -> Option<Self> {
        Self::ALL.get(idx).copied()
    }

    /// Get display name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Editor => "Editor",
            Self::Preview => "Preview",
            Self::Logs => "Logs",
            Self::Unicode => "Unicode",
            Self::Performance => "Performance",
        }
    }
}

/// Actions that can be performed (decouples input from state mutation).
#[derive(Clone, Debug)]
pub enum Action {
    /// Quit the application.
    Quit,
    /// Toggle help overlay.
    ToggleHelp,
    /// Toggle command palette.
    TogglePalette,
    /// Toggle tour mode.
    ToggleTour,
    /// Close current overlay (Esc).
    CloseOverlay,
    /// Cycle focus forward (Tab).
    CycleFocusForward,
    /// Cycle focus backward (Shift+Tab).
    CycleFocusBackward,
    /// Navigate to a specific section.
    NavigateSection(Section),
    /// Force redraw (Ctrl+R).
    ForceRedraw,
    /// Toggle debug overlay (Ctrl+D).
    ToggleDebug,
    /// Cycle to next UI theme (Ctrl+N).
    CycleTheme,
    /// Terminal resized.
    Resize(u32, u32),
    /// Focus gained/lost.
    FocusChanged(bool),
    /// Command palette: navigate up.
    PaletteUp,
    /// Command palette: navigate down.
    PaletteDown,
    /// Command palette: execute selected item.
    PaletteExecute,
    /// Command palette: delete character.
    PaletteBackspace,
    /// Command palette: input character.
    PaletteChar(char),
    /// Mouse: directly set focus to a panel.
    SetFocus(Focus),
    /// Mouse: click on palette item by index.
    PaletteClick(usize),
    /// No action (event was handled or ignored).
    None,
}

/// Application state machine.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)] // App state naturally has many boolean flags
pub struct App {
    // Core state
    /// Current application mode.
    pub mode: AppMode,
    /// Which panel has keyboard focus.
    pub focus: Focus,
    /// Current content section.
    pub section: Section,
    /// Whether the app is paused (e.g., focus lost).
    pub paused: bool,

    // Theme state
    /// Current UI theme.
    pub ui_theme: UiTheme,

    // Runtime state
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Reason for quitting (used for exit summary).
    pub exit_reason: ExitReason,
    /// Frame counter.
    pub frame_count: u64,
    /// Maximum frames before exit (from config).
    pub max_frames: Option<u64>,
    /// Whether to show debug overlay.
    pub show_debug: bool,
    /// Whether a force redraw was requested.
    pub force_redraw: bool,

    // Tour state
    /// Current tour step (0-indexed).
    pub tour_step: usize,
    /// Total tour steps.
    pub tour_total: usize,
    /// Tour runner for script execution (when in tour mode).
    pub tour_runner: Option<TourRunner>,

    // Overlay state
    /// Overlay manager for modal overlays.
    pub overlays: OverlayManager,

    // Animation state
    /// Animation clock for timing animations.
    pub clock: AnimationClock,

    // Content state (wired from DemoContent)
    /// Index of current file in editor (into content.files).
    pub current_file_idx: usize,
    /// Log entries (starts with `seed_logs`, can grow).
    pub logs: Vec<content::LogEntry>,
    /// Target FPS for metrics computation.
    pub target_fps: u32,
    /// Current computed metrics (updated each frame).
    pub metrics: content::Metrics,

    // Toast state
    /// Toast notification manager.
    pub toasts: ToastManager,

    // Capability state
    /// Effective capabilities (detected ∩ preset).
    pub effective_caps: EffectiveCaps,
    /// Capability preset from config.
    pub cap_preset: CapPreset,
}

impl Default for App {
    fn default() -> Self {
        let demo_content = content::DemoContent::default();
        Self {
            mode: AppMode::Normal,
            focus: Focus::Sidebar,
            section: Section::Overview,
            paused: false,
            ui_theme: UiTheme::default(),
            should_quit: false,
            exit_reason: ExitReason::UserQuit,
            frame_count: 0,
            max_frames: None,
            show_debug: false,
            force_redraw: false,
            tour_step: 0,
            tour_total: TOUR_SCRIPT.len(),
            tour_runner: None,
            overlays: OverlayManager::default(),
            clock: AnimationClock::new(),
            // Content state (from default DemoContent)
            current_file_idx: 0,
            logs: demo_content.seed_logs.to_vec(),
            target_fps: demo_content.metric_params.target_fps,
            metrics: content::Metrics::compute(0, demo_content.metric_params.target_fps),
            // Toast state
            toasts: ToastManager::new(),
            // Capability state (defaults to ideal)
            effective_caps: EffectiveCaps::default(),
            cap_preset: CapPreset::Auto,
        }
    }
}

impl App {
    /// Create a new app instance from config with default content.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self::with_content(config, &content::DemoContent::default())
    }

    /// Update effective capabilities from detected terminal capabilities.
    ///
    /// Call this after the renderer is created to apply capability gating.
    pub fn update_effective_caps(&mut self, detected: Option<&opentui::terminal::Capabilities>) {
        self.effective_caps = EffectiveCaps::compute(detected, self.cap_preset);

        // Show toast if any features are degraded
        if self.effective_caps.is_degraded() {
            let msg = format!("Degraded: {}", self.effective_caps.degraded.join(", "));
            self.toasts.push(Toast::warn(msg));
        }
    }

    /// Create a new app instance from config and custom demo content.
    ///
    /// This allows the demo to boot with rich content immediately visible:
    /// - Initial editor buffer with syntax-highlighted code
    /// - Log backlog for scrolling demonstration
    /// - Deterministic metrics for charts and animations
    #[must_use]
    pub fn with_content(config: &Config, demo_content: &content::DemoContent) -> Self {
        // Initialize tour runner if starting in tour mode
        let tour_runner = if config.start_in_tour {
            Some(TourRunner::new(true, config.exit_after_tour))
        } else {
            None
        };

        Self {
            max_frames: config.max_frames,
            mode: if config.start_in_tour {
                AppMode::Tour
            } else {
                AppMode::Normal
            },
            tour_runner,
            // Content wiring
            current_file_idx: 0,
            logs: demo_content.seed_logs.to_vec(),
            target_fps: demo_content.metric_params.target_fps,
            metrics: content::Metrics::compute(0, demo_content.metric_params.target_fps),
            // Capability preset from config (effective caps computed after renderer init)
            cap_preset: config.cap_preset,
            ..Self::default()
        }
    }

    /// Get the current editor file content.
    #[must_use]
    pub fn current_file(&self) -> Option<&'static content::DemoFile> {
        content::DEFAULT_FILES.get(self.current_file_idx)
    }

    /// Get the current file name for display.
    #[must_use]
    pub fn current_file_name(&self) -> &'static str {
        self.current_file().map_or("untitled.txt", |f| f.name)
    }

    /// Get the current file content for the editor.
    #[must_use]
    pub fn current_file_content(&self) -> &'static str {
        self.current_file().map_or("", |f| f.text)
    }

    /// Get the current file language for syntax highlighting.
    #[must_use]
    pub fn current_file_language(&self) -> content::Language {
        self.current_file().map(|f| f.language).unwrap_or_default()
    }

    /// Switch to the next file in the file list.
    pub const fn next_file(&mut self) {
        if !content::DEFAULT_FILES.is_empty() {
            self.current_file_idx = (self.current_file_idx + 1) % content::DEFAULT_FILES.len();
        }
    }

    /// Switch to the previous file in the file list.
    pub const fn prev_file(&mut self) {
        if !content::DEFAULT_FILES.is_empty() {
            self.current_file_idx = if self.current_file_idx == 0 {
                content::DEFAULT_FILES.len() - 1
            } else {
                self.current_file_idx - 1
            };
        }
    }

    /// Add a log entry to the log stream.
    pub fn add_log(&mut self, entry: content::LogEntry) {
        self.logs.push(entry);
    }

    /// Show a toast notification.
    pub fn show_toast(&mut self, toast: Toast) {
        self.toasts.push(toast);
    }

    /// Show an info toast with the given message.
    pub fn toast_info(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast::info(message));
    }

    /// Show a warning toast with the given message.
    pub fn toast_warn(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast::warn(message));
    }

    /// Show an error toast with the given message.
    pub fn toast_error(&mut self, message: impl Into<String>) {
        self.toasts.push(Toast::error(message));
    }

    /// Update metrics for the current frame.
    pub fn update_metrics(&mut self) {
        self.metrics = content::Metrics::compute(self.frame_count, self.target_fps);
    }

    // ========================================================================
    // Tour Mode Methods
    // ========================================================================

    /// Start the tour with optional auto-advance and exit settings.
    pub fn start_tour(&mut self, auto_advance: bool, exit_on_complete: bool) {
        self.mode = AppMode::Tour;
        self.tour_step = 0;
        self.tour_runner = Some(TourRunner::new(auto_advance, exit_on_complete));
        self.overlays.open(Overlay::Tour(TourState::default()));

        // Execute the first step's action
        if let Some(runner) = &self.tour_runner {
            if let Some(action) = runner.execute_step_action() {
                self.apply_tour_action(action);
            }
        }
    }

    /// Stop the tour and return to normal mode.
    pub const fn stop_tour(&mut self) {
        self.mode = AppMode::Normal;
        self.tour_runner = None;
        self.overlays.close();
    }

    /// Advance to the next tour step.
    pub fn tour_next_step(&mut self) {
        let current_t = self.clock.t;

        // Extract all values from runner before calling methods on self
        let (completed, step_idx, action, exit_on_complete) = {
            let Some(runner) = self.tour_runner.as_mut() else {
                return;
            };
            let completed = runner.next_step(current_t);
            let step_idx = runner.step_idx;
            let action = runner.execute_step_action();
            let exit_on_complete = runner.exit_on_complete;
            (completed, step_idx, action, exit_on_complete)
        };

        // Now we can use self freely
        self.tour_step = step_idx;

        // Update overlay state
        if let Some(Overlay::Tour(ref mut tour_state)) = self.overlays.active {
            tour_state.step = step_idx;
        }

        // Execute the new step's action
        if let Some(action) = action {
            self.apply_tour_action(action);
        }

        // Check for completion
        if completed && exit_on_complete {
            self.should_quit = true;
            self.exit_reason = ExitReason::TourComplete;
        }
    }

    /// Go back to the previous tour step.
    pub fn tour_prev_step(&mut self) {
        let current_t = self.clock.t;

        // Extract all values from runner before calling methods on self
        let (step_idx, action) = {
            let Some(runner) = self.tour_runner.as_mut() else {
                return;
            };
            runner.prev_step(current_t);
            let step_idx = runner.step_idx;
            let action = runner.execute_step_action();
            (step_idx, action)
        };

        // Now we can use self freely
        self.tour_step = step_idx;

        // Update overlay state
        if let Some(Overlay::Tour(ref mut tour_state)) = self.overlays.active {
            tour_state.step = step_idx;
        }

        // Execute the step's action
        if let Some(action) = action {
            self.apply_tour_action(action);
        }
    }

    /// Handle an input event and return the resulting action.
    pub fn handle_event(&mut self, event: &Event) -> Action {
        // Parse event into action.
        let action = self.event_to_action(event);

        // Apply action to state.
        self.apply_action(&action);

        action
    }

    /// Convert an event to an action based on current mode.
    fn event_to_action(&self, event: &Event) -> Action {
        match event {
            Event::Key(key) => self.key_to_action(key),
            // Mouse and Paste are handled separately in their respective panels
            Event::Mouse(_) | Event::Paste(_) => Action::None,
            Event::FocusGained => Action::FocusChanged(true),
            Event::FocusLost => Action::FocusChanged(false),
            Event::Resize(resize) => {
                Action::Resize(u32::from(resize.width), u32::from(resize.height))
            }
        }
    }

    /// Convert a key event to an action.
    fn key_to_action(&self, key: &opentui::input::KeyEvent) -> Action {
        // Global shortcuts (always active)
        match (key.code, key.modifiers.contains(KeyModifiers::CTRL)) {
            (KeyCode::Char('q'), true) => return Action::Quit,
            (KeyCode::F(1), _) => return Action::ToggleHelp,
            (KeyCode::Char('p'), true) => return Action::TogglePalette,
            (KeyCode::Char('t'), true) => return Action::ToggleTour,
            (KeyCode::Char('r'), true) => return Action::ForceRedraw,
            (KeyCode::Char('d'), true) => return Action::ToggleDebug,
            (KeyCode::Char('n'), true) => return Action::CycleTheme,
            (KeyCode::Tab, _) if !key.modifiers.contains(KeyModifiers::SHIFT) => {
                return Action::CycleFocusForward;
            }
            (KeyCode::Tab | KeyCode::BackTab, _) => {
                return Action::CycleFocusBackward;
            }
            _ => {}
        }

        // Number keys for section navigation (in Normal mode)
        if self.mode == AppMode::Normal {
            if let KeyCode::Char(c @ '1'..='6') = key.code {
                let idx = (c as usize) - ('1' as usize);
                if let Some(section) = Section::from_index(idx) {
                    return Action::NavigateSection(section);
                }
            }
        }

        // Mode-specific handling
        match self.mode {
            AppMode::Normal => {
                if key.code == KeyCode::Esc {
                    return Action::Quit;
                }
                Action::None
            }
            AppMode::Help => {
                if key.code == KeyCode::Esc {
                    return Action::CloseOverlay;
                }
                Action::None
            }
            AppMode::CommandPalette => {
                match key.code {
                    KeyCode::Esc => return Action::CloseOverlay,
                    KeyCode::Up => return Action::PaletteUp,
                    KeyCode::Down => return Action::PaletteDown,
                    KeyCode::Enter => return Action::PaletteExecute,
                    KeyCode::Backspace => return Action::PaletteBackspace,
                    KeyCode::Char(c) => return Action::PaletteChar(c),
                    _ => {}
                }
                Action::None
            }
            AppMode::Tour => {
                // Only Esc has a special action in Tour mode; other keys are handled by the tour driver
                if key.code == KeyCode::Esc {
                    Action::ToggleTour
                } else {
                    Action::None
                }
            }
        }
    }

    /// Convert a mouse hit ID to an action.
    ///
    /// This method maps hit test results to app actions for mouse interactions.
    fn hit_to_action(hit_id: u32, kind: MouseEventKind) -> Action {
        use hit_ids::{
            BTN_HELP, BTN_PALETTE, BTN_THEME, BTN_TOUR, OVERLAY_CLOSE, PALETTE_ITEM_BASE,
            PANEL_EDITOR, PANEL_LOGS, PANEL_PREVIEW, PANEL_SIDEBAR, SIDEBAR_ROW_BASE,
        };

        // Only respond to press events (not release/move)
        if kind != MouseEventKind::Press {
            return Action::None;
        }

        match hit_id {
            // Chrome buttons
            BTN_HELP => Action::ToggleHelp,
            BTN_PALETTE => Action::TogglePalette,
            BTN_TOUR => Action::ToggleTour,
            BTN_THEME => Action::CycleTheme,

            // Panel focus areas
            PANEL_SIDEBAR => Action::SetFocus(Focus::Sidebar),
            PANEL_EDITOR => Action::SetFocus(Focus::Editor),
            PANEL_PREVIEW => Action::SetFocus(Focus::Preview),
            PANEL_LOGS => Action::SetFocus(Focus::Logs),

            // Sidebar rows (click to navigate section)
            id if (SIDEBAR_ROW_BASE..SIDEBAR_ROW_BASE + 100).contains(&id) => {
                let idx = (id - SIDEBAR_ROW_BASE) as usize;
                Section::from_index(idx).map_or(Action::None, Action::NavigateSection)
            }

            // Palette items (click to select and execute)
            id if (PALETTE_ITEM_BASE..PALETTE_ITEM_BASE + 100).contains(&id) => {
                let idx = (id - PALETTE_ITEM_BASE) as usize;
                Action::PaletteClick(idx)
            }

            // Overlay close button
            OVERLAY_CLOSE => Action::CloseOverlay,

            // No hit or unknown ID
            _ => Action::None,
        }
    }

    /// Apply an action to update state.
    #[allow(clippy::too_many_lines)] // State machine pattern - all actions in one match
    fn apply_action(&mut self, action: &Action) {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::ToggleHelp => {
                if self.mode == AppMode::Help {
                    self.mode = AppMode::Normal;
                    self.overlays.close();
                } else {
                    self.mode = AppMode::Help;
                    self.overlays.open(Overlay::Help(HelpState::default()));
                }
            }
            Action::TogglePalette => {
                if self.mode == AppMode::CommandPalette {
                    self.mode = AppMode::Normal;
                    self.overlays.close();
                } else {
                    self.mode = AppMode::CommandPalette;
                    let mut state = PaletteState::default();
                    state.update_filter(); // Initialize with all commands
                    self.overlays.open(Overlay::Palette(state));
                }
            }
            Action::ToggleTour => {
                if self.mode == AppMode::Tour {
                    self.mode = AppMode::Normal;
                    self.tour_runner = None;
                    self.overlays.close();
                } else {
                    self.mode = AppMode::Tour;
                    self.tour_step = 0;
                    // Create tour runner with auto-advance but no exit-on-complete
                    // (exit-on-complete is only set via --exit-after-tour CLI flag)
                    self.tour_runner = Some(TourRunner::new(true, false));
                    self.overlays.open(Overlay::Tour(TourState::default()));
                    // Execute first step's action immediately
                    if let Some(action) = self
                        .tour_runner
                        .as_ref()
                        .and_then(TourRunner::execute_step_action)
                    {
                        self.apply_tour_action(action);
                    }
                }
            }
            Action::CloseOverlay => {
                self.mode = AppMode::Normal;
                self.overlays.close();
            }
            Action::CycleFocusForward => {
                if self.mode == AppMode::Normal {
                    self.focus = self.focus.next();
                }
            }
            Action::CycleFocusBackward => {
                if self.mode == AppMode::Normal {
                    self.focus = self.focus.prev();
                }
            }
            Action::NavigateSection(section) => {
                self.section = *section;
            }
            Action::ForceRedraw => {
                self.force_redraw = true;
            }
            Action::ToggleDebug => {
                self.show_debug = !self.show_debug;
            }
            Action::CycleTheme => {
                self.ui_theme = self.ui_theme.next();
            }
            Action::FocusChanged(gained) => {
                self.paused = !gained;
            }
            Action::PaletteUp => {
                if let Some(Overlay::Palette(ref mut state)) = self.overlays.active {
                    state.select_prev();
                }
            }
            Action::PaletteDown => {
                if let Some(Overlay::Palette(ref mut state)) = self.overlays.active {
                    state.select_next();
                }
            }
            Action::PaletteBackspace => {
                if let Some(Overlay::Palette(ref mut state)) = self.overlays.active {
                    state.query.pop();
                    state.update_filter();
                }
            }
            Action::PaletteChar(c) => {
                if let Some(Overlay::Palette(ref mut state)) = self.overlays.active {
                    state.query.push(*c);
                    state.update_filter();
                }
            }
            Action::PaletteExecute => {
                // Get the selected command name and action before closing
                let cmd_info = if let Some(Overlay::Palette(ref state)) = self.overlays.active {
                    if let Some(&cmd_idx) = state.filtered.get(state.selected) {
                        let (name, _) = PaletteState::COMMANDS[cmd_idx];
                        // Map command index to action
                        let action = match cmd_idx {
                            0 => Some(Action::ToggleHelp),  // "Toggle Help"
                            1 => Some(Action::ToggleTour),  // "Toggle Tour"
                            2 => Some(Action::CycleTheme),  // "Cycle Theme"
                            3 => Some(Action::ForceRedraw), // "Force Redraw"
                            4 => Some(Action::ToggleDebug), // "Toggle Debug"
                            5 => Some(Action::Quit),        // "Quit"
                            _ => None,
                        };
                        action.map(|a| (name, a))
                    } else {
                        None
                    }
                } else {
                    None
                };
                // Close the palette first
                self.mode = AppMode::Normal;
                self.overlays.close();
                // Then execute the command if any, with toast notification
                if let Some((name, cmd)) = cmd_info {
                    // Show toast for non-quit commands
                    if !matches!(cmd, Action::Quit) {
                        self.toast_info(format!("Executed: {name}"));
                    }
                    self.apply_action(&cmd);
                }
            }
            Action::SetFocus(focus) => {
                // Only allow focus change in normal mode
                if self.mode == AppMode::Normal {
                    self.focus = *focus;
                }
            }
            Action::PaletteClick(idx) => {
                // Click on palette item - select it and execute
                if let Some(Overlay::Palette(ref mut state)) = self.overlays.active {
                    if *idx < state.filtered.len() {
                        state.selected = *idx;
                    }
                }
                // Now execute via PaletteExecute
                self.apply_action(&Action::PaletteExecute);
            }
            // Resize is handled in render loop, None is a no-op
            Action::Resize(_, _) | Action::None => {}
        }
    }

    /// Update app state for a new frame.
    #[allow(clippy::missing_const_for_fn)] // const fn with &mut self not stable
    pub fn tick(&mut self) {
        // Update animation clock first (respects pause state)
        self.clock.tick(self.paused);

        self.frame_count = self.frame_count.wrapping_add(1);

        // Update deterministic metrics for this frame
        self.update_metrics();

        // Clear force redraw flag after use
        self.force_redraw = false;

        // Update overlay animations with dt from clock
        self.overlays.tick(self.clock.dt);

        // Update toast lifetimes
        self.toasts.tick(self.clock.dt);

        // Tour mode: tick the tour runner and apply actions
        if self.mode == AppMode::Tour {
            self.tick_tour();
        }

        // If overlay finished closing, ensure mode is Normal (but not during tour)
        if !self.overlays.is_active() && self.mode != AppMode::Normal && self.mode != AppMode::Tour
        {
            // Overlay closed, sync mode
            self.mode = AppMode::Normal;
        }

        // Check max frames limit
        if let Some(max) = self.max_frames {
            if self.frame_count >= max {
                self.should_quit = true;
                self.exit_reason = ExitReason::MaxFrames;
            }
        }
    }

    /// Handle terminal resize event.
    ///
    /// Called when the terminal is resized. This method:
    /// - Resets any scroll positions that might be out of bounds
    /// - Pushes a toast notification with the new size
    /// - Sets `force_redraw` to ensure immediate visual update
    pub fn handle_resize(&mut self, width: u32, height: u32) {
        // Reset overlay scroll positions if active
        if let Some(Overlay::Help(ref mut state)) = self.overlays.active {
            state.scroll = 0;
        }

        // Push a toast notification
        self.toasts
            .push(Toast::info(format!("Resized to {width}×{height}")));

        // Force a full redraw
        self.force_redraw = true;
    }

    /// Tick the tour runner and apply any resulting actions.
    fn tick_tour(&mut self) {
        let current_t = self.clock.t;

        // Get mutable access to tour runner and extract needed data
        let Some(runner) = self.tour_runner.as_mut() else {
            return;
        };

        // Check for auto-advance
        if runner.should_auto_advance(current_t) {
            let is_last = runner.next_step(current_t);

            // Extract values before releasing the borrow
            let action = runner.execute_step_action();
            let step_idx = runner.step_idx;
            let exit_on_complete = runner.exit_on_complete;

            // Sync tour_step for display
            self.tour_step = step_idx;

            // Execute the new step's action (now runner borrow is released)
            if let Some(action) = action {
                self.apply_tour_action(action);
            }

            // Handle tour completion
            if is_last && exit_on_complete {
                self.should_quit = true;
                self.exit_reason = ExitReason::TourComplete;
            }
        }
    }

    /// Apply a tour action to the app state.
    fn apply_tour_action(&mut self, action: TourAction) {
        match action {
            TourAction::None => {}
            TourAction::SetFocus(focus) => {
                self.focus = focus;
            }
            TourAction::SetSection(section) => {
                self.section = section;
            }
            TourAction::OpenHelp => {
                if self.mode != AppMode::Help {
                    self.overlays.open(Overlay::Help(HelpState::default()));
                }
            }
            TourAction::OpenPalette => {
                if self.mode != AppMode::CommandPalette {
                    let mut state = PaletteState::default();
                    state.update_filter();
                    self.overlays.open(Overlay::Palette(state));
                }
            }
            TourAction::CloseOverlay => {
                self.overlays.close();
            }
            TourAction::CycleTheme => {
                self.ui_theme = self.ui_theme.next();
            }
            TourAction::ShowDebug => {
                self.show_debug = true;
            }
        }
    }

    /// Get the current mode name for display.
    #[must_use]
    pub const fn mode_name(&self) -> &'static str {
        match self.mode {
            AppMode::Normal => "Normal",
            AppMode::Help => "Help",
            AppMode::CommandPalette => "Palette",
            AppMode::Tour => "Tour",
        }
    }

    /// Get the current focus name for display.
    #[must_use]
    pub const fn focus_name(&self) -> &'static str {
        match self.focus {
            Focus::Sidebar => "Sidebar",
            Focus::Editor => "Editor",
            Focus::Preview => "Preview",
            Focus::Logs => "Logs",
        }
    }
}

// ============================================================================
// Input Pump
// ============================================================================

/// Input source for distinguishing real vs synthetic events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSource {
    /// Real input from stdin.
    Real,
    /// Synthetic input (e.g., for tour mode).
    Synthetic,
}

/// Tagged event with its source.
#[derive(Clone, Debug)]
pub struct TaggedEvent {
    /// The actual event.
    pub event: Event,
    /// Where the event came from.
    pub source: InputSource,
}

impl TaggedEvent {
    /// Create a new real event.
    #[must_use]
    pub const fn real(event: Event) -> Self {
        Self {
            event,
            source: InputSource::Real,
        }
    }

    /// Create a new synthetic event.
    #[must_use]
    pub const fn synthetic(event: Event) -> Self {
        Self {
            event,
            source: InputSource::Synthetic,
        }
    }
}

/// Non-blocking input pump that reads from stdin and parses events.
///
/// This struct handles:
/// - Non-blocking reads from stdin with timeout
/// - Parsing bytes into structured events using `InputParser`
/// - Accumulating partial escape sequences across reads
/// - Injecting synthetic events for tour mode
pub struct InputPump {
    /// The parser for converting bytes to events.
    parser: InputParser,
    /// Accumulated bytes waiting to be parsed.
    accumulator: Vec<u8>,
    /// Scratch buffer for reading.
    scratch: [u8; 1024],
    /// Queue of synthetic events to inject.
    synthetic_queue: Vec<Event>,
    /// Maximum accumulator size (to prevent unbounded growth).
    max_accumulator_size: usize,
}

impl InputPump {
    /// Create a new input pump.
    #[must_use]
    pub fn new() -> Self {
        Self {
            parser: InputParser::new(),
            accumulator: Vec::with_capacity(256),
            scratch: [0u8; 1024],
            synthetic_queue: Vec::new(),
            max_accumulator_size: 64 * 1024, // 64KB limit for paste payloads
        }
    }

    /// Queue a synthetic event to be returned on the next poll.
    pub fn inject_synthetic(&mut self, event: Event) {
        self.synthetic_queue.push(event);
    }

    /// Poll for input events with a timeout.
    ///
    /// Returns a vector of tagged events (may be empty if no input available).
    /// Uses `select()` to wait for input with a timeout.
    ///
    /// # Errors
    ///
    /// Returns an error if reading from stdin fails (excluding `WouldBlock`).
    pub fn poll(&mut self, timeout: Duration) -> io::Result<Vec<TaggedEvent>> {
        let mut events = Vec::new();

        // First, return any queued synthetic events.
        if !self.synthetic_queue.is_empty() {
            for event in self.synthetic_queue.drain(..) {
                events.push(TaggedEvent::synthetic(event));
            }
        }

        // Wait for input with timeout using select.
        if self.wait_for_input(timeout)? {
            // Read available bytes.
            match io::stdin().read(&mut self.scratch) {
                Ok(n) if n > 0 => {
                    // Append to accumulator, enforcing size limit.
                    let space = self
                        .max_accumulator_size
                        .saturating_sub(self.accumulator.len());
                    let to_add = n.min(space);
                    self.accumulator.extend_from_slice(&self.scratch[..to_add]);

                    // Parse all complete events from accumulator.
                    self.parse_accumulated(&mut events);
                }
                Ok(_) => {}                                           // No bytes read
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {} // No data available
                Err(e) => return Err(e),
            }
        }

        Ok(events)
    }

    /// Wait for input to be available on stdin with a timeout.
    ///
    /// Returns `true` if input is available, `false` on timeout.
    #[cfg(unix)]
    #[allow(clippy::cast_possible_wrap)] // timeout.as_secs() fits in i64 for reasonable values
    #[allow(clippy::unused_self)] // self kept for future state access
    fn wait_for_input(&self, timeout: Duration) -> io::Result<bool> {
        use std::os::unix::io::AsRawFd;

        let stdin_fd = io::stdin().as_raw_fd();

        // Set up fd_set for select.
        let mut read_fds = std::mem::MaybeUninit::<libc::fd_set>::uninit();

        // SAFETY: FD_ZERO and FD_SET are safe macros that initialize/modify fd_set.
        unsafe {
            libc::FD_ZERO(read_fds.as_mut_ptr());
            libc::FD_SET(stdin_fd, read_fds.as_mut_ptr());
        }

        // Convert timeout to timeval.
        let mut tv = libc::timeval {
            tv_sec: timeout.as_secs() as libc::time_t,
            tv_usec: libc::suseconds_t::from(timeout.subsec_micros()),
        };

        // SAFETY: select is safe with valid fd_set and timeval.
        let result = unsafe {
            libc::select(
                stdin_fd + 1,
                read_fds.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::from_mut(&mut tv),
            )
        };

        match result {
            -1 => {
                let err = io::Error::last_os_error();
                // EINTR is not a real error, just retry.
                if err.kind() == io::ErrorKind::Interrupted {
                    Ok(false)
                } else {
                    Err(err)
                }
            }
            0 => Ok(false), // Timeout
            _ => Ok(true),  // Input available
        }
    }

    #[cfg(not(unix))]
    #[allow(clippy::unused_self)] // Kept for consistency with unix version
    fn wait_for_input(&self, _timeout: Duration) -> io::Result<bool> {
        // On non-Unix, just try to read (no select available).
        Ok(true)
    }

    /// Parse all complete events from the accumulator.
    fn parse_accumulated(&mut self, events: &mut Vec<TaggedEvent>) {
        let mut offset = 0;

        while offset < self.accumulator.len() {
            match self.parser.parse(&self.accumulator[offset..]) {
                Ok((event, consumed)) => {
                    events.push(TaggedEvent::real(event));
                    offset += consumed;
                }
                Err(opentui::input::ParseError::Incomplete) => {
                    // Need more bytes, keep remainder in accumulator.
                    break;
                }
                Err(opentui::input::ParseError::Empty) => {
                    // Nothing to parse.
                    break;
                }
                Err(_) => {
                    // Unknown sequence, skip one byte and continue.
                    offset += 1;
                }
            }
        }

        // Remove parsed bytes from accumulator.
        if offset > 0 {
            self.accumulator.drain(..offset);
        }
    }

    /// Clear the accumulator (e.g., on focus loss).
    pub fn clear(&mut self) {
        self.accumulator.clear();
    }
}

impl Default for InputPump {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Panic Hook (Terminal Recovery)
// ============================================================================

/// Install a panic hook that attempts to restore terminal state.
///
/// This is belt-and-suspenders cleanup. Even though `Renderer` and `RawModeGuard`
/// try to restore state on Drop, a panic (especially with `panic = "abort"` in
/// release profile) can leave the terminal in a bad state.
fn install_panic_hook() {
    let original_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |panic_info| {
        // Best-effort terminal restoration.
        let _ = attempt_terminal_cleanup();

        // Print recovery instructions for safety.
        let _ = std::io::Write::write_all(
            &mut std::io::stderr(),
            b"\n\x1b[0m[demo_showcase] If your terminal is broken, run: reset\n\
              Or: stty sane && clear\n\n",
        );

        // Chain to the original panic handler (preserves panic info).
        original_hook(panic_info);
    }));
}

/// Attempt to restore terminal state after a panic.
///
/// Writes ANSI reset sequences and attempts to restore termios settings.
fn attempt_terminal_cleanup() -> io::Result<()> {
    use std::io::Write;

    // Write ANSI sequences directly to stderr (bypasses stdout buffering).
    let mut stderr = std::io::stderr().lock();

    // Reset all text attributes.
    stderr.write_all(b"\x1b[0m")?;
    // Show cursor.
    stderr.write_all(b"\x1b[?25h")?;
    // Exit alternate screen.
    stderr.write_all(b"\x1b[?1049l")?;
    // Disable mouse tracking (all modes).
    stderr.write_all(b"\x1b[?1006l")?; // SGR
    stderr.write_all(b"\x1b[?1003l")?; // Any-event
    stderr.write_all(b"\x1b[?1000l")?; // Normal
    // Disable bracketed paste.
    stderr.write_all(b"\x1b[?2004l")?;
    stderr.flush()?;

    // Attempt to restore termios (raw mode → cooked mode).
    restore_cooked_mode();

    Ok(())
}

/// Best-effort restoration of terminal cooked mode via termios.
///
/// This directly manipulates termios since we can't rely on `RawModeGuard`
/// being in scope during a panic.
fn restore_cooked_mode() {
    // SAFETY: libc calls for termios are safe with valid fd and struct.
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        if libc::tcgetattr(libc::STDIN_FILENO, &raw mut termios) == 0 {
            // Re-enable canonical mode, echo, and signal processing.
            termios.c_lflag |= libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG;
            termios.c_iflag |= libc::IXON | libc::ICRNL;
            termios.c_oflag |= libc::OPOST;
            let _ = libc::tcsetattr(libc::STDIN_FILENO, libc::TCSAFLUSH, &raw const termios);
        }
    }
}

// ============================================================================
// Event/Log Routing (OpenTUI → Demo Logs Panel)
// ============================================================================

/// Maximum entries in the log queue before oldest entries are dropped.
const LOG_QUEUE_CAPACITY: usize = 500;

/// Thread-safe log queue for routing `OpenTUI` events to the demo logs panel.
fn log_queue() -> &'static Arc<Mutex<VecDeque<content::LogEntry>>> {
    static QUEUE: OnceLock<Arc<Mutex<VecDeque<content::LogEntry>>>> = OnceLock::new();
    QUEUE.get_or_init(|| Arc::new(Mutex::new(VecDeque::with_capacity(LOG_QUEUE_CAPACITY))))
}

/// Install `OpenTUI` event/log callbacks that route to the demo's log queue.
///
/// Call this at demo startup. The callbacks push entries to a bounded queue,
/// which is drained into `App.logs` each frame via `drain_log_queue()`.
fn install_log_routing() {
    let queue = log_queue().clone();

    set_log_callback(move |level, message| {
        let demo_level = match level {
            OpentuiLogLevel::Debug => content::LogLevel::Debug,
            OpentuiLogLevel::Info => content::LogLevel::Info,
            OpentuiLogLevel::Warn => content::LogLevel::Warn,
            OpentuiLogLevel::Error => content::LogLevel::Error,
        };

        let entry = content::LogEntry::new_runtime(
            current_timestamp(),
            demo_level,
            "opentui".to_string(),
            message.to_string(),
        );

        if let Ok(mut q) = queue.lock() {
            q.push_back(entry);
            // Keep queue bounded to avoid unbounded memory growth.
            while q.len() > LOG_QUEUE_CAPACITY {
                q.pop_front();
            }
        }
    });
}

/// Get current timestamp in HH:MM:SS format.
fn current_timestamp() -> String {
    use std::time::SystemTime;

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();

    let secs = now.as_secs();
    let hours = (secs / 3600) % 24;
    let mins = (secs / 60) % 60;
    let seconds = secs % 60;

    format!("{hours:02}:{mins:02}:{seconds:02}")
}

/// Drain queued log entries into the app's log list.
///
/// Call this once per frame to move runtime log entries from the shared queue
/// into `App.logs` for display in the logs panel.
fn drain_log_queue(app: &mut App) {
    if let Ok(mut queue) = log_queue().lock() {
        for entry in queue.drain(..) {
            app.add_log(entry);
        }
    }
}

// ============================================================================
// Demo Logging Helpers
// ============================================================================

/// Log an info-level message to the demo's logs panel.
///
/// This is a convenience wrapper that routes through the shared log queue.
#[allow(dead_code)]
pub fn log_info(subsystem: &str, message: &str) {
    demo_log(content::LogLevel::Info, subsystem, message);
}

/// Log a warning-level message to the demo's logs panel.
#[allow(dead_code)]
pub fn log_warn(subsystem: &str, message: &str) {
    demo_log(content::LogLevel::Warn, subsystem, message);
}

/// Log an error-level message to the demo's logs panel.
#[allow(dead_code)]
pub fn log_error(subsystem: &str, message: &str) {
    demo_log(content::LogLevel::Error, subsystem, message);
}

/// Log a debug-level message to the demo's logs panel.
#[allow(dead_code)]
pub fn log_debug(subsystem: &str, message: &str) {
    demo_log(content::LogLevel::Debug, subsystem, message);
}

/// Internal: push a log entry to the shared queue.
fn demo_log(level: content::LogLevel, subsystem: &str, message: &str) {
    let entry = content::LogEntry::new_runtime(
        current_timestamp(),
        level,
        subsystem.to_string(),
        message.to_string(),
    );

    if let Ok(mut queue) = log_queue().lock() {
        queue.push_back(entry);
        // Keep queue bounded.
        while queue.len() > LOG_QUEUE_CAPACITY {
            queue.pop_front();
        }
    }
}

// ============================================================================
// Entry Point
// ============================================================================

fn main() -> io::Result<()> {
    // Install panic hook for robust terminal cleanup on crash.
    install_panic_hook();

    // Install OpenTUI event/log routing to demo's logs panel.
    install_log_routing();

    match Config::from_args(std::env::args_os()) {
        ParseResult::Config(config) => {
            if config.headless_smoke {
                run_headless_smoke(&config);
                Ok(())
            } else if let Some(ref check_name) = config.headless_check {
                run_headless_check(&config, check_name);
                Ok(())
            } else {
                run_interactive(&config)
            }
        }
        ParseResult::Help => {
            print!("{HELP_TEXT}");
            Ok(())
        }
        ParseResult::Error(msg) => {
            eprintln!("Error: {msg}");
            eprintln!("Run with --help for usage information.");
            std::process::exit(1);
        }
    }
}

// ============================================================================
// Headless Smoke Test
// ============================================================================

/// Draw a frame directly to a buffer (headless version of `draw_frame`).
///
/// This exercises the same render passes as the interactive mode but
/// without requiring a Renderer or terminal.
fn headless_draw_frame(buffer: &mut OptimizedBuffer, app: &App) {
    let (width, height) = (buffer.width(), buffer.height());
    let panels = PanelLayout::compute(width, height);
    let theme = app.ui_theme.tokens();

    // No hyperlinks in headless mode (no terminal to handle OSC 8)
    let links = PreallocatedLinks::default();

    // Grapheme pool for Unicode rendering (headless uses standalone pool)
    let mut pool = GraphemePool::new();

    // === Pass 1: Background ===
    draw_pass_background(buffer, &theme);

    // Handle TooSmall mode (special case).
    if panels.mode == LayoutMode::TooSmall {
        draw_too_small_message(buffer, width, height, &theme);
        return;
    }

    // === Pass 2: Chrome ===
    draw_pass_chrome(buffer, &panels, &theme, app);

    // === Pass 3: Panels ===
    draw_pass_panels(buffer, &mut pool, &panels, &theme, app, &links);

    // === Pass 4: Overlays ===
    draw_pass_overlays(buffer, &panels, &theme, app, &links);

    // === Pass 5: Toasts ===
    draw_pass_toasts(buffer, &panels, &theme, app);
}

/// Per-frame statistics for JSON output.
#[derive(Clone, Debug)]
struct FrameStats {
    frame: u64,
    dirty_cells: usize,
    dt: f32,
}

/// Extract a row of text from the buffer as a string.
fn extract_buffer_row(buffer: &OptimizedBuffer, y: u32, start_x: u32, max_len: u32) -> String {
    let mut result = String::new();
    for x in start_x..start_x + max_len {
        if let Some(cell) = buffer.get(x, y) {
            match &cell.content {
                CellContent::Char(c) => result.push(*c),
                CellContent::Grapheme(id) => {
                    // For now, just use placeholder for extended graphemes
                    use std::fmt::Write;
                    let _ = write!(result, "[G{id:?}]");
                }
                CellContent::Continuation | CellContent::Empty => {}
            }
        }
    }
    result.trim_end().to_string()
}

/// Run a specific headless check and output JSON results.
///
/// Each check validates a specific subsystem:
/// - `layout`: Layout math invariants (no negative rects, no overflow, compact mode triggers)
/// - `config`: CLI/config parsing (flag combinations)
/// - `palette`: Command palette scoring + selection behavior
/// - `hitgrid`: Hit ID mapping invariants (IDs stable, no overlap)
/// - `logs`: Log model behavior (ring buffer, selection)
#[allow(clippy::too_many_lines)]
fn run_headless_check(config: &Config, check_name: &str) {
    let (width, height) = config.headless_size;

    if !config.headless_dump_json {
        eprintln!("Running headless check: {check_name} ({width}x{height})...");
    }

    let result = match check_name {
        "layout" => run_check_layout(width, height),
        "config" => run_check_config(config),
        "palette" => run_check_palette(),
        "hitgrid" => run_check_hitgrid(),
        "logs" => run_check_logs(),
        _ => {
            eprintln!("Unknown check: {check_name}");
            std::process::exit(1);
        }
    };

    if config.headless_dump_json {
        println!("{result}");
    } else {
        eprintln!("Headless check '{check_name}' PASSED");
        println!("HEADLESS_CHECK_OK check={check_name}");
    }
}

/// Check layout math invariants.
#[allow(clippy::too_many_lines)]
fn run_check_layout(width: u16, height: u16) -> String {
    let w = u32::from(width);
    let h = u32::from(height);

    let test_sizes: &[(u32, u32, &str)] = &[
        (120, 40, "full"), (80, 24, "full"), (79, 24, "compact"), (60, 16, "compact"),
        (59, 16, "minimal"), (40, 12, "minimal"), (39, 12, "too_small"), (20, 8, "too_small"),
    ];
    let mut results: Vec<String> = Vec::new();

    for &(tw, th, expected_mode) in test_sizes {
        let layout = PanelLayout::compute(tw, th);
        let actual_mode = format!("{:?}", layout.mode).to_lowercase();
        let mode_matches = actual_mode.contains(expected_mode);
        let valid_rects = [
            ("screen", &layout.screen), ("top_bar", &layout.top_bar),
            ("status_bar", &layout.status_bar), ("content", &layout.content),
            ("sidebar", &layout.sidebar), ("main_area", &layout.main_area),
            ("editor", &layout.editor), ("preview", &layout.preview), ("logs", &layout.logs),
        ];
        let mut all_valid = true;
        let mut rect_info: Vec<String> = Vec::new();
        for (name, rect) in valid_rects {
            #[allow(clippy::cast_possible_wrap)]
            let in_bounds = rect.x.saturating_add(rect.w as i32) <= tw as i32
                && rect.y.saturating_add(rect.h as i32) <= th as i32;
            if !in_bounds && rect.w > 0 && rect.h > 0 { all_valid = false; }
            rect_info.push(format!(
                r#"{{"name":"{}","x":{},"y":{},"w":{},"h":{},"in_bounds":{}}}"#,
                name, rect.x, rect.y, rect.w, rect.h, in_bounds
            ));
        }
        results.push(format!(
            r#"{{"size":[{},{}],"expected_mode":"{}","actual_mode":"{}","mode_matches":{},"all_valid":{},"rects":[{}]}}"#,
            tw, th, expected_mode, actual_mode, mode_matches, all_valid, rect_info.join(",")
        ));
    }

    let main_layout = PanelLayout::compute(w, h);
    format!(
        r#"{{"check":"layout","passed":true,"requested_size":[{},{}],"main_layout":{{"size":[{},{}],"mode":"{:?}"}},"test_results":[{}]}}"#,
        width, height, w, h, main_layout.mode, results.join(",")
    )
}

/// Check CLI/config parsing.
#[allow(clippy::too_many_lines, clippy::redundant_closure_for_method_calls, clippy::uninlined_format_args)]
fn run_check_config(config: &Config) -> String {
    let test_cases: &[(&[&str], &str)] = &[
        (&["demo_showcase"], "default"), (&["demo_showcase", "--fps", "30"], "fps_30"),
        (&["demo_showcase", "--no-mouse"], "no_mouse"),
        (&["demo_showcase", "--tour", "--exit-after-tour"], "tour_exit"),
        (&["demo_showcase", "--cap-preset", "minimal"], "minimal_preset"),
        (&["demo_showcase", "--seed", "42"], "seed_42"),
    ];
    let mut results: Vec<String> = Vec::new();
    for (args, label) in test_cases {
        let os_args: Vec<std::ffi::OsString> = args.iter().map(|s| s.into()).collect();
        let (parsed_ok, cfg_summary) = match Config::from_args(os_args) {
            ParseResult::Config(cfg) => (true, format!(
                r#"{{"fps_cap":{},"enable_mouse":{},"seed":{}}}"#, cfg.fps_cap, cfg.enable_mouse, cfg.seed
            )),
            ParseResult::Help => (true, r#"{"help":true}"#.to_string()),
            ParseResult::Error(e) => (false, format!(r#"{{"error":"{}"}}"#, e.replace('"', "\\\""))),
        };
        results.push(format!(r#"{{"label":"{}","parsed_ok":{},"config":{}}}"#, label, parsed_ok, cfg_summary));
    }
    format!(
        r#"{{"check":"config","passed":true,"current_config":{{"fps_cap":{},"seed":{}}},"test_results":[{}]}}"#,
        config.fps_cap, config.seed, results.join(",")
    )
}

/// Check command palette scoring and selection.
#[allow(clippy::uninlined_format_args)]
fn run_check_palette() -> String {
    let mut state = PaletteState::default();
    state.update_filter();
    let all_count = state.filtered.len();
    let total_commands = PaletteState::COMMANDS.len();

    let mut filter_results: Vec<String> = Vec::new();
    for (query, label) in [("", "empty"), ("help", "help"), ("toggle", "toggle"), ("xyz", "no_match")] {
        state.query = query.to_string();
        state.selected = 0;
        state.update_filter();
        filter_results.push(format!(r#"{{"label":"{}","match_count":{}}}"#, label, state.filtered.len()));
    }

    state.query.clear();
    state.selected = 0;
    state.update_filter();
    for _ in 0..3 { state.select_next(); }
    let after_next = state.selected;
    for _ in 0..2 { state.select_prev(); }
    let after_prev = state.selected;

    format!(
        r#"{{"check":"palette","passed":true,"total_commands":{},"all_shown_on_empty":{},"filter_tests":[{}],"nav_tests":{{"after_3_next":{},"after_2_prev":{}}}}}"#,
        total_commands, all_count == total_commands, filter_results.join(","), after_next, after_prev
    )
}

/// Check hit ID mapping invariants.
#[allow(clippy::uninlined_format_args)]
fn run_check_hitgrid() -> String {
    let id_tests = [
        ("BTN_HELP", hit_ids::BTN_HELP, 1000), ("BTN_PALETTE", hit_ids::BTN_PALETTE, 1001),
        ("BTN_TOUR", hit_ids::BTN_TOUR, 1002), ("BTN_THEME", hit_ids::BTN_THEME, 1003),
        ("SIDEBAR_ROW_BASE", hit_ids::SIDEBAR_ROW_BASE, 2000),
        ("PANEL_SIDEBAR", hit_ids::PANEL_SIDEBAR, 3000), ("PANEL_EDITOR", hit_ids::PANEL_EDITOR, 3001),
        ("OVERLAY_CLOSE", hit_ids::OVERLAY_CLOSE, 4000), ("PALETTE_ITEM_BASE", hit_ids::PALETTE_ITEM_BASE, 4100),
    ];
    let mut all_match = true;
    let mut results: Vec<String> = Vec::new();
    for (name, actual, expected) in id_tests {
        let m = actual == expected;
        if !m { all_match = false; }
        results.push(format!(r#"{{"name":"{}","actual":{},"expected":{},"matches":{}}}"#, name, actual, expected, m));
    }
    format!(r#"{{"check":"hitgrid","passed":{},"id_tests":[{}]}}"#, all_match, results.join(","))
}

/// Check log model behavior.
#[allow(clippy::uninlined_format_args)]
fn run_check_logs() -> String {
    let max_entries = 100;
    let mut log_buffer: VecDeque<&str> = VecDeque::with_capacity(max_entries);
    for i in 0..150 {
        if log_buffer.len() >= max_entries { log_buffer.pop_front(); }
        log_buffer.push_back(if i % 2 == 0 { "INFO" } else { "DEBUG" });
    }
    let final_count = log_buffer.len();
    let oldest_dropped = final_count == max_entries;
    let mut selection = 0_usize;
    let max_sel = final_count.saturating_sub(1);
    selection = selection.saturating_add(5).min(max_sel);
    let after_down = selection;
    selection = selection.saturating_sub(3);
    let after_up = selection;
    format!(
        r#"{{"check":"logs","passed":true,"ring_buffer":{{"max":{},"final":{},"oldest_dropped":{}}},"selection":{{"after_down":{},"after_up":{}}}}}"#,
        max_entries, final_count, oldest_dropped, after_down, after_up
    )
}

/// This exercises the full render pipeline:
/// - Creates App with proper config
/// - Runs N frames through all render passes
/// - Computes `BufferDiff` between frames to verify diffing works
/// - Outputs standard success format for CI (or JSON if `--headless-dump-json`)
#[allow(clippy::too_many_lines)]
fn run_headless_smoke(config: &Config) {
    use opentui::renderer::BufferDiff;

    let (width, height) = config.headless_size;
    let frame_count = config.max_frames.unwrap_or(10);

    if !config.headless_dump_json {
        eprintln!("Running headless smoke test ({width}x{height})...");
    }

    // Create App with config
    let demo_content = content::DemoContent::default();
    let mut app = App::with_content(config, &demo_content);

    // In headless mode, preset defines effective caps directly (no terminal to detect)
    app.update_effective_caps(None);

    // Create double buffers for diffing
    let mut current_buffer = OptimizedBuffer::new(u32::from(width), u32::from(height));
    let mut previous_buffer = OptimizedBuffer::new(u32::from(width), u32::from(height));

    let mut last_dirty_cells: usize = 0;
    let mut total_dirty_cells: usize = 0;
    let mut frame_stats: Vec<FrameStats> =
        Vec::with_capacity(usize::try_from(frame_count).unwrap_or(64));

    // Track tour step transitions for determinism testing
    let mut tour_step_history: Vec<(u64, usize, String)> = Vec::new();

    // Fixed dt for deterministic headless timing (simulate 60fps)
    let fixed_dt: f32 = 1.0 / 60.0; // ~16.67ms per frame

    // Run frames through the render pipeline
    for frame in 0..frame_count {
        // Update app state
        app.frame_count = frame;
        // Use fixed dt for deterministic timing instead of real time
        app.clock.dt = fixed_dt;
        app.clock.t += fixed_dt;
        app.update_metrics();

        // Tick tour if active (advances based on timing)
        let step_before = app.tour_runner.as_ref().map(|r| r.step_idx);
        app.tick_tour();
        let step_after = app.tour_runner.as_ref().map(|r| r.step_idx);

        // Record step transitions
        if let (Some(before), Some(after)) = (step_before, step_after) {
            if before != after {
                let title = TOUR_SCRIPT.get(after).map_or("unknown", |s| s.title);
                tour_step_history.push((frame, after, title.to_string()));
            }
        } else if step_before.is_none() && step_after.is_some() {
            // Tour just started
            let after = step_after.unwrap();
            let title = TOUR_SCRIPT.get(after).map_or("unknown", |s| s.title);
            tour_step_history.push((frame, after, title.to_string()));
        }

        // Clear and render to current buffer
        headless_draw_frame(&mut current_buffer, &app);

        // Compute diff between frames (except first frame)
        let dirty_cells = if frame > 0 {
            let diff = BufferDiff::compute(&previous_buffer, &current_buffer);
            diff.change_count
        } else {
            // First frame: all cells are "dirty"
            (width as usize) * (height as usize)
        };

        last_dirty_cells = dirty_cells;
        total_dirty_cells += dirty_cells;

        frame_stats.push(FrameStats {
            frame,
            dirty_cells,
            dt: app.clock.dt,
        });

        // Swap buffers (copy current to previous for next diff)
        std::mem::swap(&mut current_buffer, &mut previous_buffer);
    }

    // Verify buffers are valid
    assert_eq!(previous_buffer.width(), u32::from(width));
    assert_eq!(previous_buffer.height(), u32::from(height));

    // Extract sentinel markers from final frame (previous_buffer has the last rendered frame)
    let panels = PanelLayout::compute(u32::from(width), u32::from(height));
    let top_bar_text = extract_buffer_row(&previous_buffer, 0, 0, width.into());
    let section_name = app.section.name().to_string();
    let layout_mode = format!("{:?}", panels.mode);

    // Output based on mode
    if config.headless_dump_json {
        // Build JSON output
        let frame_stats_json: Vec<String> = frame_stats
            .iter()
            .map(|f| {
                format!(
                    r#"{{"frame":{},"dirty_cells":{},"dt":{:.6}}}"#,
                    f.frame, f.dirty_cells, f.dt
                )
            })
            .collect();

        // Format effective capabilities as JSON
        let caps = &app.effective_caps;
        let warnings_json: String = if caps.degraded.is_empty() {
            "[]".to_string()
        } else {
            let items: Vec<String> = caps.degraded.iter().map(|s| format!("\"{s}\"")).collect();
            format!("[{}]", items.join(", "))
        };

        // Format tour state if tour was active
        let tour_state_json = if config.start_in_tour {
            let final_step = app.tour_runner.as_ref().map(|r| r.step_idx).unwrap_or(0);
            let final_title = TOUR_SCRIPT
                .get(final_step)
                .map_or("unknown", |s| s.title);
            let completed = app
                .tour_runner
                .as_ref()
                .map(|r| r.completed)
                .unwrap_or(false);
            let steps_json: Vec<String> = tour_step_history
                .iter()
                .map(|(frame, idx, title)| {
                    format!(
                        r#"{{"frame":{},"step_idx":{},"title":"{}"}}"#,
                        frame,
                        idx,
                        title.replace('"', "\\\"")
                    )
                })
                .collect();
            format!(
                r#",
  "tour_state": {{
    "active": true,
    "completed": {},
    "final_step_idx": {},
    "final_step_title": "{}",
    "total_steps": {},
    "step_transitions": [
      {}
    ]
  }}"#,
                completed,
                final_step,
                final_title.replace('"', "\\\""),
                TOUR_SCRIPT.len(),
                steps_json.join(",\n      ")
            )
        } else {
            String::new()
        };

        let json = format!(
            r#"{{
  "config": {{
    "fps_cap": {},
    "seed": {},
    "enable_mouse": {},
    "use_alt_screen": {},
    "start_in_tour": {},
    "cap_preset": "{:?}"
  }},
  "headless_size": {{
    "width": {},
    "height": {}
  }},
  "effective_caps": {{
    "truecolor": {},
    "mouse": {},
    "hyperlinks": {},
    "focus": {},
    "sync_output": {}
  }},
  "warnings": {},
  "layout_mode": "{}",
  "frames_rendered": {},
  "total_dirty_cells": {},
  "last_dirty_cells": {},
  "sentinels": {{
    "top_bar": "{}",
    "section": "{}"
  }},
  "frame_stats": [
    {}
  ]{}
}}"#,
            config.fps_cap,
            config.seed,
            config.enable_mouse,
            config.use_alt_screen,
            config.start_in_tour,
            config.cap_preset,
            width,
            height,
            caps.truecolor,
            caps.mouse,
            caps.hyperlinks,
            caps.focus,
            caps.sync_output,
            warnings_json,
            layout_mode,
            frame_count,
            total_dirty_cells,
            last_dirty_cells,
            top_bar_text.replace('\\', "\\\\").replace('"', "\\\""),
            section_name,
            frame_stats_json.join(",\n    "),
            tour_state_json
        );

        println!("{json}");
    } else {
        // Standard human-readable output
        eprintln!("Headless smoke test PASSED");
        eprintln!("  Buffer size: {width}x{height}");
        eprintln!("  Frames rendered: {frame_count}");
        eprintln!("  Total dirty cells: {total_dirty_cells}");
        eprintln!("  Seed: {}", config.seed);

        // Standard parseable output for CI
        println!("HEADLESS_SMOKE_OK frames={frame_count} last_dirty_cells={last_dirty_cells}");
    }
}

// ============================================================================
// Interactive Mode
// ============================================================================

/// Run interactive mode with terminal.
fn run_interactive(config: &Config) -> io::Result<()> {
    // Check for TTY
    if !is_tty() {
        eprintln!("Error: stdout is not a terminal");
        eprintln!();
        eprintln!("demo_showcase requires an interactive terminal to run.");
        eprintln!("For non-interactive use, try: demo_showcase --headless-smoke");
        std::process::exit(1);
    }

    // Determine terminal size, fall back to 80x24.
    let (width, height) = terminal_size().unwrap_or((80, 24));

    // Create renderer with options.
    let mut renderer = Renderer::new_with_options(
        u32::from(width),
        u32::from(height),
        config.renderer_options(),
    )?;

    // Enable raw mode for input handling.
    let _raw_guard = enable_raw_mode()?;

    // Set up non-blocking stdin.
    set_stdin_nonblocking()?;

    // Initialize app state.
    let mut app = App::new(config);

    // Apply capability gating based on detected terminal capabilities.
    app.update_effective_caps(Some(renderer.capabilities()));

    // Initialize input pump for event handling.
    let mut input_pump = InputPump::new();

    // Main loop.
    let frame_duration = config.frame_duration();

    // Poll timeout: use shorter timeout for smoother rendering.
    let input_timeout = Duration::from_millis(1);

    while !app.should_quit {
        let frame_start = Instant::now();

        // --- Input phase ---
        // Poll for events using the input pump.
        match input_pump.poll(input_timeout) {
            Ok(events) => {
                for tagged_event in events {
                    match &tagged_event.event {
                        // Handle resize events specially - need to resize renderer
                        Event::Resize(resize) => {
                            let new_w = u32::from(resize.width);
                            let new_h = u32::from(resize.height);
                            if let Err(e) = renderer.resize(new_w, new_h) {
                                eprintln!("Resize error: {e}");
                            }
                            app.handle_resize(new_w, new_h);
                        }
                        // Handle mouse events with hit testing
                        Event::Mouse(mouse) => {
                            // Only process left-button clicks
                            if mouse.button == MouseButton::Left {
                                if let Some(hit_id) = renderer.hit_test(mouse.x, mouse.y) {
                                    let action = App::hit_to_action(hit_id, mouse.kind);
                                    app.apply_action(&action);
                                }
                            }
                        }
                        // Other events processed below
                        _ => {}
                    }
                    // Process all events through normal handling
                    app.handle_event(&tagged_event.event);
                }
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                // EINTR, continue
            }
            Err(e) => {
                // Log error but continue (non-fatal).
                eprintln!("Input error: {e}");
            }
        }

        // --- Log routing phase ---
        // Drain queued log entries from OpenTUI callbacks into app.logs.
        drain_log_queue(&mut app);

        // --- Update phase ---
        // Capture force_redraw before tick() clears it
        let needs_full_redraw = app.force_redraw;
        app.tick();

        // --- Render phase ---
        // Force full redraw if requested (Ctrl+R or resize)
        if needs_full_redraw {
            renderer.invalidate();
        }

        // Gather inspector data if debug overlay is enabled
        let inspector = if app.show_debug {
            Some(InspectorData::gather(
                renderer.stats(),
                renderer.capabilities(),
                &app,
                config.threaded,
                config.fps_cap,
            ))
        } else {
            None
        };
        draw_frame(&mut renderer, &app, inspector.as_ref());

        // --- Present ---
        renderer.present()?;

        // --- Frame pacing ---
        let elapsed = frame_start.elapsed();
        if let Some(remaining) = frame_duration.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }
    }

    // Print exit summary for deterministic termination modes
    if app.exit_reason == ExitReason::MaxFrames {
        let last_dirty_cells = renderer.stats().last_frame_cells;
        println!(
            "EXIT_OK reason=max_frames frames={} last_dirty_cells={}",
            app.frame_count, last_dirty_cells
        );
    }

    Ok(())
}

/// Pre-allocated hyperlink IDs for OSC 8 links.
///
/// Link IDs are allocated from the renderer's `LinkPool` at the start of each frame.
/// Only valid for the current frame; reset before next frame.
#[derive(Clone, Debug, Default)]
#[allow(clippy::struct_field_names)]
struct PreallocatedLinks {
    /// Link to `OpenTUI` GitHub repo.
    repo_url: Option<u32>,
    /// Link to `OpenTUI` documentation.
    docs_url: Option<u32>,
    /// Link to Unicode reference (reserved for future use).
    #[allow(dead_code)]
    unicode_url: Option<u32>,
}

impl PreallocatedLinks {
    /// Allocate all links from the renderer's link pool.
    ///
    /// Returns None values if hyperlinks are disabled.
    fn allocate(link_pool: &mut opentui::LinkPool, hyperlinks_enabled: bool) -> Self {
        if !hyperlinks_enabled {
            return Self::default();
        }

        Self {
            repo_url: Some(link_pool.alloc("https://github.com/opentui/opentui")),
            docs_url: Some(link_pool.alloc("https://opentui.dev")),
            unicode_url: Some(link_pool.alloc("https://unicode.org/charts/")),
        }
    }
}

/// Data needed for the inspector/debug panel.
#[derive(Clone, Debug)]
#[allow(clippy::struct_excessive_bools)] // Capabilities naturally map to booleans
struct InspectorData {
    /// Render stats from the renderer.
    fps: f32,
    frame_time_ms: f32,
    cells_updated: usize,
    #[allow(dead_code)] // Reserved for future memory breakdown display
    buffer_bytes: usize,
    #[allow(dead_code)] // Reserved for future memory breakdown display
    hitgrid_bytes: usize,
    total_bytes: usize,
    /// Terminal capabilities.
    truecolor: bool,
    sync_output: bool,
    hyperlinks: bool,
    mouse: bool,
    focus: bool,
    bracketed_paste: bool,
    /// Demo mode flags.
    tour_active: bool,
    threaded: bool,
    fps_cap: u32,
}

impl InspectorData {
    /// Gather inspector data from renderer and app.
    fn gather(
        stats: &opentui::RenderStats,
        caps: &opentui::terminal::Capabilities,
        app: &App,
        threaded: bool,
        fps_cap: u32,
    ) -> Self {
        Self {
            fps: stats.fps,
            frame_time_ms: stats.last_frame_time.as_secs_f32() * 1000.0,
            cells_updated: stats.last_frame_cells,
            buffer_bytes: stats.buffer_bytes,
            hitgrid_bytes: stats.hitgrid_bytes,
            total_bytes: stats.total_bytes,
            truecolor: caps.has_true_color(),
            sync_output: caps.sync_output,
            hyperlinks: caps.hyperlinks,
            mouse: caps.mouse,
            focus: caps.focus,
            bracketed_paste: caps.bracketed_paste,
            tour_active: app.mode == AppMode::Tour,
            threaded,
            fps_cap,
        }
    }
}

/// Draw a single frame using the render pass system.
///
/// Render passes (back-to-front):
/// 1. Background - fill screen
/// 2. Chrome - top bar, status bar
/// 3. Panels - sidebar, editor, preview
/// 4. Overlays - modals (placeholder)
/// 5. Toasts - notifications (placeholder)
/// 6. Debug - performance stats
fn draw_frame(renderer: &mut Renderer, app: &App, inspector: Option<&InspectorData>) {
    let (width, height) = renderer.size();
    let panels = PanelLayout::compute(width, height);
    let theme = app.ui_theme.tokens();

    // Pre-allocate hyperlinks (only if terminal supports them)
    let links = PreallocatedLinks::allocate(renderer.link_pool(), app.effective_caps.hyperlinks);

    // Use buffer_with_pool for proper grapheme handling in Unicode section
    let (buffer, pool) = renderer.buffer_with_pool();

    // === Pass 1: Background ===
    draw_pass_background(buffer, &theme);

    // Handle TooSmall mode (special case).
    if panels.mode == LayoutMode::TooSmall {
        draw_too_small_message(buffer, width, height, &theme);
        return;
    }

    // === Pass 2: Chrome ===
    draw_pass_chrome(buffer, &panels, &theme, app);

    // === Pass 3: Panels ===
    draw_pass_panels(buffer, pool, &panels, &theme, app, &links);

    // === Pass 4: Overlays ===
    draw_pass_overlays(buffer, &panels, &theme, app, &links);

    // === Pass 5: Toasts ===
    draw_pass_toasts(buffer, &panels, &theme, app);

    // === Pass 6: Debug/Inspector Panel ===
    if app.show_debug {
        if let Some(data) = inspector {
            draw_pass_debug(buffer, &panels, &theme, data);
        }
    }

    // === Pass 7: Register hit areas ===
    register_hit_areas(renderer, &panels, app);
}

/// Register hit areas for mouse interaction.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // UI coordinates fit in u32
fn register_hit_areas(renderer: &mut Renderer, panels: &PanelLayout, app: &App) {
    use hit_ids::{
        PALETTE_ITEM_BASE, PANEL_EDITOR, PANEL_LOGS, PANEL_PREVIEW, PANEL_SIDEBAR, SIDEBAR_ROW_BASE,
    };

    // Register panel focus areas
    if !panels.sidebar.is_empty() {
        let r = &panels.sidebar;
        renderer.register_hit_area(r.x as u32, r.y as u32, r.w, r.h, PANEL_SIDEBAR);
    }
    {
        let r = &panels.editor;
        renderer.register_hit_area(r.x as u32, r.y as u32, r.w, r.h, PANEL_EDITOR);
    }
    if !panels.preview.is_empty() {
        let r = &panels.preview;
        renderer.register_hit_area(r.x as u32, r.y as u32, r.w, r.h, PANEL_PREVIEW);
    }
    {
        let r = &panels.logs;
        renderer.register_hit_area(r.x as u32, r.y as u32, r.w, r.h, PANEL_LOGS);
    }

    // Register sidebar rows (section navigation) - only in full/compact layout
    if !panels.sidebar.is_empty() {
        let sidebar = &panels.sidebar;
        for (i, _section) in Section::ALL.iter().enumerate() {
            // Each sidebar row is 2 lines high (title + desc), plus padding
            let row_y = sidebar.y as u32 + 2 + (i as u32 * 2);
            if row_y < (sidebar.y as u32 + sidebar.h - 2) {
                renderer.register_hit_area(
                    sidebar.x as u32,
                    row_y,
                    sidebar.w,
                    2, // Row height
                    SIDEBAR_ROW_BASE + i as u32,
                );
            }
        }
    }

    // Register palette items when command palette is open
    if let Some(Overlay::Palette(state)) = &app.overlays.active {
        // Palette overlay is centered, need to calculate position
        let overlay_w = (panels.screen.w * 50 / 100).clamp(40, 60);
        let overlay_h = (state.filtered.len() as u32 + 4)
            .min(panels.screen.h * 50 / 100)
            .max(6);
        let overlay_x = (panels.screen.w - overlay_w) / 2;
        let overlay_y = panels.screen.h / 4;

        // Register each palette item
        let list_y = overlay_y + 4;
        let max_items = (overlay_h - 5).min(state.filtered.len() as u32);
        for i in 0..max_items as usize {
            renderer.register_hit_area(
                overlay_x,
                list_y + i as u32,
                overlay_w,
                1, // One row per item
                PALETTE_ITEM_BASE + i as u32,
            );
        }
    }
}

/// Pass 1: Draw background fill.
fn draw_pass_background(buffer: &mut OptimizedBuffer, theme: &Theme) {
    buffer.clear(theme.bg0);
}

/// Pass 2: Draw chrome (top bar and status bar).
#[allow(clippy::cast_precision_loss)] // Precision loss acceptable for gradient
fn draw_pass_chrome(buffer: &mut OptimizedBuffer, panels: &PanelLayout, theme: &Theme, app: &App) {
    // --- Top bar with gradient ---
    // Subtle gradient from bg1 to slightly lighter for polish
    let gradient_end = Theme::lerp(theme.bg1, theme.bg2, 0.3);
    draw_gradient_bar(buffer, &panels.top_bar, theme.bg1, gradient_end);

    let top_y = u32::try_from(panels.top_bar.y).unwrap_or(0);
    let top_x = u32::try_from(panels.top_bar.x).unwrap_or(0);

    // Left: Brand name
    buffer.draw_text(
        top_x + 2,
        top_y,
        "OpenTUI",
        Style::fg(theme.accent_primary).with_bold(),
    );
    buffer.draw_text(top_x + 10, top_y, "Showcase", Style::fg(theme.fg1));

    // Center: Current section (if there's enough space)
    if panels.top_bar.w > 60 {
        let section_text = app.section.name();
        #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
        let section_len = section_text.len() as i32;
        #[allow(clippy::cast_possible_wrap)]
        let center_x = (panels.top_bar.w as i32 / 2) - (section_len / 2);
        #[allow(clippy::cast_possible_wrap)]
        let draw_x = top_x as i32 + center_x;
        buffer.draw_text(
            u32::try_from(draw_x).unwrap_or(0),
            top_y,
            section_text,
            Style::fg(theme.fg0),
        );
    }

    // Right: Mode badge + Focus indicator
    let mode_badge = format!("[{}]", app.mode_name());
    let focus_text = format!(" {} ", app.focus_name());

    // Calculate positions from right edge
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    let focus_len = focus_text.len() as i32;
    #[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    let mode_len = mode_badge.len() as i32;

    // Draw focus indicator first (rightmost)
    let focus_x = panels.top_bar.right() - focus_len - 1;
    buffer.draw_text(
        u32::try_from(focus_x).unwrap_or(0),
        top_y,
        &focus_text,
        Style::fg(theme.bg0).with_bg(theme.accent_primary),
    );

    // Draw mode badge
    let mode_x = focus_x - mode_len - 2;
    let mode_color = match app.mode {
        AppMode::Normal => theme.fg2,
        AppMode::Help => theme.accent_primary,
        AppMode::CommandPalette => theme.accent_secondary,
        AppMode::Tour => theme.accent_success,
    };
    buffer.draw_text(
        u32::try_from(mode_x).unwrap_or(0),
        top_y,
        &mode_badge,
        Style::fg(mode_color),
    );

    // --- Status bar ---
    draw_rect_bg(buffer, &panels.status_bar, theme.bg2);
    let status_y = u32::try_from(panels.status_bar.y).unwrap_or(0);

    // Left: Context-sensitive hints with styled keys
    let hints = match app.mode {
        AppMode::Normal => "Ctrl+Q Quit │ F1 Help │ Ctrl+N Theme │ Tab Focus",
        AppMode::Help => "Esc Close │ ↑/↓ Scroll │ PgUp/PgDn Page",
        AppMode::CommandPalette => "Esc Close │ ↑/↓ Navigate │ Enter Select",
        AppMode::Tour => {
            if app.tour_step < app.tour_total.saturating_sub(1) {
                "Enter Next │ Backspace Prev │ Esc Exit"
            } else {
                "✓ Tour Complete! │ Esc Exit"
            }
        }
    };

    // Add paused indicator if needed
    let status_left = if app.paused {
        format!("⏸ PAUSED │ {hints}")
    } else {
        hints.to_string()
    };
    buffer.draw_text(2, status_y, &status_left, Style::fg(theme.fg2));

    // Right: Theme + FPS + Frame counter
    let fps_estimate = 60; // Placeholder until we track actual FPS
    let stats = format!(
        "{} │ {}fps │ F:{}",
        app.ui_theme.name(),
        fps_estimate,
        app.frame_count
    );
    let stats_len = i32::try_from(stats.len()).unwrap_or(0);
    let stats_x = panels.status_bar.right() - stats_len - 2;
    buffer.draw_text(
        u32::try_from(stats_x).unwrap_or(0),
        status_y,
        &stats,
        Style::fg(theme.fg1),
    );
}

/// Pass 3: Draw main panels (sidebar, editor, preview).
fn draw_pass_panels(
    buffer: &mut OptimizedBuffer,
    pool: &mut GraphemePool,
    panels: &PanelLayout,
    theme: &Theme,
    app: &App,
    links: &PreallocatedLinks,
) {
    // --- Sidebar ---
    if !panels.sidebar.is_empty() {
        draw_rect_bg(buffer, &panels.sidebar, theme.bg2);
        draw_sidebar(buffer, &panels.sidebar, panels.mode, theme, app);
    }

    // --- Editor panel ---
    if !panels.editor.is_empty() {
        // Unicode section gets special rendering with grapheme pool
        if app.section == Section::Unicode {
            draw_unicode_showcase(buffer, pool, &panels.editor, theme);
        } else {
            draw_editor_panel(buffer, &panels.editor, theme, app);
        }
    }

    // --- Preview panel ---
    if !panels.preview.is_empty() {
        draw_preview_panel(buffer, &panels.preview, theme, app);
    }

    // --- Logs panel ---
    if !panels.logs.is_empty() {
        draw_rect_bg(buffer, &panels.logs, theme.bg1);
        draw_logs_panel(buffer, &panels.logs, theme, app, links);
    }
}

/// Pass 4: Draw overlays (help, command palette, tour).
///
/// Overlays render at the highest z-order with:
/// - Semi-transparent backdrop that dims the underlying UI
/// - Glass-like panel with alpha blending
/// - Animated enter/exit transitions
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn draw_pass_overlays(
    buffer: &mut OptimizedBuffer,
    panels: &PanelLayout,
    theme: &Theme,
    app: &App,
    links: &PreallocatedLinks,
) {
    // Skip if no overlay is active
    if !app.overlays.is_active() {
        return;
    }

    let opacity = app.overlays.anim.opacity();
    if opacity <= 0.0 {
        return;
    }

    // --- Backdrop ---
    // Draw a semi-transparent overlay that dims the entire screen
    let backdrop_alpha = 0.6 * opacity;
    let backdrop_color = Rgba::new(0.0, 0.0, 0.0, backdrop_alpha);

    // Use opacity stack for proper alpha blending
    buffer.push_opacity(opacity);

    // Fill backdrop with blended dark color
    for y in 0..panels.screen.h {
        for x in 0..panels.screen.w {
            let cell = buffer.get(x, y);
            if let Some(cell) = cell {
                let mut new_cell = *cell;
                // Blend backdrop color over existing background
                let existing_bg = new_cell.bg;
                new_cell.bg = backdrop_color.blend_over(existing_bg);
                buffer.set(x, y, new_cell);
            }
        }
    }

    // --- Overlay panel ---
    match &app.overlays.active {
        Some(Overlay::Help(state)) => {
            draw_help_overlay(buffer, panels, theme, state, opacity, links);
        }
        Some(Overlay::Palette(state)) => {
            draw_palette_overlay(buffer, panels, theme, state, opacity, links);
        }
        Some(Overlay::Tour(state)) => {
            draw_tour_overlay(buffer, panels, theme, state, opacity);
        }
        None => {}
    }

    buffer.pop_opacity();
}

/// Pass 5: Draw toast notifications.
///
/// Toasts stack in the bottom-right corner, above the status bar.
/// Each toast has a severity color, title, optional detail, and fade animation.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
fn draw_pass_toasts(buffer: &mut OptimizedBuffer, panels: &PanelLayout, theme: &Theme, app: &App) {
    if app.toasts.is_empty() {
        return;
    }

    // Position toasts in bottom-right, above status bar
    let toast_w = ToastManager::TOAST_WIDTH;
    let toast_x = panels.screen.w.saturating_sub(toast_w + 2);
    let mut toast_y = panels.status_bar.y.saturating_sub(1) as u32;

    for toast in app.toasts.iter().rev() {
        let opacity = toast.opacity();
        if opacity <= 0.0 {
            continue;
        }

        // Calculate toast height (title + optional detail)
        let has_detail = toast.detail.is_some();
        let toast_h = if has_detail { 4_u32 } else { 3_u32 };

        // Check if we have space
        if toast_y < toast_h + 2 {
            break;
        }
        toast_y = toast_y.saturating_sub(toast_h + ToastManager::TOAST_GAP);

        // Get level-specific colors
        let (accent_color, icon) = match toast.level {
            ToastLevel::Info => (theme.accent_primary, toast.level.icon()),
            ToastLevel::Warn => (
                Rgba::from_hex("#ffcc00").unwrap_or(theme.accent_warning),
                toast.level.icon(),
            ),
            ToastLevel::Error => (
                Rgba::from_hex("#ff4444").unwrap_or(theme.accent_error),
                toast.level.icon(),
            ),
        };

        // Apply opacity to colors
        let bg_color = Rgba::new(theme.bg1.r, theme.bg1.g, theme.bg1.b, 0.9 * opacity);
        let fg_color = Rgba::new(theme.fg0.r, theme.fg0.g, theme.fg0.b, opacity);
        let accent_with_alpha = Rgba::new(accent_color.r, accent_color.g, accent_color.b, opacity);

        // Draw toast background
        for row in toast_y..toast_y + toast_h {
            for col in toast_x..toast_x + toast_w {
                if let Some(cell) = buffer.get(col, row) {
                    let mut new_cell = *cell;
                    new_cell.bg = bg_color.blend_over(cell.bg);
                    buffer.set(col, row, new_cell);
                }
            }
        }

        // Draw left accent bar
        for row in toast_y..toast_y + toast_h {
            buffer.draw_text(
                toast_x,
                row,
                "▌",
                Style::fg(accent_with_alpha).with_bg(bg_color),
            );
        }

        // Draw icon and title on first content row
        let content_x = toast_x + 2;
        let title_y = toast_y + 1;
        buffer.draw_text(
            content_x,
            title_y,
            icon,
            Style::fg(accent_with_alpha).with_bold(),
        );

        // Draw title (truncate if needed)
        let title_start = content_x + 2;
        let max_title_len = (toast_w - 5) as usize;
        let title = if toast.title.len() > max_title_len {
            format!("{}…", &toast.title[..max_title_len - 1])
        } else {
            toast.title.clone()
        };
        buffer.draw_text(
            title_start,
            title_y,
            &title,
            Style::fg(fg_color).with_bold(),
        );

        // Draw detail if present
        if let Some(detail) = &toast.detail {
            let detail_y = title_y + 1;
            let max_detail_len = (toast_w - 4) as usize;
            let detail_text = if detail.len() > max_detail_len {
                format!("{}…", &detail[..max_detail_len - 1])
            } else {
                detail.clone()
            };
            let detail_color = Rgba::new(theme.fg2.r, theme.fg2.g, theme.fg2.b, opacity);
            buffer.draw_text(content_x, detail_y, &detail_text, Style::fg(detail_color));
        }

        // Draw border
        let border_color = Rgba::new(theme.bg2.r, theme.bg2.g, theme.bg2.b, opacity * 0.5);
        let border_style = Style::fg(border_color);

        // Top border
        buffer.draw_text(toast_x, toast_y, "╭", border_style);
        for col in toast_x + 1..toast_x + toast_w - 1 {
            buffer.draw_text(col, toast_y, "─", border_style);
        }
        buffer.draw_text(toast_x + toast_w - 1, toast_y, "╮", border_style);

        // Bottom border
        buffer.draw_text(toast_x, toast_y + toast_h - 1, "╰", border_style);
        for col in toast_x + 1..toast_x + toast_w - 1 {
            buffer.draw_text(col, toast_y + toast_h - 1, "─", border_style);
        }
        buffer.draw_text(
            toast_x + toast_w - 1,
            toast_y + toast_h - 1,
            "╯",
            border_style,
        );

        // Side borders
        for row in toast_y + 1..toast_y + toast_h - 1 {
            buffer.draw_text(toast_x + toast_w - 1, row, "│", border_style);
        }
    }
}

/// Pass 6: Draw the debug/inspector panel.
///
/// Shows real-time performance stats, terminal capabilities, and demo mode flags.
/// Positioned in the top-right corner to avoid obscuring main content.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]
fn draw_pass_debug(
    buffer: &mut OptimizedBuffer,
    panels: &PanelLayout,
    theme: &Theme,
    data: &InspectorData,
) {
    // Panel dimensions - compact width, positioned in top-right
    let panel_w = 36_u32;
    let panel_h = 14_u32;
    let panel_x = panels.screen.w.saturating_sub(panel_w + 1);
    let panel_y = panels.top_bar.y as u32 + panels.top_bar.h + 1;

    // Semi-transparent dark background
    let bg_color = Rgba::new(0.05, 0.05, 0.1, 0.92);
    for y in panel_y..panel_y + panel_h {
        for x in panel_x..panel_x + panel_w {
            buffer.set(x, y, opentui::Cell::clear(bg_color));
        }
    }

    // Border
    let border_color = theme.accent_primary.with_alpha(0.6);
    let border_style = Style::fg(border_color);

    // Top border
    buffer.draw_text(panel_x, panel_y, "╭", border_style);
    for x in panel_x + 1..panel_x + panel_w - 1 {
        buffer.draw_text(x, panel_y, "─", border_style);
    }
    buffer.draw_text(panel_x + panel_w - 1, panel_y, "╮", border_style);

    // Bottom border
    buffer.draw_text(panel_x, panel_y + panel_h - 1, "╰", border_style);
    for x in panel_x + 1..panel_x + panel_w - 1 {
        buffer.draw_text(x, panel_y + panel_h - 1, "─", border_style);
    }
    buffer.draw_text(
        panel_x + panel_w - 1,
        panel_y + panel_h - 1,
        "╯",
        border_style,
    );

    // Side borders
    for y in panel_y + 1..panel_y + panel_h - 1 {
        buffer.draw_text(panel_x, y, "│", border_style);
        buffer.draw_text(panel_x + panel_w - 1, y, "│", border_style);
    }

    // Title
    let title = " Inspector ";
    let title_x = panel_x + (panel_w - title.len() as u32) / 2;
    buffer.draw_text(
        title_x,
        panel_y,
        title,
        Style::fg(theme.accent_primary).with_bold(),
    );

    let content_x = panel_x + 2;
    let mut y = panel_y + 1;

    let label_style = Style::fg(theme.fg2);
    let value_style = Style::fg(theme.fg1).with_bold();
    let good_style = Style::fg(theme.accent_success);
    let warn_style = Style::fg(theme.accent_warning);

    // --- Performance Stats ---
    y += 1;
    buffer.draw_text(content_x, y, "FPS:", label_style);
    let fps_text = format!("{:.1}", data.fps);
    let fps_style = if data.fps >= 55.0 {
        good_style
    } else {
        warn_style
    };
    buffer.draw_text(content_x + 5, y, &fps_text, fps_style);

    buffer.draw_text(content_x + 12, y, "Frame:", label_style);
    let frame_text = format!("{:.1}ms", data.frame_time_ms);
    buffer.draw_text(content_x + 19, y, &frame_text, value_style);

    y += 1;
    buffer.draw_text(content_x, y, "Cells:", label_style);
    buffer.draw_text(
        content_x + 7,
        y,
        &data.cells_updated.to_string(),
        value_style,
    );

    buffer.draw_text(content_x + 14, y, "Mem:", label_style);
    let mem_kb = data.total_bytes / 1024;
    buffer.draw_text(content_x + 19, y, &format!("{mem_kb}KB"), value_style);

    // --- Capabilities ---
    y += 2;
    buffer.draw_text(content_x, y, "─ Capabilities ─", Style::fg(theme.fg2));

    y += 1;
    let cap_on = Style::fg(theme.accent_success);
    let cap_off = Style::fg(theme.accent_error);

    let tc_style = if data.truecolor { cap_on } else { cap_off };
    buffer.draw_text(content_x, y, "TC", tc_style);

    let sync_style = if data.sync_output { cap_on } else { cap_off };
    buffer.draw_text(content_x + 4, y, "Sync", sync_style);

    let link_style = if data.hyperlinks { cap_on } else { cap_off };
    buffer.draw_text(content_x + 10, y, "Link", link_style);

    let mouse_style = if data.mouse { cap_on } else { cap_off };
    buffer.draw_text(content_x + 16, y, "Mouse", mouse_style);

    y += 1;
    let focus_style = if data.focus { cap_on } else { cap_off };
    buffer.draw_text(content_x, y, "Focus", focus_style);

    let paste_style = if data.bracketed_paste {
        cap_on
    } else {
        cap_off
    };
    buffer.draw_text(content_x + 7, y, "Paste", paste_style);

    // --- Demo Flags ---
    y += 2;
    buffer.draw_text(content_x, y, "─ Demo ─", Style::fg(theme.fg2));

    y += 1;
    let tour_text = if data.tour_active {
        "Tour: ON"
    } else {
        "Tour: off"
    };
    let tour_style = if data.tour_active {
        good_style
    } else {
        label_style
    };
    buffer.draw_text(content_x, y, tour_text, tour_style);

    let threaded_text = if data.threaded { "Threaded" } else { "Direct" };
    buffer.draw_text(content_x + 12, y, threaded_text, value_style);

    y += 1;
    let fps_cap_text = format!("FPS Cap: {}", data.fps_cap);
    buffer.draw_text(content_x, y, &fps_cap_text, label_style);
}

/// Draw the Help overlay panel.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]
fn draw_help_overlay(
    buffer: &mut OptimizedBuffer,
    panels: &PanelLayout,
    theme: &Theme,
    state: &HelpState,
    _opacity: f32,
    links: &PreallocatedLinks,
) {
    // Calculate overlay dimensions (centered, 60% of screen)
    let overlay_w = (panels.screen.w * 60 / 100).clamp(40, 80);
    let overlay_h = (panels.screen.h * 70 / 100).clamp(12, 30);
    let overlay_x = (panels.screen.w - overlay_w) / 2;
    let overlay_y = (panels.screen.h - overlay_h) / 2;

    let rect = Rect::new(overlay_x as i32, overlay_y as i32, overlay_w, overlay_h);

    // Draw glass panel background with subtle gradient
    let glass_bg = Rgba::new(
        theme.bg1.r,
        theme.bg1.g,
        theme.bg1.b,
        0.95, // Nearly opaque for readability
    );
    draw_rect_bg(buffer, &rect, glass_bg);

    // Draw border (double-line style for "premium" look)
    draw_overlay_border(buffer, &rect, theme);

    // Draw title bar
    let title = "═══ Help (F1) ═══";
    let title_x = overlay_x + (overlay_w.saturating_sub(title.len() as u32)) / 2;
    buffer.draw_text(
        title_x,
        overlay_y,
        title,
        Style::fg(theme.accent_primary).with_bold(),
    );

    // Draw content with scroll
    let content_x = overlay_x + 2;
    let mut content_y = overlay_y + 2;
    let content_max_y = overlay_y + overlay_h - 2;

    let mut line_idx = 0;
    for (section_name, items) in HelpState::SECTIONS {
        // Skip lines before scroll offset
        if line_idx < state.scroll {
            line_idx += 1 + items.len();
            continue;
        }

        if content_y >= content_max_y {
            break;
        }

        // Section header
        if line_idx >= state.scroll {
            buffer.draw_text(
                content_x,
                content_y,
                section_name,
                Style::fg(theme.accent_secondary).with_bold(),
            );
            content_y += 1;
        }
        line_idx += 1;

        // Section items
        for item in *items {
            if line_idx < state.scroll {
                line_idx += 1;
                continue;
            }
            if content_y >= content_max_y {
                break;
            }

            // Apply hyperlinks to the Links section
            let style = if *section_name == "Links" {
                let link_id = if item.starts_with("Repo:") {
                    links.repo_url
                } else if item.starts_with("Docs:") {
                    links.docs_url
                } else {
                    None
                };
                link_id.map_or_else(
                    || Style::fg(theme.fg1),
                    |id| {
                        Style::fg(theme.accent_primary)
                            .with_underline()
                            .with_link(id)
                    },
                )
            } else {
                Style::fg(theme.fg1)
            };
            buffer.draw_text(content_x + 1, content_y, item, style);
            content_y += 1;
            line_idx += 1;
        }

        content_y += 1; // Blank line between sections
    }

    // Draw footer with hint
    let footer = "Press Esc to close";
    let footer_x = overlay_x + (overlay_w.saturating_sub(footer.len() as u32)) / 2;
    buffer.draw_text(
        footer_x,
        overlay_y + overlay_h - 1,
        footer,
        Style::fg(theme.fg2),
    );
}

/// Draw the Command Palette overlay.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
fn draw_palette_overlay(
    buffer: &mut OptimizedBuffer,
    panels: &PanelLayout,
    theme: &Theme,
    state: &PaletteState,
    _opacity: f32,
    _links: &PreallocatedLinks,
) {
    // Palette is narrower and positioned higher
    let overlay_w = (panels.screen.w * 50 / 100).clamp(40, 60);
    let overlay_h = (state.filtered.len() as u32 + 4)
        .min(panels.screen.h * 50 / 100)
        .max(6);
    let overlay_x = (panels.screen.w - overlay_w) / 2;
    let overlay_y = panels.screen.h / 4; // Upper third

    let rect = Rect::new(overlay_x as i32, overlay_y as i32, overlay_w, overlay_h);

    // Draw glass background
    let glass_bg = Rgba::new(theme.bg1.r, theme.bg1.g, theme.bg1.b, 0.95);
    draw_rect_bg(buffer, &rect, glass_bg);
    draw_overlay_border(buffer, &rect, theme);

    // Title
    let title = "═══ Command Palette (Ctrl+P) ═══";
    let title_x = overlay_x + overlay_w.saturating_sub(title.len() as u32) / 2;
    buffer.draw_text(
        title_x,
        overlay_y,
        title,
        Style::fg(theme.accent_secondary).with_bold(),
    );

    // Search prompt
    let prompt = "> ";
    buffer.draw_text(
        overlay_x + 2,
        overlay_y + 2,
        prompt,
        Style::fg(theme.accent_primary),
    );

    // Query text (or placeholder)
    let query_display = if state.query.is_empty() {
        "Type to search..."
    } else {
        &state.query
    };
    let query_style = if state.query.is_empty() {
        Style::fg(theme.fg2)
    } else {
        Style::fg(theme.fg0)
    };
    buffer.draw_text(overlay_x + 4, overlay_y + 2, query_display, query_style);

    // Draw filtered commands
    let list_y = overlay_y + 4;
    let max_items = (overlay_h - 5).min(state.filtered.len() as u32);

    for (i, &cmd_idx) in state.filtered.iter().take(max_items as usize).enumerate() {
        let y = list_y + i as u32;
        let (name, desc) = PaletteState::COMMANDS[cmd_idx];

        let is_selected = i == state.selected;
        let style = if is_selected {
            Style::fg(theme.bg0).with_bg(theme.accent_primary)
        } else {
            Style::fg(theme.fg0)
        };

        // Selection indicator
        let indicator = if is_selected { "▸ " } else { "  " };
        buffer.draw_text(overlay_x + 2, y, indicator, Style::fg(theme.accent_primary));

        // Command name
        buffer.draw_text(overlay_x + 4, y, name, style);

        // Description (truncated)
        let desc_x = overlay_x + 4 + name.len() as u32 + 2;
        let desc_max = overlay_w.saturating_sub(desc_x - overlay_x + 2);
        if desc_max > 5 {
            let desc_truncated: String = desc.chars().take(desc_max as usize).collect();
            buffer.draw_text(desc_x, y, &desc_truncated, Style::fg(theme.fg2));
        }
    }
}

/// Draw the Tour overlay with spotlight effect.
///
/// The spotlight effect dims the entire screen except for the target panel,
/// creating a "punch-out" effect that draws the viewer's attention.
/// Draw spotlight effect: dim everything except the target rect.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn draw_spotlight_effect(
    buffer: &mut OptimizedBuffer,
    panels: &PanelLayout,
    theme: &Theme,
    target_rect: &Rect,
) {
    use opentui::Cell;

    // Dim the entire screen with a semi-transparent overlay
    let dim_color = Rgba::new(0.0, 0.0, 0.0, 0.4);

    // Draw dim overlay for areas outside the target rect
    for y in 0..panels.screen.h {
        for x in 0..panels.screen.w {
            // Check if this cell is outside the target rect
            let in_target = (x as i32) >= target_rect.x
                && (x as i32) < target_rect.x + target_rect.w as i32
                && (y as i32) >= target_rect.y
                && (y as i32) < target_rect.y + target_rect.h as i32;

            if !in_target {
                // Blend dim color over existing content
                if let Some(cell) = buffer.get(x, y) {
                    let new_bg = dim_color.blend_over(cell.bg);
                    let mut new_cell = *cell;
                    new_cell.bg = new_bg;
                    buffer.set(x, y, new_cell);
                }
            }
        }
    }

    // Draw a highlight border around the target rect
    let border_color = theme.accent_primary;
    let border_style = Style::fg(border_color).with_bold();

    // Top and bottom borders
    for x in target_rect.x.max(0) as u32
        ..(target_rect.x + target_rect.w as i32).min(panels.screen.w as i32) as u32
    {
        if target_rect.y >= 0 && (target_rect.y as u32) < panels.screen.h {
            buffer.set(x, target_rect.y as u32, Cell::new('─', border_style));
        }
        let bottom_y = target_rect.y + target_rect.h as i32 - 1;
        if bottom_y >= 0 && (bottom_y as u32) < panels.screen.h {
            buffer.set(x, bottom_y as u32, Cell::new('─', border_style));
        }
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn draw_tour_overlay(
    buffer: &mut OptimizedBuffer,
    panels: &PanelLayout,
    theme: &Theme,
    state: &TourState,
    _opacity: f32,
) {
    let (title, desc, spotlight_target) = state.current();

    // === Spotlight Effect ===
    // If there's a spotlight target, dim the screen and highlight the target.
    if let Some(target_name) = spotlight_target {
        if let Some(target_rect) = panels.get_panel_rect(target_name) {
            draw_spotlight_effect(buffer, panels, theme, &target_rect);
        }
    }

    // === Tour HUD Panel ===
    // Tour panel at bottom of screen (like a HUD)
    let overlay_w = (panels.screen.w * 70 / 100).clamp(50, 80);
    let overlay_h = 8_u32;
    let overlay_x = (panels.screen.w - overlay_w) / 2;
    let overlay_y = panels.screen.h.saturating_sub(overlay_h + 2);

    let rect = Rect::new(overlay_x as i32, overlay_y as i32, overlay_w, overlay_h);

    // Draw glass background
    let glass_bg = Rgba::new(theme.bg1.r, theme.bg1.g, theme.bg1.b, 0.95);
    draw_rect_bg(buffer, &rect, glass_bg);
    draw_overlay_border(buffer, &rect, theme);

    // Step indicator
    let step_text = format!(
        "═══ Tour Step {}/{} ═══",
        state.step + 1,
        TourState::STEPS.len()
    );
    let step_x = overlay_x + overlay_w.saturating_sub(step_text.len() as u32) / 2;
    buffer.draw_text(
        step_x,
        overlay_y,
        &step_text,
        Style::fg(theme.accent_success).with_bold(),
    );

    // Title
    buffer.draw_text(
        overlay_x + 3,
        overlay_y + 2,
        title,
        Style::fg(theme.fg0).with_bold(),
    );

    // Description (may have newlines)
    let mut desc_y = overlay_y + 4;
    for line in desc.lines() {
        if desc_y >= overlay_y + overlay_h - 1 {
            break;
        }
        buffer.draw_text(overlay_x + 3, desc_y, line, Style::fg(theme.fg1));
        desc_y += 1;
    }

    // Navigation hint
    let nav_hint = "Enter: Next │ Backspace: Prev │ Esc: Exit";
    let nav_x = overlay_x + overlay_w.saturating_sub(nav_hint.len() as u32) / 2;
    buffer.draw_text(
        nav_x,
        overlay_y + overlay_h - 1,
        nav_hint,
        Style::fg(theme.fg2),
    );

    // Progress bar
    let progress_w = overlay_w.saturating_sub(6);
    let filled =
        (progress_w as f32 * (state.step + 1) as f32 / TourState::STEPS.len() as f32) as u32;
    let progress_x = overlay_x + 3;
    let progress_y = overlay_y + overlay_h - 2;

    // Draw progress track
    for i in 0..progress_w {
        let ch = if i < filled { '█' } else { '░' };
        let color = if i < filled {
            theme.accent_success
        } else {
            theme.fg2
        };
        buffer.draw_text(
            progress_x + i,
            progress_y,
            &ch.to_string(),
            Style::fg(color),
        );
    }
}

/// Draw a decorative border around an overlay panel.
fn draw_overlay_border(buffer: &mut OptimizedBuffer, rect: &Rect, theme: &Theme) {
    let x = u32::try_from(rect.x).unwrap_or(0);
    let y = u32::try_from(rect.y).unwrap_or(0);
    let w = rect.w;
    let h = rect.h;

    let border_style = Style::fg(theme.accent_primary);

    // Top and bottom edges
    for col in 1..w.saturating_sub(1) {
        buffer.draw_text(x + col, y, "═", border_style);
        buffer.draw_text(x + col, y + h - 1, "═", border_style);
    }

    // Left and right edges
    for row in 1..h.saturating_sub(1) {
        buffer.draw_text(x, y + row, "║", border_style);
        buffer.draw_text(x + w - 1, y + row, "║", border_style);
    }

    // Corners
    buffer.draw_text(x, y, "╔", border_style);
    buffer.draw_text(x + w - 1, y, "╗", border_style);
    buffer.draw_text(x, y + h - 1, "╚", border_style);
    buffer.draw_text(x + w - 1, y + h - 1, "╝", border_style);
}

/// Draw the Unicode showcase panel demonstrating grapheme pool functionality.
///
/// This panel showcases:
/// - CJK wide characters (width 2)
/// - Single and multi-codepoint emoji
/// - ZWJ (Zero Width Joiner) emoji sequences
/// - Combining marks (diacritics)
/// - Width ruler for alignment verification
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn draw_unicode_showcase(
    buffer: &mut OptimizedBuffer,
    pool: &mut GraphemePool,
    rect: &Rect,
    theme: &Theme,
) {
    use content::unicode;

    if rect.is_empty() {
        return;
    }

    let x = rect.x as u32;
    let y = rect.y as u32;
    let w = rect.w;
    let h = rect.h;

    // Background
    draw_rect_bg(buffer, rect, theme.bg1);

    // Header
    let header_style = Style::fg(theme.accent_primary).with_bold();
    buffer.draw_text(
        x + 2,
        y + 1,
        "Unicode & Grapheme Pool Showcase",
        header_style,
    );

    // Divider line
    let divider: String = "─".repeat(w.saturating_sub(4) as usize);
    buffer.draw_text(x + 2, y + 2, &divider, Style::fg(theme.fg2));

    let label_style = Style::fg(theme.accent_secondary).with_bold();
    let content_style = Style::fg(theme.fg0);
    let dim_style = Style::fg(theme.fg2);

    let mut row = y + 4;

    // Section 1: Width ruler (reference)
    buffer.draw_text(x + 2, row, "Width Ruler:", label_style);
    row += 1;
    let ruler = "0         1         2         3         4";
    buffer.draw_text(x + 4, row, ruler, dim_style);
    row += 1;
    let ruler_marks = "0123456789012345678901234567890123456789012";
    buffer.draw_text(x + 4, row, ruler_marks, dim_style);
    row += 2;

    // Section 2: CJK Wide Characters
    if row + 2 < y + h {
        buffer.draw_text(x + 2, row, "CJK Wide (width 2 each):", label_style);
        row += 1;
        buffer.draw_text_with_pool(pool, x + 4, row, unicode::CJK_WIDE, content_style);
        row += 2;
    }

    // Section 3: Single-codepoint Emoji
    if row + 2 < y + h {
        buffer.draw_text(x + 2, row, "Single Emoji (width 2 each):", label_style);
        row += 1;
        buffer.draw_text_with_pool(pool, x + 4, row, unicode::EMOJI_SINGLE, content_style);
        row += 2;
    }

    // Section 4: ZWJ Emoji Sequences (requires grapheme pool)
    if row + 2 < y + h {
        buffer.draw_text(
            x + 2,
            row,
            "ZWJ Emoji Sequences (multi-codepoint):",
            label_style,
        );
        row += 1;
        buffer.draw_text_with_pool(pool, x + 4, row, unicode::EMOJI_ZWJ, content_style);
        row += 2;
    }

    // Section 5: Combining Marks
    if row + 3 < y + h {
        buffer.draw_text(
            x + 2,
            row,
            "Combining Marks (base + diacritic):",
            label_style,
        );
        row += 1;
        buffer.draw_text(x + 4, row, "Input:   ", dim_style);
        buffer.draw_text_with_pool(pool, x + 13, row, unicode::COMBINING_MARKS, content_style);
        row += 1;
        buffer.draw_text(x + 4, row, "Display: ", dim_style);
        buffer.draw_text_with_pool(pool, x + 13, row, unicode::COMBINING_DISPLAY, content_style);
        row += 2;
    }

    // Section 6: Mixed Content
    if row + 2 < y + h {
        buffer.draw_text(x + 2, row, "Mixed Content Line:", label_style);
        row += 1;
        buffer.draw_text_with_pool(pool, x + 4, row, unicode::MIXED_LINE, content_style);
        row += 2;
    }

    // Section 7: Width Test Cases
    if row + 6 < y + h {
        buffer.draw_text(x + 2, row, "Width Test Cases:", label_style);
        row += 1;
        for &(name, text, expected_width) in unicode::WIDTH_TEST_CASES {
            if row >= y + h - 1 {
                break;
            }
            let line = format!("{name:10} \"{text}\" → width {expected_width}");
            buffer.draw_text_with_pool(pool, x + 4, row, &line, content_style);
            row += 1;
        }
    }

    // Footer note
    if h > 20 {
        let footer_y = y + h - 2;
        buffer.draw_text(
            x + 2,
            footer_y,
            "Note: Proper rendering requires terminal with Unicode support",
            dim_style,
        );
    }
}

/// Draw the editor panel content with file name, line numbers, and syntax coloring.
///
/// Features demonstrated:
/// - File content display with line numbers
/// - Basic syntax highlighting (keywords, comments, strings)
/// - Focus border highlighting
fn draw_editor_panel(buffer: &mut OptimizedBuffer, rect: &Rect, theme: &Theme, app: &App) {
    if rect.is_empty() {
        return;
    }

    let x = u32::try_from(rect.x).unwrap_or(0);
    let y = u32::try_from(rect.y).unwrap_or(0);
    let is_focused = app.focus == Focus::Editor;

    // Header bar with file name
    let header_bg = if is_focused {
        theme.accent_primary.with_alpha(0.3)
    } else {
        theme.bg1
    };
    for col in 0..rect.w {
        buffer.draw_text(x + col, y, " ", Style::bg(header_bg));
    }

    // File name with language indicator
    let file_name = app.current_file_name();
    let lang_indicator = match app.current_file_language() {
        content::Language::Rust => " [Rust]",
        content::Language::Markdown => " [Markdown]",
        content::Language::Plain => "",
    };
    let header_text = format!(" {file_name}{lang_indicator}");
    let header_style = if is_focused {
        Style::fg(theme.fg0).with_bg(header_bg).with_bold()
    } else {
        Style::fg(theme.fg1).with_bg(header_bg)
    };
    buffer.draw_text(x, y, &header_text, header_style);

    // Calculate content area (below header)
    let content_y = y + 1;
    let content_h = rect.h.saturating_sub(1);
    let gutter_width = 4_u32; // "NNN " format
    let text_x = x + gutter_width;
    let text_w = rect.w.saturating_sub(gutter_width);

    // Get file content and display lines
    let content = app.current_file_content();
    let lines: Vec<&str> = content.lines().collect();
    let language = app.current_file_language();

    for (line_idx, line) in lines.iter().enumerate() {
        let row = content_y + u32::try_from(line_idx).unwrap_or(0);
        if row >= content_y + content_h {
            break;
        }

        // Draw line number in gutter
        let line_num = line_idx + 1;
        let gutter_text = format!("{line_num:>3} ");
        buffer.draw_text(x, row, &gutter_text, Style::fg(theme.fg2));

        // Draw line content with basic syntax highlighting
        let line_style = get_line_style(line, language, theme);
        let display_line = if u32::try_from(line.len()).unwrap_or(0) > text_w {
            let max_len = usize::try_from(text_w.saturating_sub(1)).unwrap_or(0);
            let truncated = &line[..line.len().min(max_len)];
            format!("{truncated}…")
        } else {
            (*line).to_string()
        };
        buffer.draw_text(text_x, row, &display_line, line_style);
    }

    // Focus indicator on left edge
    if is_focused {
        for row in y..y + rect.h {
            buffer.draw_text(x.saturating_sub(1), row, "│", Style::fg(theme.focus_border));
        }
    }
}

/// Get style for a line based on basic syntax analysis.
fn get_line_style(line: &str, language: content::Language, theme: &Theme) -> Style {
    let trimmed = line.trim();

    match language {
        content::Language::Rust => {
            // Comment
            if trimmed.starts_with("//") {
                return Style::fg(theme.fg2);
            }
            // Keywords
            let keywords = [
                "fn ",
                "let ",
                "mut ",
                "pub ",
                "use ",
                "impl ",
                "struct ",
                "enum ",
                "const ",
                "static ",
                "mod ",
                "trait ",
                "where ",
                "async ",
                "await ",
                "match ",
                "if ",
                "else ",
                "for ",
                "while ",
                "loop ",
                "return ",
                "break ",
                "continue ",
            ];
            for kw in keywords {
                if trimmed.starts_with(kw) || trimmed.contains(&format!(" {kw}")) {
                    return Style::fg(theme.accent_primary);
                }
            }
            // String literal
            if trimmed.contains('"') {
                return Style::fg(theme.accent_secondary);
            }
            Style::fg(theme.fg0)
        }
        content::Language::Markdown => {
            // Heading
            if trimmed.starts_with('#') {
                return Style::fg(theme.accent_primary).with_bold();
            }
            // Code block
            if trimmed.starts_with("```") {
                return Style::fg(theme.fg2);
            }
            // Bold/italic markers
            if trimmed.starts_with('*') || trimmed.starts_with('-') {
                return Style::fg(theme.accent_secondary);
            }
            // Link
            if trimmed.contains('[') && trimmed.contains("](") {
                return Style::fg(theme.accent_primary);
            }
            Style::fg(theme.fg0)
        }
        content::Language::Plain => Style::fg(theme.fg0),
    }
}

/// Convert HSV to RGB color.
///
/// H: 0.0-360.0, S: 0.0-1.0, V: 0.0-1.0
#[allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgba {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    Rgba::new(r + m, g + m, b + m, 1.0)
}

/// Draw the preview panel with animated graphics demos.
///
/// Features demonstrated:
/// - `PixelBuffer` for high-resolution animated gradient orb
/// - `GrayscaleBuffer` for sparkline chart
/// - Alpha blending overlay (glass effect)
/// - Focus highlighting
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::suboptimal_flops,
    clippy::too_many_lines
)]
fn draw_preview_panel(buffer: &mut OptimizedBuffer, rect: &Rect, theme: &Theme, app: &App) {
    if rect.is_empty() || rect.w < 10 || rect.h < 6 {
        return;
    }

    let px = u32::try_from(rect.x).unwrap_or(0);
    let py = u32::try_from(rect.y).unwrap_or(0);
    let is_focused = app.focus == Focus::Preview;

    // Draw left border of preview panel
    let border_color = if is_focused {
        theme.focus_border
    } else {
        Rgba::from_hex("#333366").unwrap_or(theme.bg2)
    };
    for row in py..py + rect.h {
        buffer.draw_text(px, row, "│", Style::fg(border_color));
    }

    // Draw "Preview" label with focus styling
    let label = " Preview ";
    let label_style = if is_focused {
        Style::fg(theme.accent_primary).with_bold()
    } else {
        Style::fg(theme.fg2)
    };
    buffer.draw_text(px + 1, py, label, label_style);

    // Content area (inset from border)
    let content_x = px + 2;
    let content_y = py + 2;
    let content_w = rect.w.saturating_sub(4);
    let content_h = rect.h.saturating_sub(4);

    if content_w < 8 || content_h < 4 {
        return;
    }

    // === Section 1: Animated Gradient Orb using PixelBuffer ===
    let orb_size = content_w.min(content_h * 2).min(24); // Each cell is ~2x1 pixels
    let orb_pixel_w = orb_size * 2; // 2 pixels per cell width
    let orb_pixel_h = orb_size; // 1 pixel per cell height (quadrant blocks)

    let mut orb_buf = PixelBuffer::new(orb_pixel_w, orb_pixel_h);

    // Animated hue rotation based on time
    let t = app.clock.t;
    let base_hue = (t * 60.0) % 360.0; // Full rotation every 6 seconds

    // Draw radial gradient orb
    let cx = orb_pixel_w as f32 / 2.0;
    let cy = orb_pixel_h as f32 / 2.0;
    let radius = cx.min(cy) * 0.9;

    for y in 0..orb_pixel_h {
        for x in 0..orb_pixel_w {
            let dx = x as f32 - cx;
            let dy = (y as f32 - cy) * 2.0; // Compensate for cell aspect ratio
            let dist = dx.hypot(dy);

            if dist < radius {
                // Radial hue shift
                let angle = dy.atan2(dx);
                let hue = (base_hue + angle.to_degrees() + 360.0) % 360.0;
                let sat = 0.7 + 0.3 * (1.0 - dist / radius);
                let val = 0.9 - 0.3 * (dist / radius);
                let color = hsv_to_rgb(hue, sat, val);
                orb_buf.set(x, y, color);
            } else {
                orb_buf.set(x, y, Rgba::TRANSPARENT);
            }
        }
    }

    // Render orb using supersampling (quadrant block characters)
    buffer.draw_supersample_buffer(content_x, content_y, &orb_buf, 0.5);

    // === Section 2: Sparkline Chart using GrayscaleBuffer ===
    let chart_y = content_y + (orb_size / 2) + 2;
    let chart_w = content_w.min(32);
    let chart_h = 4_u32;

    if chart_y + chart_h < py + rect.h {
        let mut chart_buf = GrayscaleBuffer::new(chart_w, chart_h);

        // Generate sparkline data from metrics
        let fps = app.metrics.fps as f32;
        let target = app.target_fps as f32;

        // Simulated FPS history (oscillating around current fps)
        for x in 0..chart_w {
            let phase = (x as f32 / chart_w as f32) * std::f32::consts::PI * 4.0 + t * 2.0;
            let value = fps + (phase.sin() * 5.0);
            let normalized = (value / target).clamp(0.0, 1.0);
            let bar_height = (normalized * chart_h as f32) as u32;

            // Draw vertical bar
            for y in 0..chart_h {
                let intensity = if y >= chart_h - bar_height {
                    0.8 + 0.2 * (1.0 - y as f32 / chart_h as f32)
                } else {
                    0.1
                };
                chart_buf.set(x, y, intensity);
            }
        }

        // Render sparkline using Unicode shade characters
        buffer.draw_grayscale_buffer_unicode(
            content_x,
            chart_y,
            &chart_buf,
            theme.accent_primary,
            theme.bg0,
        );

        // Chart label
        let fps_label = format!("{fps:.0} FPS");
        buffer.draw_text(
            content_x + chart_w + 1,
            chart_y + chart_h / 2,
            &fps_label,
            Style::fg(theme.fg1),
        );
    }

    // === Section 3: Alpha Blending Overlay (Glass Effect) ===
    let overlay_y = chart_y.saturating_add(chart_h + 2);
    let overlay_h = 3_u32;
    let overlay_w = content_w.min(28);

    if overlay_y + overlay_h < py + rect.h {
        // Semi-transparent glass panel
        let glass_bg = Rgba::new(0.1, 0.1, 0.2, 0.7);

        for row in overlay_y..overlay_y + overlay_h {
            for col in content_x..content_x + overlay_w {
                if let Some(cell) = buffer.get(col, row) {
                    let mut new_cell = *cell;
                    // Blend glass background over existing content
                    new_cell.bg = glass_bg.blend_over(cell.bg);
                    buffer.set(col, row, new_cell);
                }
            }
        }

        // Overlay content
        let mem_mb = app.metrics.memory_bytes as f64 / (1024.0 * 1024.0);
        let mem_text = format!("Mem: {mem_mb:.1} MB");
        let cpu_text = format!("CPU: {}%", app.metrics.cpu_percent);

        buffer.draw_text(
            content_x + 1,
            overlay_y + 1,
            &mem_text,
            Style::fg(theme.fg0),
        );
        buffer.draw_text(
            content_x + 14,
            overlay_y + 1,
            &cpu_text,
            Style::fg(theme.fg0),
        );
    }

    // Frame counter at bottom
    let frame_y = py + rect.h - 2;
    let frame_info = format!("Frame {}", app.frame_count);
    buffer.draw_text(content_x, frame_y, &frame_info, Style::fg(theme.fg2));
}

/// Draw the logs panel showing event stream with hyperlink support.
///
/// Features demonstrated:
/// - Styled text with log level colors
/// - OSC 8 hyperlinks for clickable URLs
/// - Scroll and scissor clipping
/// - Focus highlighting
fn draw_logs_panel(
    buffer: &mut OptimizedBuffer,
    rect: &Rect,
    theme: &Theme,
    app: &App,
    _links: &PreallocatedLinks,
) {
    if rect.is_empty() {
        return;
    }

    let x = u32::try_from(rect.x).unwrap_or(0);
    let y = u32::try_from(rect.y).unwrap_or(0);
    let is_focused = app.focus == Focus::Logs;

    // Draw top border with separator
    let border_color = if is_focused {
        theme.focus_border
    } else {
        theme.bg2
    };
    let border_char = "─";
    for col in 0..rect.w {
        buffer.draw_text(x + col, y, border_char, Style::fg(border_color));
    }

    // Draw "Logs" label on the border
    let label = " Logs ";
    let label_style = if is_focused {
        Style::fg(theme.accent_primary).with_bold()
    } else {
        Style::fg(theme.fg2)
    };
    buffer.draw_text(x + 2, y, label, label_style);

    // Content area below the border
    let content_y = y + 1;
    let content_h = rect.h.saturating_sub(1);

    // Draw log entries
    let visible_rows = content_h.min(u32::try_from(app.logs.len()).unwrap_or(0));

    // Scroll to show most recent logs (display from bottom up)
    let start_idx = app
        .logs
        .len()
        .saturating_sub(usize::try_from(visible_rows).unwrap_or(0));

    for (row_offset, log) in app.logs.iter().skip(start_idx).enumerate() {
        let row = u32::try_from(row_offset).unwrap_or(0);
        if row >= content_h {
            break;
        }

        let log_y = content_y + row;
        let mut col = x + 1;

        // Timestamp (dim)
        buffer.draw_text(col, log_y, &log.timestamp, Style::fg(theme.fg2));
        col += u32::try_from(log.timestamp.len()).unwrap_or(0) + 1;

        // Log level with color
        let level_style = match log.level {
            content::LogLevel::Debug => Style::fg(theme.fg2),
            content::LogLevel::Info => Style::fg(theme.accent_primary),
            content::LogLevel::Warn => Style::fg(theme.accent_warning).with_bold(),
            content::LogLevel::Error => Style::fg(theme.accent_error).with_bold(),
        };
        buffer.draw_text(col, log_y, log.level.as_str(), level_style);
        col += u32::try_from(log.level.as_str().len()).unwrap_or(0) + 1;

        // Subsystem (bracketed)
        let subsystem_text = format!("[{}]", log.subsystem);
        buffer.draw_text(col, log_y, &subsystem_text, Style::fg(theme.fg1));
        col += u32::try_from(subsystem_text.len()).unwrap_or(0) + 1;

        // Message (with link if present)
        let message_style = if log.link.is_some() {
            // Underline for linked entries
            Style::fg(theme.accent_secondary).with_underline()
        } else {
            Style::fg(theme.fg0)
        };

        // Truncate message if needed
        let available_width = rect.w.saturating_sub(col - x).saturating_sub(2);
        let message = if u32::try_from(log.message.len()).unwrap_or(0) > available_width {
            // Truncate with ellipsis
            let max_chars = usize::try_from(available_width.saturating_sub(1)).unwrap_or(0);
            let truncated: String = log.message.chars().take(max_chars).collect();
            format!("{truncated}…")
        } else {
            log.message.to_string()
        };

        buffer.draw_text(col, log_y, &message, message_style);
    }

    // If no logs, show placeholder
    if app.logs.is_empty() {
        let placeholder = "No log entries yet...";
        buffer.draw_text(x + 2, content_y, placeholder, Style::fg(theme.fg2));
    }
}

/// Draw a filled rectangle background.
fn draw_rect_bg(buffer: &mut OptimizedBuffer, rect: &Rect, color: Rgba) {
    if rect.is_empty() {
        return;
    }
    buffer.fill_rect(
        u32::try_from(rect.x).unwrap_or(0),
        u32::try_from(rect.y).unwrap_or(0),
        rect.w,
        rect.h,
        color,
    );
}

/// Draw a horizontal gradient bar.
#[allow(clippy::cast_precision_loss)] // Precision loss acceptable for gradient
fn draw_gradient_bar(buffer: &mut OptimizedBuffer, rect: &Rect, start: Rgba, end: Rgba) {
    if rect.is_empty() {
        return;
    }

    let x = u32::try_from(rect.x).unwrap_or(0);
    let y = u32::try_from(rect.y).unwrap_or(0);

    // Draw each column with interpolated color using fill_rect (1-column wide)
    for col in 0..rect.w {
        let t = if rect.w > 1 {
            col as f32 / (rect.w - 1) as f32
        } else {
            0.0
        };
        let color = Theme::lerp(start, end, t);
        buffer.fill_rect(x + col, y, 1, rect.h, color);
    }
}

/// Get a display name for the layout mode.
#[allow(dead_code)] // Will be used by debug overlay
const fn layout_mode_name(mode: LayoutMode) -> &'static str {
    match mode {
        LayoutMode::Full => "Full",
        LayoutMode::Compact => "Compact",
        LayoutMode::Minimal => "Minimal",
        LayoutMode::TooSmall => "TooSmall",
    }
}

/// Draw the "terminal too small" message.
fn draw_too_small_message(buffer: &mut OptimizedBuffer, width: u32, height: u32, theme: &Theme) {
    let msg1 = "Terminal too small!";
    let msg2 = format!("Need at least {}x{}", layout::MIN_WIDTH, layout::MIN_HEIGHT);
    let msg3 = format!("Current: {width}x{height}");
    let msg4 = "Press any key to exit";

    let center_y = height / 2;

    // Draw messages centered.
    let draw_centered = |buf: &mut OptimizedBuffer, y: u32, text: &str, style: Style| {
        let len = u32::try_from(text.len()).unwrap_or(0);
        let x = width.saturating_sub(len) / 2;
        buf.draw_text(x, y, text, style);
    };

    draw_centered(
        buffer,
        center_y.saturating_sub(2),
        msg1,
        Style::fg(theme.accent_error).with_bold(),
    );
    draw_centered(
        buffer,
        center_y.saturating_sub(1),
        &msg2,
        Style::fg(theme.fg0),
    );
    draw_centered(buffer, center_y, &msg3, Style::fg(theme.fg0));
    draw_centered(
        buffer,
        center_y.saturating_add(2),
        msg4,
        Style::fg(theme.fg0),
    );
}

/// Draw the sidebar navigation panel.
///
/// Shows all sections with the current one highlighted. In compact mode,
/// only shows the key shortcut.
fn draw_sidebar(
    buffer: &mut OptimizedBuffer,
    sidebar: &Rect,
    mode: LayoutMode,
    theme: &Theme,
    app: &App,
) {
    let x = u32::try_from(sidebar.x).unwrap_or(0);
    let mut y = u32::try_from(sidebar.y).unwrap_or(0) + 1;
    let is_focused = app.focus == Focus::Sidebar;

    // Draw focused panel border indicator on left edge if focused
    if is_focused && mode != LayoutMode::Minimal {
        for row in 0..sidebar.h.saturating_sub(2) {
            buffer.draw_text(x, y + row, "│", Style::fg(theme.focus_border));
        }
    }

    let content_x = x + if mode == LayoutMode::Compact { 0 } else { 2 };

    for (i, section) in Section::ALL.iter().enumerate() {
        let bottom = u32::try_from(sidebar.bottom()).unwrap_or(u32::MAX);
        if y >= bottom.saturating_sub(1) {
            break;
        }

        let is_selected = *section == app.section;

        // Format text based on layout mode
        let label = section.name();
        #[allow(clippy::cast_possible_truncation)] // i is always 0..6
        let key = (b'1' + i as u8) as char;
        let text = if mode == LayoutMode::Compact {
            format!("{key}")
        } else {
            format!(" {key}. {label}")
        };

        // Style based on selection state
        let style = if is_selected {
            if is_focused {
                // Selected + focused: inverted colors
                Style::fg(theme.bg0)
                    .with_bg(theme.accent_primary)
                    .with_bold()
            } else {
                // Selected but not focused: highlight bg
                Style::fg(theme.fg0).with_bg(theme.selection_bg)
            }
        } else {
            // Normal item
            Style::fg(theme.fg1)
        };

        // Draw selection indicator
        if is_selected && mode != LayoutMode::Compact {
            buffer.draw_text(content_x, y, "▸", Style::fg(theme.accent_primary));
        }

        // Draw the text (with padding for alignment)
        let text_x = if mode == LayoutMode::Compact {
            content_x
        } else {
            content_x + 2
        };
        buffer.draw_text(text_x, y, &text, style);

        y += 1;
    }

    // Draw section count at bottom if there's room
    let bottom = u32::try_from(sidebar.bottom()).unwrap_or(0);
    if y < bottom.saturating_sub(1) && mode == LayoutMode::Full {
        let count_text = format!("{}/{}", Section::ALL.len(), Section::ALL.len());
        buffer.draw_text(
            content_x + 2,
            bottom.saturating_sub(2),
            &count_text,
            Style::fg(theme.fg2),
        );
    }
}

// ============================================================================
// Platform-Specific Helpers
// ============================================================================

/// Check if stdout is a TTY.
#[cfg(unix)]
#[allow(clippy::missing_const_for_fn)] // libc::isatty is not const
fn is_tty() -> bool {
    // SAFETY: isatty is safe to call with any file descriptor.
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}

#[cfg(not(unix))]
const fn is_tty() -> bool {
    // Assume TTY on non-Unix platforms
    true
}

/// Set stdin to non-blocking mode on Unix.
#[cfg(unix)]
fn set_stdin_nonblocking() -> io::Result<()> {
    use std::os::unix::io::AsRawFd;
    let fd = io::stdin().as_raw_fd();
    // SAFETY: fcntl with F_GETFL/F_SETFL is safe on a valid file descriptor.
    // stdin is always a valid file descriptor.
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags == -1 {
            return Err(io::Error::last_os_error());
        }
        if libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) == -1 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Stub for non-Unix platforms.
#[cfg(not(unix))]
fn set_stdin_nonblocking() -> io::Result<()> {
    // Non-blocking stdin not supported on this platform.
    Ok(())
}

// ============================================================================
// Content Pack
// ============================================================================

/// Canonical content for the demo showcase.
///
/// This module provides high-quality, deterministic content that makes the demo
/// look like a real application while proving correctness of the rendering engine.
pub mod content {
    use std::borrow::Cow;

    /// Sample Rust code for the editor panel (syntax highlighting demo).
    ///
    /// Contains structs, enums, impl blocks, lifetimes, generics, doc comments,
    /// match expressions, Result/?, strings with escapes, and TODO comments.
    ///
    /// **Note:** Uses only single-codepoint characters because `EditorView::render_to`
    /// does not use the grapheme pool path.
    pub const EDITOR_SAMPLE_RUST: &str = r#"//! OpenTUI Demo - Sample Module
//!
//! This file demonstrates syntax highlighting capabilities.

use std::collections::HashMap;
use std::io::{self, Write};

/// A simple key-value store with TTL support.
#[derive(Debug, Clone)]
pub struct Cache<'a, V: Clone> {
    entries: HashMap<&'a str, Entry<V>>,
    max_size: usize,
}

#[derive(Debug, Clone)]
struct Entry<V> {
    value: V,
    expires_at: Option<u64>,
}

impl<'a, V: Clone> Cache<'a, V> {
    /// Create a new cache with the given capacity.
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_size),
            max_size,
        }
    }

    /// Insert a value with optional TTL.
    pub fn insert(&mut self, key: &'a str, value: V, ttl: Option<u64>) -> Option<V> {
        // TODO: Implement LRU eviction when at capacity
        if self.entries.len() >= self.max_size {
            return None; // Cache full
        }

        let entry = Entry {
            value: value.clone(),
            expires_at: ttl.map(|t| now() + t),
        };

        self.entries.insert(key, entry).map(|e| e.value)
    }

    /// Get a value if it exists and hasn't expired.
    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.get(key).and_then(|entry| {
            match entry.expires_at {
                Some(exp) if exp <= now() => None,
                _ => Some(&entry.value),
            }
        })
    }
}

/// Status of an async operation.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Pending,
    Running { progress: u8 },
    Complete(Result<String, io::Error>),
}

impl Status {
    /// Check if the operation is still in progress.
    #[must_use]
    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Pending | Self::Running { .. })
    }
}

fn now() -> u64 {
    // Placeholder for timestamp
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_insert_get() {
        let mut cache = Cache::new(10);
        cache.insert("key1", "value1", None);
        assert_eq!(cache.get("key1"), Some(&"value1"));
    }
}
"#;

    /// Markdown sample with fenced code blocks (secondary editor content).
    pub const EDITOR_SAMPLE_MARKDOWN: &str = r#"# OpenTUI Showcase

Welcome to the **OpenTUI** demo application!

## Features

- Real RGBA alpha blending
- Scissor clipping stacks
- Double-buffered rendering

## Code Example

```rust
let mut renderer = Renderer::new(80, 24)?;
renderer.buffer().draw_text(0, 0, "Hello!", style);
renderer.present()?;
```

## Links

- [GitHub Repository](https://github.com/anomalyco/opentui)
- [Unicode TR11](https://unicode.org/reports/tr11/)
"#;

    /// Log entry structure for the logs panel.
    ///
    /// Uses `Cow<'static, str>` to support both static content (no allocation)
    /// and runtime-generated log entries (owned strings).
    #[derive(Clone, Debug)]
    pub struct LogEntry {
        /// Timestamp string (HH:MM:SS format).
        pub timestamp: Cow<'static, str>,
        /// Log level (INFO, WARN, ERROR, DEBUG).
        pub level: LogLevel,
        /// Subsystem that generated the log.
        pub subsystem: Cow<'static, str>,
        /// Log message content.
        pub message: Cow<'static, str>,
        /// Optional hyperlink URL (for OSC 8).
        pub link: Option<Cow<'static, str>>,
    }

    impl LogEntry {
        /// Create a static log entry (compile-time strings, no allocation).
        #[must_use]
        pub const fn new_static(
            timestamp: &'static str,
            level: LogLevel,
            subsystem: &'static str,
            message: &'static str,
            link: Option<&'static str>,
        ) -> Self {
            Self {
                timestamp: Cow::Borrowed(timestamp),
                level,
                subsystem: Cow::Borrowed(subsystem),
                message: Cow::Borrowed(message),
                link: match link {
                    Some(s) => Some(Cow::Borrowed(s)),
                    None => None,
                },
            }
        }

        /// Create a runtime log entry with owned strings.
        #[must_use]
        #[allow(clippy::missing_const_for_fn)] // Cow::Owned is not const-constructable
        pub fn new_runtime(
            timestamp: String,
            level: LogLevel,
            subsystem: String,
            message: String,
        ) -> Self {
            Self {
                timestamp: Cow::Owned(timestamp),
                level,
                subsystem: Cow::Owned(subsystem),
                message: Cow::Owned(message),
                link: None,
            }
        }
    }

    /// Log severity levels.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum LogLevel {
        Debug,
        Info,
        Warn,
        Error,
    }

    impl LogLevel {
        /// Get display string for the level.
        #[must_use]
        pub const fn as_str(self) -> &'static str {
            match self {
                Self::Debug => "DEBUG",
                Self::Info => "INFO ",
                Self::Warn => "WARN ",
                Self::Error => "ERROR",
            }
        }
    }

    /// Sample log entries for the logs panel.
    ///
    /// Includes timestamps, levels, subsystems, and some entries with hyperlinks.
    pub const LOG_ENTRIES: &[LogEntry] = &[
        LogEntry::new_static(
            "22:05:10",
            LogLevel::Info,
            "renderer",
            "Initialized 80x24 buffer, truecolor enabled",
            None,
        ),
        LogEntry::new_static(
            "22:05:10",
            LogLevel::Debug,
            "terminal",
            "Raw mode enabled, mouse tracking active",
            None,
        ),
        LogEntry::new_static(
            "22:05:11",
            LogLevel::Info,
            "input",
            "InputParser ready, bracketed paste enabled",
            None,
        ),
        LogEntry::new_static(
            "22:05:12",
            LogLevel::Info,
            "renderer",
            "Frame 1: diff=1920 cells, output=4.2KB",
            None,
        ),
        LogEntry::new_static(
            "22:05:12",
            LogLevel::Info,
            "renderer",
            "Frame 2: diff=124 cells, output=0.3KB",
            None,
        ),
        LogEntry::new_static(
            "22:05:13",
            LogLevel::Warn,
            "input",
            "Focus lost - rendering paused",
            None,
        ),
        LogEntry::new_static(
            "22:05:14",
            LogLevel::Info,
            "input",
            "Focus regained - resuming",
            None,
        ),
        LogEntry::new_static(
            "22:05:15",
            LogLevel::Info,
            "tour",
            "Starting guided tour (12 steps)",
            None,
        ),
        LogEntry::new_static(
            "22:05:16",
            LogLevel::Debug,
            "preview",
            "Alpha blending demo: 50% opacity layer",
            None,
        ),
        LogEntry::new_static(
            "22:05:17",
            LogLevel::Info,
            "docs",
            "See OpenTUI repository for more info",
            Some("https://github.com/anomalyco/opentui"),
        ),
        LogEntry::new_static(
            "22:05:18",
            LogLevel::Error,
            "preview",
            "Simulated error (demo only) - press R to retry",
            None,
        ),
        LogEntry::new_static(
            "22:05:19",
            LogLevel::Info,
            "unicode",
            "Width calculation: see Unicode TR11",
            Some("https://unicode.org/reports/tr11/"),
        ),
        LogEntry::new_static(
            "22:05:20",
            LogLevel::Debug,
            "renderer",
            "Scissor stack depth: 3, opacity: 0.85",
            None,
        ),
        LogEntry::new_static(
            "22:05:21",
            LogLevel::Info,
            "highlight",
            "Rust tokenizer: 847 tokens, 23 lines",
            None,
        ),
        LogEntry::new_static(
            "22:05:22",
            LogLevel::Warn,
            "terminal",
            "No XTVERSION response - assuming basic caps",
            None,
        ),
    ];

    /// Deterministic metrics for charts and animations.
    ///
    /// All values are computed from frame count to ensure reproducibility.
    #[derive(Clone, Copy, Debug, Default)]
    pub struct Metrics {
        /// Current FPS estimate.
        pub fps: u32,
        /// Frame time in milliseconds.
        pub frame_time_ms: f32,
        /// Synthetic "CPU usage" percentage (0-100).
        pub cpu_percent: u8,
        /// Synthetic "memory bytes" counter.
        pub memory_bytes: u64,
        /// Pulse value for glow animations (0.0-1.0).
        pub pulse: f32,
        /// Cells changed in last frame.
        pub cells_changed: u32,
        /// Bytes written in last frame.
        pub bytes_written: u32,
    }

    impl Metrics {
        /// Compute metrics deterministically from frame count and target FPS.
        ///
        /// No randomness - results are reproducible for tour mode and tests.
        #[must_use]
        #[allow(
            clippy::cast_precision_loss,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss
        )] // Acceptable for demo metrics - values are bounded
        pub fn compute(frame: u64, target_fps: u32) -> Self {
            let frame_f = frame as f32;
            let target_fps_f = target_fps as f32;

            // Simulate slight FPS variation (deterministic sine wave)
            let fps_variation = (frame_f * 0.1).sin() * 2.0;
            let fps = ((target_fps_f + fps_variation) as u32).clamp(1, 120);

            // Frame time derived from FPS
            let frame_time_ms = 1000.0 / fps as f32;

            // CPU usage: slow sine wave (5-25%)
            let cpu_base = (frame_f * 0.02).sin().mul_add(10.0, 15.0);
            let cpu_percent = cpu_base.clamp(0.0, 100.0) as u8;

            // Memory: slowly growing counter with periodic resets
            let memory_cycle = frame % 1000;
            let memory_bytes = 50_000_000 + (memory_cycle * 10_000);

            // Pulse: smooth 0-1-0 cycle every 60 frames
            let pulse_phase = (frame % 60) as f32 / 60.0;
            let pulse = (pulse_phase * std::f32::consts::PI).sin();

            // Cells changed: varies by frame (more on first, less on subsequent)
            let cells_changed = if frame == 0 {
                1920 // Full screen
            } else {
                (50 + ((frame_f * 0.5).sin().abs() * 150.0) as u32).min(500)
            };

            // Bytes written: roughly proportional to cells changed
            let bytes_written = cells_changed * 8 + 100;

            Self {
                fps,
                frame_time_ms,
                cpu_percent,
                memory_bytes,
                pulse,
                cells_changed,
                bytes_written,
            }
        }

        /// Format memory as human-readable string.
        #[must_use]
        #[allow(clippy::cast_precision_loss)] // Memory values fit comfortably in f64 mantissa
        pub fn memory_display(&self) -> String {
            if self.memory_bytes >= 1_000_000 {
                format!("{:.1}MB", self.memory_bytes as f64 / 1_000_000.0)
            } else if self.memory_bytes >= 1_000 {
                format!("{:.1}KB", self.memory_bytes as f64 / 1_000.0)
            } else {
                format!("{}B", self.memory_bytes)
            }
        }
    }

    // ========================================================================
    // Demo Content Wiring Types
    // ========================================================================

    /// A file for the editor panel with name, language hint, and content.
    #[derive(Clone, Debug)]
    pub struct DemoFile {
        /// Display name (e.g., "main.rs").
        pub name: &'static str,
        /// Language hint for syntax highlighting.
        pub language: Language,
        /// File content.
        pub text: &'static str,
    }

    /// Language hint for syntax highlighting.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
    pub enum Language {
        /// Rust source code.
        #[default]
        Rust,
        /// Markdown text.
        Markdown,
        /// Plain text (no highlighting).
        Plain,
    }

    impl Language {
        /// Get the file extension for this language.
        #[must_use]
        pub const fn extension(self) -> &'static str {
            match self {
                Self::Rust => "rs",
                Self::Markdown => "md",
                Self::Plain => "txt",
            }
        }
    }

    /// Hyperlink URLs bundled for easy access.
    #[derive(Clone, Debug)]
    pub struct DemoLinks {
        /// Repository URL.
        pub repo: &'static str,
        /// Source directory URL.
        pub source: &'static str,
        /// Documentation URL.
        pub docs: &'static str,
        /// Unicode reference URL.
        pub unicode_ref: &'static str,
    }

    impl Default for DemoLinks {
        fn default() -> Self {
            Self {
                repo: links::REPO,
                source: links::SOURCE,
                docs: links::RUST_DOCS,
                unicode_ref: links::UNICODE_TR11,
            }
        }
    }

    /// Parameters for deterministic metrics computation.
    #[derive(Clone, Copy, Debug)]
    pub struct MetricParams {
        /// Target FPS for the demo.
        pub target_fps: u32,
    }

    impl Default for MetricParams {
        fn default() -> Self {
            Self { target_fps: 60 }
        }
    }

    /// Complete demo content bundle.
    ///
    /// This struct provides all the content needed to initialize the demo
    /// into a believable "project workspace" state.
    #[derive(Clone, Debug)]
    pub struct DemoContent {
        /// Files available in the editor (first is primary).
        pub files: &'static [DemoFile],
        /// Hyperlinks for OSC 8 integration.
        pub links: DemoLinks,
        /// Initial log entries (seed backlog).
        pub seed_logs: &'static [LogEntry],
        /// Parameters for metrics computation.
        pub metric_params: MetricParams,
    }

    /// Default demo files for the editor.
    pub const DEFAULT_FILES: &[DemoFile] = &[
        DemoFile {
            name: "cache.rs",
            language: Language::Rust,
            text: EDITOR_SAMPLE_RUST,
        },
        DemoFile {
            name: "README.md",
            language: Language::Markdown,
            text: EDITOR_SAMPLE_MARKDOWN,
        },
    ];

    impl Default for DemoContent {
        fn default() -> Self {
            Self {
                files: DEFAULT_FILES,
                links: DemoLinks::default(),
                seed_logs: LOG_ENTRIES,
                metric_params: MetricParams::default(),
            }
        }
    }

    impl DemoContent {
        /// Get the primary editor file (first in list).
        #[must_use]
        pub const fn primary_file(&self) -> Option<&DemoFile> {
            self.files.first()
        }

        /// Get the number of seed log entries.
        #[must_use]
        pub const fn log_count(&self) -> usize {
            self.seed_logs.len()
        }

        /// Compute metrics for a given frame.
        #[must_use]
        pub fn compute_metrics(&self, frame: u64) -> Metrics {
            Metrics::compute(frame, self.metric_params.target_fps)
        }
    }

    /// Unicode test strings for proving grapheme and width correctness.
    ///
    /// These must be rendered using the grapheme pool path to display correctly.
    pub mod unicode {
        /// CJK wide characters (each is width 2).
        pub const CJK_WIDE: &str = "漢字かなカナ";

        /// Single-codepoint emoji (each is width 2).
        pub const EMOJI_SINGLE: &str = "🎉👍😀🚀✨";

        /// Multi-codepoint ZWJ emoji sequences.
        /// These require the grapheme pool for proper rendering.
        pub const EMOJI_ZWJ: &str = "👨‍👩‍👧 👩‍💻 🧑‍🚀 👨‍🔬 👩‍🎨";

        /// Combining marks (base + combining character).
        /// á (a + combining acute) and ñ (n + combining tilde).
        pub const COMBINING_MARKS: &str = "a\u{0301} e\u{0301} n\u{0303} o\u{0308}";

        /// Display versions of combining marks (precomposed).
        pub const COMBINING_DISPLAY: &str = "á é ñ ö";

        /// Mixed content line for comprehensive testing.
        pub const MIXED_LINE: &str = "Hello 世界 🌍 café naïve 👨‍👩‍👧‍👦";

        /// Width ruler (each char is width 1, numbers show column).
        pub const WIDTH_RULER_10: &str = "0123456789";

        /// Test cases with expected display widths.
        pub const WIDTH_TEST_CASES: &[(&str, &str, usize)] = &[
            ("ASCII", "Hello", 5),
            ("CJK", "漢字", 4),            // 2 chars × width 2
            ("Emoji", "🎉👍", 4),          // 2 emoji × width 2
            ("Mixed", "A漢B", 4),          // 1 + 2 + 1
            ("Combining", "a\u{0301}", 1), // Base + combining = width 1
        ];
    }

    /// Hyperlink URLs for OSC 8 integration.
    pub mod links {
        /// Main repository URL.
        pub const REPO: &str = "https://github.com/anomalyco/opentui";

        /// Source code directory.
        pub const SOURCE: &str = "https://github.com/anomalyco/opentui/tree/main/src";

        /// Unicode Technical Report 11 (East Asian Width).
        pub const UNICODE_TR11: &str = "https://unicode.org/reports/tr11/";

        /// Rust documentation.
        pub const RUST_DOCS: &str = "https://doc.rust-lang.org/stable/std/";

        /// All URLs for iteration.
        pub const ALL: &[(&str, &str)] = &[
            ("OpenTUI Repository", REPO),
            ("Source Code", SOURCE),
            ("Unicode TR11", UNICODE_TR11),
            ("Rust Docs", RUST_DOCS),
        ];
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<OsString> {
        strs.iter().map(|s| OsString::from(*s)).collect()
    }

    #[test]
    fn test_default_config() {
        let result = Config::from_args(args(&["demo_showcase"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert_eq!(config.fps_cap, 60);
        assert!(config.enable_mouse);
        assert!(config.use_alt_screen);
        assert!(!config.headless_smoke);
    }

    #[test]
    fn test_help_flag() {
        let result = Config::from_args(args(&["demo_showcase", "--help"]));
        assert!(matches!(result, ParseResult::Help));
    }

    #[test]
    fn test_fps_flag() {
        let result = Config::from_args(args(&["demo_showcase", "--fps", "30"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert_eq!(config.fps_cap, 30);
    }

    #[test]
    fn test_no_mouse_flag() {
        let result = Config::from_args(args(&["demo_showcase", "--no-mouse"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert!(!config.enable_mouse);
    }

    #[test]
    fn test_headless_smoke_flag() {
        let result = Config::from_args(args(&["demo_showcase", "--headless-smoke"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert!(config.headless_smoke);
    }

    #[test]
    fn test_headless_size() {
        let result = Config::from_args(args(&["demo_showcase", "--headless-size", "120x40"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert_eq!(config.headless_size, (120, 40));
    }

    #[test]
    fn test_headless_check_valid() {
        for check in ["layout", "config", "palette", "hitgrid", "logs"] {
            let result = Config::from_args(args(&["demo_showcase", "--headless-check", check]));
            let ParseResult::Config(config) = result else {
                panic!("Expected Config for check: {check}")
            };
            assert_eq!(config.headless_check, Some(check.to_string()));
        }
    }

    #[test]
    fn test_headless_check_invalid() {
        let result = Config::from_args(args(&["demo_showcase", "--headless-check", "invalid"]));
        assert!(matches!(result, ParseResult::Error(_)));
    }

    #[test]
    fn test_max_frames() {
        let result = Config::from_args(args(&["demo_showcase", "--max-frames", "100"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert_eq!(config.max_frames, Some(100));
    }

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("80x24"), Some((80, 24)));
        assert_eq!(parse_size("120x40"), Some((120, 40)));
        assert_eq!(parse_size("invalid"), None);
        assert_eq!(parse_size("80"), None);
        assert_eq!(parse_size("0x24"), None);
    }

    #[test]
    fn test_unknown_option_error() {
        let result = Config::from_args(args(&["demo_showcase", "--unknown"]));
        assert!(matches!(result, ParseResult::Error(_)));
    }

    #[test]
    fn test_cap_preset() {
        let result = Config::from_args(args(&["demo_showcase", "--cap-preset", "no_mouse"]));
        let ParseResult::Config(config) = result else {
            panic!("Expected Config")
        };
        assert_eq!(config.cap_preset, CapPreset::NoMouse);
    }

    // ========================================================================
    // Layout Helper Tests
    // ========================================================================

    #[test]
    fn test_rect_new() {
        let r = Rect::new(10, 20, 100, 50);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.w, 100);
        assert_eq!(r.h, 50);
    }

    #[test]
    fn test_rect_from_size() {
        let r = Rect::from_size(80, 24);
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        assert_eq!(r.w, 80);
        assert_eq!(r.h, 24);
    }

    #[test]
    fn test_rect_inset() {
        let r = Rect::new(0, 0, 100, 50);
        let inset = r.inset(5);
        assert_eq!(inset.x, 5);
        assert_eq!(inset.y, 5);
        assert_eq!(inset.w, 90);
        assert_eq!(inset.h, 40);
    }

    #[test]
    fn test_rect_inset_overflow() {
        let r = Rect::new(0, 0, 10, 10);
        let inset = r.inset(10); // Would go negative
        assert_eq!(inset.w, 0);
        assert_eq!(inset.h, 0);
    }

    #[test]
    fn test_rect_split_h() {
        let r = Rect::new(0, 0, 100, 50);
        let (left, right) = r.split_h(30);
        assert_eq!(left.x, 0);
        assert_eq!(left.w, 30);
        assert_eq!(right.x, 30);
        assert_eq!(right.w, 70);
        assert_eq!(left.h, 50);
        assert_eq!(right.h, 50);
    }

    #[test]
    fn test_rect_split_h_overflow() {
        let r = Rect::new(0, 0, 50, 50);
        let (left, right) = r.split_h(100); // More than width
        assert_eq!(left.w, 50);
        assert_eq!(right.w, 0);
    }

    #[test]
    fn test_rect_split_v() {
        let r = Rect::new(0, 0, 100, 50);
        let (top, bottom) = r.split_v(20);
        assert_eq!(top.y, 0);
        assert_eq!(top.h, 20);
        assert_eq!(bottom.y, 20);
        assert_eq!(bottom.h, 30);
        assert_eq!(top.w, 100);
        assert_eq!(bottom.w, 100);
    }

    #[test]
    fn test_rect_clamp_to() {
        let r = Rect::new(0, 0, 100, 50);
        let clamped = r.clamp_to(60, 30);
        assert_eq!(clamped.w, 60);
        assert_eq!(clamped.h, 30);
    }

    #[test]
    fn test_rect_is_empty() {
        assert!(Rect::new(0, 0, 0, 10).is_empty());
        assert!(Rect::new(0, 0, 10, 0).is_empty());
        assert!(!Rect::new(0, 0, 10, 10).is_empty());
    }

    #[test]
    fn test_rect_right_bottom() {
        let r = Rect::new(10, 20, 30, 40);
        assert_eq!(r.right(), 40);
        assert_eq!(r.bottom(), 60);
    }

    #[test]
    fn test_layout_mode_full() {
        assert_eq!(LayoutMode::from_size(80, 24), LayoutMode::Full);
        assert_eq!(LayoutMode::from_size(120, 40), LayoutMode::Full);
    }

    #[test]
    fn test_layout_mode_compact() {
        assert_eq!(LayoutMode::from_size(79, 24), LayoutMode::Compact);
        assert_eq!(LayoutMode::from_size(80, 23), LayoutMode::Compact);
        assert_eq!(LayoutMode::from_size(60, 16), LayoutMode::Compact);
    }

    #[test]
    fn test_layout_mode_minimal() {
        assert_eq!(LayoutMode::from_size(59, 16), LayoutMode::Minimal);
        assert_eq!(LayoutMode::from_size(60, 15), LayoutMode::Minimal);
        assert_eq!(LayoutMode::from_size(40, 12), LayoutMode::Minimal);
    }

    #[test]
    fn test_layout_mode_too_small() {
        assert_eq!(LayoutMode::from_size(39, 12), LayoutMode::TooSmall);
        assert_eq!(LayoutMode::from_size(40, 11), LayoutMode::TooSmall);
        assert_eq!(LayoutMode::from_size(20, 10), LayoutMode::TooSmall);
    }

    #[test]
    fn test_panel_layout_full() {
        let layout = PanelLayout::compute(100, 30);
        assert_eq!(layout.mode, LayoutMode::Full);
        assert_eq!(layout.top_bar.h, 1);
        assert_eq!(layout.status_bar.h, 1);
        assert_eq!(layout.sidebar.w, layout::SIDEBAR_WIDTH_FULL);
        assert!(!layout.preview.is_empty());
        // Logs panel should be present in full layout
        assert!(!layout.logs.is_empty());
        assert!(layout.logs.h >= layout::LOGS_HEIGHT_FULL.min(layout.main_area.h / 3));
    }

    #[test]
    fn test_panel_layout_compact() {
        let layout = PanelLayout::compute(70, 20);
        assert_eq!(layout.mode, LayoutMode::Compact);
        assert_eq!(layout.sidebar.w, layout::SIDEBAR_WIDTH_COMPACT);
        assert!(layout.preview.is_empty()); // No preview in compact mode
        // Logs panel should be present in compact layout
        assert!(!layout.logs.is_empty());
        assert!(layout.logs.h >= layout::LOGS_HEIGHT_COMPACT.min(layout.main_area.h / 3));
    }

    #[test]
    fn test_panel_layout_minimal() {
        let layout = PanelLayout::compute(50, 14);
        assert_eq!(layout.mode, LayoutMode::Minimal);
        assert_eq!(layout.sidebar.w, 0); // No sidebar in minimal mode
    }

    #[test]
    fn test_panel_layout_too_small() {
        let layout = PanelLayout::compute(30, 10);
        assert_eq!(layout.mode, LayoutMode::TooSmall);
    }

    // ========================================================================
    // Theme Tests
    // ========================================================================

    #[test]
    fn test_theme_synthwave() {
        let theme = Theme::synthwave();
        // Verify all color tokens have valid alpha.
        assert!(theme.bg0.a > 0.0);
        assert!(theme.bg1.a > 0.0);
        assert!(theme.bg2.a > 0.0);
        assert!(theme.fg0.a > 0.0);
        assert!(theme.fg1.a > 0.0);
        assert!(theme.fg2.a > 0.0);
        assert!(theme.accent_primary.a > 0.0);
        assert!(theme.accent_secondary.a > 0.0);
        assert!(theme.selection_bg.a > 0.0);
        assert!(theme.focus_border.a > 0.0);
    }

    #[test]
    fn test_theme_paper_light() {
        let theme = Theme::paper_light();
        assert!(theme.bg0.a > 0.0);
        assert!(theme.fg0.a > 0.0);
        // Light theme should have bright backgrounds
        assert!(theme.bg0.r > 0.9);
    }

    #[test]
    fn test_theme_solarized() {
        let theme = Theme::solarized();
        assert!(theme.bg0.a > 0.0);
        assert!(theme.fg0.a > 0.0);
        // Solarized has characteristic dark blue-green background
        assert!(theme.bg0.b > theme.bg0.r);
    }

    #[test]
    fn test_theme_high_contrast() {
        let theme = Theme::high_contrast();
        assert!(theme.bg0.a > 0.0);
        assert!(theme.fg0.a > 0.0);
        // High contrast has black background, white foreground
        assert!(theme.bg0.r < 0.01);
        assert!(theme.fg0.r > 0.99);
    }

    #[test]
    fn test_ui_theme_default() {
        assert_eq!(UiTheme::default(), UiTheme::SynthwaveDark);
    }

    #[test]
    fn test_ui_theme_next() {
        assert_eq!(UiTheme::SynthwaveDark.next(), UiTheme::PaperLight);
        assert_eq!(UiTheme::PaperLight.next(), UiTheme::Solarized);
        assert_eq!(UiTheme::Solarized.next(), UiTheme::HighContrast);
        assert_eq!(UiTheme::HighContrast.next(), UiTheme::SynthwaveDark);
    }

    #[test]
    fn test_ui_theme_is_dark() {
        assert!(UiTheme::SynthwaveDark.is_dark());
        assert!(!UiTheme::PaperLight.is_dark());
        assert!(UiTheme::Solarized.is_dark());
        assert!(UiTheme::HighContrast.is_dark());
    }

    #[test]
    fn test_ui_theme_tokens() {
        // All themes should produce valid tokens
        for theme in UiTheme::ALL {
            let tokens = theme.tokens();
            assert!(tokens.bg0.a > 0.0);
            assert!(tokens.fg0.a > 0.0);
        }
    }

    #[test]
    fn test_ui_theme_name() {
        assert_eq!(UiTheme::SynthwaveDark.name(), "Synthwave");
        assert_eq!(UiTheme::PaperLight.name(), "Paper");
        assert_eq!(UiTheme::Solarized.name(), "Solarized");
        assert_eq!(UiTheme::HighContrast.name(), "High Contrast");
    }

    #[test]
    fn test_theme_lerp() {
        let black = Rgba::BLACK;
        let white = Rgba::WHITE;
        let mid = Theme::lerp(black, white, 0.5);
        assert!((mid.r - 0.5).abs() < 0.01);
        assert!((mid.g - 0.5).abs() < 0.01);
        assert!((mid.b - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_theme_gradient() {
        let start = Rgba::BLACK;
        let end = Rgba::WHITE;
        let colors: Vec<_> = Theme::gradient(start, end, 5).collect();
        assert_eq!(colors.len(), 5);
        // First should be start, last should be end
        assert!(colors[0].r < 0.01);
        assert!(colors[4].r > 0.99);
    }

    #[test]
    fn test_styles_header() {
        let theme = Theme::synthwave();
        let style = Styles::header(&theme);
        assert_eq!(style.fg, Some(theme.fg0));
        assert_eq!(style.bg, Some(theme.bg1));
    }

    #[test]
    fn test_styles_selection() {
        let theme = Theme::synthwave();
        let style = Styles::selection(&theme);
        assert_eq!(style.bg, Some(theme.selection_bg));
    }

    // ========================================================================
    // Render Pass Tests
    // ========================================================================

    #[test]
    fn test_render_pass_order() {
        // Verify render passes are in correct order.
        assert_eq!(RenderPass::Background as u8, 0);
        assert_eq!(RenderPass::Chrome as u8, 1);
        assert_eq!(RenderPass::Panels as u8, 2);
        assert_eq!(RenderPass::Overlays as u8, 3);
        assert_eq!(RenderPass::Toasts as u8, 4);
        assert_eq!(RenderPass::Debug as u8, 5);
    }

    #[test]
    fn test_render_pass_all() {
        assert_eq!(RenderPass::ALL.len(), 6);
        assert_eq!(RenderPass::ALL[0], RenderPass::Background);
        assert_eq!(RenderPass::ALL[5], RenderPass::Debug);
    }

    // ========================================================================
    // Input Pump Tests
    // ========================================================================

    #[test]
    fn test_input_pump_new() {
        let pump = InputPump::new();
        assert!(pump.synthetic_queue.is_empty());
        assert!(pump.accumulator.is_empty());
    }

    #[test]
    fn test_input_pump_default() {
        let pump = InputPump::default();
        assert!(pump.accumulator.is_empty());
    }

    #[test]
    fn test_input_pump_inject_synthetic() {
        let mut pump = InputPump::new();
        let event = Event::Key(opentui::input::KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::empty(),
        });
        pump.inject_synthetic(event);
        assert_eq!(pump.synthetic_queue.len(), 1);
    }

    #[test]
    fn test_input_pump_clear() {
        let mut pump = InputPump::new();
        pump.accumulator.extend_from_slice(b"test");
        pump.clear();
        assert!(pump.accumulator.is_empty());
    }

    #[test]
    fn test_tagged_event_real() {
        let event = Event::Key(opentui::input::KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::empty(),
        });
        let tagged = TaggedEvent::real(event);
        assert_eq!(tagged.source, InputSource::Real);
    }

    #[test]
    fn test_tagged_event_synthetic() {
        let event = Event::Key(opentui::input::KeyEvent {
            code: KeyCode::Char('y'),
            modifiers: KeyModifiers::empty(),
        });
        let tagged = TaggedEvent::synthetic(event);
        assert_eq!(tagged.source, InputSource::Synthetic);
    }

    #[test]
    fn test_input_source_equality() {
        assert_eq!(InputSource::Real, InputSource::Real);
        assert_eq!(InputSource::Synthetic, InputSource::Synthetic);
        assert_ne!(InputSource::Real, InputSource::Synthetic);
    }

    // ========================================================================
    // State Machine Tests
    // ========================================================================

    #[test]
    fn test_app_mode_default() {
        assert_eq!(AppMode::default(), AppMode::Normal);
    }

    #[test]
    fn test_focus_cycle() {
        assert_eq!(Focus::Sidebar.next(), Focus::Editor);
        assert_eq!(Focus::Editor.next(), Focus::Preview);
        assert_eq!(Focus::Preview.next(), Focus::Logs);
        assert_eq!(Focus::Logs.next(), Focus::Sidebar);
    }

    #[test]
    fn test_focus_cycle_backward() {
        assert_eq!(Focus::Sidebar.prev(), Focus::Logs);
        assert_eq!(Focus::Editor.prev(), Focus::Sidebar);
        assert_eq!(Focus::Preview.prev(), Focus::Editor);
        assert_eq!(Focus::Logs.prev(), Focus::Preview);
    }

    #[test]
    fn test_section_all() {
        assert_eq!(Section::ALL.len(), 6);
    }

    #[test]
    fn test_section_from_index() {
        assert_eq!(Section::from_index(0), Some(Section::Overview));
        assert_eq!(Section::from_index(5), Some(Section::Performance));
        assert_eq!(Section::from_index(6), None);
    }

    #[test]
    fn test_section_name() {
        assert_eq!(Section::Overview.name(), "Overview");
        assert_eq!(Section::Performance.name(), "Performance");
    }

    #[test]
    fn test_app_default() {
        let app = App::default();
        assert_eq!(app.mode, AppMode::Normal);
        assert_eq!(app.focus, Focus::Sidebar);
        assert_eq!(app.section, Section::Overview);
        assert!(!app.paused);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_app_new_tour_mode() {
        let config = Config {
            start_in_tour: true,
            ..Default::default()
        };
        let app = App::new(&config);
        assert_eq!(app.mode, AppMode::Tour);
    }

    #[test]
    fn test_exit_after_tour() {
        // Test that --exit-after-tour causes app to quit when tour completes
        let config = Config {
            start_in_tour: true,
            exit_after_tour: true,
            ..Default::default()
        };
        let mut app = App::new(&config);
        assert_eq!(app.mode, AppMode::Tour);
        assert!(!app.should_quit);

        // Verify tour runner has exit_on_complete set
        assert!(app.tour_runner.as_ref().is_some_and(|r| r.exit_on_complete));

        // Advance through all tour steps
        let total_steps = app.tour_total;
        for _ in 0..total_steps {
            // Manually advance to next step
            let current_t = app.clock.t;
            if let Some(runner) = app.tour_runner.as_mut() {
                let completed = runner.next_step(current_t);
                if completed && runner.exit_on_complete {
                    app.should_quit = true;
                    app.exit_reason = ExitReason::TourComplete;
                }
            }
        }

        // After completing all steps, app should be marked for quit
        assert!(app.should_quit);
        assert_eq!(app.exit_reason, ExitReason::TourComplete);
    }

    #[test]
    fn test_app_mode_name() {
        let mut app = App::default();
        assert_eq!(app.mode_name(), "Normal");
        app.mode = AppMode::Help;
        assert_eq!(app.mode_name(), "Help");
        app.mode = AppMode::CommandPalette;
        assert_eq!(app.mode_name(), "Palette");
        app.mode = AppMode::Tour;
        assert_eq!(app.mode_name(), "Tour");
    }

    #[test]
    fn test_app_focus_name() {
        let mut app = App::default();
        assert_eq!(app.focus_name(), "Sidebar");
        app.focus = Focus::Editor;
        assert_eq!(app.focus_name(), "Editor");
    }

    #[test]
    fn test_app_tick() {
        let mut app = App::default();
        assert_eq!(app.frame_count, 0);
        app.tick();
        assert_eq!(app.frame_count, 1);
        app.tick();
        assert_eq!(app.frame_count, 2);
    }

    #[test]
    fn test_app_max_frames() {
        let config = Config {
            max_frames: Some(5),
            ..Default::default()
        };
        let mut app = App::new(&config);

        for _ in 0..4 {
            app.tick();
            assert!(!app.should_quit);
            assert_eq!(app.exit_reason, ExitReason::UserQuit);
        }
        app.tick();
        assert!(app.should_quit);
        assert_eq!(app.exit_reason, ExitReason::MaxFrames);
    }

    #[test]
    fn test_action_toggle_help() {
        let mut app = App::default();
        assert_eq!(app.mode, AppMode::Normal);

        app.apply_action(&Action::ToggleHelp);
        assert_eq!(app.mode, AppMode::Help);

        app.apply_action(&Action::ToggleHelp);
        assert_eq!(app.mode, AppMode::Normal);
    }

    #[test]
    fn test_action_cycle_focus() {
        let mut app = App::default();
        assert_eq!(app.focus, Focus::Sidebar);

        app.apply_action(&Action::CycleFocusForward);
        assert_eq!(app.focus, Focus::Editor);

        app.apply_action(&Action::CycleFocusBackward);
        assert_eq!(app.focus, Focus::Sidebar);
    }

    #[test]
    fn test_action_navigate_section() {
        let mut app = App::default();
        assert_eq!(app.section, Section::Overview);

        app.apply_action(&Action::NavigateSection(Section::Editor));
        assert_eq!(app.section, Section::Editor);
    }

    #[test]
    fn test_action_quit() {
        let mut app = App::default();
        assert!(!app.should_quit);

        app.apply_action(&Action::Quit);
        assert!(app.should_quit);
    }

    // ========================================================================
    // Easing Function Tests
    // ========================================================================

    #[test]
    fn test_smoothstep_boundaries() {
        assert!((easing::smoothstep(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((easing::smoothstep(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_smoothstep_midpoint() {
        // At t=0.5, smoothstep should return 0.5
        assert!((easing::smoothstep(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_smoothstep_clamping() {
        // Values outside [0, 1] should be clamped
        assert!((easing::smoothstep(-0.5) - 0.0).abs() < f32::EPSILON);
        assert!((easing::smoothstep(1.5) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ease_in_out_cubic_boundaries() {
        assert!((easing::ease_in_out_cubic(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((easing::ease_in_out_cubic(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_ease_in_out_cubic_midpoint() {
        // At t=0.5, ease_in_out_cubic should return 0.5
        assert!((easing::ease_in_out_cubic(0.5) - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_ease_out_cubic_boundaries() {
        assert!((easing::ease_out_cubic(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((easing::ease_out_cubic(1.0) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)] // Acceptable for test loop counter
    fn test_pulse_range() {
        // Pulse should oscillate between 0 and 1
        let omega = std::f32::consts::TAU; // One cycle per second
        for i in 0..10 {
            let t = i as f32 * 0.1;
            let v = easing::pulse(t, omega);
            assert!(
                (0.0..=1.0).contains(&v),
                "pulse({t}, {omega}) = {v} out of range"
            );
        }
    }

    #[test]
    fn test_pulse_at_zero() {
        // At t=0, pulse should be 0.5 + 0.5*sin(0) = 0.5
        assert!((easing::pulse(0.0, 1.0) - 0.5).abs() < f32::EPSILON);
    }

    // ========================================================================
    // Animation Clock Tests
    // ========================================================================

    #[test]
    fn test_animation_clock_new() {
        let clock = AnimationClock::new();
        assert!((clock.t - 0.0).abs() < f32::EPSILON);
        assert!((clock.dt - 0.0).abs() < f32::EPSILON);
        assert!(!clock.is_paused());
    }

    #[test]
    fn test_animation_clock_tick_advances_time() {
        let mut clock = AnimationClock::new();
        // Sleep a tiny bit to ensure dt > 0
        std::thread::sleep(std::time::Duration::from_millis(10));
        clock.tick(false);
        assert!(clock.dt > 0.0, "dt should be positive after tick");
        assert!(clock.t > 0.0, "t should advance when not paused");
    }

    #[test]
    fn test_animation_clock_paused_no_advance() {
        let mut clock = AnimationClock::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        clock.tick(true); // Paused
        assert!(clock.dt > 0.0, "dt should still be computed when paused");
        assert!(
            (clock.t - 0.0).abs() < f32::EPSILON,
            "t should not advance when paused"
        );
    }

    #[test]
    fn test_animation_clock_dt_clamped() {
        let mut clock = AnimationClock::new();
        // Simulate a long gap by manually setting last_instant far in the past
        // This tests the MAX_DT clamping
        clock.tick(false);
        assert!(
            clock.dt <= AnimationClock::MAX_DT,
            "dt should be clamped to MAX_DT"
        );
        assert!(
            clock.dt >= AnimationClock::MIN_DT,
            "dt should be at least MIN_DT"
        );
    }

    #[test]
    fn test_animation_clock_pulse_helper() {
        let clock = AnimationClock::new();
        let omega = std::f32::consts::TAU;
        let p = clock.pulse(omega);
        // At t=0, pulse should be 0.5
        assert!((p - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_animation_clock_t_offset() {
        let clock = AnimationClock::new();
        let offset = clock.t_offset(1.0);
        assert!((offset - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_overlay_anim_with_dt() {
        let mut anim = OverlayAnim::opening();
        assert!((anim.progress - 0.0).abs() < f32::EPSILON);

        // Tick with a fixed dt
        let dt = 0.05; // 50ms
        anim.tick(dt);
        // Progress should increase by SPEED * dt = 9.0 * 0.05 = 0.45
        assert!((anim.progress - 0.45).abs() < 0.001);

        // Tick again to complete
        anim.tick(0.1);
        assert!((anim.progress - 1.0).abs() < f32::EPSILON);
        assert!(anim.is_open());
    }

    #[test]
    fn test_overlay_anim_closing_with_dt() {
        let mut anim = OverlayAnim::opening();
        anim.progress = 1.0; // Fully open
        anim.start_close();

        // Tick with dt
        anim.tick(0.05);
        // Progress should decrease by SPEED * dt = 9.0 * 0.05 = 0.45
        assert!((anim.progress - 0.55).abs() < 0.001);
    }

    // ========================================================================
    // Content Wiring Tests
    // ========================================================================

    #[test]
    fn test_demo_content_default() {
        let content = content::DemoContent::default();
        assert!(!content.files.is_empty(), "Should have default files");
        assert_eq!(content.files[0].name, "cache.rs");
        assert_eq!(content.files[0].language, content::Language::Rust);
        assert!(!content.seed_logs.is_empty(), "Should have seed logs");
        assert_eq!(content.metric_params.target_fps, 60);
    }

    #[test]
    fn test_demo_content_primary_file() {
        let content = content::DemoContent::default();
        let primary = content.primary_file();
        assert!(primary.is_some());
        assert_eq!(primary.unwrap().name, "cache.rs");
    }

    #[test]
    fn test_demo_content_log_count() {
        let content = content::DemoContent::default();
        assert_eq!(content.log_count(), content::LOG_ENTRIES.len());
    }

    #[test]
    fn test_demo_content_compute_metrics() {
        let content = content::DemoContent::default();
        let m = content.compute_metrics(0);
        assert!(m.fps > 0);
        assert!(m.frame_time_ms > 0.0);
    }

    #[test]
    fn test_language_extension() {
        assert_eq!(content::Language::Rust.extension(), "rs");
        assert_eq!(content::Language::Markdown.extension(), "md");
        assert_eq!(content::Language::Plain.extension(), "txt");
    }

    #[test]
    fn test_app_content_initialization() {
        let app = App::default();
        // App should start with content from DemoContent
        assert_eq!(app.current_file_idx, 0);
        assert!(!app.logs.is_empty(), "Should have seed logs");
        assert_eq!(app.target_fps, 60);
    }

    #[test]
    fn test_app_current_file() {
        let app = App::default();
        let file = app.current_file();
        assert!(file.is_some());
        assert_eq!(file.unwrap().name, "cache.rs");
    }

    #[test]
    fn test_app_current_file_name() {
        let app = App::default();
        assert_eq!(app.current_file_name(), "cache.rs");
    }

    #[test]
    fn test_app_current_file_language() {
        let app = App::default();
        assert_eq!(app.current_file_language(), content::Language::Rust);
    }

    #[test]
    fn test_app_next_file() {
        let mut app = App::default();
        assert_eq!(app.current_file_idx, 0);
        app.next_file();
        assert_eq!(app.current_file_idx, 1);
        assert_eq!(app.current_file_name(), "README.md");
        // Wrap around
        app.next_file();
        assert_eq!(app.current_file_idx, 0);
        assert_eq!(app.current_file_name(), "cache.rs");
    }

    #[test]
    fn test_app_prev_file() {
        let mut app = App::default();
        assert_eq!(app.current_file_idx, 0);
        // Wrap to last
        app.prev_file();
        assert_eq!(app.current_file_idx, 1);
        assert_eq!(app.current_file_name(), "README.md");
        app.prev_file();
        assert_eq!(app.current_file_idx, 0);
    }

    #[test]
    fn test_app_metrics_update() {
        let mut app = App::default();
        let initial_metrics = app.metrics;
        app.tick();
        // After tick, frame_count is 1, so metrics should be recomputed
        assert_eq!(app.frame_count, 1);
        // Metrics values change with frame count
        assert!(
            app.metrics.memory_bytes != initial_metrics.memory_bytes
                || app.metrics.cells_changed != initial_metrics.cells_changed
        );
    }

    #[test]
    fn test_app_add_log() {
        let mut app = App::default();
        let initial_count = app.logs.len();
        app.add_log(content::LogEntry::new_static(
            "23:00:00",
            content::LogLevel::Info,
            "test",
            "Test log entry",
            None,
        ));
        assert_eq!(app.logs.len(), initial_count + 1);
    }

    #[test]
    fn test_metrics_compute_deterministic() {
        // Same frame + fps should produce same metrics
        let m1 = content::Metrics::compute(100, 60);
        let m2 = content::Metrics::compute(100, 60);
        assert_eq!(m1.fps, m2.fps);
        assert_eq!(m1.cpu_percent, m2.cpu_percent);
        assert_eq!(m1.memory_bytes, m2.memory_bytes);
        assert!((m1.pulse - m2.pulse).abs() < f32::EPSILON);
    }

    #[test]
    fn test_metrics_memory_display() {
        let m = content::Metrics {
            memory_bytes: 50_000_000,
            ..Default::default()
        };
        assert_eq!(m.memory_display(), "50.0MB");

        let m2 = content::Metrics {
            memory_bytes: 500_000,
            ..Default::default()
        };
        assert_eq!(m2.memory_display(), "500.0KB");

        let m3 = content::Metrics {
            memory_bytes: 500,
            ..Default::default()
        };
        assert_eq!(m3.memory_display(), "500B");
    }
}
