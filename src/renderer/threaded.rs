//! Threaded renderer with channel-based communication.
//!
//! This module provides [`ThreadedRenderer`], which offloads terminal I/O to a
//! dedicated render thread while the main thread continues drawing.
//!
//! # Architecture
//!
//! ```text
//! Main Thread                         Render Thread
//! -----------                         -------------
//! draw into back_buffer
//! send Present(buffer, pool, links)  ─────────────▶
//!                                       receive
//!                                       diff against front
//!                                       write ANSI to terminal
//!                                       swap buffers
//! receive buffer  ◀─────────────────   send BufferReady(buffer)
//! continue drawing
//! ```
//!
//! # Usage
//!
//! ```no_run
//! use opentui::renderer::ThreadedRenderer;
//! use opentui::{Style, Rgba};
//!
//! let mut renderer = ThreadedRenderer::new(80, 24)?;
//!
//! loop {
//!     renderer.clear();
//!     renderer.buffer().draw_text(10, 5, "Hello!", Style::fg(Rgba::GREEN));
//!     renderer.present()?;
//!     // ... handle input, break on quit
//!     break;
//! }
//!
//! renderer.shutdown()?;
//! # Ok::<(), std::io::Error>(())
//! ```

use crate::ansi::AnsiWriter;
use crate::buffer::OptimizedBuffer;
use crate::color::Rgba;
use crate::grapheme_pool::GraphemePool;
use crate::link::LinkPool;
use crate::renderer::{BufferDiff, RendererOptions};
use crate::terminal::{CursorStyle, Terminal};
use std::io::{self, Stdout, Write};
use std::panic::AssertUnwindSafe;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

/// Commands sent from main thread to render thread.
#[allow(clippy::large_enum_variant)] // Large variants move buffers to avoid per-frame heap allocations.
enum RenderCommand {
    /// Submit a frame for rendering.
    Present {
        buffer: OptimizedBuffer,
        grapheme_pool: GraphemePool,
        link_pool: LinkPool,
    },
    /// Resize the terminal.
    Resize { width: u32, height: u32 },
    /// Set cursor position and visibility.
    SetCursor { x: u32, y: u32, visible: bool },
    /// Set cursor style.
    SetCursorStyle { style: CursorStyle, blinking: bool },
    /// Set window title.
    SetTitle { title: String },
    /// Force a full redraw on next present.
    Invalidate,
    /// Shutdown the render thread.
    Shutdown,
}

/// Replies sent from render thread to main thread.
#[allow(clippy::large_enum_variant)] // Buffer replies intentionally avoid extra boxing/allocations.
enum RenderReply {
    /// Buffer returned after rendering.
    BufferReady {
        buffer: OptimizedBuffer,
        grapheme_pool: GraphemePool,
        link_pool: LinkPool,
    },
    /// Resize completed.
    ResizeComplete,
    /// Cursor operation completed.
    CursorComplete,
    /// Title set.
    TitleComplete,
    /// Invalidation acknowledged.
    InvalidateComplete,
    /// Shutdown complete.
    ShutdownComplete,
    /// An error occurred.
    Error(String),
}

/// Threaded renderer statistics.
#[derive(Clone, Debug, Default)]
pub struct ThreadedRenderStats {
    /// Total frames rendered.
    pub frames: u64,
    /// Last frame render time.
    pub last_frame_time: Duration,
    /// Last frame cells updated.
    pub last_frame_cells: usize,
    /// Approximate FPS.
    pub fps: f32,
}

/// Threaded renderer with channel-based communication.
///
/// The main thread owns this struct and uses it for drawing. Terminal I/O
/// happens on a separate render thread, allowing the main thread to continue
/// processing while frames are being written.
///
/// # Ownership Model
///
/// Buffers are moved via channels, not shared:
/// - Main thread draws into `back_buffer`
/// - On `present()`, buffer is sent to render thread
/// - Render thread diffs, writes ANSI, then returns the buffer
/// - Main thread receives the buffer back for the next frame
///
/// This avoids locks and per-frame allocations.
pub struct ThreadedRenderer {
    /// Channel to send commands to render thread.
    tx: Sender<RenderCommand>,
    /// Channel to receive replies from render thread.
    rx: Receiver<RenderReply>,
    /// Handle to join the render thread.
    handle: Option<JoinHandle<()>>,

    /// Current back buffer for drawing.
    back_buffer: OptimizedBuffer,
    /// Grapheme pool for multi-codepoint graphemes.
    grapheme_pool: GraphemePool,
    /// Link pool for hyperlinks.
    link_pool: LinkPool,

    /// Buffer dimensions.
    width: u32,
    height: u32,

    /// Background color for clear.
    background: Rgba,

    /// Statistics.
    stats: ThreadedRenderStats,
    last_present_at: Instant,
}

impl ThreadedRenderer {
    /// Create a new threaded renderer with the given dimensions.
    ///
    /// This spawns a render thread that handles all terminal I/O.
    pub fn new(width: u32, height: u32) -> io::Result<Self> {
        Self::new_with_options(width, height, RendererOptions::default())
    }

    /// Create a new threaded renderer with custom options.
    pub fn new_with_options(width: u32, height: u32, options: RendererOptions) -> io::Result<Self> {
        let (tx, render_rx) = mpsc::channel::<RenderCommand>();
        let (render_tx, rx) = mpsc::channel::<RenderReply>();

        // Spawn the render thread
        let handle = thread::Builder::new()
            .name("opentui-render".to_string())
            .spawn(move || {
                render_thread_main(render_rx, render_tx, width, height, options);
            })?;

        Ok(Self {
            tx,
            rx,
            handle: Some(handle),
            back_buffer: OptimizedBuffer::new(width, height),
            grapheme_pool: GraphemePool::new(),
            link_pool: LinkPool::new(),
            width,
            height,
            background: Rgba::BLACK,
            stats: ThreadedRenderStats::default(),
            last_present_at: Instant::now(),
        })
    }

    /// Get buffer dimensions.
    #[must_use]
    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the back buffer for drawing.
    pub fn buffer(&mut self) -> &mut OptimizedBuffer {
        &mut self.back_buffer
    }

    /// Get the grapheme pool for multi-codepoint graphemes.
    pub fn grapheme_pool(&mut self) -> &mut GraphemePool {
        &mut self.grapheme_pool
    }

    /// Get the link pool for hyperlinks.
    pub fn link_pool(&mut self) -> &mut LinkPool {
        &mut self.link_pool
    }

    /// Set the background color for clear operations.
    pub fn set_background(&mut self, color: Rgba) {
        self.background = color;
    }

    /// Clear the back buffer with the current background color.
    pub fn clear(&mut self) {
        self.back_buffer.clear(self.background);
    }

    /// Get rendering statistics.
    #[must_use]
    pub fn stats(&self) -> &ThreadedRenderStats {
        &self.stats
    }

    /// Submit the current frame for rendering.
    ///
    /// This blocks until the render thread returns the buffer.
    pub fn present(&mut self) -> io::Result<()> {
        // Take ownership of current buffer and pools
        let buffer = std::mem::replace(
            &mut self.back_buffer,
            OptimizedBuffer::new(self.width, self.height),
        );
        let grapheme_pool = std::mem::take(&mut self.grapheme_pool);
        let link_pool = std::mem::take(&mut self.link_pool);

        // Send to render thread
        self.tx
            .send(RenderCommand::Present {
                buffer,
                grapheme_pool,
                link_pool,
            })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected"))?;

        // Wait for buffer to be returned
        match self.rx.recv() {
            Ok(RenderReply::BufferReady {
                buffer,
                grapheme_pool,
                link_pool,
            }) => {
                self.back_buffer = buffer;
                self.grapheme_pool = grapheme_pool;
                self.link_pool = link_pool;
                self.update_stats();
                Ok(())
            }
            Ok(RenderReply::Error(msg)) => Err(io::Error::other(msg)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "render thread disconnected",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected reply",
            )),
        }
    }

    /// Resize the renderer.
    pub fn resize(&mut self, width: u32, height: u32) -> io::Result<()> {
        self.tx
            .send(RenderCommand::Resize { width, height })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected"))?;

        match self.rx.recv() {
            Ok(RenderReply::ResizeComplete) => {
                self.width = width;
                self.height = height;
                self.back_buffer = OptimizedBuffer::new(width, height);
                Ok(())
            }
            Ok(RenderReply::Error(msg)) => Err(io::Error::other(msg)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "render thread disconnected",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected reply",
            )),
        }
    }

    /// Set cursor position and visibility.
    pub fn set_cursor(&mut self, x: u32, y: u32, visible: bool) -> io::Result<()> {
        self.tx
            .send(RenderCommand::SetCursor { x, y, visible })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected"))?;

        match self.rx.recv() {
            Ok(RenderReply::CursorComplete) => Ok(()),
            Ok(RenderReply::Error(msg)) => Err(io::Error::other(msg)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "render thread disconnected",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected reply",
            )),
        }
    }

    /// Set cursor style.
    pub fn set_cursor_style(&mut self, style: CursorStyle, blinking: bool) -> io::Result<()> {
        self.tx
            .send(RenderCommand::SetCursorStyle { style, blinking })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected"))?;

        match self.rx.recv() {
            Ok(RenderReply::CursorComplete) => Ok(()),
            Ok(RenderReply::Error(msg)) => Err(io::Error::other(msg)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "render thread disconnected",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected reply",
            )),
        }
    }

    /// Set window title.
    pub fn set_title(&mut self, title: &str) -> io::Result<()> {
        self.tx
            .send(RenderCommand::SetTitle {
                title: title.to_string(),
            })
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected"))?;

        match self.rx.recv() {
            Ok(RenderReply::TitleComplete) => Ok(()),
            Ok(RenderReply::Error(msg)) => Err(io::Error::other(msg)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "render thread disconnected",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected reply",
            )),
        }
    }

    /// Force a full redraw on next present.
    pub fn invalidate(&mut self) -> io::Result<()> {
        self.tx
            .send(RenderCommand::Invalidate)
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected"))?;

        match self.rx.recv() {
            Ok(RenderReply::InvalidateComplete) => Ok(()),
            Ok(RenderReply::Error(msg)) => Err(io::Error::other(msg)),
            Err(_) => Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "render thread disconnected",
            )),
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unexpected reply",
            )),
        }
    }

    /// Gracefully shutdown the render thread.
    ///
    /// This waits for the render thread to complete cleanup and restore
    /// terminal state.
    pub fn shutdown(mut self) -> io::Result<()> {
        self.shutdown_internal()
    }

    fn shutdown_internal(&mut self) -> io::Result<()> {
        // Send shutdown command
        if self.tx.send(RenderCommand::Shutdown).is_err() {
            // Thread already gone, try to join it
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
            return Ok(());
        }

        // Wait for acknowledgment with timeout
        // Ignore other replies and timeout/disconnect.
        let _ = self.rx.recv_timeout(Duration::from_secs(5));

        // Join the thread
        if let Some(handle) = self.handle.take() {
            handle
                .join()
                .map_err(|_| io::Error::other("render thread panicked"))?;
        }

        Ok(())
    }

    fn update_stats(&mut self) {
        let now = Instant::now();
        let frame_time = now.duration_since(self.last_present_at);
        self.last_present_at = now;

        self.stats.frames = self.stats.frames.saturating_add(1);
        self.stats.last_frame_time = frame_time;
        self.stats.fps = if frame_time.as_secs_f32() > 0.0 {
            1.0 / frame_time.as_secs_f32()
        } else {
            0.0
        };
    }
}

impl Drop for ThreadedRenderer {
    fn drop(&mut self) {
        if self.handle.is_some() {
            // Try to send shutdown (may fail if thread is already dead)
            let _ = self.tx.send(RenderCommand::Shutdown);

            // Wait for thread to finish (blocking to ensure cleanup)
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }
}

/// Main function for the render thread.
fn render_thread_main(
    rx: Receiver<RenderCommand>,
    tx: Sender<RenderReply>,
    width: u32,
    height: u32,
    options: RendererOptions,
) {
    // Wrap the main loop in catch_unwind for panic safety
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        render_thread_inner(rx, tx.clone(), width, height, options)
    }));

    // If we panicked, send an error and try to cleanup
    if let Err(e) = result {
        let msg = panic_message(e.as_ref());
        let _ = tx.send(RenderReply::Error(msg));

        // Try to restore terminal (best effort)
        let mut terminal = create_terminal();
        let _ = terminal.cleanup();
    }
}

fn create_terminal() -> Terminal<Stdout> {
    Terminal::new(io::stdout())
}

fn panic_message(payload: &dyn std::any::Any) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "render thread panicked".to_string())
}

#[allow(clippy::too_many_lines)] // Long render loop is easier to follow inline.
fn render_thread_inner(
    rx: Receiver<RenderCommand>,
    tx: Sender<RenderReply>,
    width: u32,
    height: u32,
    options: RendererOptions,
) {
    // Initialize terminal
    let mut terminal = create_terminal();

    // Apply options
    if options.use_alt_screen {
        if let Err(e) = terminal.enter_alt_screen() {
            let _ = tx.send(RenderReply::Error(format!(
                "failed to enter alt screen: {e}"
            )));
            return;
        }
    }
    if options.hide_cursor {
        let _ = terminal.hide_cursor();
    }
    if options.enable_mouse {
        let _ = terminal.enable_mouse();
    }
    if options.query_capabilities {
        let _ = terminal.query_capabilities();
    }

    // Initialize front buffer
    let mut front_buffer = OptimizedBuffer::new(width, height);
    let mut force_redraw = true;
    let mut scratch_buffer: Vec<u8> = Vec::with_capacity(
        (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(20),
    );
    let mut current_width = width;
    let mut current_height = height;

    // Message loop
    loop {
        match rx.recv() {
            Ok(RenderCommand::Present {
                buffer,
                grapheme_pool,
                link_pool,
            }) => {
                // Compute diff and render
                let total_cells = (current_width as usize).saturating_mul(current_height as usize);
                let diff = BufferDiff::compute(&front_buffer, &buffer);

                let render_result = if force_redraw || diff.should_full_redraw(total_cells) {
                    render_full(
                        &mut terminal,
                        &mut scratch_buffer,
                        &buffer,
                        &grapheme_pool,
                        &link_pool,
                        current_width,
                        current_height,
                    )
                } else {
                    render_diff(
                        &mut terminal,
                        &mut scratch_buffer,
                        &buffer,
                        &grapheme_pool,
                        &link_pool,
                        &diff,
                    )
                };

                if let Err(e) = render_result {
                    let _ = tx.send(RenderReply::Error(format!("render error: {e}")));
                    // Return buffer anyway so main thread can continue
                    let _ = tx.send(RenderReply::BufferReady {
                        buffer,
                        grapheme_pool,
                        link_pool,
                    });
                    continue;
                }

                force_redraw = false;

                // Swap buffers
                front_buffer = buffer.clone();

                // Return buffer to main thread
                let _ = tx.send(RenderReply::BufferReady {
                    buffer,
                    grapheme_pool,
                    link_pool,
                });
            }

            Ok(RenderCommand::Resize { width, height }) => {
                current_width = width;
                current_height = height;
                front_buffer = OptimizedBuffer::new(width, height);
                scratch_buffer = Vec::with_capacity(
                    (width as usize)
                        .saturating_mul(height as usize)
                        .saturating_mul(20),
                );
                force_redraw = true;
                let _ = terminal.clear();
                let _ = tx.send(RenderReply::ResizeComplete);
            }

            Ok(RenderCommand::SetCursor { x, y, visible }) => {
                if visible {
                    let _ = terminal.show_cursor();
                    let _ = terminal.move_cursor(x, y);
                } else {
                    let _ = terminal.hide_cursor();
                }
                let _ = tx.send(RenderReply::CursorComplete);
            }

            Ok(RenderCommand::SetCursorStyle { style, blinking }) => {
                let _ = terminal.set_cursor_style(style, blinking);
                let _ = tx.send(RenderReply::CursorComplete);
            }

            Ok(RenderCommand::SetTitle { title }) => {
                let _ = terminal.set_title(&title);
                let _ = tx.send(RenderReply::TitleComplete);
            }

            Ok(RenderCommand::Invalidate) => {
                force_redraw = true;
                let _ = tx.send(RenderReply::InvalidateComplete);
            }

            Ok(RenderCommand::Shutdown) => {
                // Cleanup terminal
                let _ = terminal.cleanup();
                let _ = tx.send(RenderReply::ShutdownComplete);
                break;
            }

            Err(_) => {
                // Channel closed, cleanup and exit
                let _ = terminal.cleanup();
                break;
            }
        }
    }
}

fn render_full(
    terminal: &mut Terminal<Stdout>,
    scratch: &mut Vec<u8>,
    buffer: &OptimizedBuffer,
    grapheme_pool: &GraphemePool,
    link_pool: &LinkPool,
    width: u32,
    height: u32,
) -> io::Result<()> {
    if terminal.capabilities().sync_output {
        terminal.begin_sync()?;
    }

    scratch.clear();
    let mut writer = AnsiWriter::new(&mut *scratch);
    // Emit cursor home to synchronize terminal cursor with writer's internal tracking.
    // The writer starts tracking at (0,0), but the terminal cursor may be elsewhere
    // (e.g., pending-wrap state at end of previous frame).
    writer.write_str("\x1b[H");

    for y in 0..height {
        writer.move_cursor(y, 0);
        for x in 0..width {
            if let Some(cell) = buffer.get(x, y) {
                if !cell.is_continuation() {
                    let url = cell.attributes.link_id().and_then(|id| link_pool.get(id));
                    writer.write_cell_with_pool_and_link(cell, grapheme_pool, url);
                }
            }
        }
    }

    writer.reset();
    writer.flush()?;

    terminal.flush()?;
    io::stdout().write_all(scratch)?;
    io::stdout().flush()?;

    if terminal.capabilities().sync_output {
        terminal.end_sync()?;
    }
    terminal.flush()
}

fn render_diff(
    terminal: &mut Terminal<Stdout>,
    scratch: &mut Vec<u8>,
    buffer: &OptimizedBuffer,
    grapheme_pool: &GraphemePool,
    link_pool: &LinkPool,
    diff: &BufferDiff,
) -> io::Result<()> {
    if terminal.capabilities().sync_output {
        terminal.begin_sync()?;
    }

    scratch.clear();
    let mut writer = AnsiWriter::new(&mut *scratch);

    for &(x, y) in &diff.changed_cells {
        if let Some(cell) = buffer.get(x, y) {
            if !cell.is_continuation() {
                let url = cell.attributes.link_id().and_then(|id| link_pool.get(id));
                writer.write_cell_at_with_pool_and_link(y, x, cell, grapheme_pool, url);
            }
        }
    }

    writer.reset();
    writer.flush()?;

    if !scratch.is_empty() {
        io::stdout().write_all(scratch)?;
        io::stdout().flush()?;
    }

    if terminal.capabilities().sync_output {
        terminal.end_sync()?;
    }
    terminal.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Full integration tests require a real terminal.
    // Unit tests verify struct operations and API contracts.

    #[test]
    fn test_stats_default() {
        let stats = ThreadedRenderStats::default();
        assert_eq!(stats.frames, 0);
        assert_eq!(stats.last_frame_cells, 0);
        assert_eq!(stats.last_frame_time, Duration::ZERO);
        assert!(stats.fps.abs() < f32::EPSILON);
    }

    #[test]
    fn test_stats_clone() {
        let stats = ThreadedRenderStats {
            frames: 100,
            fps: 60.0,
            last_frame_cells: 1920,
            last_frame_time: Duration::from_millis(16),
        };

        let cloned = stats.clone();
        assert_eq!(cloned.frames, 100);
        assert!((cloned.fps - 60.0).abs() < f32::EPSILON);
        assert_eq!(cloned.last_frame_cells, 1920);
        assert_eq!(cloned.last_frame_time, Duration::from_millis(16));
    }

    #[test]
    fn test_stats_debug() {
        let stats = ThreadedRenderStats::default();
        let debug_str = format!("{stats:?}");
        assert!(debug_str.contains("ThreadedRenderStats"));
        assert!(debug_str.contains("frames"));
        assert!(debug_str.contains("fps"));
    }

    #[test]
    fn test_render_command_sizes() {
        // Verify enum variants don't cause unexpected memory bloat.
        // The enum should be reasonable in size (buffers are large but that's expected).
        use std::mem::size_of;

        // Commands contain full buffers, so they're necessarily large.
        // This test documents the current size rather than asserting a specific limit.
        let cmd_size = size_of::<RenderCommand>();
        eprintln!("[TEST] RenderCommand size: {cmd_size} bytes");
        // Should be dominated by OptimizedBuffer + GraphemePool + LinkPool
        assert!(cmd_size > 0);

        let reply_size = size_of::<RenderReply>();
        eprintln!("[TEST] RenderReply size: {reply_size} bytes");
        assert!(reply_size > 0);
    }

    #[test]
    fn test_render_options_default() {
        let opts = RendererOptions::default();
        assert!(opts.use_alt_screen);
        assert!(opts.hide_cursor);
        assert!(opts.enable_mouse);
        assert!(opts.query_capabilities);
    }

    #[test]
    fn test_render_options_custom() {
        let opts = RendererOptions {
            use_alt_screen: false,
            hide_cursor: false,
            enable_mouse: false,
            query_capabilities: false,
        };
        assert!(!opts.use_alt_screen);
        assert!(!opts.hide_cursor);
        assert!(!opts.enable_mouse);
        assert!(!opts.query_capabilities);
    }

    // Note: Tests that spawn the render thread require a real terminal
    // because Terminal::new(io::stdout()) is hardcoded. These tests
    // would need to be run as integration tests with PTY allocation.
    //
    // The following scenarios are covered by manual testing:
    // - ThreadedRenderer::new() spawns thread successfully
    // - present() sends buffer and receives it back
    // - resize() notifies render thread
    // - shutdown() cleanly terminates the thread
    // - Drop impl calls shutdown if not already called
    // - Panic in render thread is caught and reported
}
