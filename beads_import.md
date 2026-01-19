# Beads Import for OpenTUI Rust

<!-- This file will be imported using: br create -f beads_import.md -->

---

## ANSI Input Sequence Parser

**type:** feature
**priority:** P0
**labels:** input,parser,core
**estimate:** 480

### Overview
Implement a parser for ANSI escape sequences received from the terminal. This is the foundation for all input handling - without this, no interactive TUI applications can be built.

### Scope
Parse raw bytes from stdin into structured input events:
- CSI sequences (cursor keys, function keys, modifiers)
- SS3 sequences (alternate cursor key format)
- OSC sequences (for bracketed paste, focus events)
- UTF-8 character sequences
- Partial sequence handling (need more data)

### Implementation Details

#### Parser State Machine
```rust
pub enum ParseState {
    Ground,           // Normal character input
    Escape,           // Received ESC (0x1B)
    CsiEntry,         // Received ESC [
    CsiParam,         // Collecting CSI parameters
    CsiIntermediate,  // Collecting intermediate bytes
    SsThree,          // Received ESC O
    OscString,        // Collecting OSC payload
}

pub enum ParseResult {
    Event(InputEvent),
    NeedMoreData,
    Invalid(Vec<u8>),
}
```

#### Input Event Types
```rust
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    FocusGained,
    FocusLost,
    Unknown(Vec<u8>),
}

pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: Modifiers,
}

pub enum KeyCode {
    Char(char),
    Enter, Tab, Backspace, Escape,
    Left, Right, Up, Down,
    Home, End, PageUp, PageDown,
    Insert, Delete,
    F(u8),  // F1-F12
}

bitflags! {
    pub struct Modifiers: u8 {
        const SHIFT = 0x01;
        const ALT = 0x02;
        const CTRL = 0x04;
        const SUPER = 0x08;
    }
}
```

#### Key Sequence Mappings (must support all)
- Arrow keys: `ESC[A` (Up), `ESC[B` (Down), `ESC[C` (Right), `ESC[D` (Left)
- With modifiers: `ESC[1;5A` (Ctrl+Up), `ESC[1;2A` (Shift+Up)
- Application mode: `ESC O A` (Up), `ESC O B` (Down)
- Function keys: `ESC[11~` (F1), `ESC[15~` (F5), `ESC[24~` (F12)
- Special: `ESC[2~` (Insert), `ESC[3~` (Delete), `ESC[5~` (PageUp), `ESC[6~` (PageDown)
- Home/End: `ESC[H`, `ESC[F` or `ESC[1~`, `ESC[4~`

### Files to Create/Modify
- `src/input/mod.rs` - module exports
- `src/input/parser.rs` - state machine parser (~300 lines)
- `src/input/events.rs` - event types (~100 lines)
- `src/input/keys.rs` - key code definitions (~150 lines)
- `src/lib.rs` - export input module

### Testing Requirements

#### Unit Tests (minimum 15 tests in src/input/parser.rs)
```rust
#[test] fn test_parse_simple_ascii_char()
#[test] fn test_parse_utf8_2byte_char()
#[test] fn test_parse_utf8_3byte_char()
#[test] fn test_parse_utf8_4byte_emoji()
#[test] fn test_parse_arrow_up()
#[test] fn test_parse_arrow_down()
#[test] fn test_parse_arrow_with_shift()
#[test] fn test_parse_arrow_with_ctrl()
#[test] fn test_parse_arrow_with_alt()
#[test] fn test_parse_f1_through_f12()
#[test] fn test_parse_home_end_variants()
#[test] fn test_parse_page_up_down()
#[test] fn test_parse_insert_delete()
#[test] fn test_parse_application_mode_arrows()
#[test] fn test_parse_incomplete_csi_returns_need_more()
#[test] fn test_parse_invalid_sequence_returns_unknown()
#[test] fn test_parse_multiple_events_in_buffer()
#[test] fn test_parse_escape_key_with_timeout()
```

#### E2E Test Script (tests/e2e/input_parser.sh)
```bash
#!/bin/bash
# Feed recorded terminal sequences and verify parsing
# Log all inputs and outputs for debugging
```

### Acceptance Criteria
- [ ] Parser handles all xterm-compatible key sequences
- [ ] UTF-8 characters parse correctly (including 4-byte emoji)
- [ ] Partial sequences return NeedMoreData (not error)
- [ ] Invalid sequences return Unknown (not panic)
- [ ] Performance: parse 100K events/sec
- [ ] All 18+ unit tests pass
- [ ] E2E test with recorded sequences passes
- [ ] No clippy warnings

---

## Mouse Input Parser

**type:** feature
**priority:** P0
**labels:** input,mouse,core
**depends:** ANSI Input Sequence Parser

### Overview
Extend the input parser to handle SGR extended mouse sequences. This enables mouse-driven TUI applications.

### Scope
Parse mouse sequences into structured events:
- SGR extended format: `ESC[<button;x;y M/m`
- Button press, release, motion, wheel events
- Coordinate extraction (1-indexed from terminal)
- Modifier detection (Ctrl+click, etc.)

### Implementation Details

#### Mouse Event Types
```rust
pub struct MouseInputEvent {
    pub kind: MouseEventKind,
    pub button: MouseButton,
    pub x: u16,
    pub y: u16,
    pub modifiers: Modifiers,
}

pub enum MouseEventKind {
    Press,
    Release,
    Move,
    ScrollUp,
    ScrollDown,
}

pub enum MouseButton {
    Left,
    Middle,
    Right,
    None,  // For motion without button
}
```

#### SGR Mouse Format
```
ESC [ < Cb ; Cx ; Cy M   (button press)
ESC [ < Cb ; Cx ; Cy m   (button release)

Cb bits:
  0-1: button (0=left, 1=middle, 2=right, 3=release)
  2:   shift
  3:   meta/alt
  4:   control
  5:   motion
  6-7: wheel (64=up, 65=down)
```

### Files to Create/Modify
- `src/input/mouse.rs` - mouse event types and parsing (~150 lines)
- `src/input/parser.rs` - integrate mouse parsing into state machine
- `src/input/events.rs` - add MouseInputEvent to InputEvent enum

### Testing Requirements

#### Unit Tests (minimum 12 tests)
```rust
#[test] fn test_parse_mouse_left_press()
#[test] fn test_parse_mouse_left_release()
#[test] fn test_parse_mouse_right_click()
#[test] fn test_parse_mouse_middle_click()
#[test] fn test_parse_mouse_motion()
#[test] fn test_parse_mouse_scroll_up()
#[test] fn test_parse_mouse_scroll_down()
#[test] fn test_parse_mouse_with_shift()
#[test] fn test_parse_mouse_with_ctrl()
#[test] fn test_parse_mouse_coordinates_large()
#[test] fn test_parse_mouse_at_origin()
#[test] fn test_parse_incomplete_mouse_sequence()
```

#### E2E Test (tests/e2e/mouse_input.rs)
Interactive test that logs mouse events when running in terminal.

### Acceptance Criteria
- [ ] All mouse button types detected correctly
- [ ] Press vs release distinguished (M vs m)
- [ ] Coordinates parsed correctly (convert from 1-indexed)
- [ ] Modifiers detected (Ctrl+click, Shift+click)
- [ ] Scroll wheel events parsed
- [ ] Motion events with/without button
- [ ] All 12+ unit tests pass
- [ ] Manual testing with real terminal mouse

---

## Event System

**type:** feature
**priority:** P0
**labels:** events,core,architecture

### Overview
Implement a global event bus for dispatching terminal events and internal notifications. Required for clean input handling architecture.

### Scope
- Event callback registration
- Event dispatching with type filtering
- Logger callback for debug output
- Thread-safe event handling

### Implementation Details

#### Event Bus
```rust
pub struct EventBus {
    handlers: Vec<Box<dyn Fn(&Event) + Send + Sync>>,
    log_callback: Option<Box<dyn Fn(LogLevel, &str) + Send + Sync>>,
}

pub enum Event {
    Input(InputEvent),
    TerminalResponse(TerminalResponse),
    Resize { width: u16, height: u16 },
    Custom(Box<dyn Any + Send>),
}

pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl EventBus {
    pub fn new() -> Self;
    pub fn subscribe<F>(&mut self, handler: F) where F: Fn(&Event) + Send + Sync + 'static;
    pub fn emit(&self, event: Event);
    pub fn set_log_callback<F>(&mut self, callback: F) where F: Fn(LogLevel, &str) + Send + Sync + 'static;
    pub fn log(&self, level: LogLevel, msg: &str);
}
```

### Files to Create/Modify
- `src/event/mod.rs` - event types and bus (~200 lines)
- `src/event/logger.rs` - logging utilities (~50 lines)
- `src/lib.rs` - export event module

### Testing Requirements

#### Unit Tests (minimum 8 tests)
```rust
#[test] fn test_event_bus_subscribe_and_emit()
#[test] fn test_event_bus_multiple_handlers()
#[test] fn test_event_bus_handler_receives_correct_event()
#[test] fn test_log_callback_receives_messages()
#[test] fn test_log_levels_filter_correctly()
#[test] fn test_event_bus_is_send_sync()
#[test] fn test_custom_event_dispatch()
#[test] fn test_event_bus_empty_handlers_no_panic()
```

### Acceptance Criteria
- [ ] Event handlers can be registered
- [ ] Events dispatch to all registered handlers
- [ ] Log callback works with all levels
- [ ] Thread-safe (Send + Sync)
- [ ] All 8+ unit tests pass

---

## TextBufferView Highlight Rendering

**type:** bug
**priority:** P0
**labels:** rendering,text,bugfix

### Overview
CRITICAL BUG: The TextBufferView.render_line() method ignores all highlights/styled segments in the TextBuffer. This means syntax highlighting, selections, and all styling is completely broken.

### Current Broken Code
```rust
// In src/text/view.rs render_line():
let style = self.buffer.default_style();  // WRONG! Always uses default
let cell = Cell::from_grapheme(grapheme, style);
```

### Required Fix
```rust
// Must query buffer for style at each position
let byte_pos = /* calculate byte position */;
let style = self.buffer.style_at(byte_pos);
let cell = Cell::from_grapheme(grapheme, style);
```

### Scope
- Track byte position while iterating graphemes
- Query TextBuffer.style_at() for each character
- Apply selection style if position is within selection
- Handle priority ordering (selection > highlight > default)

### Implementation Details
```rust
fn render_line(&self, output: &mut OptimizedBuffer, dest_x: i32, dest_y: u32, line: &str, line_idx: usize) {
    let line_start_byte = self.buffer.rope().line_to_byte(line_idx);
    let mut byte_offset = 0;
    let mut col = 0u32;

    for grapheme in line.graphemes(true) {
        let abs_byte_pos = line_start_byte + byte_offset;

        // Get base style from segments
        let mut style = self.buffer.style_at(abs_byte_pos);

        // Apply selection if active
        if let Some(sel) = &self.selection {
            let char_pos = self.buffer.rope().byte_to_char(abs_byte_pos);
            if sel.contains(char_pos) {
                style = style.merge(sel.style);
            }
        }

        let cell = Cell::from_grapheme(grapheme, style);
        // ... rest of rendering

        byte_offset += grapheme.len();
    }
}
```

### Files to Modify
- `src/text/view.rs` - fix render_line() method
- `src/text/buffer.rs` - ensure style_at() handles edge cases

### Testing Requirements

#### Unit Tests (minimum 6 tests)
```rust
#[test] fn test_render_applies_highlight_style()
#[test] fn test_render_applies_selection_style()
#[test] fn test_render_selection_overrides_highlight()
#[test] fn test_render_multiple_highlights_priority()
#[test] fn test_render_partial_line_highlight()
#[test] fn test_render_utf8_with_highlights()
```

#### Visual E2E Test (tests/e2e/highlight_rendering.rs)
Render text with highlights to buffer, dump to file, verify ANSI codes present.

### Acceptance Criteria
- [ ] Highlights from add_highlight() render with correct style
- [ ] Selection renders with selection style
- [ ] Selection style takes priority over highlights
- [ ] Multiple overlapping highlights use priority ordering
- [ ] UTF-8/grapheme boundaries don't break highlighting
- [ ] All 6+ unit tests pass
- [ ] Visual verification in terminal shows colored text

---

## TextBufferView Line Info Cache

**type:** feature
**priority:** P1
**labels:** text,performance,caching

### Overview
Implement line information caching for efficient rendering of wrapped text. Without this, wrapping is O(n) on every render.

### Scope
Cache precomputed line information:
- `starts[]` - byte offset where each virtual line starts
- `widths[]` - display width of each virtual line
- `sources[]` - source (logical) line index for each virtual line
- `wraps[]` - boolean: is this line a continuation?
- `max_width` - maximum line width

### Implementation Details

#### LineInfo Structure
```rust
#[derive(Clone, Debug)]
pub struct LineInfo {
    pub byte_start: usize,
    pub byte_end: usize,
    pub width: usize,
    pub source_line: usize,
    pub is_continuation: bool,
}

pub struct LineCache {
    lines: Vec<LineInfo>,
    max_width: usize,
    wrap_mode: WrapMode,
    wrap_width: u32,
    content_hash: u64,  // Invalidation check
}

impl LineCache {
    pub fn compute(buffer: &TextBuffer, wrap_mode: WrapMode, wrap_width: u32) -> Self;
    pub fn is_valid(&self, buffer: &TextBuffer, wrap_mode: WrapMode, wrap_width: u32) -> bool;
    pub fn virtual_line_count(&self) -> usize;
    pub fn get_line(&self, idx: usize) -> Option<&LineInfo>;
    pub fn source_to_virtual(&self, source_line: usize) -> usize;
    pub fn virtual_to_source(&self, virtual_line: usize) -> usize;
}
```

#### Word Wrap Algorithm
```rust
fn compute_wrapped_lines(line: &str, wrap_width: u32, wrap_mode: WrapMode) -> Vec<(usize, usize, usize)> {
    match wrap_mode {
        WrapMode::None => vec![(0, line.len(), display_width(line))],
        WrapMode::Char => wrap_at_chars(line, wrap_width),
        WrapMode::Word => wrap_at_words(line, wrap_width),
    }
}

fn wrap_at_words(line: &str, width: u32) -> Vec<(usize, usize, usize)> {
    // Break at whitespace when possible
    // Fall back to char break if word > width
}
```

### Files to Create/Modify
- `src/text/line_cache.rs` - new file (~250 lines)
- `src/text/view.rs` - integrate cache, update virtual_line_count()
- `src/text/mod.rs` - export LineCache

### Testing Requirements

#### Unit Tests (minimum 10 tests)
```rust
#[test] fn test_line_cache_no_wrap()
#[test] fn test_line_cache_char_wrap_exact()
#[test] fn test_line_cache_char_wrap_overflow()
#[test] fn test_line_cache_word_wrap_simple()
#[test] fn test_line_cache_word_wrap_long_word()
#[test] fn test_line_cache_multiple_lines()
#[test] fn test_line_cache_empty_lines()
#[test] fn test_line_cache_utf8_width()
#[test] fn test_line_cache_invalidation()
#[test] fn test_source_to_virtual_mapping()
#[test] fn test_virtual_to_source_mapping()
```

### Acceptance Criteria
- [ ] Cache computes correct line info for all wrap modes
- [ ] Word wrap breaks at spaces when possible
- [ ] Wide characters (CJK) don't break mid-character
- [ ] Cache invalidates when content changes
- [ ] source_to_virtual and virtual_to_source mappings correct
- [ ] All 10+ unit tests pass
- [ ] Performance: 10K lines cached in <10ms

---

## TextBufferView measureForDimensions

**type:** feature
**priority:** P1
**labels:** text,layout
**depends:** TextBufferView Line Info Cache

### Overview
Implement measureForDimensions() to calculate required viewport size for text content.

### Scope
```rust
impl TextBufferView {
    /// Calculate the dimensions needed to display all content.
    /// Returns (virtual_line_count, max_line_width)
    pub fn measure_for_dimensions(&self, wrap_width: Option<u32>) -> (usize, usize) {
        // Use line cache if available
        // Otherwise compute on the fly
    }
}
```

### Files to Modify
- `src/text/view.rs` - add measure_for_dimensions()

### Testing Requirements

#### Unit Tests (minimum 4 tests)
```rust
#[test] fn test_measure_no_wrap()
#[test] fn test_measure_with_wrap()
#[test] fn test_measure_empty_buffer()
#[test] fn test_measure_single_long_line()
```

### Acceptance Criteria
- [ ] Returns correct line count for wrapped/unwrapped
- [ ] Returns correct max width
- [ ] Works with empty buffer
- [ ] All 4+ unit tests pass

---

## EditBuffer Word Boundary Navigation

**type:** feature
**priority:** P1
**labels:** editing,navigation

### Overview
Implement word boundary navigation for Ctrl+Left/Right and word deletion.

### Scope
```rust
impl EditBuffer {
    /// Find the next word boundary from cursor position.
    /// Returns the character offset of the boundary.
    pub fn next_word_boundary(&self) -> usize;

    /// Find the previous word boundary from cursor position.
    pub fn prev_word_boundary(&self) -> usize;

    /// Move cursor to next word.
    pub fn move_word_right(&mut self);

    /// Move cursor to previous word.
    pub fn move_word_left(&mut self);

    /// Delete from cursor to next word boundary.
    pub fn delete_word_forward(&mut self);

    /// Delete from previous word boundary to cursor.
    pub fn delete_word_backward(&mut self);
}
```

### Word Boundary Rules
A word boundary is:
- Transition from alphanumeric to non-alphanumeric (or vice versa)
- Transition from lowercase to uppercase (camelCase)
- Start/end of line
- Unicode word boundaries (use unicode-segmentation)

### Files to Modify
- `src/text/edit.rs` - add word navigation methods

### Testing Requirements

#### Unit Tests (minimum 10 tests)
```rust
#[test] fn test_next_word_simple()
#[test] fn test_prev_word_simple()
#[test] fn test_word_boundary_punctuation()
#[test] fn test_word_boundary_camelcase()
#[test] fn test_word_boundary_at_line_end()
#[test] fn test_word_boundary_at_line_start()
#[test] fn test_move_word_right()
#[test] fn test_move_word_left()
#[test] fn test_delete_word_forward()
#[test] fn test_delete_word_backward()
```

### Acceptance Criteria
- [ ] next_word_boundary finds correct position
- [ ] prev_word_boundary finds correct position
- [ ] Handles punctuation as word separators
- [ ] Handles start/end of buffer
- [ ] delete_word integrates with undo
- [ ] All 10+ unit tests pass

---

## EditBuffer Line Operations

**type:** feature
**priority:** P1
**labels:** editing,navigation

### Overview
Add missing line-level operations to EditBuffer.

### Scope
```rust
impl EditBuffer {
    /// Delete the entire current line.
    pub fn delete_line(&mut self);

    /// Go to a specific line number (0-indexed).
    pub fn goto_line(&mut self, line: usize);

    /// Duplicate the current line.
    pub fn duplicate_line(&mut self);

    /// Move current line up.
    pub fn move_line_up(&mut self);

    /// Move current line down.
    pub fn move_line_down(&mut self);
}
```

### Files to Modify
- `src/text/edit.rs` - add line operations

### Testing Requirements

#### Unit Tests (minimum 8 tests)
```rust
#[test] fn test_delete_line_middle()
#[test] fn test_delete_line_first()
#[test] fn test_delete_line_last()
#[test] fn test_delete_line_only_line()
#[test] fn test_goto_line_valid()
#[test] fn test_goto_line_clamp()
#[test] fn test_duplicate_line()
#[test] fn test_move_line_up_down()
```

### Acceptance Criteria
- [ ] delete_line removes line and newline
- [ ] goto_line moves cursor correctly
- [ ] goto_line clamps to valid range
- [ ] Line operations integrate with undo
- [ ] All 8+ unit tests pass

---

## EditorView Visual Navigation

**type:** feature
**priority:** P1
**labels:** editor,navigation
**depends:** TextBufferView Line Info Cache, EditBuffer Word Boundary Navigation

### Overview
Implement visual cursor navigation that respects text wrapping.

### Scope
```rust
impl EditorView {
    /// Move cursor up in the visual (wrapped) view.
    pub fn move_up_visual(&mut self);

    /// Move cursor down in the visual (wrapped) view.
    pub fn move_down_visual(&mut self);

    /// Get the start of the current visual line.
    pub fn visual_line_start(&self) -> usize;

    /// Get the end of the current visual line.
    pub fn visual_line_end(&self) -> usize;

    /// Move to visual line start.
    pub fn move_to_visual_line_start(&mut self);

    /// Move to visual line end.
    pub fn move_to_visual_line_end(&mut self);
}
```

### Implementation Details
Visual navigation requires:
1. Getting current cursor position in visual coordinates
2. Using line cache to map between logical and visual lines
3. Moving within visual lines respecting column position

### Files to Modify
- `src/text/editor.rs` - add visual navigation methods

### Testing Requirements

#### Unit Tests (minimum 8 tests)
```rust
#[test] fn test_visual_move_up_no_wrap()
#[test] fn test_visual_move_up_with_wrap()
#[test] fn test_visual_move_down_no_wrap()
#[test] fn test_visual_move_down_with_wrap()
#[test] fn test_visual_line_start()
#[test] fn test_visual_line_end()
#[test] fn test_visual_nav_preserves_column()
#[test] fn test_visual_nav_at_buffer_boundary()
```

### Acceptance Criteria
- [ ] Up/down navigate visual lines correctly
- [ ] Column position preserved when possible
- [ ] Works correctly with wrapped text
- [ ] All 8+ unit tests pass

---

## EditorView Scroll Margins and Selection

**type:** feature
**priority:** P2
**labels:** editor,scroll,selection
**depends:** EditorView Visual Navigation

### Overview
Add scroll margins and selection-follows-cursor behavior.

### Scope
```rust
impl EditorView {
    /// Set scroll margin (0.0-0.5, portion of viewport to keep visible).
    pub fn set_scroll_margin(&mut self, margin: f32);

    /// Enable selection following cursor.
    pub fn set_selection_follow_cursor(&mut self, enabled: bool);

    /// Update selection to current cursor position.
    pub fn extend_selection_to_cursor(&mut self);

    /// Start a new selection at cursor.
    pub fn start_selection(&mut self);

    /// Clear selection.
    pub fn clear_selection(&mut self);
}
```

### Scroll Margin Behavior
- margin=0.0: cursor can be at edge
- margin=0.25: cursor kept 25% from edges
- Auto-scroll when cursor approaches margin

### Files to Modify
- `src/text/editor.rs` - add scroll margin and selection methods

### Testing Requirements

#### Unit Tests (minimum 6 tests)
```rust
#[test] fn test_scroll_margin_keeps_cursor_visible()
#[test] fn test_scroll_margin_top_edge()
#[test] fn test_scroll_margin_bottom_edge()
#[test] fn test_selection_follow_cursor()
#[test] fn test_extend_selection()
#[test] fn test_start_clear_selection()
```

### Acceptance Criteria
- [ ] Scroll margin prevents cursor at edge
- [ ] Selection extends correctly with cursor movement
- [ ] All 6+ unit tests pass

---

## Terminal Capability Queries

**type:** feature
**priority:** P2
**labels:** terminal,capabilities

### Overview
Send actual capability queries to the terminal instead of just checking environment variables.

### Scope
Query sequences to send:
- DA1 (Primary Device Attributes): `ESC[c`
- XTVERSION: `ESC[>0q`
- DA2 (Secondary Device Attributes): `ESC[>c`

Parse responses to detect:
- Terminal type and version
- True color support
- Kitty keyboard protocol
- Synchronized output support

### Implementation Details
```rust
impl Terminal {
    /// Send capability queries and collect responses.
    pub fn query_capabilities(&mut self) -> io::Result<()>;

    /// Parse a terminal response sequence.
    fn parse_response(&mut self, response: &[u8]) -> Option<TerminalResponse>;
}

pub enum TerminalResponse {
    DeviceAttributes { params: Vec<u32> },
    XtVersion { version: String },
    Unknown(Vec<u8>),
}
```

### Files to Modify
- `src/terminal/mod.rs` - add query methods
- `src/terminal/capabilities.rs` - parse responses

### Testing Requirements

#### Unit Tests (minimum 6 tests)
```rust
#[test] fn test_parse_da1_response()
#[test] fn test_parse_xtversion_response()
#[test] fn test_parse_unknown_response()
#[test] fn test_query_sequences_correct()
#[test] fn test_capabilities_from_da1()
#[test] fn test_timeout_handling()
```

### Acceptance Criteria
- [ ] Queries sent in correct format
- [ ] Responses parsed correctly
- [ ] Capabilities updated from responses
- [ ] Timeout prevents hanging
- [ ] All 6+ unit tests pass

---

## ANSI Cursor Save/Restore

**type:** feature
**priority:** P3
**labels:** ansi,terminal

### Overview
Add cursor save and restore sequences.

### Scope
```rust
// In src/ansi/sequences.rs
pub const CURSOR_SAVE: &str = "\x1b[s";
pub const CURSOR_RESTORE: &str = "\x1b[u";

// Alternative DEC sequences
pub const CURSOR_SAVE_DEC: &str = "\x1b7";
pub const CURSOR_RESTORE_DEC: &str = "\x1b8";
```

```rust
impl Terminal {
    pub fn save_cursor(&mut self) -> io::Result<()>;
    pub fn restore_cursor(&mut self) -> io::Result<()>;
}
```

### Files to Modify
- `src/ansi/sequences.rs` - add constants
- `src/terminal/mod.rs` - add methods

### Testing Requirements

#### Unit Tests
```rust
#[test] fn test_cursor_save_sequence()
#[test] fn test_cursor_restore_sequence()
```

### Acceptance Criteria
- [ ] Sequences are correct
- [ ] Terminal methods work
- [ ] Unit tests pass

---

## ANSI Cursor Color

**type:** feature
**priority:** P3
**labels:** ansi,terminal

### Overview
Add OSC 12 cursor color sequence.

### Scope
```rust
// In src/ansi/sequences.rs
pub fn cursor_color(r: u8, g: u8, b: u8) -> String {
    format!("\x1b]12;#{:02x}{:02x}{:02x}\x07", r, g, b)
}

pub const CURSOR_COLOR_RESET: &str = "\x1b]112\x07";
```

```rust
impl Terminal {
    pub fn set_cursor_color(&mut self, color: Rgba) -> io::Result<()>;
    pub fn reset_cursor_color(&mut self) -> io::Result<()>;
}
```

### Files to Modify
- `src/ansi/sequences.rs` - add cursor color function
- `src/terminal/mod.rs` - add methods

### Testing Requirements

#### Unit Tests
```rust
#[test] fn test_cursor_color_sequence()
#[test] fn test_cursor_color_reset()
```

### Acceptance Criteria
- [ ] Sequence format is correct
- [ ] Color conversion works
- [ ] Unit tests pass

---

## E2E Test Harness

**type:** feature
**priority:** P1
**labels:** testing,e2e
**depends:** ANSI Input Sequence Parser, Mouse Input Parser

### Overview
Create a comprehensive E2E test harness for verifying the entire input/output pipeline.

### Scope
- Test runner that can inject input sequences
- Output capture and verification
- Logging framework for debugging
- CI-compatible (no real terminal needed)

### Implementation Details

#### Test Harness
```rust
// tests/e2e/harness.rs
pub struct TestHarness {
    input_buffer: Vec<u8>,
    output_buffer: Vec<u8>,
    renderer: Renderer,
    log: Vec<String>,
}

impl TestHarness {
    pub fn new(width: u32, height: u32) -> Self;

    /// Inject input bytes as if from terminal
    pub fn inject_input(&mut self, bytes: &[u8]);

    /// Inject a key press
    pub fn press_key(&mut self, key: KeyCode, modifiers: Modifiers);

    /// Get captured output
    pub fn output(&self) -> &[u8];

    /// Get rendered buffer state
    pub fn buffer(&self) -> &OptimizedBuffer;

    /// Log message
    pub fn log(&mut self, msg: &str);

    /// Dump logs for debugging
    pub fn dump_logs(&self) -> &[String];
}
```

#### Test Scripts
```bash
# tests/e2e/run_all.sh
#!/bin/bash
set -e
echo "Running E2E tests..."

# Test 1: Input parsing
cargo test --test input_e2e -- --nocapture 2>&1 | tee test_input.log

# Test 2: Rendering pipeline
cargo test --test render_e2e -- --nocapture 2>&1 | tee test_render.log

# Test 3: Editor operations
cargo test --test editor_e2e -- --nocapture 2>&1 | tee test_editor.log

echo "All E2E tests passed!"
```

### Files to Create
- `tests/e2e/mod.rs` - test module
- `tests/e2e/harness.rs` - test harness
- `tests/e2e/input_e2e.rs` - input tests
- `tests/e2e/render_e2e.rs` - render tests
- `tests/e2e/editor_e2e.rs` - editor tests
- `tests/e2e/run_all.sh` - test runner script

### Testing Requirements (meta!)

#### E2E Tests
```rust
// tests/e2e/input_e2e.rs
#[test] fn e2e_parse_arrow_sequence()
#[test] fn e2e_parse_mouse_click()
#[test] fn e2e_parse_utf8_input()

// tests/e2e/render_e2e.rs
#[test] fn e2e_render_simple_text()
#[test] fn e2e_render_with_highlights()
#[test] fn e2e_render_diff_detection()

// tests/e2e/editor_e2e.rs
#[test] fn e2e_editor_type_text()
#[test] fn e2e_editor_navigate()
#[test] fn e2e_editor_undo_redo()
```

### Acceptance Criteria
- [ ] Harness can inject arbitrary input
- [ ] Harness captures all output
- [ ] Logging is comprehensive
- [ ] Tests run in CI (no real terminal)
- [ ] All E2E tests pass
- [ ] Logs provide clear debugging info

---

## Integration Example

**type:** feature
**priority:** P2
**labels:** documentation,example
**depends:** ANSI Input Sequence Parser, Mouse Input Parser, Event System

### Overview
Create a working example demonstrating the full rendering loop with input handling.

### Scope
Simple text editor example showing:
- Terminal setup/cleanup
- Input event loop
- Rendering pipeline
- Proper error handling

### Implementation
```rust
// examples/simple_editor.rs
use opentui::prelude::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup
    let mut renderer = Renderer::new(80, 24)?;
    let mut buffer = EditBuffer::with_text("Hello, OpenTUI!");
    let mut view = EditorView::new(buffer);
    view.set_line_numbers(true);

    // Event loop
    let stdin = std::io::stdin();
    let mut parser = InputParser::new();

    loop {
        // Render
        renderer.clear();
        view.render_to(renderer.buffer(), 0, 0, 80, 24);
        renderer.present()?;

        // Handle input
        let mut buf = [0u8; 32];
        let n = stdin.read(&mut buf)?;

        for event in parser.parse(&buf[..n]) {
            match event {
                InputEvent::Key(KeyEvent { code: KeyCode::Char('q'), modifiers })
                    if modifiers.contains(Modifiers::CTRL) => {
                    return Ok(());
                }
                InputEvent::Key(key) => view.handle_key(key),
                InputEvent::Mouse(mouse) => view.handle_mouse(mouse),
                _ => {}
            }
        }
    }
}
```

### Files to Create
- `examples/simple_editor.rs` - basic editor example
- `examples/README.md` - example documentation

### Acceptance Criteria
- [ ] Example compiles and runs
- [ ] Demonstrates full input/output loop
- [ ] Handles Ctrl+Q to quit
- [ ] Properly cleans up terminal on exit
- [ ] README explains how to run
