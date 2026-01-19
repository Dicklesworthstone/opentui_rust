//! Interactive editor example demonstrating the full `OpenTUI` rendering loop.
//!
//! This example shows how to build a complete interactive terminal application
//! with keyboard and mouse input handling, visual navigation in wrapped text,
//! and efficient double-buffered rendering.
//!
//! Run with: cargo run --example editor
//!
//! Keys:
//! - Ctrl+Q: Quit
//! - Ctrl+W: Toggle word wrap
//! - Ctrl+L: Toggle line numbers
//! - Ctrl+D: Toggle debug overlay
//! - Arrow keys: Move cursor
//! - Ctrl+Left/Right: Move by word
//! - Home/End: Line start/end
//! - Page Up/Down: Scroll
//! - Mouse: Click to position cursor

// Example uses libc select for polling input.
#![allow(unsafe_code)]
#![allow(clippy::borrow_as_ptr)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::if_not_else)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::needless_raw_string_hashes)]

use opentui::buffer::BoxStyle;
use opentui::input::{Event, KeyCode, MouseEventKind, ParseError};
use opentui::terminal::terminal_size;
use opentui::{
    EditBuffer, EditorView, InputParser, OptimizedBuffer, Renderer, RendererOptions, Rgba, Style,
    WrapMode,
};
use std::io::{self, Read};
use std::time::Duration;

const SAMPLE_TEXT: &str = r"Welcome to OpenTUI for Rust!

This is a demonstration of the full rendering loop with:
- Double-buffered rendering for flicker-free updates
- Full keyboard input with modifiers (Ctrl, Alt, Shift)
- SGR mouse tracking with click-to-position
- Visual line navigation for wrapped text
- Word boundary movement (Ctrl+Arrow keys)
- Efficient diff-based screen updates

Try pressing Ctrl+W to toggle word wrap mode, then use the
arrow keys to navigate. Notice how visual navigation moves
through wrapped lines correctly.

The quick brown fox jumps over the lazy dog. This sentence
is long enough to demonstrate word wrapping when enabled.
Pack my box with five dozen liquor jugs.

Lorem ipsum dolor sit amet, consectetur adipiscing elit.
Sed do eiusmod tempor incididunt ut labore et dolore magna
aliqua. Ut enim ad minim veniam, quis nostrud exercitation
ullamco laboris nisi ut aliquip ex ea commodo consequat.

Happy editing!";

fn main() -> io::Result<()> {
    // Get terminal size
    let (width, height) = terminal_size().unwrap_or((80, 24));
    let width = width as u32;
    let height = height as u32;

    // Create renderer with options
    let options = RendererOptions {
        use_alt_screen: true,
        hide_cursor: false, // We'll manage cursor ourselves
        enable_mouse: true,
        query_capabilities: true,
    };
    let mut renderer = Renderer::new_with_options(width, height, options)?;
    renderer.set_title("OpenTUI Editor Demo")?;
    renderer.set_background(Rgba::from_rgb_u8(25, 25, 35));

    // Create editor with sample text
    let edit_buffer = EditBuffer::with_text(SAMPLE_TEXT);
    let mut editor = EditorView::new(edit_buffer);

    // Configure editor
    editor.set_wrap_mode(WrapMode::None);
    editor.set_line_numbers(true);
    editor.set_cursor_style(Style::builder().inverse().build());
    editor.set_selection_style(Style::builder().bg(Rgba::from_rgb_u8(60, 80, 140)).build());
    editor.set_line_number_style(
        Style::builder()
            .fg(Rgba::from_rgb_u8(100, 100, 120))
            .build(),
    );

    // Calculate viewport (leave room for status bar)
    let gutter_width = editor.gutter_width();
    let text_width = width.saturating_sub(gutter_width).saturating_sub(2); // borders
    let text_height = height.saturating_sub(3); // title + status
    editor.set_viewport(1 + gutter_width, 2, text_width, text_height);

    // Input parser
    let mut parser = InputParser::new();
    let mut input_buf = [0u8; 64];

    // State
    let mut show_debug = false;
    let mut wrap_mode = WrapMode::None;
    let mut running = true;

    // Set stdin to non-blocking
    let stdin = io::stdin();

    // Enable raw mode on the renderer's terminal
    // (The renderer manages this, but we need non-blocking reads)

    while running {
        // 1. Draw frame
        renderer.clear();
        let buffer = renderer.buffer();

        // Draw border
        let border_style = Style::fg(Rgba::from_rgb_u8(80, 100, 140));
        buffer.draw_box(0, 1, width, height - 2, BoxStyle::rounded(border_style));

        // Draw title bar
        let title_style = Style::builder()
            .fg(Rgba::from_hex("#FF9900").unwrap())
            .bold()
            .build();
        buffer.draw_text(2, 0, " OpenTUI Editor ", title_style);

        // Render editor content
        render_editor(&editor, buffer, 1, 2, width - 2, height - 4);

        // Draw status bar
        draw_status_bar(buffer, &editor, width, height, wrap_mode, show_debug);

        // Toggle debug overlay
        renderer.set_debug_overlay(show_debug);

        // Present frame
        renderer.present()?;

        // 2. Read input (with timeout for responsive updates)
        // Note: In a real app you'd use non-blocking I/O or async
        if let Ok(n) = read_with_timeout(&stdin, &mut input_buf, Duration::from_millis(16)) {
            if n > 0 {
                let mut offset = 0;
                while offset < n {
                    match parser.parse(&input_buf[offset..n]) {
                        Ok((event, consumed)) => {
                            offset += consumed;

                            // Handle event
                            match handle_event(
                                &event,
                                &mut editor,
                                &mut wrap_mode,
                                &mut show_debug,
                                text_width,
                                text_height,
                            ) {
                                EventResult::Continue => {}
                                EventResult::Quit => running = false,
                            }
                        }
                        Err(ParseError::Incomplete) => break,
                        Err(ParseError::Empty) => break,
                        Err(_) => {
                            offset += 1; // Skip unrecognized byte
                        }
                    }
                }
            }
        }
    }

    // Cleanup is automatic via Drop
    Ok(())
}

/// Result of handling an event.
enum EventResult {
    Continue,
    Quit,
}

/// Handle a terminal event.
fn handle_event(
    event: &Event,
    editor: &mut EditorView,
    wrap_mode: &mut WrapMode,
    show_debug: &mut bool,
    viewport_width: u32,
    viewport_height: u32,
) -> EventResult {
    match event {
        Event::Key(key) => {
            // Check for quit (Ctrl+Q or Ctrl+C)
            if key.is_ctrl_c() || (key.ctrl() && key.code == KeyCode::Char('q')) {
                return EventResult::Quit;
            }

            // Check for other control keys
            if key.ctrl() {
                match key.code {
                    KeyCode::Char('w') => {
                        // Toggle word wrap
                        *wrap_mode = match *wrap_mode {
                            WrapMode::None => WrapMode::Word,
                            WrapMode::Word => WrapMode::Char,
                            WrapMode::Char => WrapMode::None,
                        };
                        editor.set_wrap_mode(*wrap_mode);
                    }
                    KeyCode::Char('l') => {
                        // Toggle line numbers
                        let enabled = editor.gutter_width() > 0;
                        editor.set_line_numbers(!enabled);
                    }
                    KeyCode::Char('d') => {
                        // Toggle debug overlay
                        *show_debug = !*show_debug;
                    }
                    KeyCode::Left => {
                        // Move word left
                        editor.edit_buffer_mut().move_word_left();
                    }
                    KeyCode::Right => {
                        // Move word right
                        editor.edit_buffer_mut().move_word_right();
                    }
                    _ => {}
                }
            } else {
                // Regular keys
                match key.code {
                    KeyCode::Up => {
                        if *wrap_mode != WrapMode::None {
                            editor.move_up_visual(viewport_width, viewport_height);
                        } else {
                            editor.edit_buffer_mut().move_up();
                        }
                    }
                    KeyCode::Down => {
                        if *wrap_mode != WrapMode::None {
                            editor.move_down_visual(viewport_width, viewport_height);
                        } else {
                            editor.edit_buffer_mut().move_down();
                        }
                    }
                    KeyCode::Left => {
                        editor.edit_buffer_mut().move_left();
                    }
                    KeyCode::Right => {
                        editor.edit_buffer_mut().move_right();
                    }
                    KeyCode::Home => {
                        if *wrap_mode != WrapMode::None {
                            editor.move_to_visual_sol(viewport_width, viewport_height);
                        } else {
                            editor.edit_buffer_mut().move_to_line_start();
                        }
                    }
                    KeyCode::End => {
                        if *wrap_mode != WrapMode::None {
                            editor.move_to_visual_eol(viewport_width, viewport_height);
                        } else {
                            editor.edit_buffer_mut().move_to_line_end();
                        }
                    }
                    KeyCode::PageUp => {
                        for _ in 0..viewport_height {
                            if *wrap_mode != WrapMode::None {
                                editor.move_up_visual(viewport_width, viewport_height);
                            } else {
                                editor.edit_buffer_mut().move_up();
                            }
                        }
                    }
                    KeyCode::PageDown => {
                        for _ in 0..viewport_height {
                            if *wrap_mode != WrapMode::None {
                                editor.move_down_visual(viewport_width, viewport_height);
                            } else {
                                editor.edit_buffer_mut().move_down();
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        editor.edit_buffer_mut().insert(&c.to_string());
                    }
                    KeyCode::Enter => {
                        editor.edit_buffer_mut().insert("\n");
                    }
                    KeyCode::Backspace => {
                        editor.edit_buffer_mut().delete_backward();
                    }
                    KeyCode::Delete => {
                        editor.edit_buffer_mut().delete_forward();
                    }
                    KeyCode::Tab => {
                        editor.edit_buffer_mut().insert("    ");
                    }
                    _ => {}
                }
            }
        }
        Event::Mouse(mouse) => {
            if mouse.kind == MouseEventKind::Press {
                // Click to position cursor (rough approximation)
                // In a real editor, you'd compute the exact text position
                let text_x = mouse.x.saturating_sub(editor.gutter_width() + 1);
                let text_y = mouse.y.saturating_sub(2);
                let eb = editor.edit_buffer_mut();
                eb.goto_line(text_y as usize);
                for _ in 0..text_x {
                    eb.move_right();
                }
            }
        }
        Event::Resize(resize) => {
            // Would need to resize renderer here
            let _ = resize;
        }
        _ => {}
    }
    EventResult::Continue
}

/// Render the editor to a buffer region.
fn render_editor(
    editor: &EditorView,
    buffer: &mut OptimizedBuffer,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    let eb = editor.edit_buffer();
    let text = eb.text();
    let cursor = eb.cursor();
    let gutter_w = editor.gutter_width();

    // Draw line numbers and text
    let lines: Vec<&str> = text.lines().collect();
    let num_lines = lines.len();

    for row in 0..height {
        let line_idx = row as usize;
        if line_idx >= num_lines {
            break;
        }

        // Line number
        if gutter_w > 0 {
            let num_str = format!("{:>width$} ", line_idx + 1, width = (gutter_w - 1) as usize);
            buffer.draw_text(
                x,
                y + row,
                &num_str,
                Style::fg(Rgba::from_rgb_u8(80, 80, 100)),
            );
        }

        // Line text
        let line = lines.get(line_idx).unwrap_or(&"");
        let text_x = x + gutter_w;
        let available_width = width.saturating_sub(gutter_w) as usize;
        let display_line: String = line.chars().take(available_width).collect();
        buffer.draw_text(text_x, y + row, &display_line, Style::fg(Rgba::WHITE));

        // Draw cursor if on this line
        if line_idx == cursor.row {
            let cursor_x = text_x + cursor.col as u32;
            if cursor_x < x + width {
                // Draw cursor character with inverse style
                let cursor_char = line.chars().nth(cursor.col).unwrap_or(' ');
                buffer.draw_text(
                    cursor_x,
                    y + row,
                    &cursor_char.to_string(),
                    Style::builder().inverse().build(),
                );
            }
        }
    }
}

/// Draw the status bar.
fn draw_status_bar(
    buffer: &mut OptimizedBuffer,
    editor: &EditorView,
    width: u32,
    height: u32,
    wrap_mode: WrapMode,
    show_debug: bool,
) {
    let eb = editor.edit_buffer();
    let cursor = eb.cursor();
    let y = height - 1;

    // Status bar background
    for x in 0..width {
        buffer.draw_text(
            x,
            y,
            " ",
            Style::builder().bg(Rgba::from_rgb_u8(40, 45, 55)).build(),
        );
    }

    // Left side: mode and position
    let wrap_str = match wrap_mode {
        WrapMode::None => "nowrap",
        WrapMode::Word => "word",
        WrapMode::Char => "char",
    };
    let left = format!(
        " Ln {}, Col {} | {} | {}",
        cursor.row + 1,
        cursor.col + 1,
        wrap_str,
        if show_debug { "DEBUG" } else { "" }
    );
    buffer.draw_text(
        0,
        y,
        &left,
        Style::builder()
            .fg(Rgba::WHITE)
            .bg(Rgba::from_rgb_u8(40, 45, 55))
            .build(),
    );

    // Right side: help
    let help = "^Q Quit | ^W Wrap | ^D Debug ";
    let help_x = width.saturating_sub(help.len() as u32);
    buffer.draw_text(
        help_x,
        y,
        help,
        Style::builder()
            .fg(Rgba::from_rgb_u8(150, 150, 170))
            .bg(Rgba::from_rgb_u8(40, 45, 55))
            .build(),
    );
}

/// Read from stdin with a timeout (platform-specific).
#[cfg(unix)]
fn read_with_timeout(stdin: &io::Stdin, buf: &mut [u8], timeout: Duration) -> io::Result<usize> {
    use std::os::unix::io::AsRawFd;

    let fd = stdin.as_raw_fd();

    // Use select() for timeout
    let mut read_fds = std::mem::MaybeUninit::<libc::fd_set>::uninit();
    unsafe {
        libc::FD_ZERO(read_fds.as_mut_ptr());
        libc::FD_SET(fd, read_fds.as_mut_ptr());
    }

    let mut tv = libc::timeval {
        tv_sec: timeout.as_secs() as libc::time_t,
        tv_usec: timeout.subsec_micros() as libc::suseconds_t,
    };

    let result = unsafe {
        libc::select(
            fd + 1,
            read_fds.as_mut_ptr(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut tv,
        )
    };

    if result > 0 {
        stdin.lock().read(buf)
    } else {
        Ok(0)
    }
}

#[cfg(not(unix))]
fn read_with_timeout(_stdin: &io::Stdin, _buf: &mut [u8], _timeout: Duration) -> io::Result<usize> {
    // On non-Unix, just return 0 (no input)
    // A real implementation would use platform-specific APIs
    std::thread::sleep(_timeout);
    Ok(0)
}
