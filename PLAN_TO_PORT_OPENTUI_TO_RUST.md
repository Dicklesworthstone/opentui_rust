# Plan to Port OpenTUI to Rust

> **Essence Extraction Approach:** Extract the spec from legacy → implement from spec → never translate line-by-line.

## Project Overview

OpenTUI is a TypeScript+Zig library for building terminal user interfaces (TUIs). The architecture consists of:

1. **Zig Core** (~15,900 LOC) - High-performance rendering engine
2. **TypeScript Layer** - API wrapper exposing Zig via FFI
3. **React/SolidJS Reconcilers** - Declarative UI bindings

We are porting **only the Zig core** to idiomatic Rust, creating a standalone TUI library.

---

## Scope

### IN SCOPE

| Component | Zig Source | Lines | Purpose |
|-----------|------------|-------|---------|
| Buffer | `buffer.zig` | 2042 | Cell-based frame buffer with alpha, scissoring |
| UTF-8 | `utf8.zig` | 1799 | Unicode/grapheme handling, wcwidth |
| TextBufferView | `text-buffer-view.zig` | 1385 | Viewport for text with wrapping |
| Renderer | `renderer.zig` | 1339 | Double-buffered diff rendering |
| Rope | `rope.zig` | 1220 | Persistent immutable rope data structure |
| TextBuffer | `text-buffer.zig` | 1046 | Styled text buffer with highlights |
| EditorView | `editor-view.zig` | 791 | Editor cursor, selection, visual navigation |
| EditBuffer | `edit-buffer.zig` | 784 | Editable text with undo/redo |
| Terminal | `terminal.zig` | 657 | Terminal capabilities, cursor, mouse |
| Grapheme | `grapheme.zig` | 465 | Grapheme pool with ref-counting |
| TextIterators | `text-buffer-iterators.zig` | 461 | Line/grapheme iteration |
| TextSegment | `text-buffer-segment.zig` | 404 | Styled segment handling |
| ANSI | `ansi.zig` | 259 | ANSI escape codes, colors, attributes |
| Link | `link.zig` | 254 | Hyperlink handling (OSC 8) |
| SyntaxStyle | `syntax-style.zig` | 161 | Syntax highlighting styles |

**Total: ~12,000 lines of core logic** (excluding lib.zig FFI exports)

### EXPLICIT EXCLUSIONS

| Excluded | Reason |
|----------|--------|
| TypeScript layer | Replaced by native Rust API |
| React reconciler | Framework-specific, out of scope |
| SolidJS reconciler | Framework-specific, out of scope |
| `lib.zig` FFI exports | Replaced by Rust public API |
| `build.zig` | Replaced by Cargo |
| Bun-specific optimizations | N/A for Rust |
| Test files (`*_test.zig`) | Will write new Rust tests |
| Benchmark files (`*_bench.zig`) | Will write Criterion benchmarks |

---

## Architecture Decisions

### 1. Rope Implementation

The Zig rope is a persistent/immutable data structure with:
- O(log n) insert/delete
- Marker tracking for positions
- Undo/redo history
- Custom metrics (line count, byte size)

**Rust approach:** Use `ropey` crate as foundation, extend with:
- Styled text segments
- Marker/highlight tracking
- History management

### 2. Grapheme Handling

The Zig code uses a custom grapheme pool with ref-counting for efficient memory reuse.

**Rust approach:** Use `unicode-segmentation` + `unicode-width` crates. Consider pooling if benchmarks show need.

### 3. Terminal Output

The Zig renderer uses:
- Double buffering with diff detection
- ANSI SGR sequences for styling
- Cell-by-cell comparison

**Rust approach:** Similar architecture with:
- `crossterm` or custom ANSI output
- Smart diff detection to minimize writes

### 4. API Design

**Rust API will be idiomatic:**
```rust
// Create renderer
let mut renderer = Renderer::new(80, 24)?;

// Create text buffer
let mut buffer = TextBuffer::new();
buffer.set_text("Hello, world!");
buffer.add_highlight(0..5, Style::bold());

// Create view
let view = TextBufferView::new(&buffer)
    .wrap_mode(WrapMode::Word)
    .viewport(0, 0, 80, 24);

// Render
renderer.draw(&view)?;
renderer.present()?;
```

---

## Phases

### Phase 1: Foundation (Core Types)
- [ ] RGBA color type with alpha blending
- [ ] TextAttributes (bold, italic, underline, etc.)
- [ ] Cell type (char + fg + bg + attributes)
- [ ] ANSI escape code generation

### Phase 2: Buffer & Rendering
- [ ] OptimizedBuffer with cell array
- [ ] Scissor rect stack
- [ ] Opacity stack
- [ ] Box drawing
- [ ] Text drawing with grapheme support

### Phase 3: Text Data Structures
- [ ] Rope wrapper or implementation
- [ ] StyledText segments
- [ ] Highlight management
- [ ] Line iteration

### Phase 4: Text Views
- [ ] TextBuffer (styled text storage)
- [ ] TextBufferView (viewport, wrapping, selection)
- [ ] EditBuffer (cursor, undo/redo)
- [ ] EditorView (visual cursor, word boundaries)

### Phase 5: Renderer
- [ ] CliRenderer with double buffering
- [ ] Diff detection
- [ ] Terminal capability detection
- [ ] Mouse support
- [ ] Hit testing grid

### Phase 6: Polish
- [ ] Comprehensive test suite
- [ ] Benchmarks
- [ ] Documentation
- [ ] Examples

---

## Key Rust Crates

| Purpose | Crate |
|---------|-------|
| Unicode segmentation | `unicode-segmentation` |
| Character width | `unicode-width` |
| Terminal I/O | `crossterm` |
| Rope data structure | `ropey` |
| Color parsing | `csscolorparser` (optional) |
| Benchmarking | `criterion` |

---

## Success Criteria

1. **Functionality:** All OpenTUI Zig features work in Rust
2. **Performance:** Equal or better than Zig (benchmark comparison)
3. **Binary size:** Smaller than Zig+Bun bundle
4. **API:** Idiomatic Rust, well-documented
5. **Tests:** High coverage matching Zig test suite

---

## Next Steps

1. Create `EXISTING_OPENTUI_STRUCTURE.md` - detailed spec extraction
2. Create `PROPOSED_RUST_ARCHITECTURE.md` - Rust module design
3. Bootstrap Cargo project with dependencies
4. Implement Phase 1 (Foundation)
