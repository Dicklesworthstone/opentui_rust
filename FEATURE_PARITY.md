# Feature Parity Tracking - OpenTUI Rust

> Tracks implementation status of the Rust port against the Zig specification.

**Last Updated:** 2026-01-25
**Test Status:** 328/328 tests passing (248 unit + 72 conformance + 3 e2e + 5 benchmarks)
**Clippy Status:** 0 errors (format suggestions only)
**Estimated Completion:** ~97%

---

## Summary

| Category | Implemented | Total | Status |
|----------|-------------|-------|--------|
| Core Types | 4 | 4 | ✅ Complete |
| ANSI Sequences | 10 | 10 | ✅ Complete |
| Buffer | 8 | 8 | ✅ Complete |
| Text/Rope | 5 | 6 | ⚠️ Partial |
| Text Views | 8 | 8 | ✅ Complete |
| Editor | 8 | 8 | ✅ Complete |
| Renderer | 8 | 9 | ⚠️ Partial |
| Terminal | 6 | 6 | ✅ Complete |
| Event/Input | 4 | 4 | ✅ Complete |
| **Total** | **61** | **63** | **~97%** |

---

## 1. Core Types ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| RGBA Color | ✅ | f32 components, alpha blending, HSV conversion |
| Text Attributes | ✅ | bitflags u32: flags in bits 0-7, link ID in bits 8-31 |
| Cell | ✅ | CellContent enum: Char, Grapheme (Arc<str>), Empty, Continuation |
| Style | ✅ | fg, bg, attributes (packed link ID) with builder pattern |

---

## 2. ANSI Escape Sequences ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Cursor Movement | ✅ | Absolute and relative moves |
| True Color (SGR) | ✅ | 24-bit foreground/background |
| 256-Color Mode | ✅ | ColorMode::Color256 with palette conversion |
| 16-Color Mode | ✅ | ColorMode::Color16 for basic terminals |
| Text Attributes | ✅ | All 8 attributes |
| Cursor Styles | ✅ | Block, underline, bar with blink |
| Screen Management | ✅ | Alt screen, clear, home |
| Mouse Support | ✅ | Tracking mode sequences |
| Hyperlinks (OSC 8) | ✅ | Start/end link sequences |
| Synchronized Output | ✅ | Begin/end sync |

---

## 3. Buffer (OptimizedBuffer) ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Cell Storage | ✅ | Vec<Cell> with width × height |
| set/get Operations | ✅ | With bounds checking |
| Clear | ✅ | Fill with background color |
| Fill Rectangle | ✅ | Area fill |
| Text Drawing | ✅ | UTF-8 with wide char support |
| Box Drawing | ✅ | ASCII, light, heavy, double, rounded styles |
| Scissor Stack | ✅ | Clipping rectangles with intersection |
| Opacity Stack | ✅ | Alpha multiplier stack |

---

## 4. Text / Rope ⚠️ Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Rope Wrapper | ✅ | Using `ropey` crate |
| TextBuffer | ✅ | Styled text storage with segments |
| Highlighting | ✅ | Priority-based, ref ID for batch removal |
| Memory Registry | ✅ | For external text sources |
| **Grapheme Pool** | ⚠️ | Uses Arc<str> instead of 24-bit ID pool (intentional simplification) |
| Line Iterators | ✅ | Iterator over rope lines |

---

## 5. Text Buffer View ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Viewport | ✅ | x, y, width, height |
| Wrap Mode | ✅ | None, Char, Word (enum exists) |
| Selection (offset) | ✅ | Start/end with style |
| Local Selection | ✅ | Anchor/focus viewport coords |
| Scroll Position | ✅ | scroll_x, scroll_y |
| Virtual Line Count | ✅ | Accurate for wrap modes |
| Line Info Cache | ✅ | starts, widths, sources, wraps, max_width |
| Render with Highlights | ✅ | style_at() applies segment highlights with priority |
| measureForDimensions() | ✅ | Returns line count + max width |

---

## 6. Edit Buffer ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Cursor Position | ✅ | row, col, offset |
| Basic Movement | ✅ | left, right, up, down, line start/end |
| Insert/Delete | ✅ | At cursor position |
| Undo/Redo | ✅ | With configurable depth limit |
| Commit Groups | ✅ | Group operations for undo |
| Word Boundaries | ✅ | get_next/prev_word_boundary, move_word_left/right, delete_word_forward/backward |
| deleteLine() | ✅ | delete_line() removes current line |
| gotoLine() | ✅ | goto_line(n) moves cursor to line n |

---

## 7. Editor View ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Edit Buffer Wrapper | ✅ | With cursor and selection styles |
| Line Numbers | ✅ | Optional gutter with styling, dynamic width |
| Scroll to Cursor | ✅ | Keep cursor visible |
| Render to Buffer | ✅ | Basic rendering |
| Visual Cursor Nav | ✅ | move_up_visual(), move_down_visual() for wrapped text |
| Visual Line Bounds | ✅ | get_visual_sol(), get_visual_eol(), move_to_visual_sol/eol() |
| Scroll Margins | ✅ | Configurable via set_scroll_margin() |
| Selection Follow Cursor | ✅ | set_selection_follow_cursor() auto-updates selection |

---

## 8. Renderer ⚠️ Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Double Buffering | ✅ | Front/back buffer swap |
| Diff Detection | ✅ | Only redraw changed cells |
| Hit Grid | ✅ | Mouse event dispatch |
| Hit Scissor Stack | ✅ | Clipped hit areas |
| Link Pool | ✅ | Hyperlink URL storage |
| Render Stats | ✅ | FPS, frame time, cells updated |
| Debug Overlay | ✅ | Optional stats display |
| Integration Example | ✅ | examples/editor.rs - Full rendering loop demo |
| **Threaded Rendering** | ❌ | No render thread support |
| Memory Stats | ✅ | Estimated buffer + hit grid bytes |

---

## 9. Terminal ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Terminal State | ✅ | Writer, cursor, alt screen, mouse |
| Capabilities (env) | ✅ | Detect from TERM, COLORTERM, etc. |
| Cursor Control | ✅ | Show, hide, style, position |
| Raw Mode | ✅ | termios-based with RawModeGuard RAII |
| Terminal Size | ✅ | TIOCGWINSZ ioctl |
| Capability Queries | ✅ | DA1/XTVERSION/pixel/kitty queries sent |

---

## 10. Event System & Input ✅ Complete

| Feature | Status | Notes |
|---------|--------|-------|
| Event Callback | ✅ | Global event dispatcher with set_event_callback |
| Logger Callback | ✅ | Log levels with set_log_callback |
| Keyboard Input Parsing | ✅ | Full ANSI sequence parser (arrows, F-keys, modifiers) |
| Mouse Input Parsing | ✅ | SGR and X10 mouse protocol support |
| Focus Events | ✅ | Terminal focus in/out |
| Paste Events | ✅ | Bracketed paste mode support |

---

## Parity Decision Record (2026-01-25)

Purpose: freeze parity decisions for the remaining spec gaps so downstream work can proceed
without re-reading plan docs. All decisions below are "match Zig spec" (no intentional
divergence).

### Decision A - Grapheme pool encoding (match Zig)

Target behavior (EXISTING_OPENTUI_STRUCTURE.md - sections 3 and 1.3):
- Ref-counted grapheme pool with 24-bit IDs.
- `Cell.char` encodes grapheme usage via high bit, width in bits 24-30, and ID in low 24 bits.
- Pool operations: alloc, incref, decref, get.

API impact (Rust):
- Replace `CellContent::Grapheme(Arc<str>)` with ID-backed representation (ID + width).
- `Cell`/`OptimizedBuffer`/text drawing must allocate from a pool and encode width + ID in
  the stored `u32` char field.
- Public APIs that expose grapheme content must provide a way to resolve IDs to UTF-8 bytes
  (likely via a `GraphemePool` handle).

Migration strategy:
- Update text/buffer/conformance fixtures to compare resolved grapheme strings via the pool.
- Add test helpers to allocate graphemes and assert refcount reuse.
- Replace Arc<str>-specific expectations in text/editor tests.

Accept/reject criteria for parity:
- Encoding uses high-bit flag + width bits + 24-bit ID exactly as spec.
- Pool reuse/refcount behavior matches Zig; repeated graphemes reuse IDs and decref frees.
- Conformance fixtures for grapheme width/rendering match legacy outputs.

### Decision B - Link ID packing into TextAttributes (match Zig)

Target behavior (EXISTING_OPENTUI_STRUCTURE.md - section 1.2):
- `attributes` is a u32; bits 0-7 are style flags, bits 8-31 store link ID.
- `link_id == 0` means no link; set/get must preserve style bits.

API impact (Rust):
- `TextAttributes` and `Style` become 32-bit with packed link ID.
- `Cell` stores packed attributes; hyperlink resolution uses the packed ID (no separate field).
- Any public API that accepted raw bitflags must be updated (breaking change).

Migration strategy:
- Update attribute tests to assert bit packing and link ID extraction.
- Update hyperlink fixtures and API examples; adjust serialization/Debug output as needed.

Accept/reject criteria for parity:
- Bit layout matches spec; style bits unchanged when link ID is set/cleared.
- Link ID propagates through render/diff and OSC 8 output without regression.

### Decision C - Threaded renderer parity (match Zig)

Target behavior (EXISTING_OPENTUI_STRUCTURE.md - section 10.1):
- Renderer supports optional threaded mode (`useThread`) with synchronization (mutex) around
  render operations.
- Threaded mode must not change diff output, sync-output behavior, or hit-grid semantics.

API impact (Rust):
- `Renderer` gains a threaded variant or options to enable a render thread.
- Concurrency boundaries must be explicit (channel or guarded access to buffers).
- This is a breaking API addition but optional at runtime.

Migration strategy:
- Extend renderer tests/fixtures to cover threaded mode parity with single-threaded output.
- Add deterministic shutdown tests to avoid terminal state leaks.

Accept/reject criteria for parity:
- Threaded mode produces identical ANSI output for a given frame sequence.
- Render thread starts/stops cleanly and restores terminal state; no data races.

---

## Open Parity Gaps (must match Zig spec)

1. Grapheme pool encoding + ID-backed cells (Decision A).
2. Link ID packing into TextAttributes bits 8-31 (Decision B).
3. Threaded renderer support and API (Decision C).

---

## Test Coverage

328 tests covering:

**Unit Tests (248)**
- Core types (color, style, cell)
- Buffer operations (set, get, clear, draw)
- Scissor and opacity stacks
- Rope operations
- Edit buffer with undo/redo, word movement, delete operations
- Text buffer highlighting
- Renderer diff detection
- Hit grid
- Terminal capabilities
- Input parsing (keyboard, mouse, paste, focus)
- EditorView with visual navigation

**Conformance Tests (72)**
- Color (blending, hex parsing, HSV conversion, 256/16 color mapping)
- Buffer (box drawing, clear, text rendering, scissor clipping)
- Text (wrap modes, multiline, selection)
- Input (arrow keys, F-keys, modifiers, SGR mouse, focus, paste, UTF-8)
- ANSI (cursor positioning, true color, 256/16 color, attributes)
- Unicode (grapheme counting, display width)

**E2E Tests (3)**
- Keyboard input and rendering
- Mouse click and selection
- Bracketed paste mode

---

## Benchmarks

Run with: `cargo bench`

Benchmarks implemented:
- `buffer_new_80x24` / `buffer_new_200x50`
- `buffer_clear`
- `buffer_draw_text_short` / `buffer_draw_text_long`
- `buffer_set_cell` / `buffer_get_cell`

---

## Next Steps (Post-Parity Enhancements)

Reserved for non-spec improvements after the parity gaps above are closed.
