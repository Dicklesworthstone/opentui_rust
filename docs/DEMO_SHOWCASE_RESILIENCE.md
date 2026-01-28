# demo_showcase Resilience & Degradation Rules

> **Version:** 1.0
> **Status:** Draft
> **Bead:** bd-1i7

This document defines how `demo_showcase` gracefully handles constrained
environments while remaining impressive in ideal terminals.

---

## 1. Design Principles

1. **Never embarrassing** — Demo should look good even in worst-case terminals
2. **Fail gracefully** — Clear error messages, clean exit on fatal conditions
3. **Progressive enhancement** — Best experience on capable terminals, functional on limited ones
4. **Always usable** — Core functionality accessible regardless of capabilities

---

## 2. Capability Detection

### 2.1 Detected Capabilities

```rust
pub struct TerminalCapabilities {
    // Display
    pub size: Option<(u16, u16)>,      // Terminal dimensions
    pub is_tty: bool,                   // Connected to a TTY
    pub color_support: ColorSupport,    // Color depth

    // Features
    pub has_mouse: bool,                // Mouse reporting available
    pub has_synchronized_output: bool,  // DE/CSI 2026 support
    pub has_bracketed_paste: bool,      // Paste mode support
    pub has_focus_events: bool,         // Focus tracking support

    // Text attributes
    pub has_italic: bool,               // Italic text support
    pub has_strikethrough: bool,        // Strikethrough support
    pub has_hyperlinks: bool,           // OSC 8 hyperlink support
}
```

### 2.2 Color Support Levels

```rust
pub enum ColorSupport {
    TrueColor,   // 24-bit (16M colors)
    Extended,    // 256-color palette
    Basic,       // 16-color ANSI
    None,        // Monochrome
}
```

### 2.3 Detection Methods

| Capability | Detection Method |
|------------|------------------|
| TTY | `libc::isatty(stdout)` |
| Size | `ioctl TIOCGWINSZ` or `$COLUMNS`/`$LINES` |
| TrueColor | `$COLORTERM == "truecolor"` or `"24bit"` |
| 256-color | `$TERM` contains "256" |
| Mouse | Assume yes unless `--no-mouse` flag |
| Sync output | Query via CSI, timeout fallback |

---

## 3. Layout Degradation

### 3.1 Size Thresholds

| Layout Mode | Min Width | Min Height |
|-------------|-----------|------------|
| Full | 80 | 24 |
| Compact | 60 | 16 |
| Minimal | 40 | 12 |
| Too Small | < 40 | < 12 |

### 3.2 Full Layout (80+ x 24+)

Standard layout as defined in DEMO_SHOWCASE_SPEC.md:
- TopBar + Sidebar + Editor + Preview + StatusBar

### 3.3 Compact Layout (60-79 x 16-23)

```
┌────────────────────────────────────────────────────────────┐
│  OpenTUI            Mode: Normal               22:05       │
├──────┬─────────────────────────────────────────────────────┤
│ [O]  │  1 │ fn main() {                                    │
│ [E]  │  2 │     println!("hello");                         │
│ [P]  │  3 │ }                                              │
│ [L]  │  4 │                                                │
│ [U]  │  5 │                                                │
│ [F]  │  6 │                                                │
├──────┴─────────────────────────────────────────────────────┤
│  F1 Help  Ctrl+Q Quit                          FPS:60      │
└────────────────────────────────────────────────────────────┘
```

Changes from full layout:
- Sidebar collapses to icons only (3 columns)
- Preview panel hidden (merged into editor area)
- TopBar simplified (no project name)
- StatusBar abbreviated

### 3.4 Minimal Layout (40-59 x 12-15)

```
┌──────────────────────────────────────┐
│ OpenTUI                  Mode:Normal │
├──────────────────────────────────────┤
│  fn main() {                         │
│      println!("hello");              │
│  }                                   │
│                                      │
├──────────────────────────────────────┤
│ F1 Help  Ctrl+Q Quit                 │
└──────────────────────────────────────┘
```

Changes from compact layout:
- Sidebar completely hidden
- Single panel fills screen
- Tab to switch panels (section shown in title)
- Minimal chrome

### 3.5 Too Small (< 40 x 12)

Display error message and exit:

```
┌─────────────────────────┐
│ Terminal too small!     │
│ Need at least 40x12     │
│ Current: 30x10          │
│                         │
│ Press any key to exit   │
└─────────────────────────┘
```

### 3.6 Layout Transition Rules

```rust
fn compute_layout_mode(width: u16, height: u16) -> LayoutMode {
    if width < 40 || height < 12 {
        LayoutMode::TooSmall
    } else if width < 60 || height < 16 {
        LayoutMode::Minimal
    } else if width < 80 || height < 24 {
        LayoutMode::Compact
    } else {
        LayoutMode::Full
    }
}
```

### 3.7 Resize Handling

When terminal resizes:
1. Detect new size via SIGWINCH or resize event
2. Recompute layout mode
3. If mode changes, trigger full redraw
4. Preserve scroll positions and selection where possible

---

## 4. Color Degradation

### 4.1 Strategy by Color Support

| Support | Strategy |
|---------|----------|
| TrueColor | Use exact theme colors |
| Extended (256) | Map to nearest 256-color |
| Basic (16) | Use named ANSI colors |
| None | Use bold/dim/inverse only |

### 4.2 256-Color Mapping

```rust
fn map_to_256(rgba: Rgba) -> u8 {
    let r = (rgba.r * 5.0).round() as u8;
    let g = (rgba.g * 5.0).round() as u8;
    let b = (rgba.b * 5.0).round() as u8;
    16 + 36 * r + 6 * g + b
}
```

### 4.3 16-Color Mapping

Map theme colors to closest ANSI equivalents:

| Theme Token | ANSI Fallback |
|-------------|---------------|
| `bg0/bg1` | Black (30/40) |
| `bg2` | Bright Black (90) |
| `fg0` | White (97) |
| `fg1` | White (37) |
| `fg2` | Bright Black (90) |
| `accent_primary` | Cyan (36) |
| `accent_secondary` | Magenta (35) |
| `accent_success` | Green (32) |
| `accent_warning` | Yellow (33) |
| `accent_error` | Red (31) |

### 4.4 Monochrome Fallback

When no colors available:
- Use **bold** for emphasis (headings, focus)
- Use **dim** for de-emphasis (secondary text)
- Use **inverse** for selection/focus
- Use **underline** for links

### 4.5 Auto Theme Selection

```rust
fn default_theme_for_capability(color: ColorSupport) -> ThemeId {
    match color {
        ColorSupport::TrueColor | ColorSupport::Extended => ThemeId::Synthwave,
        ColorSupport::Basic | ColorSupport::None => ThemeId::HighContrast,
    }
}
```

---

## 5. Text Attribute Degradation

### 5.1 Attribute Availability Matrix

| Attribute | Wide Support | Fallback |
|-----------|-------------|----------|
| Bold | Yes | Always use |
| Dim | Mostly | Use normal |
| Underline | Yes | Always use |
| Italic | Partial | Use normal |
| Strikethrough | Rare | Use dim or normal |
| Blink | Rare | Ignore |
| Inverse | Yes | Always use |

### 5.2 Feature Check at Runtime

```rust
fn apply_text_attr(attr: TextAttribute, caps: &TerminalCapabilities) -> TextAttributes {
    match attr {
        TextAttribute::Bold => TextAttributes::BOLD,
        TextAttribute::Italic => {
            if caps.has_italic {
                TextAttributes::ITALIC
            } else {
                TextAttributes::empty()
            }
        }
        TextAttribute::Strikethrough => {
            if caps.has_strikethrough {
                TextAttributes::STRIKETHROUGH
            } else {
                TextAttributes::DIM
            }
        }
        // ... etc
    }
}
```

---

## 6. Input Degradation

### 6.1 Mouse Disabled Fallback

When mouse is unavailable or disabled (`--no-mouse`):

| Mouse Action | Keyboard Equivalent |
|--------------|---------------------|
| Click panel | Tab to panel |
| Click sidebar item | Arrow keys + Enter |
| Scroll | PageUp/PageDown, Arrow keys |
| Hover preview | Not available |
| Click button | Tab + Enter |

### 6.2 Keyboard-Only Navigation

All functionality must be accessible via keyboard:

| Action | Key |
|--------|-----|
| Cycle focus | Tab / Shift+Tab |
| Navigate in panel | Arrow keys |
| Activate selection | Enter / Space |
| Open command palette | Ctrl+P |
| Open help | F1 |
| Switch section | 1-6 number keys |
| Quit | Ctrl+Q / Esc |

### 6.3 Focus Visibility

When mouse is disabled, focus visibility becomes critical:
- Always show clear focus indicator
- Use contrasting colors for focused element
- Show keyboard hints in status bar

---

## 7. Output Degradation

### 7.1 Synchronized Output

If synchronized output (`CSI ? 2026 h/l`) is unsupported:
- Fall back to buffered writes
- May experience slight flicker on complex updates
- Consider reducing update frequency

### 7.2 Alternate Screen

If alternate screen is unsupported:
- Continue in main screen
- Clear screen on exit
- May interfere with scrollback

### 7.3 Cursor Control

If cursor hiding fails:
- Continue with visible cursor
- Position cursor at a sensible location (end of status bar)

---

## 8. Non-TTY Behavior

### 8.1 Detection

```rust
fn is_tty() -> bool {
    unsafe { libc::isatty(libc::STDOUT_FILENO) != 0 }
}
```

### 8.2 Interactive Mode Failure

When stdout is not a TTY and no headless mode:

```
Error: stdout is not a terminal

demo_showcase requires an interactive terminal to run.
For non-interactive use, try: demo_showcase --headless-smoke

Exit code: 1
```

### 8.3 Headless Smoke Mode (`--headless-smoke`)

A special mode for CI testing:

1. Create renderer in headless mode (no terminal I/O)
2. Run deterministic render steps to internal buffer
3. Verify no panics, proper cleanup
4. Exit with code 0 if successful

```rust
fn run_headless_smoke() -> io::Result<()> {
    let mut buffer = OptimizedBuffer::new(80, 24);

    // Run through all sections
    for section in Section::all() {
        draw_section(&mut buffer, section);
    }

    // Verify buffer is valid
    assert!(buffer.width() == 80);
    assert!(buffer.height() == 24);

    eprintln!("Headless smoke test passed");
    Ok(())
}
```

---

## 9. Error Handling

### 9.1 Fatal Errors

| Condition | Behavior |
|-----------|----------|
| No TTY (without headless) | Print error, exit 1 |
| Terminal too small | Show message, exit 1 |
| Renderer init failure | Print error, exit 1 |
| Raw mode failure | Print error, exit 1 |

### 9.2 Recoverable Errors

| Condition | Behavior |
|-----------|----------|
| Resize to too-small | Show "too small" message, wait for resize |
| Input parse error | Ignore and continue |
| Single frame render failure | Log and continue |
| Color query timeout | Assume TrueColor |

### 9.3 Panic Recovery

Install panic hook to restore terminal:

```rust
fn install_panic_hook() {
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Restore terminal first
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, Show);

        // Then call original hook
        original(info);
    }));
}
```

---

## 10. CLI Flags for Degradation Control

| Flag | Effect |
|------|--------|
| `--no-mouse` | Disable mouse input |
| `--no-color` | Force monochrome mode |
| `--16-color` | Force 16-color mode |
| `--256-color` | Force 256-color mode |
| `--no-sync-output` | Disable synchronized output |
| `--headless-smoke` | Run headless smoke test |
| `--compact` | Force compact layout |
| `--minimal` | Force minimal layout |

---

## 11. Acceptance Criteria Checklist

- [x] **Compact layout rules defined** — Section 3 defines Full/Compact/Minimal/TooSmall modes
- [x] **Color fallback rules defined** — Section 4 defines degradation for 256/16/mono
- [x] **Mouse-disabled fallback rules defined** — Section 6 defines keyboard-only navigation
- [x] **Non-TTY behavior defined** — Section 8 defines error message and headless mode

---

## 12. Related Beads

| Bead | Dependency | Description |
|------|------------|-------------|
| bd-1i7 | This bead | Resilience + degradation rules |
| bd-1ob | Blocked by this | Resilience implementation |
| bd-3ii | Blocked by this | Layout helpers implementation |
| bd-2iv | Blocked by this | CLI args + Config |
