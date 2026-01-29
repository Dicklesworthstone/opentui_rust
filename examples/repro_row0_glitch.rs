//! Reproduction for row-0 duplication glitch in present_force().
//!
//! Before the fix, the first visible row would appear duplicated because
//! AnsiWriter assumed the cursor started at (0,0) but the terminal cursor
//! was actually at the end of the last row from the previous frame (in
//! pending-wrap state). The first move_cursor(0,0) was a no-op, causing
//! row 0 content to be written at the wrong position.
//!
//! Run: cargo run --example repro_row0_glitch
//! Use j/k to scroll, q to quit. Without the fix, the first row duplicates.

use std::io::Read;
use std::time::Duration;

use opentui::input::ParseError;
use opentui::{
    enable_raw_mode, terminal_size, Event, InputParser, KeyCode, KeyModifiers, OptimizedBuffer,
    Renderer, RendererOptions, Rgba, Style,
};

struct App {
    items: Vec<String>,
    index: usize,
    scroll: usize,
    width: u32,
    height: u32,
    needs_redraw: bool,
}

impl App {
    fn visible_height(&self) -> usize {
        self.height.saturating_sub(2) as usize
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = terminal_size().unwrap_or((80, 24));
    let (width, height) = (u32::from(width), u32::from(height));
    let mut app = App {
        items: (1..=30)
            .map(|i| format!("item {i:02} - example entry"))
            .collect(),
        index: 0,
        scroll: 0,
        width,
        height,
        needs_redraw: true,
    };

    let _raw_guard = enable_raw_mode()?;
    let options = RendererOptions {
        use_alt_screen: true,
        hide_cursor: true,
        enable_mouse: false,
        query_capabilities: false,
    };
    let mut renderer = Renderer::new_with_options(width, height, options)?;
    let bg = Rgba::from_hex("#1a1b26").unwrap_or(Rgba::BLACK);
    renderer.set_background(bg);

    let mut input = InputParser::new();

    loop {
        if let Ok((tw, th)) = terminal_size() {
            let (tw, th) = (u32::from(tw), u32::from(th));
            if tw != app.width || th != app.height {
                app.width = tw;
                app.height = th;
                app.needs_redraw = true;
                renderer.resize(tw, th)?;
            }
        }

        if app.needs_redraw {
            renderer.invalidate();
            app.needs_redraw = false;
        }

        renderer.clear();
        render(&app, renderer.buffer());
        renderer.present()?;

        let mut buf = [0u8; 32];
        if let Ok(n) = read_stdin_timeout(&mut buf, Duration::from_millis(100)) {
            if n > 0 {
                let mut offset = 0;
                while offset < n {
                    match input.parse(&buf[offset..n]) {
                        Ok((event, consumed)) => {
                            offset += consumed;
                            if handle_event(&mut app, event) {
                                return Ok(());
                            }
                        }
                        Err(ParseError::Empty | ParseError::Incomplete) => break,
                        Err(_) => offset += 1,
                    }
                }
            }
        }
    }
}

fn handle_event(app: &mut App, event: Event) -> bool {
    if let Event::Key(key) = event {
        if key.modifiers.contains(KeyModifiers::CTRL) && key.code == KeyCode::Char('c') {
            return true;
        }
        match key.code {
            KeyCode::Char('q') => return true,
            KeyCode::Char('j') | KeyCode::Down => {
                if app.index + 1 < app.items.len() {
                    app.index += 1;
                    let vis = app.visible_height();
                    if app.index >= app.scroll + vis {
                        app.scroll = app.index - vis + 1;
                    }
                    app.needs_redraw = true;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.index > 0 {
                    app.index -= 1;
                    if app.index < app.scroll {
                        app.scroll = app.index;
                    }
                    app.needs_redraw = true;
                }
            }
            _ => {}
        }
    }
    false
}

fn render(app: &App, buffer: &mut OptimizedBuffer) {
    let bg = Rgba::from_hex("#1a1b26").unwrap_or(Rgba::BLACK);
    let fg = Rgba::from_hex("#c0caf5").unwrap_or(Rgba::WHITE);
    let muted = Rgba::from_hex("#565f89").unwrap_or(Rgba::WHITE);
    let sel_bg = Rgba::from_hex("#33467c").unwrap_or(Rgba::BLUE);

    let (w, h) = (app.width, app.height);

    // Header
    buffer.fill_rect(0, 0, w, 1, bg);
    buffer.draw_text(2, 0, "Row-0 Glitch Repro", Style::fg(fg).with_bold());

    // List
    let list_y = 1u32;
    let list_h = h.saturating_sub(2);
    buffer.fill_rect(0, list_y, w, list_h, bg);

    let vis = app.visible_height();
    let start = app.scroll.min(app.items.len());
    let end = (start + vis).min(app.items.len());

    for (row, item) in app.items[start..end].iter().enumerate() {
        let idx = start + row;
        let y = list_y + row as u32;
        let selected = idx == app.index;
        let row_bg = if selected { sel_bg } else { bg };
        let row_fg = if selected { fg } else { fg };

        buffer.fill_rect(0, y, w, 1, row_bg);
        let prefix = if selected { "> " } else { "  " };
        buffer.draw_text(0, y, prefix, Style::fg(row_fg).with_bg(row_bg));
        buffer.draw_text(2, y, item, Style::fg(row_fg).with_bg(row_bg));
    }

    // Status bar
    let help_y = h.saturating_sub(1);
    buffer.fill_rect(0, help_y, w, 1, bg);
    buffer.draw_text(2, help_y, "j/k move  q quit", Style::fg(muted));
}

fn read_stdin_timeout(buf: &mut [u8], _timeout: Duration) -> std::io::Result<usize> {
    std::io::stdin().read(buf)
}
