# Golden File Test Cases

This directory contains golden files for visual regression testing of OpenTUI rendering output.

## Purpose

Golden files capture expected ANSI output from buffer rendering operations. When tests run, they compare the actual output against these files to detect visual regressions.

## File Format

Each `.golden` file has a metadata header followed by raw ANSI output:

```
# Golden file: test_name
# Generated: YYYY-MM-DD
# Terminal: xterm-256color
# Size: WIDTHxHEIGHT
---
<raw ANSI escape sequences and text>
```

## Test Categories

### Basic Rendering (5 files)
- `empty_buffer_80x24.golden` - Empty buffer render
- `single_char_center.golden` - One character in center
- `full_screen_text.golden` - Checkered pattern filling screen
- `box_single_line.golden` - Single-line box drawing
- `box_double_line.golden` - Double-line box drawing

### Color Rendering (5 files)
- `truecolor_gradient.golden` - RGB gradient
- `color256_palette.golden` - 256-color mode palette
- `color16_palette.golden` - 16-color mode palette
- `bold_colors.golden` - Bold attribute effects
- `dim_colors.golden` - Dim attribute effects

### Complex Rendering (5 files)
- `alpha_blend_50.golden` - 50% alpha overlay
- `scissor_clipped.golden` - Content clipped by scissor
- `nested_scissor.golden` - Nested scissor regions
- `opacity_stack.golden` - Stacked opacity effects
- `wide_chars_cjk.golden` - CJK wide characters

### Unicode (5 files)
- `emoji_basic.golden` - Basic emoji (smileys, animals, food)
- `emoji_zwj.golden` - ZWJ sequences (family, flags, professions)
- `combining_marks.golden` - Combining diacriticals
- `rtl_text.golden` - Right-to-left text (Arabic, Hebrew)
- `mixed_width.golden` - Mixed ASCII and wide characters

### Demo Showcase (5 files)
- `tour_screen_1.golden` - Welcome tour screen
- `tour_screen_5.golden` - Alpha blending demo screen
- `help_overlay.golden` - F1 help overlay
- `debug_panel.golden` - Ctrl+D debug panel
- `command_palette.golden` - Ctrl+P command palette

## Updating Golden Files

To regenerate all golden files (e.g., after intentional visual changes):

```bash
GOLDEN_UPDATE=1 cargo test --test golden_rendering
# or
BLESS=1 cargo test --test golden_rendering
```

## Adding New Tests

1. Add a test function in `tests/golden_rendering.rs`
2. Use `render_buffer_to_ansi()` to capture output
3. Use `assert_golden("test_name", &output, width, height)` to compare/create
4. Run with `GOLDEN_UPDATE=1` on first run to create the golden file
5. Review the generated `.golden` file to ensure correctness
6. Commit the new golden file to source control

## CI Integration

In CI, golden file tests fail if:
- Actual output differs from golden file
- Golden file is missing (first run should create it locally)

Review any golden file changes carefully in pull requests as they represent visual changes to the rendering output.
