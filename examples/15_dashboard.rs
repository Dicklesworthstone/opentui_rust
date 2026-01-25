//! Example 15: Multi-Panel Dashboard Layout
//!
//! Demonstrates:
//! - Split-pane layouts (horizontal and vertical)
//! - Multiple independent panels
//! - Focused panel indicator
//! - Panel resizing concepts
//! - Real-world dashboard pattern

use opentui::buffer::{BoxStyle, ClipRect};
use opentui::input::{Event, InputParser, KeyCode, KeyModifiers};
use opentui::terminal::{enable_raw_mode, terminal_size};
use opentui::{OptimizedBuffer, Renderer, Rgba, Style};
use std::io::{self, Read};

const BG_COLOR: &str = "#0f111a";
const BORDER_NORMAL: &str = "#555555";
const BORDER_FOCUSED: &str = "#00cec9";
const TITLE_COLOR: &str = "#74b9ff";
const LABEL_COLOR: &str = "#ffeaa7";
const INFO_COLOR: &str = "#00cec9";
const WARN_COLOR: &str = "#fdcb6e";
const ERROR_COLOR: &str = "#e74c3c";
const BAR_FILLED: &str = "#00b894";
const BAR_EMPTY: &str = "#2d3436";

/// Panel definition with position and title
#[derive(Clone)]
struct Panel {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    title: String,
}

impl Panel {
    fn new(x: u32, y: u32, width: u32, height: u32, title: &str) -> Self {
        Self {
            x,
            y,
            width,
            height,
            title: title.to_string(),
        }
    }

    fn inner_rect(&self) -> (u32, u32, u32, u32) {
        (
            self.x + 1,
            self.y + 1,
            self.width.saturating_sub(2),
            self.height.saturating_sub(2),
        )
    }
}

/// A log entry for the event log panel
struct LogEntry {
    time: String,
    level: LogLevel,
    message: String,
}

#[derive(Clone, Copy)]
enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Main dashboard state
struct Dashboard {
    panels: Vec<Panel>,
    focused: usize,
    sidebar_selection: usize,
    sidebar_items: Vec<&'static str>,
    cpu_usage: f32,
    memory_usage: f32,
    disk_read: u32,
    disk_write: u32,
    event_log: Vec<LogEntry>,
    frame_count: u64,
}

impl Dashboard {
    fn new(width: u32, height: u32) -> Self {
        let panels = Self::calculate_layout(width, height);
        Self {
            panels,
            focused: 0,
            sidebar_selection: 0,
            sidebar_items: vec!["System", "Network", "Storage", "Logs"],
            cpu_usage: 0.68,
            memory_usage: 0.31,
            disk_read: 125,
            disk_write: 42,
            event_log: vec![
                LogEntry {
                    time: "12:34:56".to_string(),
                    level: LogLevel::Info,
                    message: "Service started".to_string(),
                },
                LogEntry {
                    time: "12:34:57".to_string(),
                    level: LogLevel::Warn,
                    message: "High memory usage".to_string(),
                },
                LogEntry {
                    time: "12:34:58".to_string(),
                    level: LogLevel::Info,
                    message: "Cache cleared".to_string(),
                },
            ],
            frame_count: 0,
        }
    }

    fn calculate_layout(width: u32, height: u32) -> Vec<Panel> {
        // Reserve 1 row for header, 1 row for footer
        let content_y = 2;
        let content_h = height.saturating_sub(4);

        // Sidebar: 20% width, minimum 15 chars
        let sidebar_w = (width / 5).max(15).min(25);
        let main_w = width.saturating_sub(sidebar_w);

        // Main panel split: 65% content, 35% logs (vertical split)
        let main_content_h = (content_h * 65) / 100;
        let logs_h = content_h.saturating_sub(main_content_h);

        vec![
            Panel::new(0, content_y, sidebar_w, content_h, "Sidebar"),
            Panel::new(sidebar_w, content_y, main_w, main_content_h, "Main Panel"),
            Panel::new(
                sidebar_w,
                content_y + main_content_h,
                main_w,
                logs_h,
                "Recent Events",
            ),
        ]
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.panels = Self::calculate_layout(width, height);
    }

    fn handle_input(&mut self, event: &Event) -> bool {
        match event {
            Event::Key(key) => {
                if key.code == KeyCode::Char('q') || key.is_ctrl_c() {
                    return false; // Exit
                }

                if key.code == KeyCode::Tab {
                    if key.modifiers.contains(KeyModifiers::SHIFT) {
                        self.focused = if self.focused == 0 {
                            self.panels.len() - 1
                        } else {
                            self.focused - 1
                        };
                    } else {
                        self.focused = (self.focused + 1) % self.panels.len();
                    }
                    return true;
                }

                // Handle input for focused panel
                match self.focused {
                    0 => {
                        // Sidebar
                        match key.code {
                            KeyCode::Up => {
                                if self.sidebar_selection > 0 {
                                    self.sidebar_selection -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if self.sidebar_selection < self.sidebar_items.len() - 1 {
                                    self.sidebar_selection += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                    1 => {
                        // Main panel (no specific interaction)
                    }
                    2 => {
                        // Logs panel (could scroll, but simplified for demo)
                    }
                    _ => {}
                }
            }
            Event::Resize(resize) => {
                self.resize(u32::from(resize.width), u32::from(resize.height));
            }
            _ => {}
        }
        true
    }

    fn update(&mut self) {
        self.frame_count += 1;

        // Simulate changing data
        if self.frame_count % 30 == 0 {
            self.cpu_usage = 0.3 + (self.frame_count as f32 / 100.0).sin().abs() * 0.6;
            self.memory_usage = 0.25 + (self.frame_count as f32 / 150.0).cos().abs() * 0.4;
            self.disk_read = 80 + ((self.frame_count / 10) % 100) as u32;
            self.disk_write = 30 + ((self.frame_count / 15) % 50) as u32;
        }

        // Add log entry occasionally
        if self.frame_count % 120 == 0 {
            let levels = [LogLevel::Info, LogLevel::Warn, LogLevel::Info, LogLevel::Error];
            let messages = [
                "Heartbeat received",
                "Memory threshold exceeded",
                "Connection established",
                "Timeout waiting for response",
            ];
            let idx = (self.frame_count / 120) as usize % levels.len();
            let time = format!(
                "12:{}:{}",
                35 + (self.frame_count / 600) % 60,
                (self.frame_count / 10) % 60
            );
            self.event_log.push(LogEntry {
                time,
                level: levels[idx],
                message: messages[idx].to_string(),
            });
            // Keep log bounded
            if self.event_log.len() > 10 {
                self.event_log.remove(0);
            }
        }
    }

    fn render(&self, buffer: &mut OptimizedBuffer, width: u32, height: u32) {
        // Clear background
        buffer.clear(Rgba::from_hex(BG_COLOR).expect("valid"));

        // Draw header
        let title = "Dashboard Demo";
        let title_x = (width.saturating_sub(title.len() as u32)) / 2;
        buffer.draw_text(
            title_x,
            0,
            title,
            Style::fg(Rgba::from_hex(TITLE_COLOR).expect("valid")).with_bold(),
        );

        let help = "[Tab] Switch Focus  [q] Quit";
        let help_x = width.saturating_sub(help.len() as u32 + 1);
        buffer.draw_text(help_x, 0, help, Style::dim());

        // Draw each panel
        for (i, panel) in self.panels.iter().enumerate() {
            self.render_panel(buffer, panel, i == self.focused);
            match i {
                0 => self.render_sidebar(buffer, panel),
                1 => self.render_main(buffer, panel),
                2 => self.render_logs(buffer, panel),
                _ => {}
            }
        }

        // Draw footer
        let footer_y = height.saturating_sub(1);
        let footer = format!(
            "Frame: {} | CPU: {:.0}% | Mem: {:.0}%",
            self.frame_count,
            self.cpu_usage * 100.0,
            self.memory_usage * 100.0
        );
        buffer.draw_text(1, footer_y, &footer, Style::dim());
    }

    fn render_panel(&self, buffer: &mut OptimizedBuffer, panel: &Panel, focused: bool) {
        let border_color = if focused {
            Rgba::from_hex(BORDER_FOCUSED).expect("valid")
        } else {
            Rgba::from_hex(BORDER_NORMAL).expect("valid")
        };

        let box_style = BoxStyle::single(Style::fg(border_color));
        buffer.draw_box(panel.x, panel.y, panel.width, panel.height, box_style);

        // Draw title
        if !panel.title.is_empty() && panel.width > 4 {
            let title = format!(" {} ", panel.title);
            let title_x = panel.x + 2;
            buffer.draw_text(
                title_x,
                panel.y,
                &title,
                Style::fg(border_color).with_bold(),
            );
        }
    }

    fn render_sidebar(&self, buffer: &mut OptimizedBuffer, panel: &Panel) {
        let (ix, iy, _iw, _ih) = panel.inner_rect();

        // Push scissor for content
        buffer.push_scissor(ClipRect::new(
            ix as i32,
            iy as i32,
            panel.width.saturating_sub(2),
            panel.height.saturating_sub(2),
        ));

        for (i, item) in self.sidebar_items.iter().enumerate() {
            let y = iy + i as u32;
            let style = if i == self.sidebar_selection {
                Style::fg(Rgba::from_hex(INFO_COLOR).expect("valid")).with_bold()
            } else {
                Style::fg(Rgba::WHITE)
            };

            let prefix = if i == self.sidebar_selection {
                "\u{25B6} "
            } else {
                "  "
            };
            buffer.draw_text(ix, y, &format!("{prefix}{item}"), style);
        }

        buffer.pop_scissor();
    }

    fn render_main(&self, buffer: &mut OptimizedBuffer, panel: &Panel) {
        let (ix, iy, iw, _ih) = panel.inner_rect();

        buffer.push_scissor(ClipRect::new(
            ix as i32,
            iy as i32,
            panel.width.saturating_sub(2),
            panel.height.saturating_sub(2),
        ));

        // CPU Usage
        buffer.draw_text(
            ix,
            iy,
            "CPU Usage",
            Style::fg(Rgba::from_hex(LABEL_COLOR).expect("valid")).with_bold(),
        );
        self.render_progress_bar(buffer, ix, iy + 1, iw.saturating_sub(2), self.cpu_usage);

        // Memory Usage
        buffer.draw_text(
            ix,
            iy + 3,
            "Memory Usage",
            Style::fg(Rgba::from_hex(LABEL_COLOR).expect("valid")).with_bold(),
        );
        self.render_progress_bar(buffer, ix, iy + 4, iw.saturating_sub(2), self.memory_usage);

        // Disk I/O
        buffer.draw_text(
            ix,
            iy + 6,
            "Disk I/O",
            Style::fg(Rgba::from_hex(LABEL_COLOR).expect("valid")).with_bold(),
        );
        buffer.draw_text(
            ix,
            iy + 7,
            &format!("Read:  {} MB/s  Write: {} MB/s", self.disk_read, self.disk_write),
            Style::fg(Rgba::WHITE),
        );

        buffer.pop_scissor();
    }

    fn render_progress_bar(&self, buffer: &mut OptimizedBuffer, x: u32, y: u32, width: u32, value: f32) {
        let filled = ((width as f32 * value) as u32).min(width);
        let empty = width.saturating_sub(filled);

        let filled_str: String = "\u{2588}".repeat(filled as usize);
        let empty_str: String = "\u{2591}".repeat(empty as usize);

        buffer.draw_text(
            x,
            y,
            &filled_str,
            Style::fg(Rgba::from_hex(BAR_FILLED).expect("valid")),
        );
        buffer.draw_text(
            x + filled,
            y,
            &empty_str,
            Style::fg(Rgba::from_hex(BAR_EMPTY).expect("valid")),
        );

        let pct = format!(" {:.0}%", value * 100.0);
        buffer.draw_text(
            x + width + 1,
            y,
            &pct,
            Style::fg(Rgba::WHITE),
        );
    }

    fn render_logs(&self, buffer: &mut OptimizedBuffer, panel: &Panel) {
        let (ix, iy, _iw, ih) = panel.inner_rect();

        buffer.push_scissor(ClipRect::new(
            ix as i32,
            iy as i32,
            panel.width.saturating_sub(2),
            panel.height.saturating_sub(2),
        ));

        // Show last N entries that fit
        let max_entries = ih as usize;
        let start = self.event_log.len().saturating_sub(max_entries);

        for (i, entry) in self.event_log.iter().skip(start).enumerate() {
            let y = iy + i as u32;
            if i >= max_entries {
                break;
            }

            let (level_str, level_style) = match entry.level {
                LogLevel::Info => (
                    "[INFO] ",
                    Style::fg(Rgba::from_hex(INFO_COLOR).expect("valid")).with_bold(),
                ),
                LogLevel::Warn => (
                    "[WARN] ",
                    Style::fg(Rgba::from_hex(WARN_COLOR).expect("valid")).with_bold(),
                ),
                LogLevel::Error => (
                    "[ERROR]",
                    Style::fg(Rgba::from_hex(ERROR_COLOR).expect("valid")).with_bold(),
                ),
            };

            buffer.draw_text(ix, y, &entry.time, Style::dim());
            buffer.draw_text(ix + 9, y, level_str, level_style);
            buffer.draw_text(ix + 17, y, &entry.message, Style::fg(Rgba::WHITE));
        }

        buffer.pop_scissor();
    }
}

fn main() -> io::Result<()> {
    let (term_w, term_h) = terminal_size().unwrap_or((80, 24));
    let mut renderer = Renderer::new(u32::from(term_w), u32::from(term_h))?;
    let _raw_guard = enable_raw_mode()?;

    let (width, height) = renderer.size();
    let mut dashboard = Dashboard::new(width, height);
    let mut parser = InputParser::new();
    let mut stdin = io::stdin();
    let mut buf = [0u8; 64];

    // Set non-blocking mode for animation
    use std::os::unix::io::AsRawFd;
    let fd = stdin.as_raw_fd();
    // SAFETY: Setting O_NONBLOCK on stdin fd
    unsafe {
        let flags = libc::fcntl(fd, libc::F_GETFL);
        libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    loop {
        // Update simulation
        dashboard.update();

        // Render
        let (width, height) = renderer.size();
        dashboard.render(renderer.buffer(), width, height);
        renderer.present()?;

        // Process input (non-blocking)
        match stdin.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => {
                let mut offset = 0;
                while offset < n {
                    let Ok((event, used)) = parser.parse(&buf[offset..n]) else {
                        break;
                    };
                    offset += used;

                    if let Event::Resize(r) = &event {
                        renderer.resize(u32::from(r.width), u32::from(r.height))?;
                    }

                    if !dashboard.handle_input(&event) {
                        return Ok(());
                    }
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => return Err(e),
        }

        // Small delay for animation (roughly 30 FPS)
        std::thread::sleep(std::time::Duration::from_millis(33));
    }
}
