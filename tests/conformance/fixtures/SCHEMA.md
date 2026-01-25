# Conformance Fixture Schema

This document describes the JSON schema for conformance test fixtures.

## Top-Level Structure

```json
{
  "crate": "opentui",
  "version": "0.1.0",
  "captured_at": "2026-01-19T00:00:00Z",
  "tests": [ ... ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `crate` | string | Source library name (always "opentui") |
| `version` | string | Version of the source library when captured |
| `captured_at` | string | ISO 8601 timestamp when fixtures were generated |
| `tests` | array | Array of test case objects |

## Test Case Structure

```json
{
  "name": "rgba_blend_half_red_over_blue",
  "category": "color",
  "input": { ... },
  "expected_output": { ... }
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Unique test identifier (snake_case) |
| `category` | string | Test category (see below) |
| `input` | object | Test inputs specific to the category |
| `expected_output` | object | Expected results to verify |

## Categories

### `color` - Color operations

Tests for RGBA color manipulation.

**Input fields:**
- `fg`, `bg`: Hex color strings (e.g., "#FF000080")
- `hex`: Hex color to parse
- `h`, `s`, `v`: HSV values (0-360 for h, 0-1 for s/v)

**Expected output fields:**
- `hex`: Result as hex string
- `r`, `g`, `b`, `a`: RGB(A) components (0-255)
- `valid`: Boolean for parse validity
- `index`, `index_min`, `index_max`: For palette mapping

### `buffer` - Buffer operations

Tests for cell buffer drawing operations.

**Input fields:**
- `width`, `height`: Buffer dimensions
- `x`, `y`: Drawing position
- `text`: Text to draw
- `title`: Box title (optional)
- `scissor`: Clipping region `{x, y, w, h}`
- `fill_char`: Character to fill

**Expected output fields:**
- `lines`: Array of rendered line strings
- `line`: Single rendered line
- `all_empty`: Boolean for empty buffer check
- `cell_count`: Number of cells

### `text` - Text buffer operations

Tests for text wrapping and selection.

**Input fields:**
- `text`: Source text
- `width`: Viewport width
- `mode`: Wrap mode ("char", "word", "none")
- `start`, `end`: Selection range

**Expected output fields:**
- `line_count`: Number of virtual lines after wrapping
- `selected`: Selected text content

### `input` - Input parsing

Tests for terminal input sequence parsing.

**Input fields:**
- `bytes`: Array of raw bytes (e.g., `[27, 91, 65]` for ESC[A)

**Expected output fields:**
- `event_type`: "key", "mouse", "focus", "paste"
- `key_code`: Key identifier (e.g., "Up", "Char", "F1")
- `char`: Character for Char key events
- `modifiers`: Array of modifier strings ("Ctrl", "Shift", "Alt")
- `button`: Mouse button ("Left", "Right", "Middle")
- `kind`: Mouse event kind ("Press", "Release", "Move", "ScrollUp", etc.)
- `x`, `y`: Mouse coordinates
- `gained`: Boolean for focus events
- `content`: Paste content string

### `ansi` - ANSI sequence generation

Tests for ANSI escape sequence output.

**Input fields:**
- `row`, `col`: Cursor position
- `r`, `g`, `b`: RGB values for color
- `is_bg`: Boolean for background color
- `index`: Palette index
- `mode`: Color mode ("true_color", "256_color", "16_color")
- `attributes`: Array of attribute names

**Expected output fields:**
- `sequence`: Expected ANSI escape sequence
- `contains`: Array of substrings that must be present

### `unicode` - Unicode handling

Tests for grapheme segmentation and width calculation.

**Input fields:**
- `text`: Unicode text to analyze

**Expected output fields:**
- `grapheme_count`: Number of grapheme clusters
- `display_width`: Terminal display width

### `grapheme` - GraphemePool operations (planned)

Tests for grapheme pool allocation and deduplication.

**Input fields:**
- `graphemes`: Array of grapheme strings to intern
- `operations`: Sequence of alloc/free operations

**Expected output fields:**
- `slot_count`: Number of allocated slots
- `dedup_count`: Number of deduplicated entries
- `grapheme_ids`: Array of assigned IDs

## Naming Convention

Test names follow the pattern: `{category}_{operation}_{variant}`

Examples:
- `rgba_blend_half_red_over_blue`
- `buffer_draw_box_title`
- `input_parse_arrow_up`
- `unicode_width_cjk`

## Notes Field

Test cases may include an optional `note` field in `expected_output` for human-readable context. This field is ignored during test execution.

```json
{
  "expected_output": {
    "index": 1,
    "note": "Pure red maps to dark red (1) not bright red (9)"
  }
}
```
