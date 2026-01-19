# Proposed Rust Architecture

> **Design Principle:** Idiomatic Rust, not a line-by-line translation.

---

## Module Structure

```
opentui/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API exports
│   ├── color.rs            # RGBA, color operations
│   ├── style.rs            # TextAttributes, Style builder
│   ├── cell.rs             # Cell type, grapheme handling
│   ├── ansi/
│   │   ├── mod.rs          # ANSI escape sequence generation
│   │   ├── sequences.rs    # Constant sequences
│   │   └── output.rs       # Buffered ANSI writer
│   ├── buffer/
│   │   ├── mod.rs          # OptimizedBuffer
│   │   ├── scissor.rs      # Scissor stack
│   │   ├── opacity.rs      # Opacity stack
│   │   └── drawing.rs      # Text/box drawing operations
│   ├── text/
│   │   ├── mod.rs          # Text module exports
│   │   ├── rope.rs         # Rope wrapper (using ropey)
│   │   ├── segment.rs      # StyledSegment
│   │   ├── buffer.rs       # TextBuffer (styled text storage)
│   │   ├── view.rs         # TextBufferView (viewport)
│   │   ├── edit.rs         # EditBuffer (cursor, undo)
│   │   └── editor.rs       # EditorView (visual cursor)
│   ├── highlight/
│   │   ├── mod.rs          # Highlight management
│   │   └── syntax.rs       # SyntaxStyle registry
│   ├── terminal/
│   │   ├── mod.rs          # Terminal abstraction
│   │   ├── capabilities.rs # Capability detection
│   │   ├── cursor.rs       # Cursor state/styles
│   │   └── mouse.rs        # Mouse event handling
│   ├── renderer/
│   │   ├── mod.rs          # CliRenderer
│   │   ├── diff.rs         # Buffer diffing
│   │   └── hitgrid.rs      # Hit testing
│   └── unicode/
│       ├── mod.rs          # Unicode utilities
│       ├── grapheme.rs     # Grapheme clustering
│       └── width.rs        # Display width calculation
└── examples/
    ├── hello.rs
    ├── editor.rs
    └── styled_text.rs
```

---

## Core Types

### 1. Color (`color.rs`)

```rust
/// RGBA color with f32 components [0.0, 1.0]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const TRANSPARENT: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    pub const BLACK: Self = Self { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
    pub const WHITE: Self = Self { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { ... }
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self { ... }
    pub fn from_hex(hex: &str) -> Result<Self, ColorError> { ... }
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self { ... }

    pub fn blend_over(self, other: Self) -> Self { ... }
    pub fn with_alpha(self, alpha: f32) -> Self { ... }

    pub fn to_rgb_u8(self) -> (u8, u8, u8) { ... }
}
```

### 2. Style (`style.rs`)

```rust
bitflags! {
    #[derive(Clone, Copy, Debug, Default)]
    pub struct TextAttributes: u8 {
        const BOLD          = 0x01;
        const DIM           = 0x02;
        const ITALIC        = 0x04;
        const UNDERLINE     = 0x08;
        const BLINK         = 0x10;
        const INVERSE       = 0x20;
        const HIDDEN        = 0x40;
        const STRIKETHROUGH = 0x80;
    }
}

/// Complete style with colors and attributes
#[derive(Clone, Copy, Debug, Default)]
pub struct Style {
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
    pub attributes: TextAttributes,
    pub link_id: Option<u32>,
}

impl Style {
    pub fn builder() -> StyleBuilder { ... }
    pub fn bold() -> Self { ... }
    pub fn fg(color: Rgba) -> Self { ... }
    // ...
}
```

### 3. Cell (`cell.rs`)

```rust
/// A single terminal cell
#[derive(Clone, Debug)]
pub struct Cell {
    content: CellContent,
    fg: Rgba,
    bg: Rgba,
    attributes: TextAttributes,
    link_id: Option<u32>,
}

#[derive(Clone, Debug)]
pub enum CellContent {
    /// Simple character (1 codepoint, width 1-2)
    Char(char),
    /// Grapheme cluster (emoji, combining chars)
    Grapheme(Arc<str>),
    /// Empty cell (cleared)
    Empty,
    /// Continuation of wide character
    Continuation,
}

impl Cell {
    pub fn new(ch: char, style: Style) -> Self { ... }
    pub fn from_grapheme(s: &str, style: Style) -> Self { ... }
    pub fn clear(bg: Rgba) -> Self { ... }

    pub fn display_width(&self) -> usize { ... }
    pub fn write_to(&self, w: &mut impl Write) -> io::Result<()> { ... }
}
```

---

## Buffer Module

### OptimizedBuffer (`buffer/mod.rs`)

```rust
pub struct OptimizedBuffer {
    width: u32,
    height: u32,
    cells: Vec<Cell>,

    scissor_stack: Vec<ClipRect>,
    opacity_stack: Vec<f32>,

    id: String,
}

impl OptimizedBuffer {
    pub fn new(width: u32, height: u32) -> Self { ... }

    // Cell access
    pub fn get(&self, x: u32, y: u32) -> Option<&Cell> { ... }
    pub fn set(&mut self, x: u32, y: u32, cell: Cell) { ... }
    pub fn set_blended(&mut self, x: u32, y: u32, cell: Cell) { ... }

    // Drawing
    pub fn clear(&mut self, bg: Rgba) { ... }
    pub fn fill_rect(&mut self, rect: Rect, bg: Rgba) { ... }
    pub fn draw_text(&mut self, x: u32, y: u32, text: &str, style: Style) { ... }
    pub fn draw_box(&mut self, rect: Rect, border: BoxStyle) { ... }

    // Scissoring
    pub fn push_scissor(&mut self, rect: ClipRect) { ... }
    pub fn pop_scissor(&mut self) { ... }
    pub fn clear_scissors(&mut self) { ... }

    // Opacity
    pub fn push_opacity(&mut self, opacity: f32) { ... }
    pub fn pop_opacity(&mut self) { ... }
    pub fn current_opacity(&self) -> f32 { ... }

    // Composition
    pub fn draw_buffer(&mut self, x: i32, y: i32, src: &OptimizedBuffer) { ... }

    // Resize
    pub fn resize(&mut self, width: u32, height: u32) { ... }
}
```

---

## Text Module

### TextBuffer (`text/buffer.rs`)

```rust
pub struct TextBuffer {
    rope: Rope,                     // Using ropey
    segments: Vec<StyledSegment>,   // Style information
    highlights: HighlightManager,
    syntax_style: Option<Arc<SyntaxStyle>>,

    defaults: Style,
    tab_width: u8,
}

impl TextBuffer {
    pub fn new() -> Self { ... }

    // Content
    pub fn set_text(&mut self, text: &str) { ... }
    pub fn append(&mut self, text: &str) { ... }
    pub fn set_styled_text(&mut self, chunks: &[StyledChunk]) { ... }
    pub fn clear(&mut self) { ... }

    // Queries
    pub fn len_bytes(&self) -> usize { ... }
    pub fn len_chars(&self) -> usize { ... }
    pub fn len_lines(&self) -> usize { ... }
    pub fn line(&self, idx: usize) -> Option<RopeSlice> { ... }

    // Highlighting
    pub fn add_highlight(&mut self, range: Range<usize>, style_id: u32, priority: u8) { ... }
    pub fn clear_highlights(&mut self) { ... }
}
```

### TextBufferView (`text/view.rs`)

```rust
pub struct TextBufferView<'a> {
    buffer: &'a TextBuffer,
    viewport: Viewport,
    wrap_mode: WrapMode,

    selection: Option<Selection>,
    line_cache: LineCache,
}

pub enum WrapMode {
    None,
    Char,
    Word,
}

impl<'a> TextBufferView<'a> {
    pub fn new(buffer: &'a TextBuffer) -> Self { ... }

    pub fn viewport(self, x: u32, y: u32, w: u32, h: u32) -> Self { ... }
    pub fn wrap_mode(self, mode: WrapMode) -> Self { ... }

    pub fn virtual_line_count(&self) -> usize { ... }
    pub fn set_selection(&mut self, start: usize, end: usize, style: Style) { ... }
    pub fn selected_text(&self) -> Option<String> { ... }

    pub fn render_to(&self, buffer: &mut OptimizedBuffer, x: i32, y: i32) { ... }
}
```

### EditBuffer (`text/edit.rs`)

```rust
pub struct EditBuffer {
    buffer: TextBuffer,
    cursor: Cursor,
    history: History,
}

pub struct Cursor {
    pub offset: usize,
    pub row: usize,
    pub col: usize,
}

impl EditBuffer {
    pub fn new() -> Self { ... }

    // Cursor movement
    pub fn move_left(&mut self) { ... }
    pub fn move_right(&mut self) { ... }
    pub fn move_up(&mut self) { ... }
    pub fn move_down(&mut self) { ... }
    pub fn move_to(&mut self, row: usize, col: usize) { ... }

    // Editing
    pub fn insert(&mut self, text: &str) { ... }
    pub fn delete_backward(&mut self) { ... }
    pub fn delete_forward(&mut self) { ... }
    pub fn delete_range(&mut self, start: Cursor, end: Cursor) { ... }

    // History
    pub fn undo(&mut self) -> bool { ... }
    pub fn redo(&mut self) -> bool { ... }
    pub fn can_undo(&self) -> bool { ... }
    pub fn can_redo(&self) -> bool { ... }
}
```

---

## Renderer Module

### CliRenderer (`renderer/mod.rs`)

```rust
pub struct Renderer {
    width: u32,
    height: u32,

    front_buffer: OptimizedBuffer,
    back_buffer: OptimizedBuffer,

    terminal: Terminal,
    hit_grid: HitGrid,

    background: Rgba,
}

impl Renderer {
    pub fn new(width: u32, height: u32) -> io::Result<Self> { ... }

    // Drawing target
    pub fn buffer(&mut self) -> &mut OptimizedBuffer {
        &mut self.back_buffer
    }

    // Present frame
    pub fn present(&mut self) -> io::Result<()> { ... }
    pub fn present_force(&mut self) -> io::Result<()> { ... }

    // Terminal
    pub fn resize(&mut self, width: u32, height: u32) -> io::Result<()> { ... }
    pub fn set_cursor(&mut self, x: u32, y: u32, visible: bool) { ... }
    pub fn set_cursor_style(&mut self, style: CursorStyle, blinking: bool) { ... }
    pub fn set_title(&mut self, title: &str) -> io::Result<()> { ... }

    // Hit testing
    pub fn register_hit_area(&mut self, rect: Rect, id: u32) { ... }
    pub fn hit_test(&self, x: u32, y: u32) -> Option<u32> { ... }

    // Cleanup
    pub fn cleanup(&mut self) -> io::Result<()> { ... }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
```

---

## Example Usage

```rust
use opentui::{Renderer, TextBuffer, TextBufferView, Style, Rgba, WrapMode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create renderer
    let mut renderer = Renderer::new(80, 24)?;

    // Create styled text
    let mut buffer = TextBuffer::new();
    buffer.set_text("Hello, OpenTUI!");
    buffer.add_highlight(0..5, Style::bold().fg(Rgba::from_hex("#FF0000")?));

    // Create view with wrapping
    let view = TextBufferView::new(&buffer)
        .viewport(0, 0, 80, 24)
        .wrap_mode(WrapMode::Word);

    // Draw and present
    view.render_to(renderer.buffer(), 0, 0);
    renderer.present()?;

    // Wait for input...

    Ok(())
}
```

---

## Crate Dependencies

```toml
[dependencies]
ropey = "1.6"
unicode-segmentation = "1.10"
unicode-width = "0.1"
crossterm = "0.27"
bitflags = "2.4"

[dev-dependencies]
criterion = "0.5"
```

---

## Design Decisions

### 1. Grapheme Handling
- Use `Arc<str>` for grapheme storage instead of custom pool
- Pool if benchmarks show memory pressure

### 2. Buffer Diffing
- Compare cells directly (PartialEq)
- Track dirty regions for optimization

### 3. Error Handling
- Use `thiserror` for custom errors
- Return `io::Result` for terminal operations

### 4. Thread Safety
- Buffer operations are single-threaded
- Renderer owns its buffers exclusively

### 5. Zero-Copy Where Possible
- `RopeSlice` for text queries
- `&str` for transient text
