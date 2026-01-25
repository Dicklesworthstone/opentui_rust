//! Example 11: Syntax highlighting demonstration.
//!
//! Run with:
//!   cargo run --example `11_highlighting`
//!
//! Controls:
//!   1-4 : Switch themes (Dark, Light, Solarized, High Contrast)
//!   q   : Quit
//!   Ctrl+C : Quit

use std::io::{self, Read};

use opentui::highlight::{Theme, TokenizerRegistry};
use opentui::input::{Event, KeyCode, KeyModifiers, ParseError};
use opentui::terminal::terminal_size;
use opentui::{EditBuffer, EditorView, InputParser, Renderer, RendererOptions, Style, WrapMode};

const SAMPLE_CODE: &str = r#"
/// A tiny demo type with a lifetime.
#[derive(Debug, Clone)]
struct Widget<'a> {
    name: &'a str,
    size: u32,
}

/// Compute a factorial for highlighting.
fn factorial(n: u64) -> u64 {
    match n {
        0 | 1 => 1,
        _ => n * factorial(n - 1),
    }
}

fn main() -> Result<(), String> {
    let raw = r"raw string";
    let bytes = b"bytes";
    let hex = 0xDEAD_BEEF_u64;
    let float = 3.14e-2;
    let items = vec![Widget { name: "bolt", size: 3 }];

    'outer: for item in &items {
        let msg = format!("{item:?} -> {}", factorial(item.size as u64));
        println!("{}", msg);
        break 'outer;
    }

    Ok(())
}
"#;

fn main() -> io::Result<()> {
    if wants_help() {
        print_help();
        return Ok(());
    }

    let (width, height) = terminal_size().unwrap_or((80, 24));
    let width = u32::from(width);
    let height = u32::from(height);

    let options = RendererOptions {
        use_alt_screen: true,
        hide_cursor: false,
        enable_mouse: false,
        query_capabilities: true,
    };
    let mut renderer = Renderer::new_with_options(width, height, options)?;
    renderer.set_title("OpenTUI Highlighting Demo")?;

    let edit_buffer = EditBuffer::with_text(SAMPLE_CODE);
    let mut editor = EditorView::new(edit_buffer);
    configure_editor(&mut editor);

    let registry = TokenizerRegistry::with_builtins();
    let _ = editor.enable_highlighting_for_extension(&registry, "rs");

    let themes = [
        Theme::dark(),
        Theme::light(),
        Theme::solarized_dark(),
        Theme::high_contrast(),
    ];
    run_loop(&mut renderer, &mut editor, &themes, width, height)
}

fn configure_editor(editor: &mut EditorView) {
    editor.set_wrap_mode(WrapMode::None);
    editor.set_line_numbers(true);
}

fn run_loop(
    renderer: &mut Renderer,
    editor: &mut EditorView,
    themes: &[Theme; 4],
    width: u32,
    height: u32,
) -> io::Result<()> {
    let mut current_theme = 0usize;
    apply_theme(renderer, editor, &themes[current_theme]);
    update_viewport(editor, width, height);

    let mut parser = InputParser::new();
    let mut input_accumulator = Vec::with_capacity(256);
    let mut read_buf = [0u8; 256];
    let stdin = io::stdin();

    draw_frame(renderer, editor, &themes[current_theme])?;

    let mut running = true;
    while running {
        let n = stdin.lock().read(&mut read_buf)?;
        if n == 0 {
            continue;
        }

        input_accumulator.extend_from_slice(&read_buf[..n]);
        let mut offset = 0usize;
        let mut needs_redraw = false;

        while offset < input_accumulator.len() {
            match parser.parse(&input_accumulator[offset..]) {
                Ok((event, consumed)) => {
                    offset += consumed;
                    match event {
                        Event::Key(key) => {
                            if key.is_ctrl_c()
                                || key.matches(KeyCode::Char('q'), KeyModifiers::empty())
                            {
                                running = false;
                            } else if let Some(next) = theme_index(key.code) {
                                current_theme = next;
                                apply_theme(renderer, editor, &themes[current_theme]);
                                needs_redraw = true;
                            }
                        }
                        Event::Resize(resize) => {
                            let new_width = u32::from(resize.width);
                            let new_height = u32::from(resize.height);
                            renderer.resize(new_width, new_height)?;
                            update_viewport(editor, new_width, new_height);
                            apply_theme(renderer, editor, &themes[current_theme]);
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                }
                Err(ParseError::Incomplete | ParseError::Empty) => break,
                Err(_) => {
                    offset += 1;
                }
            }
        }

        if offset > 0 {
            input_accumulator.drain(..offset);
        }

        if !running {
            break;
        }

        if needs_redraw {
            draw_frame(renderer, editor, &themes[current_theme])?;
        }
    }

    Ok(())
}

fn apply_theme(renderer: &mut Renderer, editor: &mut EditorView, theme: &Theme) {
    renderer.set_background(theme.background());
    editor.set_highlighting_theme(theme.clone());
    editor.set_selection_style(Style::builder().bg(theme.selection()).build());
    editor.set_cursor_style(Style::builder().bg(theme.cursor()).build());
}

fn update_viewport(editor: &mut EditorView, width: u32, height: u32) {
    let content_x = 1;
    let content_y = 2;
    let content_width = width.saturating_sub(2);
    let content_height = height.saturating_sub(3);
    editor.set_viewport(content_x, content_y, content_width, content_height);
}

fn draw_frame(renderer: &mut Renderer, editor: &mut EditorView, theme: &Theme) -> io::Result<()> {
    renderer.clear();
    let (_, height) = renderer.size();
    let buffer = renderer.buffer();

    let title = format!(
        " Syntax Highlighting Demo - Theme: {} (1-4 switch, q quit) ",
        theme.name()
    );
    buffer.draw_text(
        1,
        0,
        &title,
        Style::builder().fg(theme.foreground()).bold().build(),
    );

    let help = "1-4: switch theme | q: quit | Ctrl+C: quit";
    let help_y = height.saturating_sub(1);
    buffer.draw_text(
        1,
        help_y,
        help,
        Style::builder().fg(theme.line_number()).build(),
    );

    editor.render_to(buffer, 0, 0, 0, 0);
    renderer.present()
}

const fn theme_index(code: KeyCode) -> Option<usize> {
    match code {
        KeyCode::Char('1') => Some(0),
        KeyCode::Char('2') => Some(1),
        KeyCode::Char('3') => Some(2),
        KeyCode::Char('4') => Some(3),
        _ => None,
    }
}

fn wants_help() -> bool {
    std::env::args().any(|arg| arg == "--help" || arg == "-h")
}

fn print_help() {
    println!("OpenTUI Highlighting Example");
    println!();
    println!("Usage:");
    println!("  cargo run --example 11_highlighting");
    println!();
    println!("Controls:");
    println!("  1-4 : Switch themes (Dark, Light, Solarized, High Contrast)");
    println!("  q   : Quit");
    println!("  Ctrl+C : Quit");
}
