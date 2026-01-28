//! `demo_showcase` — `OpenTUI` demonstration binary
//!
//! A comprehensive showcase of `OpenTUI`'s rendering capabilities, presenting
//! a Developer Workbench with editor, preview, logs, and overlays.
//!
//! # Usage
//!
//! ```bash
//! cargo run --bin demo_showcase
//! ```
//!
//! Press Ctrl+Q to quit.

// Required for libc FFI (fcntl for non-blocking stdin).
#![allow(unsafe_code)]

use opentui::input::{Event, InputParser, KeyCode, KeyModifiers};
use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{Renderer, RendererOptions, Rgba, Style};
use std::io::{self, Read};
use std::time::{Duration, Instant};

/// Application configuration (placeholder for CLI args).
#[derive(Clone, Copy, Debug)]
struct Config {
    /// Target frames per second.
    fps_cap: u32,
}

impl Config {
    /// Parse configuration from command-line arguments.
    fn from_args<I>(_args: I) -> Self
    where
        I: Iterator<Item = std::ffi::OsString>,
    {
        // TODO: Parse actual CLI args (bd-2iv)
        Self { fps_cap: 60 }
    }

    /// Get renderer options.
    const fn renderer_options() -> RendererOptions {
        RendererOptions {
            use_alt_screen: true,
            hide_cursor: true,
            enable_mouse: true,
            query_capabilities: true,
        }
    }

    /// Get target frame duration.
    fn frame_duration(self) -> Duration {
        Duration::from_micros(1_000_000 / u64::from(self.fps_cap))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self { fps_cap: 60 }
    }
}

/// Application state (placeholder for state machine).
#[derive(Debug, Default)]
struct App {
    /// Whether the app should quit.
    should_quit: bool,
    /// Frame counter.
    frame_count: u64,
}

impl App {
    /// Create a new app instance.
    fn new() -> Self {
        Self::default()
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
    #[allow(clippy::missing_const_for_fn)] // const fn with &mut self not stable
    fn tick(&mut self) {
        self.frame_count = self.frame_count.wrapping_add(1);
    }
}

fn main() -> io::Result<()> {
    let config = Config::from_args(std::env::args_os());
    run(config)
}

/// Main application loop.
fn run(config: Config) -> io::Result<()> {
    // Determine terminal size, fall back to 80x24.
    let (width, height) = terminal_size().unwrap_or((80, 24));

    // Create renderer with options.
    let mut renderer = Renderer::new_with_options(
        u32::from(width),
        u32::from(height),
        Config::renderer_options(),
    )?;

    // Enable raw mode for input handling.
    let _raw_guard = enable_raw_mode()?;

    // Set up non-blocking stdin.
    set_stdin_nonblocking()?;

    // Initialize app state.
    let mut app = App::new();
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
