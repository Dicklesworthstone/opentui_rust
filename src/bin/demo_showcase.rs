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

use opentui::buffer::OptimizedBuffer;
use opentui::input::{Event, InputParser, KeyCode, KeyModifiers};
use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Renderer, RendererOptions, Rgba, Style};
use std::ffi::OsString;
use std::io::{self, Read};
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

    --cap-preset <NAME>     Capability preset: auto, ideal, no_truecolor,
                            no_mouse, minimal (default: auto)

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
    NoMouse,
    Minimal,
}

impl CapPreset {
    fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "ideal" => Some(Self::Ideal),
            "no_truecolor" | "notruecolor" => Some(Self::NoTruecolor),
            "no_mouse" | "nomouse" => Some(Self::NoMouse),
            "minimal" => Some(Self::Minimal),
            _ => None,
        }
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
                            ))
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
                            return ParseResult::Error("--max-frames requires a value".to_string())
                        }
                    };
                    match value.parse::<u64>() {
                        Ok(n) => config.max_frames = Some(n),
                        Err(_) => {
                            return ParseResult::Error(format!(
                                "Invalid --max-frames value: {value}"
                            ))
                        }
                    }
                }

                "--exit-after-tour" => config.exit_after_tour = true,

                "--headless-smoke" => config.headless_smoke = true,

                "--headless-size" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => {
                            return ParseResult::Error(
                                "--headless-size requires a value (e.g., 80x24)".to_string(),
                            )
                        }
                    };
                    match parse_size(&value) {
                        Some((w, h)) => config.headless_size = (w, h),
                        None => {
                            return ParseResult::Error(format!(
                                "Invalid --headless-size: {value} (use WxH format, e.g., 80x24)"
                            ))
                        }
                    }
                }

                "--cap-preset" => {
                    let value = match args.next() {
                        Some(v) => v.to_string_lossy().to_string(),
                        None => {
                            return ParseResult::Error("--cap-preset requires a value".to_string())
                        }
                    };
                    match CapPreset::from_str(&value) {
                        Some(preset) => config.cap_preset = preset,
                        None => {
                            return ParseResult::Error(format!(
                                "Unknown --cap-preset: {value} \
                                 (valid: auto, ideal, no_truecolor, no_mouse, minimal)"
                            ))
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
                            return ParseResult::Error(format!("Invalid --seed value: {value}"))
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
        Style::builder()
            .fg(theme.fg0)
            .bg(theme.bg1)
            .bold()
            .build()
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
        Style::builder().fg(theme.fg0).bg(theme.selection_bg).build()
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
        Style::builder().fg(theme.accent_primary).underline().build()
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
    /// Editor panel (left portion of main area in Full mode).
    pub editor: Rect,
    /// Preview panel (right portion of main area in Full mode).
    pub preview: Rect,
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

        // Editor/Preview split only in Full mode.
        let (editor, preview) = if mode == LayoutMode::Full && main_area.w > layout::EDITOR_MIN_WIDTH {
            let preview_w = main_area.w * layout::PREVIEW_WIDTH_RATIO / 100;
            let editor_w = main_area.w.saturating_sub(preview_w);
            main_area.split_h(editor_w)
        } else {
            // Compact/Minimal: editor takes all main area, no preview.
            (main_area, Rect::default())
        };

        Self {
            mode,
            screen,
            top_bar,
            status_bar,
            content,
            sidebar,
            main_area,
            editor,
            preview,
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
}

impl Default for App {
    fn default() -> Self {
        Self {
            mode: AppMode::Normal,
            focus: Focus::Sidebar,
            section: Section::Overview,
            paused: false,
            ui_theme: UiTheme::default(),
            should_quit: false,
            frame_count: 0,
            max_frames: None,
            show_debug: false,
            force_redraw: false,
            tour_step: 0,
            tour_total: 12, // Placeholder tour length
        }
    }
}

impl App {
    /// Create a new app instance from config.
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Self {
            max_frames: config.max_frames,
            mode: if config.start_in_tour {
                AppMode::Tour
            } else {
                AppMode::Normal
            },
            ..Self::default()
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
            AppMode::Help | AppMode::CommandPalette => {
                if key.code == KeyCode::Esc {
                    return Action::CloseOverlay;
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

    /// Apply an action to update state.
    fn apply_action(&mut self, action: &Action) {
        match action {
            Action::Quit => {
                self.should_quit = true;
            }
            Action::ToggleHelp => {
                self.mode = if self.mode == AppMode::Help {
                    AppMode::Normal
                } else {
                    AppMode::Help
                };
            }
            Action::TogglePalette => {
                self.mode = if self.mode == AppMode::CommandPalette {
                    AppMode::Normal
                } else {
                    AppMode::CommandPalette
                };
            }
            Action::ToggleTour => {
                if self.mode == AppMode::Tour {
                    self.mode = AppMode::Normal;
                } else {
                    self.mode = AppMode::Tour;
                    self.tour_step = 0;
                }
            }
            Action::CloseOverlay => {
                self.mode = AppMode::Normal;
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
            // Resize is handled in render loop, None is a no-op
            Action::Resize(_, _) | Action::None => {}
        }
    }

    /// Update app state for a new frame.
    #[allow(clippy::missing_const_for_fn)] // const fn with &mut self not stable
    pub fn tick(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);

        // Clear force redraw flag after use
        self.force_redraw = false;

        // Check max frames limit
        if let Some(max) = self.max_frames {
            if self.frame_count >= max {
                self.should_quit = true;
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
                    let space = self.max_accumulator_size.saturating_sub(self.accumulator.len());
                    let to_add = n.min(space);
                    self.accumulator.extend_from_slice(&self.scratch[..to_add]);

                    // Parse all complete events from accumulator.
                    self.parse_accumulated(&mut events);
                }
                Ok(_) => {} // No bytes read
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
// Entry Point
// ============================================================================

fn main() -> io::Result<()> {
    match Config::from_args(std::env::args_os()) {
        ParseResult::Config(config) => {
            if config.headless_smoke {
                run_headless_smoke(&config);
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

/// Run headless smoke test (no TTY required).
fn run_headless_smoke(config: &Config) {
    let (width, height) = config.headless_size;
    eprintln!("Running headless smoke test ({width}x{height})...");

    // Create buffer without terminal
    let mut buffer = OptimizedBuffer::new(u32::from(width), u32::from(height));

    // Run through some render operations
    let bg = Rgba::from_hex("#1a1a2e").unwrap_or(Rgba::BLACK);
    buffer.clear(bg);

    buffer.draw_text(
        2,
        0,
        "OpenTUI Showcase",
        Style::fg(Rgba::WHITE).with_bold(),
    );

    buffer.draw_text(
        2,
        u32::from(height) / 2,
        "Headless smoke test",
        Style::fg(Rgba::GREEN),
    );

    buffer.draw_text(
        2,
        u32::from(height).saturating_sub(1),
        "Test completed successfully",
        Style::fg(Rgba::WHITE),
    );

    // Verify buffer is valid
    assert_eq!(buffer.width(), u32::from(width));
    assert_eq!(buffer.height(), u32::from(height));

    eprintln!("Headless smoke test PASSED");
    eprintln!("  Buffer size: {}x{}", buffer.width(), buffer.height());
    eprintln!("  Seed: {}", config.seed);
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

        // --- Update phase ---
        app.tick();

        // --- Render phase ---
        draw_frame(&mut renderer, &app);

        // --- Present ---
        renderer.present()?;

        // --- Frame pacing ---
        let elapsed = frame_start.elapsed();
        if let Some(remaining) = frame_duration.checked_sub(elapsed) {
            std::thread::sleep(remaining);
        }
    }

    Ok(())
}

/// Draw a single frame using the render pass system.
///
/// Render passes (back-to-front):
/// 1. Background - fill screen
/// 2. Chrome - top bar, status bar
/// 3. Panels - sidebar, editor, preview
/// 4. Overlays - modals (placeholder)
/// 5. Toasts - notifications (placeholder)
/// 6. Debug - performance stats (placeholder)
fn draw_frame(renderer: &mut Renderer, app: &App) {
    let (width, height) = renderer.size();
    let panels = PanelLayout::compute(width, height);
    let theme = app.ui_theme.tokens();

    let buffer = renderer.buffer();

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
    draw_pass_panels(buffer, &panels, &theme, app);

    // === Pass 4: Overlays (placeholder) ===
    // Will be implemented when overlay system is added.

    // === Pass 5: Toasts (placeholder) ===
    // Will be implemented when toast system is added.

    // === Pass 6: Debug (placeholder) ===
    // Will show FPS, frame time, etc.
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
    buffer.draw_text(
        top_x + 10,
        top_y,
        "Showcase",
        Style::fg(theme.fg1),
    );

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
fn draw_pass_panels(buffer: &mut OptimizedBuffer, panels: &PanelLayout, theme: &Theme, app: &App) {
    // --- Sidebar ---
    if !panels.sidebar.is_empty() {
        draw_rect_bg(buffer, &panels.sidebar, theme.bg2);
        draw_sidebar(buffer, &panels.sidebar, panels.mode, theme, app);
    }

    // --- Editor panel ---
    if !panels.editor.is_empty() {
        draw_editor_panel(buffer, &panels.editor, theme);
    }

    // --- Preview panel ---
    if !panels.preview.is_empty() {
        draw_preview_panel(buffer, &panels.preview, theme, app);
    }
}

/// Draw the editor panel content.
fn draw_editor_panel(buffer: &mut OptimizedBuffer, rect: &Rect, theme: &Theme) {
    let center_x = u32::try_from(rect.x).unwrap_or(0) + rect.w / 2;
    let center_y = u32::try_from(rect.y).unwrap_or(0) + rect.h / 2;

    let welcome = "Welcome to OpenTUI Showcase!";
    let welcome_len = u32::try_from(welcome.len()).unwrap_or(0);
    let welcome_x = center_x.saturating_sub(welcome_len / 2);
    buffer.draw_text(
        welcome_x,
        center_y.saturating_sub(1),
        welcome,
        Style::fg(theme.fg0).with_bold(),
    );

    let subtext = "Press Ctrl+Q to quit";
    let subtext_len = u32::try_from(subtext.len()).unwrap_or(0);
    let subtext_x = center_x.saturating_sub(subtext_len / 2);
    buffer.draw_text(subtext_x, center_y.saturating_add(1), subtext, Style::fg(theme.fg2));
}

/// Draw the preview panel with border.
fn draw_preview_panel(buffer: &mut OptimizedBuffer, rect: &Rect, theme: &Theme, app: &App) {
    let px = u32::try_from(rect.x).unwrap_or(0);
    let py = u32::try_from(rect.y).unwrap_or(0);

    // Draw left border of preview panel.
    let border_color = Rgba::from_hex("#333366").unwrap_or(theme.bg2);
    for row in py..py + rect.h {
        buffer.draw_text(px, row, "│", Style::fg(border_color));
    }

    // Draw "Preview" label.
    buffer.draw_text(px + 2, py + 1, "Preview", Style::fg(theme.fg2));

    let frame_info = format!("Frame {}", app.frame_count);
    buffer.draw_text(px + 2, py + 3, &frame_info, Style::fg(theme.fg2));
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

    draw_centered(buffer, center_y.saturating_sub(2), msg1, Style::fg(theme.accent_error).with_bold());
    draw_centered(buffer, center_y.saturating_sub(1), &msg2, Style::fg(theme.fg0));
    draw_centered(buffer, center_y, &msg3, Style::fg(theme.fg0));
    draw_centered(buffer, center_y.saturating_add(2), msg4, Style::fg(theme.fg0));
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
                Style::fg(theme.bg0).with_bg(theme.accent_primary).with_bold()
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
        let text_x = if mode == LayoutMode::Compact { content_x } else { content_x + 2 };
        buffer.draw_text(text_x, y, &text, style);

        y += 1;
    }

    // Draw section count at bottom if there's room
    let bottom = u32::try_from(sidebar.bottom()).unwrap_or(0);
    if y < bottom.saturating_sub(1) && mode == LayoutMode::Full {
        let count_text = format!("{}/{}", Section::ALL.len(), Section::ALL.len());
        buffer.draw_text(content_x + 2, bottom.saturating_sub(2), &count_text, Style::fg(theme.fg2));
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
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
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
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
        };
        assert_eq!(config.fps_cap, 30);
    }

    #[test]
    fn test_no_mouse_flag() {
        let result = Config::from_args(args(&["demo_showcase", "--no-mouse"]));
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
        };
        assert!(!config.enable_mouse);
    }

    #[test]
    fn test_headless_smoke_flag() {
        let result = Config::from_args(args(&["demo_showcase", "--headless-smoke"]));
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
        };
        assert!(config.headless_smoke);
    }

    #[test]
    fn test_headless_size() {
        let result = Config::from_args(args(&[
            "demo_showcase",
            "--headless-size",
            "120x40",
        ]));
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
        };
        assert_eq!(config.headless_size, (120, 40));
    }

    #[test]
    fn test_max_frames() {
        let result = Config::from_args(args(&["demo_showcase", "--max-frames", "100"]));
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
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
        let result = Config::from_args(args(&[
            "demo_showcase",
            "--cap-preset",
            "no_mouse",
        ]));
        let config = match result {
            ParseResult::Config(c) => c,
            _ => panic!("Expected Config"),
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
    }

    #[test]
    fn test_panel_layout_compact() {
        let layout = PanelLayout::compute(70, 20);
        assert_eq!(layout.mode, LayoutMode::Compact);
        assert_eq!(layout.sidebar.w, layout::SIDEBAR_WIDTH_COMPACT);
        assert!(layout.preview.is_empty()); // No preview in compact mode
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
        let mut config = Config::default();
        config.start_in_tour = true;
        let app = App::new(&config);
        assert_eq!(app.mode, AppMode::Tour);
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
        let mut config = Config::default();
        config.max_frames = Some(5);
        let mut app = App::new(&config);

        for _ in 0..4 {
            app.tick();
            assert!(!app.should_quit);
        }
        app.tick();
        assert!(app.should_quit);
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
}
