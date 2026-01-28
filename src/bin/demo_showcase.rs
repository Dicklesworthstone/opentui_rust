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
// Application State
// ============================================================================

/// Application state (placeholder for state machine).
#[derive(Debug, Default)]
struct App {
    /// Whether the app should quit.
    should_quit: bool,
    /// Frame counter.
    frame_count: u64,
    /// Maximum frames before exit (from config).
    max_frames: Option<u64>,
}

impl App {
    /// Create a new app instance from config.
    fn new(config: &Config) -> Self {
        Self {
            should_quit: false,
            frame_count: 0,
            max_frames: config.max_frames,
        }
    }

    /// Handle an input event.
    fn handle_event(&mut self, event: &Event) {
        if let Event::Key(key) = event {
            // Ctrl+Q quits
            if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CTRL) {
                self.should_quit = true;
            }
            // Escape also quits for convenience during development
            if key.code == KeyCode::Esc {
                self.should_quit = true;
            }
        }
    }

    /// Update app state for a new frame.
    fn tick(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);

        // Check max frames limit
        if let Some(max) = self.max_frames {
            if self.frame_count >= max {
                self.should_quit = true;
            }
        }
    }
}

// ============================================================================
// Entry Point
// ============================================================================

fn main() -> io::Result<()> {
    match Config::from_args(std::env::args_os()) {
        ParseResult::Config(config) => {
            if config.headless_smoke {
                run_headless_smoke(&config)
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
fn run_headless_smoke(config: &Config) -> io::Result<()> {
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

    Ok(())
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
    let mut app = App::new(&config);
    let mut parser = InputParser::new();
    let mut input_buf = [0u8; 256];

    // Main loop.
    let frame_duration = config.frame_duration();

    while !app.should_quit {
        let frame_start = Instant::now();

        // --- Input phase ---
        // Read available input (non-blocking).
        if let Ok(n) = io::stdin().read(&mut input_buf) {
            if n > 0 {
                let mut offset = 0;
                while offset < n {
                    match parser.parse(&input_buf[offset..n]) {
                        Ok((event, consumed)) => {
                            app.handle_event(&event);
                            offset += consumed;
                        }
                        Err(_) => break,
                    }
                }
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

/// Draw a single frame.
fn draw_frame(renderer: &mut Renderer, app: &App) {
    let (width, height) = renderer.size();

    // Clear buffer.
    let buffer = renderer.buffer();
    let bg_color = Rgba::from_hex("#1a1a2e").unwrap_or(Rgba::BLACK);
    buffer.clear(bg_color);

    // --- Top bar ---
    let top_bar_bg = Rgba::from_hex("#16213e").unwrap_or(Rgba::BLACK);
    buffer.fill_rect(0, 0, width, 1, top_bar_bg);
    buffer.draw_text(
        2,
        0,
        "OpenTUI Showcase",
        Style::fg(Rgba::from_hex("#e94560").unwrap_or(Rgba::WHITE)).with_bold(),
    );

    let mode_text = "Mode: Normal";
    let mode_x = width.saturating_sub(u32::try_from(mode_text.len()).unwrap_or(0) + 2);
    buffer.draw_text(
        mode_x,
        0,
        mode_text,
        Style::fg(Rgba::from_hex("#4ecca3").unwrap_or(Rgba::GREEN)),
    );

    // --- Status bar ---
    let status_y = height.saturating_sub(1);
    let status_bg = Rgba::from_hex("#16213e").unwrap_or(Rgba::BLACK);
    buffer.fill_rect(0, status_y, width, 1, status_bg);

    let hints = "Ctrl+Q Quit  |  Esc Exit";
    buffer.draw_text(
        2,
        status_y,
        hints,
        Style::fg(Rgba::from_hex("#888888").unwrap_or(Rgba::WHITE)),
    );

    let stats = format!("Frame: {}", app.frame_count);
    let stats_x = width.saturating_sub(u32::try_from(stats.len()).unwrap_or(0) + 2);
    buffer.draw_text(
        stats_x,
        status_y,
        &stats,
        Style::fg(Rgba::from_hex("#888888").unwrap_or(Rgba::WHITE)),
    );

    // --- Center content (placeholder) ---
    let center_y = height / 2;
    let welcome = "Welcome to OpenTUI Showcase!";
    let welcome_len = u32::try_from(welcome.len()).unwrap_or(0);
    let welcome_x = width.saturating_sub(welcome_len) / 2;
    buffer.draw_text(
        welcome_x,
        center_y.saturating_sub(1),
        welcome,
        Style::fg(Rgba::WHITE).with_bold(),
    );

    let subtext = "Press Ctrl+Q to quit";
    let subtext_len = u32::try_from(subtext.len()).unwrap_or(0);
    let subtext_x = width.saturating_sub(subtext_len) / 2;
    buffer.draw_text(
        subtext_x,
        center_y.saturating_add(1),
        subtext,
        Style::fg(Rgba::from_hex("#888888").unwrap_or(Rgba::WHITE)),
    );
}

// ============================================================================
// Platform-Specific Helpers
// ============================================================================

/// Check if stdout is a TTY.
#[cfg(unix)]
fn is_tty() -> bool {
    // SAFETY: isatty is safe to call with any file descriptor.
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}

#[cfg(not(unix))]
fn is_tty() -> bool {
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
}
