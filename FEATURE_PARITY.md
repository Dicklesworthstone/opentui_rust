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
| Cell | ✅ | CellContent enum: Char, Grapheme(GraphemeId), Empty, Continuation. Cell is Copy. |
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
- ✅ Replaced `CellContent::Grapheme(Arc<str>)` with `GraphemeId` (ID + width encoded per spec).
- ✅ `Cell` and `CellContent` are now `Copy` for zero-allocation hot paths.
- 🔲 `OptimizedBuffer`/text drawing must allocate from a pool and encode width + ID in
  the stored `u32` char field.
- 🔲 Public APIs that expose grapheme content must provide a way to resolve IDs to UTF-8 bytes
  (likely via a `GraphemePool` handle).

Migration strategy:
- Update text/buffer/conformance fixtures to compare resolved grapheme strings via the pool.
- Add test helpers to allocate graphemes and assert refcount reuse.
- Replace Arc<str>-specific expectations in text/editor tests.

Accept/reject criteria for parity:
- Encoding uses high-bit flag + width bits + 24-bit ID exactly as spec.
- Pool reuse/refcount behavior matches Zig; repeated graphemes reuse IDs and decref frees.
- Conformance fixtures for grapheme width/rendering match legacy outputs.

---

## Grapheme Pool Spec + API Design (bd-2qg.2)

Goal: define a self-contained, Rust-ready spec for the grapheme pool and its
cell encoding so implementation/testing is mechanical.

### Encoding + Bit Layout

`Cell.char` is stored as a `u32` with the following layout:

```
31           30..24            23..0
[ G ] [  width (7 bits) ] [ grapheme_id (24 bits) ]

G = 1 => grapheme pool reference
G = 0 => direct Unicode scalar value
```

Constants + helpers:

```rust
const GRAPHEME_FLAG: u32 = 0x8000_0000;
const WIDTH_SHIFT: u32 = 24;
const WIDTH_MASK: u32 = 0x7F00_0000;
const ID_MASK: u32 = 0x00FF_FFFF;

fn is_grapheme_char(c: u32) -> bool { c & GRAPHEME_FLAG != 0 }
fn grapheme_id(c: u32) -> u32 { c & ID_MASK }
fn grapheme_width(c: u32) -> u8 { ((c >> WIDTH_SHIFT) & 0x7F) as u8 }
fn pack_grapheme(id: u32, width: u8) -> u32 {
    GRAPHEME_FLAG | ((u32::from(width) & 0x7F) << WIDTH_SHIFT) | (id & ID_MASK)
}
```

Width semantics:
- Width is the display width of the grapheme cluster (typically 1 or 2).
- Width **must be >= 1** for grapheme start cells; width 0 is invalid.
- Width is computed from the grapheme string using `unicode_width`.

### ID Range + Validity

- Grapheme IDs are **24-bit**: `1..=0x00FF_FFFF`.
- `0` is reserved to mean “invalid/unset”.
- `alloc()` must return a non-zero ID or an error.
- `get(0)`, `incref(0)`, `decref(0)` return `InvalidId`.

### Pool Semantics

Slot layout (conceptual):
- `bytes: Box<[u8]>` (UTF‑8 grapheme bytes)
- `ref_count: u32`
- Optional `hash` for interning/lookup

Rules:
- `alloc(bytes)` inserts a new slot (or reuses a free slot) and sets `ref_count = 1`.
- `incref(id)` increments refcount; `decref(id)` decrements.
- When `ref_count` reaches **0**, the slot is released to a free list.
- Underflow (`decref` when `ref_count == 0`) is an error.

### Rust API Surface (proposed)

```rust
pub struct GraphemePool { /* slots + free list + optional interner */ }
pub struct GraphemeId(NonZeroU32); // 24-bit payload

impl GraphemePool {
    pub fn new() -> Self;
    pub fn alloc(&mut self, bytes: &[u8]) -> Result<GraphemeId, GraphemeError>;
    pub fn intern(&mut self, bytes: &[u8]) -> Result<GraphemeId, GraphemeError>;
    pub fn incref(&mut self, id: GraphemeId) -> Result<(), GraphemeError>;
    pub fn decref(&mut self, id: GraphemeId) -> Result<(), GraphemeError>;
    pub fn get(&self, id: GraphemeId) -> Result<&[u8], GraphemeError>;
}

pub fn pack_grapheme_id(id: GraphemeId, width: u8) -> u32;
pub fn unpack_grapheme_id(c: u32) -> Option<(GraphemeId, u8)>;
```

Threading model:
- Pool is **single-threaded** (no locks). It is owned by `Renderer` and used
  on the same thread that performs drawing and output.

### Integration Notes (for bd-2qg.4.x)

- `draw_text`:
  - For grapheme clusters (`len > 1`), call `intern()` to get an ID and pack
    `char` with width.
  - For single codepoints, store the Unicode scalar directly in `char`.
- When a cell is overwritten:
  - If old cell contains a grapheme ID, call `decref`.
  - If new cell contains a grapheme ID, `incref` is already handled by `alloc`/`intern`.
- `AnsiWriter` resolves grapheme IDs via `pool.get(id)` when emitting text.

### Module Placement + Ownership (bd-2qg.2.3)

Candidates:
- `src/grapheme_pool.rs` (crate root)
  - ✅ Shared by buffer/renderer/text without circular deps.
  - ✅ Easy to re-export from `lib.rs`.
- `src/unicode/grapheme_pool.rs`
  - ❌ Pool is not purely Unicode logic; it’s a storage/ownership system.
- `src/buffer/grapheme_pool.rs`
  - ❌ Renderer and AnsiWriter would need to depend on buffer internals.

Decision:
- Place `GraphemePool` at `src/grapheme_pool.rs` and re-export from `lib.rs`.
- Ownership: `Renderer` owns the pool and passes `&mut GraphemePool` to draw paths
  and to render output (no locks, single-threaded).

### Performance Targets

- No allocations on hot paths for repeated graphemes (interned IDs reused).
- Free list reuse to avoid pool growth churn.
- Equality/diff uses encoded `u32` directly (no string compares).

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

## Threaded Renderer API Sketch (bd-2qg.8.1)

Goal: keep stdout/terminal I/O on a single render thread while the main thread
owns the draw API. Buffer ownership is swapped per frame to avoid per-frame
allocations.

### Public API (proposed)

```rust
pub struct ThreadedRenderer {
    tx: Sender<RenderCommand>,
    rx: Receiver<RenderReply>,
    back_buffer: OptimizedBuffer,
    link_pool: LinkPool,
    width: u32,
    height: u32,
}

impl ThreadedRenderer {
    pub fn new(width: u32, height: u32, options: RendererOptions) -> io::Result<Self>;
    pub fn buffer(&mut self) -> &mut OptimizedBuffer;
    pub fn link_pool_mut(&mut self) -> &mut LinkPool;
    pub fn present(&mut self) -> io::Result<()>;
    pub fn present_force(&mut self) -> io::Result<()>;
    pub fn resize(&mut self, width: u32, height: u32) -> io::Result<()>;
    pub fn set_cursor(&self, x: u32, y: u32, visible: bool) -> io::Result<()>;
    pub fn set_title(&self, title: &str) -> io::Result<()>;
    pub fn shutdown(self) -> io::Result<()>;
}
```

Notes:
- `buffer()` exposes the current back buffer for drawing on the main thread.
- `link_pool_mut()` stays on the main thread; the pool is moved with frames so the
  render thread can resolve IDs to URLs without shared locking.
- `present()` is synchronous: it submits the frame and blocks until a buffer is returned.

### Command/Reply Protocol (proposed)

```rust
enum RenderCommand {
    Present { buffer: OptimizedBuffer, link_pool: LinkPool, force: bool },
    Resize { width: u32, height: u32 },
    SetCursor { x: u32, y: u32, visible: bool },
    SetTitle { title: String },
    Shutdown,
}

enum RenderReply {
    Presented { buffer: OptimizedBuffer, link_pool: LinkPool },
    Ack,
}
```

### Ownership & Flow

```
Main thread                         Render thread
-----------                         -------------
draw into back_buffer
send Present(back_buffer, link_pool)  ─────────────▶
                                     diff + write ANSI
                                     swap(front, back)
recv Presented(old_front, link_pool) ◀─────────────
continue drawing into returned buffer
```

Key constraints:
- No per-frame allocation: buffers are moved, not rebuilt.
- Terminal I/O happens only on the render thread.
- `Resize` updates both sides: main thread resizes its back buffer; render thread
  resizes its front buffer before next present.

---

## Thread Lifecycle & Cleanup Semantics (bd-2qg.8.2)

### Startup Sequence

1. `ThreadedRenderer::new()` on the main thread:
   - Creates channels (command and reply)
   - Allocates initial back buffer
   - Spawns render thread via `std::thread::spawn`
   - Render thread owns: Terminal, front buffer, grapheme pool reference

2. Render thread initialization:
   - Terminal enters alt screen (if configured)
   - Cursor hidden (if configured)
   - Mouse tracking enabled (if configured)
   - Capabilities queried (if configured)
   - Thread enters message loop, waiting for commands

### Graceful Shutdown (`ThreadedRenderer::shutdown()`)

1. Main thread sends `RenderCommand::Shutdown`
2. Render thread receives shutdown command
3. Render thread performs terminal cleanup:
   - Disables mouse tracking
   - Shows cursor
   - Exits alt screen
   - Flushes any pending output
4. Render thread sends `RenderReply::ShutdownComplete`
5. Render thread exits message loop and terminates
6. Main thread receives acknowledgment
7. Main thread joins the render thread handle

```rust
pub fn shutdown(self) -> io::Result<()> {
    // Send shutdown command
    self.tx.send(RenderCommand::Shutdown).map_err(|_| {
        io::Error::new(io::ErrorKind::BrokenPipe, "render thread disconnected")
    })?;

    // Wait for acknowledgment
    match self.rx.recv() {
        Ok(RenderReply::ShutdownComplete) => {}
        Ok(_) => {} // Ignore other replies
        Err(_) => {} // Channel closed, thread is gone
    }

    // Join the thread
    self.handle.join().map_err(|_| {
        io::Error::new(io::ErrorKind::Other, "render thread panicked")
    })?;

    Ok(())
}
```

### Drop Behavior

The `Drop` implementation ensures terminal state is restored even if `shutdown()`
was not called explicitly:

```rust
impl Drop for ThreadedRenderer {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Try to send shutdown (may fail if thread is already dead)
            let _ = self.tx.send(RenderCommand::Shutdown);

            // Give the thread a brief moment to cleanup
            // then join (blocking) to ensure cleanup completes
            let _ = handle.join();
        }
    }
}
```

**Important**: Drop must be blocking to guarantee terminal cleanup. A detached
render thread that outlives the main thread would leave the terminal in a bad
state (raw mode, alt screen, cursor hidden).

### Panic Recovery

If the render thread panics:

1. Channel operations from main thread will return errors
2. `join()` will return `Err(Any)` containing the panic payload
3. Terminal state may be corrupted

Mitigation: The render thread should use `catch_unwind` around its message loop:

```rust
fn render_thread_main(rx: Receiver<RenderCommand>, ...) {
    // Set up panic hook to restore terminal before unwinding
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        // ... message loop ...
    }));

    // Always cleanup terminal, even on panic
    let _ = terminal.cleanup();

    if let Err(e) = result {
        std::panic::resume_unwind(e);
    }
}
```

### Terminal State Invariants

The render thread maintains these invariants:

| State | On Start | On Shutdown | On Panic |
|-------|----------|-------------|----------|
| Alt screen | Entered | Exited | Exited |
| Cursor | Hidden | Shown | Shown |
| Mouse tracking | Enabled | Disabled | Disabled |
| Raw mode | Enabled | Disabled | Disabled |

### Thread Safety Considerations

- `Terminal<Stdout>` is `!Send` because it holds a reference to `Stdout`
- Solution: Terminal is created on the render thread, not passed from main
- Buffers are moved via channels, never shared concurrently
- GraphemePool travels with the frame (moved, not borrowed)
- LinkPool travels with the frame (moved, not borrowed)

### Timeout Handling

For robustness, the main thread may want to timeout when waiting for the render
thread during shutdown:

```rust
match self.rx.recv_timeout(Duration::from_secs(5)) {
    Ok(RenderReply::ShutdownComplete) => {}
    Ok(_) | Err(_) => {
        // Thread unresponsive, force cleanup
        // This is a last resort - terminal state may be corrupted
    }
}
```

### Testing Strategy

1. **Startup/shutdown cycle**: Verify clean start and stop
2. **Multiple present calls**: Ensure no resource leaks
3. **Resize during render**: Verify correct buffer sizes
4. **Drop without shutdown**: Verify terminal is restored
5. **Simulated panic**: Verify terminal cleanup on panic (requires test harness)

---

## Performance Model for Threaded Renderer (bd-2qg.8.3)

### Comparison: Single-Threaded vs Threaded Rendering

| Aspect | Single-Threaded | Threaded |
|--------|-----------------|----------|
| **Latency per frame** | Lower (no channel overhead) | Slightly higher (channel round-trip) |
| **Throughput** | Limited by I/O blocking | Higher (main thread continues while I/O completes) |
| **Allocations per frame** | Zero (reuse buffers) | Zero (move buffers via channel) |
| **Lock contention** | None (single-threaded) | None (ownership transfer, not sharing) |
| **Terminal I/O** | Blocks main thread | Offloaded to render thread |
| **Complexity** | Simple | Channel + lifecycle management |

### Hot Path Analysis

The "hot path" is the frame submission loop:

```
Main: draw → draw → draw → present() → [wait] → buffer returned → draw → ...
                              ↓
Render:              [wait] → receive → diff → write ANSI → reply → [wait]
```

**Key performance properties:**

1. **Zero allocations on present()**: Buffers are moved, not cloned
2. **No locks**: Ownership transfer via channels, not shared state
3. **Minimal synchronization**: Single send + single recv per frame
4. **Diff on render thread**: Main thread doesn't wait for diff computation

### Where Work Happens

| Operation | Thread | Blocking? |
|-----------|--------|-----------|
| Drawing (set cells, draw_text) | Main | No |
| Buffer ownership transfer | Both | Channel wait |
| Diff computation | Render | No (runs in parallel) |
| ANSI sequence generation | Render | No |
| Terminal write + flush | Render | Yes (I/O) |
| Terminal capabilities query | Render | Yes (one-time) |

### Diff Strategy Decision

**Question**: Should diff run on main thread or render thread?

**Decision**: Diff runs on the render thread.

**Rationale**:
- Main thread can start drawing the next frame immediately after `present()` returns
- Diff is O(w*h) but memory-access bound, not compute bound
- Moving diff to render thread maximizes main thread utilization
- The render thread would otherwise be idle while main thread draws

**Alternative rejected**: Pre-computing diff on main thread before sending. This
would add latency to `present()` and not improve throughput since the render
thread would be idle during main-thread diff.

### Memory Layout Considerations

Buffer layout for cache-friendly diffing:

```rust
// Row-major layout for sequential access
cells: Vec<Cell>  // cells[y * width + x]
```

Diff algorithm walks buffers sequentially, which is optimal for cache prefetch.
No changes needed for threaded rendering.

### Channel Choice

Using `std::sync::mpsc` for simplicity:

- `Sender<RenderCommand>`: Main thread sends commands
- `Receiver<RenderReply>`: Main thread receives buffer back

Alternatives considered:
- `crossbeam-channel`: Better performance for MPMC, but we're SPSC
- `flume`: Similar, adds dependency
- `tokio::sync`: Would require async runtime

For SPSC with one message per frame (~60/sec max), `std::sync::mpsc` is adequate.
Can swap to crossbeam later if profiling shows channel overhead.

### Synchronous Present Design

`present()` is synchronous (blocks until buffer is returned):

**Pros**:
- Simple API (no futures, no callbacks)
- Predictable frame timing
- Easy to reason about buffer ownership

**Cons**:
- Main thread blocked during render (but only during I/O, not diff)

**Why this is acceptable**:
- At 60 FPS, each frame has ~16ms budget
- Terminal I/O is typically <1ms
- Main thread has 15+ ms to draw next frame
- For higher throughput, use double or triple buffering (future enhancement)

### Benchmarking Strategy

To verify no performance regression:

1. **Baseline**: Single-threaded `Renderer::present()` latency
2. **Threaded**: `ThreadedRenderer::present()` latency
3. **Throughput**: Frames per second with constant draw load

Expected results:
- Threaded present() latency may be 10-50μs higher (channel overhead)
- Threaded throughput should be equal or higher
- No new allocations visible in memory profiler

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
