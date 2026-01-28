# demo_showcase Specification

> **Version:** 1.0
> **Status:** Draft
> **Bead:** bd-2mu

This document defines the app concept, panel layout, and interaction model for `demo_showcase` — a demonstration binary showcasing OpenTUI's rendering capabilities.

---

## 1. App Concept

### 1.1 The Story

The demo presents itself as a **Developer Workbench**:

- A project workspace view
- A live code editor with syntax highlighting
- A preview/inspector panel showing real-time visualizations
- A log stream and quick actions

This narrative naturally exercises all OpenTUI primitives: buffers, cells, colors, text editing, grapheme pools, alpha blending, scissor clipping, and hyperlinks.

### 1.2 Goals

1. **Showcase all OpenTUI features** in a cohesive, realistic UI
2. **Demonstrate performance** with sub-millisecond frame times
3. **Provide a testable binary** for CI and visual regression
4. **Serve as a reference implementation** for OpenTUI users

---

## 2. Screen Layout

### 2.1 Baseline Layout (Wide Terminal, 80+ cols)

```
┌────────────────────────────────────────────────────────────────────────────┐
│  OpenTUI Showcase      Project: opentui_rust      Mode: Normal     22:05   │  <- TopBar
├───────────────┬───────────────────────────────────────────┬───────────────┤
│ Sidebar       │ Editor                                    │ Preview       │
│ (scrollable)  │ (rope + undo + highlight)                 │ (pixels/viz)  │
│               │                                           │               │
│  • Overview   │  fn main() {                              │   ███░░       │
│  • Editor     │      println!("hello");                   │   ▜▛▌▐        │
│  • Preview    │  }                                        │               │
│  • Logs       │                                           │  Alpha blend  │
│  • Unicode    │                                           │  Scissor clip │
│  • Perf       │                                           │               │
├───────────────┴───────────────────────────────────────────┴───────────────┤
│  F1 Help  Ctrl+P Command  Ctrl+T Tour  Ctrl+Q Quit   FPS:60  Cells:123     │  <- StatusBar
└────────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Region Definitions

| Region | Position | Width | Height | Features Demonstrated |
|--------|----------|-------|--------|----------------------|
| TopBar | row 0 | 100% | 1 | Style, text rendering, time updates |
| Sidebar | col 0, row 1..H-2 | 15 cols | dynamic | Scrollable list, selection, focus |
| Editor | col 16, row 1..H-2 | dynamic | dynamic | EditorView, syntax highlighting, undo/redo |
| Preview | right edge, row 1..H-2 | 15 cols | dynamic | PixelBuffer, alpha blending, charts |
| StatusBar | row H-1 | 100% | 1 | Key hints, FPS counter, cell count |
| ToastArea | bottom-right overlay | 30 cols | 3 rows | Transient notifications, opacity fade |

### 2.3 Layout Computation

All layout is **manual rectangle calculation** — OpenTUI is a rendering engine, not a layout system.

```rust
struct LayoutRects {
    top_bar: Rect,       // { 0, 0, width, 1 }
    sidebar: Rect,       // { 0, 1, 15, height - 2 }
    editor: Rect,        // { 16, 1, width - 32, height - 2 }
    preview: Rect,       // { width - 15, 1, 15, height - 2 }
    status_bar: Rect,    // { 0, height - 1, width, 1 }
    toast_area: Rect,    // { width - 32, height - 5, 30, 3 }
}

impl LayoutRects {
    fn compute(width: usize, height: usize) -> Self { ... }
}
```

---

## 3. Panel Specifications

### 3.1 TopBar

**Purpose:** Brand identity, context, and mode indicator.

| Element | Position | Content | Style |
|---------|----------|---------|-------|
| Brand | left | "OpenTUI Showcase" | Bold, primary color |
| Project | center-left | "Project: opentui_rust" | Dim |
| Mode | center-right | "Mode: Normal/Insert/Command" | Inverted when active |
| Clock | right | "HH:MM" | Dim, updates every second |

**Implementation:**
- Single `draw_text()` calls with computed x offsets
- Clock updates via render loop (not async)

### 3.2 Sidebar

**Purpose:** Section navigation.

**Content:**
```rust
enum Section {
    Overview,    // Default "wow" composition
    Editor,      // Focus on editor, preview shows cursor info
    Preview,     // Preview panel emphasized, editor minimal
    Logs,        // Log stream prominent
    Unicode,     // Grapheme pool showpiece, width rulers
    Performance, // FPS graphs, timing breakdowns
    Settings,    // Opens overlay or command palette
}
```

**Behavior:**
- Arrow keys navigate (wraps at boundaries)
- Enter/Space selects section
- Selected section has inverted style
- Focused section has border highlight
- Scrolls when list exceeds viewport

**Scrolling Rules:**
- Track `scroll_offset: usize`
- Visible range: `scroll_offset..scroll_offset + viewport_height`
- Auto-scroll to keep selection visible
- Scissor clip to sidebar rect

**Feature Coverage:**
- `push_scissor()` / `pop_scissor()` for clipping
- Scrollable list rendering
- Selection highlighting
- Focus indication

### 3.3 Editor

**Purpose:** Showcase `EditorView` with syntax highlighting.

**Features Demonstrated:**
- `EditBuffer` with cursor and selection
- `HighlightedBuffer` with Rust tokenizer
- Line numbers (gutter)
- Word wrap (configurable)
- Undo/redo stack
- Selection highlighting (inverted colors)
- Visual cursor (blinking optional)

**Content:** Pre-loaded Rust sample code:
```rust
//! OpenTUI demo source

fn main() {
    let greeting = "Hello, World!";
    println!("{}", greeting);

    // Unicode: 日本語 🦀 café
    for c in greeting.chars() {
        print!("{}", c);
    }
}
```

**Scrolling Rules:**
- Horizontal scroll when line exceeds viewport
- Vertical scroll to keep cursor visible
- Scissor clip to editor rect

**Keyboard Bindings (in Editor focus):**
| Key | Action |
|-----|--------|
| Arrow keys | Move cursor |
| Shift+Arrow | Extend selection |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Ctrl+A | Select all |
| Ctrl+C | Copy (noop in demo) |
| PageUp/Down | Scroll viewport |
| Home/End | Line start/end |

### 3.4 Preview

**Purpose:** Visual demonstrations of rendering primitives.

**Content Modes (varies by selected Section):**

| Section | Preview Content |
|---------|----------------|
| Overview | Animated pixel art + alpha demo |
| Editor | Cursor position, selection stats |
| Preview | Full-size visualization |
| Logs | Log stream (alternative view) |
| Unicode | Grapheme table, width rulers |
| Performance | FPS graph, timing bars |

**Features Demonstrated:**
- `PixelBuffer` → ASCII art conversion
- Alpha blending with `set_blended()`
- `push_opacity()` layers
- Dynamic updates (animation loop)
- Chart rendering (bar graphs)

**Alpha Blending Demo:**
```
   ┌───────┐
   │ Blue  │
   │  ▓▓▓  │  <- Overlapping squares with
   │  ▓▓▓  │     50% opacity blending
   └───────┘
```

### 3.5 StatusBar

**Purpose:** Contextual hints and runtime stats.

| Element | Position | Content |
|---------|----------|---------|
| Key hints | left | "F1 Help  Ctrl+P Command  Ctrl+T Tour  Ctrl+Q Quit" |
| Stats | right | "FPS:XX  Cells:NNNN  Mem:NNkB" |

**Features Demonstrated:**
- Dynamic text updates (FPS counter)
- Style variations in single line

### 3.6 ToastArea

**Purpose:** Transient notifications.

**Behavior:**
- Appears in bottom-right corner
- Displays for 3 seconds, then fades
- Uses opacity stack for fade animation
- Multiple toasts stack vertically

**Features Demonstrated:**
- `push_opacity()` with animated value
- Overlay rendering (drawn after main content)
- Timer-based animations

---

## 4. Overlay System

Overlays are full-screen or partial-screen modal panels drawn on top of the main layout.

### 4.1 Common Overlay Properties

- **Glass effect:** Semi-transparent background (30% opacity black fill)
- **Centered panel:** Bordered rectangle with solid background
- **Focus capture:** All input goes to overlay until dismissed
- **Escape dismisses:** All overlays close on Escape key

**Implementation Pattern:**
```rust
fn draw_overlay(buffer: &mut OptimizedBuffer, overlay: &Overlay) {
    // 1. Glass background
    buffer.push_opacity(0.3);
    buffer.fill_rect(0, 0, width, height, Rgba::BLACK);
    buffer.pop_opacity();

    // 2. Centered panel
    let panel = center_rect(overlay.width, overlay.height, width, height);
    buffer.push_scissor(panel);
    buffer.fill_rect(panel.x, panel.y, panel.w, panel.h, Rgba::from_hex("#1a1a2e")?);
    buffer.draw_box(panel.x, panel.y, panel.w, panel.h, BoxStyle::rounded());

    // 3. Content
    overlay.draw_content(buffer, panel);

    buffer.pop_scissor();
}
```

### 4.2 Help Overlay (F1)

**Size:** 60 x 20 (centered)

**Content:**
```
┌──────────────────── Help ────────────────────┐
│                                              │
│  Navigation                                  │
│    Tab / Shift+Tab    Cycle focus            │
│    Arrow keys         Navigate within panel  │
│    Enter              Activate selection     │
│                                              │
│  Editor                                      │
│    Ctrl+Z / Ctrl+Y    Undo / Redo            │
│    Ctrl+A             Select all             │
│                                              │
│  Global                                      │
│    F1                 Toggle help            │
│    Ctrl+P             Command palette        │
│    Ctrl+T             Start/resume tour      │
│    Ctrl+Q             Quit                   │
│                                              │
│              Press Escape to close           │
└──────────────────────────────────────────────┘
```

### 4.3 Command Palette (Ctrl+P)

**Size:** 50 x 15 (centered, upper third)

**Features:**
- Text input at top
- Filtered list below
- Fuzzy matching (optional)
- Preview pane (optional)

**Content:**
```
┌─────────────── Command ───────────────┐
│ > _                                   │
├───────────────────────────────────────┤
│   Toggle Section: Overview            │
│   Toggle Section: Editor              │
│   Toggle Section: Preview             │
│   Toggle Word Wrap                    │
│   Reset Editor Content                │
│   Start Tour                          │
│   Quit                                │
└───────────────────────────────────────┘
```

### 4.4 Tour Overlay (Ctrl+T)

**Size:** 40 x 8 (positioned near feature being demonstrated)

**Behavior:**
- Steps through demo features
- Highlights relevant UI area
- Shows description + "Next" prompt
- Auto-advances on timer (optional)

**Content:**
```
┌──────────── Tour Step 3/7 ────────────┐
│                                       │
│  Alpha Blending                       │
│                                       │
│  Watch how overlapping colors blend   │
│  using Porter-Duff compositing.       │
│                                       │
│         [Space: Next]  [Esc: Exit]    │
└───────────────────────────────────────┘
```

---

## 5. Navigation Model

### 5.1 Focus System

```rust
enum Focus {
    Sidebar,
    Editor,
    Preview,
    Overlay(OverlayKind),
}
```

**Focus Rules:**
- Tab cycles: Sidebar → Editor → Preview → Sidebar
- Shift+Tab cycles reverse
- Opening overlay captures focus
- Closing overlay returns to previous focus
- Section selection doesn't change focus (just content)

### 5.2 Section vs Focus

| Concept | Controls | Effect |
|---------|----------|--------|
| **Section** | Sidebar selection | What content is shown in panels |
| **Focus** | Tab/Shift+Tab | Which panel receives keyboard input |

Example: You can be focused on Editor while Section is "Unicode" — the editor still shows code, but the Preview panel shows the unicode demonstration.

### 5.3 Mode Indicator

```rust
enum Mode {
    Normal,   // Default navigation
    Insert,   // Editor text input
    Command,  // Command palette active
}
```

Displayed in TopBar. Changes based on focus and overlay state.

---

## 6. Layout Rules

### 6.1 Clipping

Every panel is drawn within its scissor rectangle:

```rust
fn draw_panel(buffer: &mut OptimizedBuffer, rect: Rect, content: &dyn Panel) {
    buffer.push_scissor(ClipRect::from(rect));
    content.draw(buffer, rect);
    buffer.pop_scissor();
}
```

**Nesting:** Scissors can nest (overlay within panel). The effective clip is the intersection.

### 6.2 Scrolling

**Vertical Scroll:**
- Track `scroll_y: usize` (line offset)
- Render lines `scroll_y..scroll_y + viewport_lines`
- Clamp: `scroll_y <= max(0, total_lines - viewport_lines)`

**Horizontal Scroll:**
- Track `scroll_x: usize` (column offset)
- Render columns `scroll_x..scroll_x + viewport_cols`
- Applied per-line in editor

**Auto-scroll to Cursor:**
```rust
fn ensure_cursor_visible(&mut self) {
    if self.cursor_y < self.scroll_y {
        self.scroll_y = self.cursor_y;
    } else if self.cursor_y >= self.scroll_y + self.viewport_height {
        self.scroll_y = self.cursor_y - self.viewport_height + 1;
    }
    // Similar for horizontal
}
```

### 6.3 Compact Mode (< 80 cols)

When terminal width < 80 columns:

1. Hide Preview panel (merge into Editor area)
2. Collapse Sidebar to icons only (3 cols)
3. Reduce StatusBar to essentials

**Detection:**
```rust
fn is_compact(width: usize) -> bool {
    width < 80
}
```

### 6.4 Z-Order

Draw order (back to front):
1. Background fill
2. TopBar
3. Sidebar
4. Editor
5. Preview
6. StatusBar
7. ToastArea
8. Overlay (if active)

---

## 7. Feature Coverage Matrix

| OpenTUI Feature | Panel/Component | Notes |
|----------------|-----------------|-------|
| `Rgba` colors | All | Consistent palette |
| `Style` attributes | All | Bold, dim, underline |
| `Cell` / `CellContent` | All | Character rendering |
| `GraphemePool` | Editor, Unicode section | Multi-codepoint clusters |
| `OptimizedBuffer.draw_text()` | All | Primary text rendering |
| `OptimizedBuffer.draw_box()` | Overlays, panels | Borders |
| `OptimizedBuffer.fill_rect()` | All | Backgrounds |
| `push_scissor()` / `pop_scissor()` | All panels, overlays | Clipping |
| `push_opacity()` / `pop_opacity()` | Overlays, ToastArea | Glass effect, fades |
| `set_blended()` | Preview alpha demo | Porter-Duff blending |
| `PixelBuffer` → ASCII | Preview | Image rendering |
| `TextBuffer` | Editor | Rope storage |
| `EditBuffer` | Editor | Cursor, selection, undo |
| `EditorView` | Editor | Full editor widget |
| `HighlightedBuffer` | Editor | Syntax highlighting |
| `LinkPool` | Logs panel | OSC 8 hyperlinks |
| `Renderer.present()` | Main loop | Diff-based output |
| `Renderer.hit_test()` | Mouse support | Click detection |
| Input parsing | All | Keyboard/mouse events |

---

## 8. Acceptance Criteria Checklist

- [x] **Baseline layout defined** — Section 2 defines TopBar, Sidebar, Editor, Preview, StatusBar, ToastArea
- [x] **Overlays defined** — Section 4 defines Help, Command Palette, Tour overlays
- [x] **Each region has explicit responsibilities + feature coverage** — Section 3 details each panel, Section 7 provides coverage matrix
- [x] **Layout rules for clipping/scrolling are specified** — Section 6 defines scissor usage, scroll behavior, compact mode, z-order

---

## 9. Related Beads

| Bead | Dependency | Description |
|------|------------|-------------|
| bd-2mu | This bead | App concept + panel layout wireframe |
| bd-1ok | Blocked by this | Visual design system (palette, typography) |
| bd-3l0 | Blocked by this | Keybindings + interaction model |
| bd-1ei | Blocked by this | Content pack (sample code, logs, etc.) |
| bd-z1b | Independent | Bin target + skeleton main loop |
| bd-35g | Blocked by this | App state machine |

---

## Appendix A: Color Palette (Preview)

To be defined in bd-1ok (visual design system). Placeholder:

```rust
mod colors {
    pub const BG_PRIMARY: Rgba = Rgba::from_hex("#1a1a2e").unwrap();
    pub const BG_SECONDARY: Rgba = Rgba::from_hex("#16213e").unwrap();
    pub const FG_PRIMARY: Rgba = Rgba::from_hex("#eaeaea").unwrap();
    pub const FG_DIM: Rgba = Rgba::from_hex("#888888").unwrap();
    pub const ACCENT: Rgba = Rgba::from_hex("#e94560").unwrap();
    pub const SUCCESS: Rgba = Rgba::from_hex("#4ecca3").unwrap();
}
```

---

## Appendix B: Sample ASCII Wireframes

### B.1 Wide Layout (120 cols)
```
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│  OpenTUI Showcase                    Project: opentui_rust                              Mode: Normal        22:05   │
├───────────────┬────────────────────────────────────────────────────────────────────────────────────┬───────────────┤
│               │                                                                                    │               │
│  • Overview   │  1 │ //! OpenTUI demo source                                                       │   ███░░░███   │
│  • Editor     │  2 │                                                                               │   █▓▓░░░▓▓█   │
│  • Preview    │  3 │ fn main() {                                                                   │   ░░░▓▓▓░░░   │
│  • Logs       │  4 │     let greeting = "Hello, World!";                                           │               │
│  • Unicode    │  5 │     println!("{}", greeting);                                                 │  Alpha: 50%   │
│  • Perf       │  6 │                                                                               │  Blend: Over  │
│               │  7 │     // Unicode: 日本語 🦀 café                                                   │               │
│               │  8 │     for c in greeting.chars() {                                               │  FPS ████░░   │
│               │  9 │         print!("{}", c);                                                      │       60/60   │
│               │ 10 │     }                                                                         │               │
│               │ 11 │ }                                                                             │               │
├───────────────┴────────────────────────────────────────────────────────────────────────────────────┴───────────────┤
│  F1 Help  Ctrl+P Command  Ctrl+T Tour  Ctrl+Q Quit                                   FPS:60  Cells:8640  Mem:128kB  │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### B.2 Compact Layout (60 cols)
```
┌────────────────────────────────────────────────────────────┐
│  OpenTUI            Mode: Normal               22:05       │
├──────┬─────────────────────────────────────────────────────┤
│ [O]  │  1 │ //! OpenTUI demo source                        │
│ [E]  │  2 │                                                │
│ [P]  │  3 │ fn main() {                                    │
│ [L]  │  4 │     let greeting = "Hello";                    │
│ [U]  │  5 │     println!("{}", greeting);                  │
│ [F]  │  6 │ }                                              │
├──────┴─────────────────────────────────────────────────────┤
│  F1 Help  Ctrl+Q Quit                          FPS:60      │
└────────────────────────────────────────────────────────────┘
```
