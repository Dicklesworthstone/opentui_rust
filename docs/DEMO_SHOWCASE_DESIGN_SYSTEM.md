# demo_showcase Design System

> **Version:** 1.0
> **Status:** Draft
> **Bead:** bd-1ok

This document defines the visual design system for `demo_showcase` — a consistent,
modern visual language that makes OpenTUI feel premium.

---

## 1. Design Principles

1. **Consistent spacing rhythm** — All measurements follow a 1-unit grid
2. **Crisp borders** — Unicode box drawing with clear contrast
3. **Tasteful gradients** — Subtle 5-10% perceived change
4. **Glassy overlays** — Alpha-blended modal backgrounds
5. **Clear focus/selection** — Obvious visual feedback for interactive elements

---

## 2. Theme System

The demo showcases **theme switching** as a first-class feature. Four themes are provided:

### 2.1 Theme Registry

```rust
pub enum ThemeId {
    Synthwave,    // Default dark theme
    Paper,        // Light theme
    Solarized,    // Low eye-strain
    HighContrast, // Accessibility / worst-case terminals
}
```

### 2.2 Theme Structure

```rust
pub struct Theme {
    pub id: ThemeId,
    pub name: &'static str,

    // Background layers
    pub bg0: Rgba,  // App background
    pub bg1: Rgba,  // Panel background
    pub bg2: Rgba,  // Raised surfaces / cards

    // Text colors
    pub fg0: Rgba,  // Primary text
    pub fg1: Rgba,  // Secondary text
    pub fg2: Rgba,  // Dim / disabled text

    // Accent colors
    pub accent_primary: Rgba,   // Main accent (focus, links)
    pub accent_secondary: Rgba, // Secondary accent (highlights)
    pub accent_success: Rgba,   // Success / positive
    pub accent_warning: Rgba,   // Warning / attention
    pub accent_error: Rgba,     // Error / destructive

    // Selection & Focus
    pub selection_bg: Rgba,     // Selection background (low alpha)
    pub focus_border: Rgba,     // Focus indicator color

    // Syntax highlighting theme
    pub syntax: SyntaxTheme,
}
```

---

## 3. Theme Definitions

### 3.1 Synthwave Professional (Default Dark)

The default theme — a modern dark aesthetic with neon accents.

| Token | Color | Hex |
|-------|-------|-----|
| `bg0` | App background | `#0f1220` |
| `bg1` | Panel background | `#151a2e` |
| `bg2` | Raised surfaces | `#1d2440` |
| `fg0` | Primary text | `#e6e6e6` |
| `fg1` | Secondary text | `#aeb6d6` |
| `fg2` | Dim text | `#6c7396` |
| `accent_primary` | Aqua | `#4dd6ff` |
| `accent_secondary` | Pink | `#ff4fd8` |
| `accent_success` | Green | `#2bff88` |
| `accent_warning` | Orange | `#ffb020` |
| `accent_error` | Red | `#ff4455` |
| `selection_bg` | Selection | `#2a335c` (30% opacity) |
| `focus_border` | Focus | `#4dd6ff` |

```rust
pub const SYNTHWAVE: Theme = Theme {
    id: ThemeId::Synthwave,
    name: "Synthwave Professional",
    bg0: Rgba::from_hex_const("#0f1220"),
    bg1: Rgba::from_hex_const("#151a2e"),
    bg2: Rgba::from_hex_const("#1d2440"),
    fg0: Rgba::from_hex_const("#e6e6e6"),
    fg1: Rgba::from_hex_const("#aeb6d6"),
    fg2: Rgba::from_hex_const("#6c7396"),
    accent_primary: Rgba::from_hex_const("#4dd6ff"),
    accent_secondary: Rgba::from_hex_const("#ff4fd8"),
    accent_success: Rgba::from_hex_const("#2bff88"),
    accent_warning: Rgba::from_hex_const("#ffb020"),
    accent_error: Rgba::from_hex_const("#ff4455"),
    selection_bg: Rgba::from_hex_const("#2a335c"),
    focus_border: Rgba::from_hex_const("#4dd6ff"),
    syntax: SyntaxTheme::OneDark,
};
```

### 3.2 Paper (Light Theme)

A clean, paper-like light theme for daytime use.

| Token | Color | Hex |
|-------|-------|-----|
| `bg0` | App background | `#f7f7fb` |
| `bg1` | Panel background | `#ffffff` |
| `bg2` | Raised surfaces | `#eef0f7` |
| `fg0` | Primary text | `#1a1b26` |
| `fg1` | Secondary text | `#3a3f5a` |
| `fg2` | Dim text | `#6a6f8a` |
| `accent_primary` | Blue | `#2a6fff` |
| `accent_secondary` | Purple | `#7b61ff` |
| `accent_success` | Green | `#00a86b` |
| `accent_warning` | Orange | `#ff8a00` |
| `accent_error` | Red | `#e53935` |
| `selection_bg` | Selection | `#dbe6ff` (30% opacity) |
| `focus_border` | Focus | `#2a6fff` |

```rust
pub const PAPER: Theme = Theme {
    id: ThemeId::Paper,
    name: "Paper",
    bg0: Rgba::from_hex_const("#f7f7fb"),
    bg1: Rgba::from_hex_const("#ffffff"),
    bg2: Rgba::from_hex_const("#eef0f7"),
    fg0: Rgba::from_hex_const("#1a1b26"),
    fg1: Rgba::from_hex_const("#3a3f5a"),
    fg2: Rgba::from_hex_const("#6a6f8a"),
    accent_primary: Rgba::from_hex_const("#2a6fff"),
    accent_secondary: Rgba::from_hex_const("#7b61ff"),
    accent_success: Rgba::from_hex_const("#00a86b"),
    accent_warning: Rgba::from_hex_const("#ff8a00"),
    accent_error: Rgba::from_hex_const("#e53935"),
    selection_bg: Rgba::from_hex_const("#dbe6ff"),
    focus_border: Rgba::from_hex_const("#2a6fff"),
    syntax: SyntaxTheme::OneLight,
};
```

### 3.3 Solarized (Low Eye Strain)

A Solarized-inspired theme for extended coding sessions.

| Token | Color | Hex |
|-------|-------|-----|
| `bg0` | App background | `#002b36` |
| `bg1` | Panel background | `#073642` |
| `bg2` | Raised surfaces | `#0b4452` |
| `fg0` | Primary text | `#eee8d5` |
| `fg1` | Secondary text | `#93a1a1` |
| `fg2` | Dim text | `#657b83` |
| `accent_primary` | Cyan | `#2aa198` |
| `accent_secondary` | Yellow | `#b58900` |
| `accent_success` | Green | `#859900` |
| `accent_warning` | Orange | `#cb4b16` |
| `accent_error` | Red | `#dc322f` |
| `selection_bg` | Selection | `#0d5161` (30% opacity) |
| `focus_border` | Focus | `#2aa198` |

```rust
pub const SOLARIZED: Theme = Theme {
    id: ThemeId::Solarized,
    name: "Solarized",
    bg0: Rgba::from_hex_const("#002b36"),
    bg1: Rgba::from_hex_const("#073642"),
    bg2: Rgba::from_hex_const("#0b4452"),
    fg0: Rgba::from_hex_const("#eee8d5"),
    fg1: Rgba::from_hex_const("#93a1a1"),
    fg2: Rgba::from_hex_const("#657b83"),
    accent_primary: Rgba::from_hex_const("#2aa198"),
    accent_secondary: Rgba::from_hex_const("#b58900"),
    accent_success: Rgba::from_hex_const("#859900"),
    accent_warning: Rgba::from_hex_const("#cb4b16"),
    accent_error: Rgba::from_hex_const("#dc322f"),
    selection_bg: Rgba::from_hex_const("#0d5161"),
    focus_border: Rgba::from_hex_const("#2aa198"),
    syntax: SyntaxTheme::Solarized,
};
```

### 3.4 High Contrast (Accessibility)

Maximum contrast for accessibility and low-capability terminals.

| Token | Color | Hex |
|-------|-------|-----|
| `bg0` | App background | `#000000` |
| `bg1` | Panel background | `#000000` |
| `bg2` | Raised surfaces | `#111111` |
| `fg0` | Primary text | `#ffffff` |
| `fg1` | Secondary text | `#e0e0e0` |
| `fg2` | Dim text | `#a0a0a0` |
| `accent_primary` | Cyan | `#00ffff` |
| `accent_secondary` | Magenta | `#ff00ff` |
| `accent_success` | Green | `#00ff00` |
| `accent_warning` | Yellow | `#ffff00` |
| `accent_error` | Red | `#ff0000` |
| `selection_bg` | Selection | `#333333` |
| `focus_border` | Focus | `#ffff00` |

```rust
pub const HIGH_CONTRAST: Theme = Theme {
    id: ThemeId::HighContrast,
    name: "High Contrast",
    bg0: Rgba::from_hex_const("#000000"),
    bg1: Rgba::from_hex_const("#000000"),
    bg2: Rgba::from_hex_const("#111111"),
    fg0: Rgba::from_hex_const("#ffffff"),
    fg1: Rgba::from_hex_const("#e0e0e0"),
    fg2: Rgba::from_hex_const("#a0a0a0"),
    accent_primary: Rgba::from_hex_const("#00ffff"),
    accent_secondary: Rgba::from_hex_const("#ff00ff"),
    accent_success: Rgba::from_hex_const("#00ff00"),
    accent_warning: Rgba::from_hex_const("#ffff00"),
    accent_error: Rgba::from_hex_const("#ff0000"),
    selection_bg: Rgba::from_hex_const("#333333"),
    focus_border: Rgba::from_hex_const("#ffff00"),
    syntax: SyntaxTheme::HighContrast,
};
```

---

## 4. Component Styling

### 4.1 Panel Borders

| State | Style |
|-------|-------|
| Normal | `fg2` color, thin box (`─`, `│`, `┌`, etc.) |
| Focused | `focus_border` color, bold, optional glow line |
| Disabled | `fg2` at 50% opacity |

```rust
fn panel_border_style(theme: &Theme, focused: bool) -> Style {
    if focused {
        Style::fg(theme.focus_border).with_bold()
    } else {
        Style::fg(theme.fg2)
    }
}
```

### 4.2 Headers

Headers use a gradient bar with bold title:

```
┌──────────── Editor ────────────┐
```

- Background: Gradient from `bg2` to `bg1` (left to right)
- Title: `fg0` with bold
- Border: Rounded or heavy box drawing

### 4.3 Status Bar

- Background: `bg2`
- Key hints: `fg2` normal, keys in `fg0` bold
- Stats: `fg2` right-aligned

Example:
```
  F1 Help  Ctrl+P Command  Ctrl+Q Quit                    FPS:60  Cells:8640
```

### 4.4 Toasts / Notifications

- Background: `bg2` with rounded corners
- Border: `accent_primary` or context-dependent (success/warning/error)
- Text: `fg0`
- Opacity stack: Start at 100%, fade to 0% over 3 seconds

```rust
fn toast_style(theme: &Theme, kind: ToastKind) -> (Rgba, Rgba) {
    let border = match kind {
        ToastKind::Info => theme.accent_primary,
        ToastKind::Success => theme.accent_success,
        ToastKind::Warning => theme.accent_warning,
        ToastKind::Error => theme.accent_error,
    };
    (theme.bg2, border)
}
```

### 4.5 Selection Highlighting

For text selection in the editor:

- Background: `selection_bg` with low alpha (30-40%)
- Foreground: Unchanged (text color preserved)
- Apply via `set_blended()` to preserve underlying colors

### 4.6 Focus Indicators

| Element | Focus Style |
|---------|-------------|
| Panel border | `focus_border` + bold |
| List item | Inverted colors (fg/bg swap) |
| Button | `focus_border` underline |
| Input field | `focus_border` bottom border |

---

## 5. Typography & Characters

### 5.1 Box Drawing Characters

| Purpose | Characters |
|---------|------------|
| Standard panel | `┌ ─ ┐ │ └ ─ ┘` |
| Rounded panel | `╭ ─ ╮ │ ╰ ─ ╯` |
| Heavy panel | `┏ ━ ┓ ┃ ┗ ━ ┛` |
| Double panel | `╔ ═ ╗ ║ ╚ ═ ╝` |
| Separator | `─` (horizontal), `│` (vertical) |
| Bullet | `•` or `◦` |
| Arrow | `▶` `▼` `◀` `▲` |
| Checkbox | `☐` (unchecked), `☑` (checked) |
| Radio | `○` (unselected), `●` (selected) |

### 5.2 Iconography

Keep to single-codepoint symbols for stable width:

| Concept | Symbol |
|---------|--------|
| File | `📄` or `□` |
| Folder | `📁` or `▣` |
| Edit | `✎` |
| Save | `💾` or `▪` |
| Search | `🔍` or `◉` |
| Settings | `⚙` |
| Help | `?` |
| Close | `✕` |
| Check | `✓` |
| Error | `✗` |
| Warning | `⚠` |
| Info | `ℹ` |

### 5.3 Text Styles

| Purpose | Style |
|---------|-------|
| Heading | Bold |
| Emphasis | Italic (if supported) |
| Code | Dim background |
| Link | Underline + `accent_primary` |
| Dim | `fg2` color |
| Error | `accent_error` color |
| Success | `accent_success` color |

---

## 6. Spacing System

### 6.1 Grid Unit

The base grid unit is **1 character cell**. All spacing is measured in cells.

### 6.2 Spacing Values

| Name | Value | Usage |
|------|-------|-------|
| `xs` | 0 | No spacing |
| `sm` | 1 | Tight spacing (between related items) |
| `md` | 2 | Default spacing (between sections) |
| `lg` | 3 | Generous spacing (between major areas) |
| `xl` | 4 | Large gaps (margins) |

### 6.3 Panel Padding

- Inner padding: `sm` (1 cell) from border to content
- Outer margin: `md` (2 cells) between panels

### 6.4 List Item Spacing

- Vertical: 0 (continuous)
- Left indent: `sm` (1) from border
- Icon-to-text: `sm` (1)

---

## 7. Gradient Rules

### 7.1 Principles

- Gradients should be **subtle**: 5-10% perceived brightness change
- Use `Rgba::lerp()` between two close hues
- Horizontal gradients for headers
- Vertical gradients for tall panels (optional)

### 7.2 Implementation

```rust
fn draw_gradient_bar(
    buffer: &mut OptimizedBuffer,
    x: u32, y: u32, width: u32,
    start: Rgba, end: Rgba,
) {
    for i in 0..width {
        let t = f32::from(i) / f32::from(width.saturating_sub(1).max(1));
        let color = start.lerp(end, t);
        buffer.set(x + i, y, Cell::new(' ', Style::bg(color)));
    }
}
```

### 7.3 Gradient Pairs (per Theme)

| Theme | Header Gradient |
|-------|----------------|
| Synthwave | `#1d2440` → `#151a2e` |
| Paper | `#eef0f7` → `#ffffff` |
| Solarized | `#0b4452` → `#073642` |
| High Contrast | `#111111` → `#000000` |

---

## 8. Theme Switching

### 8.1 Switching Mechanism

Theme switching changes BOTH:
1. UI palette/token set
2. Syntax highlighting theme

```rust
fn switch_theme(&mut self, new_theme: ThemeId) {
    self.current_theme = get_theme(new_theme);
    self.editor.set_syntax_theme(self.current_theme.syntax);
    self.needs_full_redraw = true;
}
```

### 8.2 Access Points

| Method | Location |
|--------|----------|
| Command palette | "Switch Theme" → theme list |
| Keyboard shortcut | Ctrl+Shift+T (cycle) |
| Number keys in Help | 1-4 for quick switch (discoverable) |

### 8.3 Persistence

Theme preference should be remembered:
- Store in config file (if implemented)
- Default to Synthwave on first run

---

## 9. Degradation Rules

### 9.1 Color Capability Detection

```rust
enum ColorCapability {
    TrueColor,     // 24-bit color
    Extended256,   // 256-color palette
    Basic16,       // 16-color ANSI
    NoColor,       // Monochrome
}
```

### 9.2 Color Mapping

| Capability | Strategy |
|------------|----------|
| TrueColor | Use exact theme colors |
| Extended256 | Map to nearest 256-color |
| Basic16 | Map to nearest 16-color ANSI |
| NoColor | Use bold/dim/inverse for contrast |

```rust
fn map_to_256(color: Rgba) -> u8 {
    // Use standard 6x6x6 color cube mapping
    let r = (color.r * 5.0).round() as u8;
    let g = (color.g * 5.0).round() as u8;
    let b = (color.b * 5.0).round() as u8;
    16 + 36 * r + 6 * g + b
}
```

### 9.3 Attribute Fallbacks

| Attribute | If Unsupported |
|-----------|---------------|
| Italic | Use normal (don't fail) |
| Strikethrough | Use dim (don't fail) |
| Underline | Usually supported |
| Bold | Usually supported |
| Dim | Fall back to normal |

### 9.4 High Contrast Auto-Switch

If terminal reports limited colors, consider auto-switching to High Contrast theme:

```rust
fn select_default_theme(caps: &TerminalCapabilities) -> ThemeId {
    match caps.color_support {
        ColorSupport::TrueColor | ColorSupport::Extended => ThemeId::Synthwave,
        ColorSupport::Basic | ColorSupport::None => ThemeId::HighContrast,
    }
}
```

---

## 10. Semantic Color Tokens

For consistent meaning across the UI:

| Token | Usage | Synthwave | Paper |
|-------|-------|-----------|-------|
| `semantic_info` | Informational messages | `accent_primary` | `accent_primary` |
| `semantic_success` | Success states | `accent_success` | `accent_success` |
| `semantic_warning` | Warnings | `accent_warning` | `accent_warning` |
| `semantic_error` | Errors | `accent_error` | `accent_error` |
| `semantic_link` | Hyperlinks | `accent_primary` | `accent_primary` |
| `semantic_muted` | De-emphasized | `fg2` | `fg2` |

---

## 11. Acceptance Criteria Checklist

- [x] **Palette tokens defined** — Section 3 defines all BG/FG/Accent colors for 4 themes
- [x] **Focus/selection styling rules defined** — Sections 4.5, 4.6 define selection and focus
- [x] **Component styling rules defined** — Section 4 defines borders, headers, status, toasts
- [x] **Gradient rules defined** — Section 7 defines gradient principles and implementation
- [x] **Degradation rules defined** — Section 9 defines color capability fallbacks

---

## 12. Related Beads

| Bead | Dependency | Description |
|------|------------|-------------|
| bd-1ok | This bead | Visual design system specification |
| bd-1pd | Blocked by this | Implement design system in code |
| bd-1i7 | Blocked by this | Resilience + degradation rules |
| bd-po1 | Blocked by this | Render pass orchestration |
