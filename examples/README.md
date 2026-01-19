# OpenTUI Examples

Interactive examples demonstrating OpenTUI's terminal UI capabilities.

## Examples

### hello.rs

A minimal buffer creation example that doesn't require terminal I/O.

```bash
cargo run --example hello
```

Shows basic usage of `OptimizedBuffer`, `Style`, `Rgba`, and box drawing.

### editor.rs

A complete interactive editor demonstrating the full rendering loop.

```bash
cargo run --example editor
```

**Features demonstrated:**
- Double-buffered rendering for flicker-free updates
- Full keyboard input with modifiers (Ctrl, Alt, Shift)
- SGR mouse tracking with click-to-position
- Visual line navigation for wrapped text
- Word boundary movement (Ctrl+Arrow keys)
- Efficient diff-based screen updates

**Controls:**
- Arrow keys: Move cursor
- Ctrl+Left/Right: Move by word
- Home/End: Line start/end
- Page Up/Down: Scroll
- Mouse click: Position cursor
- Ctrl+W: Toggle word wrap mode (none/word/char)
- Ctrl+L: Toggle line numbers
- Ctrl+D: Toggle debug overlay (shows FPS stats)
- Ctrl+Q: Quit

## Debug Mode

The editor example has a built-in debug overlay toggled with Ctrl+D that shows:
- Frame rate (FPS)
- Render statistics
- Buffer dimensions

For verbose logging to stderr, you can modify the example or use the debug overlay.

## Architecture

The examples demonstrate OpenTUI's key components:

1. **Terminal Setup**: `Renderer::new_with_options()` handles:
   - Alternate screen
   - Raw mode
   - Mouse tracking
   - Capability detection

2. **Rendering Loop**:
   ```rust
   loop {
       renderer.clear();              // Clear back buffer
       // ... draw to renderer.buffer() ...
       renderer.present()?;           // Swap and render diff
       // ... handle input ...
   }
   ```

3. **Input Handling**: `InputParser::parse()` converts raw bytes to events:
   - `Event::Key(KeyEvent)` - Keyboard input
   - `Event::Mouse(MouseEvent)` - Mouse clicks/motion/scroll
   - `Event::Resize(ResizeEvent)` - Terminal resize
   - `Event::Paste(PasteEvent)` - Bracketed paste

4. **Cleanup**: Automatic via `Drop` - terminal state is restored even on panic.

## Creating Your Own Application

```rust
use opentui::{
    InputParser, Renderer, RendererOptions, Rgba, Style,
    terminal::terminal_size,
    input::{Event, KeyCode, ParseError},
};
use std::io::{self, Read};

fn main() -> io::Result<()> {
    // 1. Get terminal size
    let (width, height) = terminal_size().unwrap_or((80, 24));

    // 2. Create renderer (handles terminal setup)
    let options = RendererOptions {
        use_alt_screen: true,
        hide_cursor: false,
        enable_mouse: true,
        query_capabilities: true,
    };
    let mut renderer = Renderer::new_with_options(width as u32, height as u32, options)?;

    // 3. Main loop
    let mut parser = InputParser::new();
    let mut input_buf = [0u8; 64];
    let stdin = io::stdin();

    loop {
        // Draw
        renderer.clear();
        renderer.buffer().draw_text(1, 1, "Hello, OpenTUI!", Style::fg(Rgba::WHITE));
        renderer.present()?;

        // Read input
        if let Ok(n) = stdin.lock().read(&mut input_buf) {
            if n > 0 {
                match parser.parse(&input_buf[..n]) {
                    Ok((Event::Key(key), _)) if key.is_ctrl_c() || key.code == KeyCode::Char('q') => {
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    // 4. Cleanup is automatic
    Ok(())
}
```
