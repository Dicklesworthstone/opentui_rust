# OpenTUI Examples

Interactive examples demonstrating OpenTUI's terminal UI capabilities.

## Examples

### 01_hello_terminal.rs

A minimal end-to-end renderer example that draws to the terminal and exits on key press.

```bash
cargo run --example 01_hello_terminal
```

Shows basic renderer setup, buffer drawing, and clean terminal restoration.

### 02_colors.rs

Demonstrates OpenTUI's RGBA color system, alpha blending, and gradients.

```bash
cargo run --example 02_colors
```

Shows color creation methods (RGB, hex, HSV), `set_blended` alpha compositing, and `lerp` gradients.

### 03_styles.rs

Demonstrates text attributes, style builder usage, and background colors.

```bash
cargo run --example 03_styles
```

Shows bold/italic/underline/dim/inverse/strikethrough, builder patterns, and color combos.

### 04_drawing.rs

Demonstrates drawing primitives (boxes, lines, fills) and simple layouts.

```bash
cargo run --example 04_drawing
```

Shows box styles, horizontal/vertical lines, filled rectangles, and composite panels.

### 05_scissor.rs

Demonstrates scissor clipping with nested clip regions.

```bash
cargo run --example 05_scissor
```

Shows how `push_scissor` limits drawing and how nested clips intersect.

### 06_opacity.rs

Demonstrates opacity stack and alpha blending for layered UI elements.

```bash
cargo run --example 06_opacity
```

Shows global opacity with overlays and blended rectangles.

### 07_input.rs

Demonstrates keyboard and mouse input parsing with `InputParser`.

```bash
cargo run --example 07_input
```

Shows latest key/mouse events and exits on `q` or Ctrl+C.

### 08_animation.rs

Demonstrates a simple render loop with a moving sprite.

```bash
cargo run --example 08_animation
```

Shows frame pacing and a bouncing dot animation.

### 09_text_buffer.rs

Demonstrates TextBuffer highlights and styled segments rendering.

```bash
cargo run --example 09_text_buffer
```

Shows styled text, log formatting, and wide-character width handling.

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

### 11_highlighting.rs

Demonstrates syntax highlighting with theme switching.

```bash
cargo run --example 11_highlighting
```

**Features demonstrated:**
- Built-in Rust tokenizer integration
- Theme switching at runtime
- Highlighted rendering via `EditorView`

**Controls:**
- 1-4: Switch themes (Dark, Light, Solarized, High Contrast)
- q / Ctrl+C: Quit

### 15_dashboard.rs

Demonstrates a multi-panel dashboard layout with focus switching.

```bash
cargo run --example 15_dashboard
```

**Features demonstrated:**
- Split-pane layout with sidebar, main panel, and log panel
- Focus indicator (Tab / Shift+Tab)
- Scissoring per panel
- Simulated metrics updates

### threaded.rs

Demonstrates `ThreadedRenderer` to move terminal I/O off the main thread.

```bash
cargo run --example threaded
```

**Features demonstrated:**
- Channel-based render thread
- `ThreadedRenderer::present()` and `shutdown()`

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
