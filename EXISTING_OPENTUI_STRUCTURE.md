# Existing OpenTUI Structure (Spec Document)

> **Purpose:** This document captures the complete behavioral specification of OpenTUI's Zig core.
> After reading this, you should NOT need to consult the legacy code during implementation.

---

## 1. Core Types

### 1.1 RGBA Color

```zig
pub const RGBA = [4]f32;  // [r, g, b, a] normalized 0.0-1.0
```

**Operations:**
- Alpha blending: `result = src * src.a + dst * (1 - src.a)`
- HSV to RGB conversion for color generation
- Component clamping to [0.0, 1.0]

### 1.2 Text Attributes

Bit-packed u32 with two regions:
- **Bits 0-7:** Style flags (8 attributes)
- **Bits 8-31:** Link ID (24 bits for hyperlink reference)

```
Attribute Flags:
  BOLD         = 0x01  (bit 0)
  DIM          = 0x02  (bit 1)
  ITALIC       = 0x04  (bit 2)
  UNDERLINE    = 0x08  (bit 3)
  BLINK        = 0x10  (bit 4)
  INVERSE      = 0x20  (bit 5)
  HIDDEN       = 0x40  (bit 6)
  STRIKETHROUGH = 0x80 (bit 7)
```

**Link ID Encoding:**
- `getLinkId(attr)` → Extract bits 8-31
- `setLinkId(attr, id)` → Set link while preserving style bits
- Link ID 0 means no link

### 1.3 Cell

A single terminal cell containing:
```
struct Cell {
    char: u32,        // Unicode codepoint or grapheme pool ID
    fg: RGBA,         // Foreground color
    bg: RGBA,         // Background color
    attributes: u32,  // Style flags + link ID
}
```

**Special char values:**
- `0x0a00` (CLEAR_CHAR) - Indicates cell should use background only
- High bit set → grapheme pool reference (multi-codepoint cluster)

---

## 2. ANSI Escape Sequences

### 2.1 Cursor Movement
- Move to position: `\x1b[{row};{col}H`
- Hide cursor: `\x1b[?25l`
- Show cursor: `\x1b[?25h`
- Save position: `\x1b[s`
- Restore position: `\x1b[u`

### 2.2 Colors (SGR True Color)
- Foreground: `\x1b[38;2;{r};{g};{b}m`
- Background: `\x1b[48;2;{r};{g};{b}m`
- Reset: `\x1b[0m`

### 2.3 Text Attributes
- Bold: `\x1b[1m`
- Dim: `\x1b[2m`
- Italic: `\x1b[3m`
- Underline: `\x1b[4m`
- Blink: `\x1b[5m`
- Inverse: `\x1b[7m`
- Hidden: `\x1b[8m`
- Strikethrough: `\x1b[9m`

### 2.4 Cursor Styles
- Block: `\x1b[2 q` (steady), `\x1b[1 q` (blink)
- Line: `\x1b[6 q` (steady), `\x1b[5 q` (blink)
- Underline: `\x1b[4 q` (steady), `\x1b[3 q` (blink)
- Cursor color: `\x1b]12;#{RRGGBB}\x07`

### 2.5 Screen Management
- Alternate screen: `\x1b[?1049h` (enter), `\x1b[?1049l` (exit)
- Clear screen: `\x1b[2J`
- Home: `\x1b[H`

### 2.6 Mouse Support
- Enable tracking: `\x1b[?1000h`
- Button events: `\x1b[?1002h`
- All motion: `\x1b[?1003h`
- SGR extended mode: `\x1b[?1006h`

### 2.7 Terminal Capabilities (Queries)
- Primary device attributes: `\x1b[c`
- XTVERSION: `\x1b[>0q`
- Pixel resolution: `\x1b[14t`
- Kitty keyboard: `\x1b[?u`

### 2.8 Hyperlinks (OSC 8)
- Start link: `\x1b]8;;{url}\x1b\\`
- End link: `\x1b]8;;\x1b\\`

### 2.9 Synchronized Output
- Begin sync: `\x1b[?2026h`
- End sync: `\x1b[?2026l`

---

## 3. Grapheme Pool

A reference-counted pool for multi-codepoint grapheme clusters.

### 3.1 Design
- Slots store UTF-8 bytes of grapheme clusters
- 24-bit ID allows ~16M unique graphemes
- Reference counting for memory reuse
- Used for emoji, combining characters, ZWJ sequences

### 3.2 Char Encoding
```
If char & 0x80000000:  // High bit set
    grapheme_id = char & 0x00FFFFFF  // Low 24 bits
    width = (char >> 24) & 0x7F      // Bits 24-30
    // Look up grapheme_id in pool to get UTF-8 bytes
Else:
    // Simple Unicode codepoint
```

### 3.3 Operations
- `alloc(bytes)` → Returns grapheme ID
- `incref(id)` → Increment reference count
- `decref(id)` → Decrement, free if zero
- `get(id)` → Returns UTF-8 bytes

---

## 4. Buffer (OptimizedBuffer)

A 2D cell array with rendering features.

### 4.1 Structure
```
struct OptimizedBuffer {
    width: u32,
    height: u32,
    chars: []u32,           // width * height codepoints
    fg: []RGBA,             // foreground colors
    bg: []RGBA,             // background colors
    attributes: []u32,      // style + link IDs

    scissorStack: []ClipRect,
    opacityStack: []f32,

    respectAlpha: bool,     // Enable alpha blending
    pool: *GraphemePool,
    id: []const u8,         // Debug identifier
}
```

### 4.2 Cell Operations
- `set(x, y, cell)` - Direct cell write
- `setCellWithAlphaBlending(x, y, char, fg, bg, attr)` - Blended write
- `clear(bg, char)` - Fill entire buffer
- `fillRect(x, y, w, h, bg)` - Fill rectangle with color

### 4.3 Text Drawing
- `drawText(text, x, y, fg, bg, attr)` - Draw UTF-8 string
- `drawChar(char, x, y, fg, bg, attr)` - Draw single character
- Respects scissor clipping
- Handles wide characters (width > 1)

### 4.4 Scissor Stack
Clipping rectangles that restrict drawing:
- `pushScissorRect(x, y, w, h)` - Add clipping region
- `popScissorRect()` - Remove top clip
- `clearScissorRects()` - Remove all clips
- Drawing outside clip region is discarded

### 4.5 Opacity Stack
Global opacity modifiers:
- `pushOpacity(opacity)` - Multiply all alpha by opacity
- `popOpacity()` - Restore previous
- `getCurrentOpacity()` - Get combined opacity
- Applied during alpha blending

### 4.6 Box Drawing
```
drawBox(x, y, w, h, borderChars, sides, borderColor, bgColor, fill, title, titleAlign)
```
- `borderChars`: [8]u32 for corners and edges
- `sides`: {top, right, bottom, left} booleans
- Optional title with alignment (left/center/right)

### 4.7 Frame Buffer Composition
- `drawFrameBuffer(destX, destY, src, srcX, srcY, srcW, srcH)`
- Composites source buffer onto destination
- Respects alpha when `respectAlpha` is true

---

## 5. Rope Data Structure

Persistent/immutable rope for efficient text editing.

### 5.1 Node Types
```
Node = Branch { left, right, left_metrics, total_metrics }
     | Leaf { data: T, is_sentinel: bool }
```

### 5.2 Metrics
Each node tracks:
- `count`: Number of elements
- `depth`: Tree depth
- `custom`: Type-specific metrics (e.g., line count, byte size)
- `marker_counts`: Counts per marker type (if enabled)

### 5.3 Operations
- **Insert at index:** O(log n), creates new nodes
- **Delete range:** O(log n), returns new root
- **Split at index:** Returns (left, right) subtrees
- **Concat:** Joins two ropes, auto-rebalances
- **Index:** O(log n) lookup by position

### 5.4 Balancing
- Maximum imbalance factor: 7
- Triggers rebalance when one side > 3/4 of total weight
- Uses node rotation/rebuilding

### 5.5 History (Undo/Redo)
- Configurable max undo depth
- Each edit creates history entry with:
  - Previous root reference
  - Cursor position
  - Optional metadata

---

## 6. Text Buffer

Styled text storage with highlighting.

### 6.1 UnifiedTextBuffer Structure
```
struct UnifiedTextBuffer {
    rope: Rope(Segment),    // Rope of styled segments
    highlights: HighlightManager,
    syntax_style: ?*SyntaxStyle,

    defaults: {
        fg: ?RGBA,
        bg: ?RGBA,
        attributes: ?u32,
    },

    tab_width: u8,          // Default 4
    mem_registry: MemRegistry,  // For external text sources
    pool: *GraphemePool,
    width_method: WidthMethod,  // .wcwidth or .unicode
}
```

### 6.2 Segment Structure
A segment represents a run of styled text:
```
struct Segment {
    text: []const u8,       // UTF-8 bytes
    fg: ?RGBA,
    bg: ?RGBA,
    attributes: ?u32,

    // Cached metrics
    byte_len: u32,
    char_count: u32,
    line_count: u32,
}
```

### 6.3 Content Operations
- `setText(text)` - Replace all content
- `append(text)` - Add text at end
- `setStyledText(chunks)` - Set with inline styles
- `clear()` - Remove all content
- `reset()` - Clear content and highlights

### 6.4 Highlighting
```
struct Highlight {
    col_start: u32,
    col_end: u32,
    style_id: u32,      // Reference to SyntaxStyle
    priority: u8,       // Higher wins on overlap
    hl_ref: u16,        // For batch removal
}
```

Operations:
- `addHighlight(line, start, end, style_id, priority, ref)`
- `addHighlightByCharRange(char_start, char_end, ...)`
- `removeHighlightsByRef(ref)` - Remove all with matching ref
- `clearLineHighlights(line)`
- `clearAllHighlights()`

### 6.5 Memory Registry
For efficient external text handling:
- `register(data, owned)` → mem_id
- `replace(id, data, owned)`
- `setTextFromMemId(id)` - Set content from registered buffer

---

## 7. Text Buffer View

Viewport into a TextBuffer with wrapping and selection.

### 7.1 Structure
```
struct UnifiedTextBufferView {
    buffer: *UnifiedTextBuffer,

    viewport: Viewport { x, y, width, height },
    wrap_mode: WrapMode,    // .none, .char, .word
    wrap_width: ?u32,

    selection: ?Selection {
        start: u32,         // Char offset
        end: u32,
        bg_color: ?RGBA,
        fg_color: ?RGBA,
    },

    local_selection: ?LocalSelection {
        anchor: {x, y},
        focus: {x, y},
        ...
    },

    tab_indicator: u32,     // Char to show for tabs
    tab_indicator_color: RGBA,
    truncate: bool,
}
```

### 7.2 Line Information
Cached arrays for efficient rendering:
- `starts[]` - Byte offset where each virtual line starts
- `widths[]` - Display width of each line
- `sources[]` - Source line index (for wrapped lines)
- `wraps[]` - Boolean flags for continuation lines
- `max_width` - Maximum line width

### 7.3 Wrapping Modes
- **None:** No wrapping, horizontal scroll
- **Char:** Break at exact width boundary
- **Word:** Break at word boundaries when possible

### 7.4 Selection
Two selection modes:
1. **Offset-based:** Start/end character offsets
2. **Local:** Anchor/focus in viewport coordinates

Operations:
- `setSelection(start, end, bg, fg)`
- `updateSelection(end, bg, fg)`
- `resetSelection()`
- `getSelectedText()` → UTF-8 string

### 7.5 Measurement
- `getVirtualLineCount()` - Lines after wrapping
- `measureForDimensions(w, h)` → {line_count, max_width}

---

## 8. Edit Buffer

Editable text with cursor and undo/redo.

### 8.1 Structure
```
struct EditBuffer {
    tb: *UnifiedTextBuffer,
    cursor: Cursor { row, col },
    history: History,
    id: u16,
}
```

### 8.2 Cursor Operations
- `setCursor(row, col)`
- `setCursorByOffset(offset)`
- `moveLeft()`, `moveRight()`, `moveUp()`, `moveDown()`
- `getCursorPosition()` → {line, visual_col, offset}

### 8.3 Text Editing
- `insertText(text)` - Insert at cursor
- `deleteRange(start, end)` - Delete range
- `backspace()` - Delete char before cursor
- `deleteForward()` - Delete char at cursor
- `deleteLine()` - Delete current line
- `replaceText(text)` - Replace all content

### 8.4 Navigation
- `getNextWordBoundary()` - Next word start/end
- `getPrevWordBoundary()` - Previous word boundary
- `getEOL()` - End of current line
- `gotoLine(line)` - Jump to line

### 8.5 History
- `undo()` → previous_metadata
- `redo()` → next_metadata
- `canUndo()`, `canRedo()`
- `clearHistory()`

---

## 9. Editor View

Visual editor with viewport cursor management.

### 9.1 Structure
```
struct EditorView {
    edit_buffer: *EditBuffer,
    text_buffer_view: *UnifiedTextBufferView,

    viewport: ?Viewport,
    scroll_margin: f32,     // 0.0-0.5, portion to keep visible
    selection_follow_cursor: bool,
}
```

### 9.2 Visual Cursor
```
struct VisualCursor {
    visual_row: u32,    // Row in wrapped view
    visual_col: u32,    // Column in wrapped view
    logical_row: u32,   // Original line number
    logical_col: u32,   // Original column
    offset: u32,        // Character offset
}
```

### 9.3 Visual Navigation
- `moveUpVisual()` - Move up in wrapped view
- `moveDownVisual()` - Move down in wrapped view
- `getVisualSOL()` - Start of visual line
- `getVisualEOL()` - End of visual line

### 9.4 Selection with Cursor
- `setLocalSelection(anchor, focus, ..., updateCursor)`
- `deleteSelectedText()`
- Auto-scroll to keep cursor visible

---

## 10. Renderer (CliRenderer)

Double-buffered terminal renderer.

### 10.1 Structure
```
struct CliRenderer {
    width, height: u32,
    currentRenderBuffer: *OptimizedBuffer,
    nextRenderBuffer: *OptimizedBuffer,

    terminal: Terminal,
    backgroundColor: RGBA,
    renderOffset: u32,

    // Hit testing
    currentHitGrid: []u32,
    nextHitGrid: []u32,
    hitScissorStack: []ClipRect,

    // Threading
    useThread: bool,
    renderMutex: Mutex,
    // ...

    // Stats
    renderStats: { fps, frameTime, cellsUpdated, ... },
}
```

### 10.2 Rendering Flow
1. Application draws to `nextRenderBuffer`
2. Call `render(force)`
3. Diff `nextRenderBuffer` against `currentRenderBuffer`
4. Generate ANSI output only for changed cells
5. Swap buffers

### 10.3 Diff Algorithm
For each cell (x, y):
- Compare char, fg, bg, attributes
- Skip if identical to current buffer
- Optimize runs of consecutive changes
- Track cursor position to minimize moves

### 10.4 Terminal Setup
```
setupTerminal(useAlternateScreen):
    - Switch to alternate screen (optional)
    - Hide cursor
    - Enable mouse tracking
    - Query terminal capabilities
```

### 10.5 Hit Grid
For mouse event dispatch:
- Each cell stores renderable ID
- `addToHitGrid(x, y, w, h, id)` - Register area
- `checkHit(x, y)` → id of element at position
- Supports scissor clipping

### 10.6 Debug Overlay
Optional stats display showing:
- FPS, frame time
- Cells updated per frame
- Memory usage
- Render time breakdown

---

## 11. Terminal Capabilities

### 11.1 Detected Features
```
struct Capabilities {
    kitty_keyboard: bool,
    kitty_graphics: bool,
    rgb: bool,              // True color support
    unicode: WidthMethod,   // .wcwidth or .unicode
    sgr_pixels: bool,       // SGR pixel mouse mode
    color_scheme_updates: bool,
    explicit_width: bool,
    scaled_text: bool,
    sixel: bool,
    focus_tracking: bool,
    sync: bool,             // Synchronized output
    bracketed_paste: bool,
    hyperlinks: bool,
    explicit_cursor_positioning: bool,
}
```

### 11.2 Detection Process
1. Send capability queries
2. Parse responses (async)
3. Enable supported features

### 11.3 Cursor State
```
struct CursorState {
    x, y: u32,
    visible: bool,
    style: CursorStyle,     // .block, .line, .underline
    blinking: bool,
    color: RGBA,
}
```

---

## 12. Syntax Style

Named style registry for syntax highlighting.

### 12.1 Structure
```
struct SyntaxStyle {
    styles: HashMap([]const u8, StyleEntry),
    id_counter: u32,
}

struct StyleEntry {
    id: u32,
    fg: ?RGBA,
    bg: ?RGBA,
    attributes: u32,
}
```

### 12.2 Operations
- `registerStyle(name, fg, bg, attr)` → style_id
- `resolveByName(name)` → ?style_id
- `getStyleCount()` → count

---

## 13. UTF-8 and Width

### 13.1 Width Methods
- **wcwidth:** POSIX-style width (ambiguous = 1)
- **unicode:** Unicode Standard Annex #11 (ambiguous = 2)

### 13.2 Grapheme Info
```
struct GraphemeInfo {
    byte_offset: u32,
    byte_len: u8,
    col_offset: u32,
    width: u8,
}
```

### 13.3 Operations
- `isAsciiOnly(text)` - Fast ASCII check
- `getWidthAt(text, offset, tab_width, method)` → display width
- `findGraphemeInfo(text, ...)` → array of GraphemeInfo

---

## 14. Link Pool

Hyperlink URL storage.

### 14.1 Structure
```
struct LinkPool {
    urls: [][]const u8,
    ref_counts: []u32,
    free_list: []u32,
}
```

### 14.2 Operations
- `alloc(url)` → link_id
- `get(id)` → url bytes
- Reference counting for cleanup

---

## 15. Event System

### 15.1 Event Bus
Global event dispatcher:
- `setEventCallback(fn(name, data))`
- Fires events for terminal responses

### 15.2 Logger
Debug logging with levels:
- `setLogCallback(fn(level, msg))`
- Levels: debug, info, warn, error
