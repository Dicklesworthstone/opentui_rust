# demo_showcase Tour Script Specification

> **Version:** 1.0
> **Status:** Draft
> **Bead:** bd-1gy

This document defines the deterministic tour script for `demo_showcase` — an automated demonstration sequence that proves out OpenTUI's rendering capabilities.

---

## 1. Purpose

### 1.1 Why Tour Mode Exists

The showcase must be **demoable without a human driver**. Tour mode provides:

1. **Deterministic sequence** for recording videos and GIFs
2. **Guided explanation** overlay for first-time users
3. **Automatic prove-out** of advanced features
4. **CI integration** via `--exit-after-tour` for E2E testing

### 1.2 Design Goals

- Every step demonstrates at least one OpenTUI capability
- Timing is fixed and reproducible across runs
- Animations derive from step index (no randomness)
- Explanatory micro-copy accompanies each step

---

## 2. Tour Model

### 2.1 Step Structure

```rust
pub struct TourStep {
    /// Step number (1-indexed for display).
    pub number: u8,

    /// Short title shown in tour HUD.
    pub title: &'static str,

    /// Longer description explaining what's happening and why.
    pub description: &'static str,

    /// Duration to hold this step (milliseconds).
    pub duration_ms: u64,

    /// Action to execute when step activates.
    pub action: TourAction,

    /// Optional highlight region (panel or element to spotlight).
    pub spotlight: Option<SpotlightTarget>,
}
```

### 2.2 Tour Actions

```rust
pub enum TourAction {
    /// No action, just display.
    None,

    /// Select a sidebar section.
    SelectSection(Section),

    /// Change focus to a panel.
    FocusPanel(Focus),

    /// Open/close an overlay.
    ToggleOverlay(AppMode),

    /// Switch UI theme.
    SwitchTheme(UiTheme),

    /// Inject synthetic input into editor.
    InjectEditorText(&'static str),

    /// Trigger undo/redo in editor.
    EditorUndo,
    EditorRedo,

    /// Inject a synthetic paste event.
    InjectPaste(&'static str),

    /// Scroll a panel by N lines.
    ScrollPanel { panel: Focus, delta: i16 },

    /// Show a toast message.
    ShowToast(&'static str),
}
```

### 2.3 Spotlight Targets

```rust
pub enum SpotlightTarget {
    Panel(Focus),
    TopBar,
    StatusBar,
    SidebarItem(usize),
    EditorLine(usize),
    PreviewRegion,
    ToastArea,
}
```

### 2.4 Determinism Rules

1. **Fixed durations** — Each step has a predetermined `duration_ms`
2. **No randomness** — If animation variation is needed, derive from `step.number`
3. **Seeded animations** — Use `config.seed + step.number` for any procedural content
4. **Reproducible timing** — Tour plays identically across runs

---

## 3. Tour Steps

### Step 1: Welcome + No Flicker

| Property | Value |
|----------|-------|
| Title | "Welcome to OpenTUI" |
| Duration | 3000ms |
| Action | `None` |
| Spotlight | `None` |

**Description:**
> Watch the smooth animated accent bar. OpenTUI uses diff rendering and synchronized output (CSI 2026) to eliminate flicker — even at 60 FPS.

**Features demonstrated:**
- Frame pacing
- Synchronized output (if terminal supports)
- Gradient rendering

---

### Step 2: Scissor-Clipped Sidebar Scroll

| Property | Value |
|----------|-------|
| Title | "Clipped Scrolling" |
| Duration | 4000ms |
| Action | `ScrollPanel { panel: Sidebar, delta: 8 }` then `ScrollPanel { delta: -8 }` |
| Spotlight | `Panel(Sidebar)` |

**Description:**
> The sidebar scrolls within its bounds. Scissor clipping ensures content never bleeds into adjacent panels — a must for TUI layout correctness.

**Features demonstrated:**
- Scissor clipping (`Renderer::with_scissor`)
- Scroll indicators
- Focus borders

---

### Step 3: Focus + Hit Testing

| Property | Value |
|----------|-------|
| Title | "Focus & Hit Testing" |
| Duration | 4000ms |
| Action | Cycle through `FocusPanel(Sidebar)`, `FocusPanel(Editor)`, `FocusPanel(Preview)`, `FocusPanel(Logs)` |
| Spotlight | Current focused panel |

**Description:**
> Click any panel to focus it. The border glow follows focus. OpenTUI's HitGrid tracks clickable regions for mouse-driven UIs.

**Features demonstrated:**
- Focus indicators
- HitGrid registration
- Border styling with glow effect

---

### Step 4: Command Palette (Glass Overlay)

| Property | Value |
|----------|-------|
| Title | "Command Palette" |
| Duration | 3500ms |
| Action | `ToggleOverlay(CommandPalette)` |
| Spotlight | `None` (overlay captures attention) |

**Description:**
> Press Ctrl+P to open the command palette. The glass overlay demonstrates OpenTUI's alpha blending — background content shows through at reduced opacity.

**Features demonstrated:**
- Alpha blending / RGBA compositing
- Overlay rendering order
- Porter-Duff blending modes

---

### Step 5: Editor Typing + Undo/Redo

| Property | Value |
|----------|-------|
| Title | "Editor: Rope + Undo" |
| Duration | 5000ms |
| Action | `InjectEditorText("// Hello from OpenTUI!\n")`, wait, `EditorUndo`, wait, `EditorRedo` |
| Spotlight | `Panel(Editor)` |

**Description:**
> Type in the editor, then undo/redo. OpenTUI's text editing uses a rope data structure for efficient edits, with grouped undo for natural workflows.

**Features demonstrated:**
- Text editing primitives
- Rope data structure efficiency
- Undo/redo groups

---

### Step 6: Editor Highlighting + Theme Switch

| Property | Value |
|----------|-------|
| Title | "Syntax Highlighting" |
| Duration | 5000ms |
| Action | `SelectSection(Editor)`, then cycle `SwitchTheme(Synthwave)`, `SwitchTheme(Paper)`, `SwitchTheme(HighContrast)`, `SwitchTheme(Synthwave)` |
| Spotlight | `Panel(Editor)` |

**Description:**
> Watch the theme change in real-time. OpenTUI supports TrueColor (16M), 256-color, 16-color, and monochrome — with automatic fallback.

**Features demonstrated:**
- Theme system
- Color degradation (if not TrueColor)
- Style composition (bold, italic, colors)

---

### Step 7: Bracketed Paste Proof

| Property | Value |
|----------|-------|
| Title | "Bracketed Paste" |
| Duration | 3000ms |
| Action | `InjectPaste("fn hello() {\n    println!(\"world\");\n}")` |
| Spotlight | `Panel(Editor)` |

**Description:**
> Pasted text is detected and inserted atomically. OpenTUI parses bracketed paste (CSI 200~/201~) so multi-line pastes don't trigger editor commands.

**Features demonstrated:**
- Bracketed paste parsing
- Atomic text insertion
- Toast notification ("Pasted N chars")

---

### Step 8: Unicode + Width + Grapheme Pool

| Property | Value |
|----------|-------|
| Title | "Unicode Rendering" |
| Duration | 4000ms |
| Action | `SelectSection(Unicode)` |
| Spotlight | `Panel(Editor)` |

**Description:**
> CJK wide characters, emoji, and ZWJ sequences render correctly. OpenTUI's grapheme pool amortizes allocation for complex grapheme clusters.

**Features demonstrated:**
- Wide character handling (CJK)
- Emoji rendering (single and multi-codepoint)
- Grapheme pool efficiency
- UAX #29 grapheme clustering

**Sample content displayed:**
```
日本語テスト   (CJK wide)
🎉 🚀 ✨       (single-codepoint emoji)
👨‍👩‍👧‍👦 👩🏽‍💻   (ZWJ sequences)
```

---

### Step 9: Preview: PixelBuffer + Supersampling

| Property | Value |
|----------|-------|
| Title | "Pixel Art & Charts" |
| Duration | 4000ms |
| Action | `SelectSection(Preview)`, animate sparkline |
| Spotlight | `Panel(Preview)` |

**Description:**
> The preview panel renders pixel art and charts. PixelBuffer converts images to half-block characters (▀▄) with optional supersampling.

**Features demonstrated:**
- PixelBuffer rendering
- Half-block characters for 2:1 resolution
- Sparkline charts
- Animation frame pacing

---

### Step 10: Alpha Blending / Opacity

| Property | Value |
|----------|-------|
| Title | "Alpha Compositing" |
| Duration | 3500ms |
| Action | Show translucent overlay on preview |
| Spotlight | `Panel(Preview)` |

**Description:**
> A translucent modal floats over the preview. OpenTUI uses Porter-Duff Source-Over compositing for proper alpha blending.

**Features demonstrated:**
- RGBA alpha channel
- Porter-Duff blending
- Z-ordering / layer compositing

---

### Step 11: Hyperlinks (OSC 8)

| Property | Value |
|----------|-------|
| Title | "Clickable Hyperlinks" |
| Duration | 3500ms |
| Action | `SelectSection(Logs)`, `FocusPanel(Logs)` |
| Spotlight | `Panel(Logs)` |

**Description:**
> Links in the log panel are clickable. OpenTUI emits OSC 8 hyperlink sequences with proper URL escaping (no C1 control char injection).

**Features demonstrated:**
- OSC 8 hyperlink emission
- URL escaping (security)
- Link styling (underline, color)
- Hover/pressed feedback

---

### Step 12: Finale — Performance Overlay

| Property | Value |
|----------|-------|
| Title | "Performance Stats" |
| Duration | 4000ms |
| Action | `SelectSection(Perf)` |
| Spotlight | `Panel(Preview)` |

**Description:**
> The performance overlay shows real-time stats. Typical frames render in under 1ms with minimal cell changes thanks to diff rendering.

**Stats displayed:**
- FPS (actual vs target)
- Cells changed per frame
- Total buffer size
- Render time (μs)

**Closing message:**
> "You can build this. OpenTUI is <1000 lines for the core renderer."

---

## 4. User Controls

### 4.1 Starting Tour

| Trigger | Behavior |
|---------|----------|
| `Ctrl+T` | Toggle tour mode (start/resume if paused) |
| `--tour` flag | Start in tour mode immediately |

### 4.2 During Tour

| Key | Action |
|-----|--------|
| `Enter` / `Space` | Advance to next step |
| `Backspace` | Go back to previous step |
| `Esc` | Exit tour mode |
| `Home` | Restart tour from step 1 |
| `1`-`9` | Jump to step N (if valid) |

### 4.3 Unattended Mode

| Flag | Behavior |
|------|----------|
| `--exit-after-tour` | Exit process (code 0) when tour completes |
| `--tour --exit-after-tour` | Combined: start in tour, exit when done |

---

## 5. Tour HUD Design

### 5.1 Layout

```
┌─────────────────────────────────────────────────────────────┐
│  Tour: Step 5/12 - Editor: Rope + Undo                      │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━░░░░░░░░░░░░░░░░░░░░ (41%)       │
│                                                              │
│  Type in the editor, then undo/redo. OpenTUI's text editing │
│  uses a rope data structure for efficient edits.            │
│                                                              │
│  Enter: Next  |  Backspace: Prev  |  Esc: Exit              │
└─────────────────────────────────────────────────────────────┘
```

### 5.2 HUD Elements

| Element | Position | Content |
|---------|----------|---------|
| Title | Top | "Tour: Step N/12 - {step.title}" |
| Progress | Below title | Visual progress bar with percentage |
| Description | Center | Multi-line description text |
| Controls | Bottom | Key hints for navigation |

### 5.3 Spotlight Effect

When a spotlight target is set:
- Dim everything except the spotlit region (overlay with 50% opacity black)
- Draw a subtle glow around the spotlit element
- Animate the glow pulse (derived from step number)

---

## 6. Timing Budget

| Step | Duration | Cumulative |
|------|----------|------------|
| 1. Welcome | 3000ms | 3s |
| 2. Sidebar Scroll | 4000ms | 7s |
| 3. Focus + Hit | 4000ms | 11s |
| 4. Command Palette | 3500ms | 14.5s |
| 5. Editor Typing | 5000ms | 19.5s |
| 6. Theme Switch | 5000ms | 24.5s |
| 7. Bracketed Paste | 3000ms | 27.5s |
| 8. Unicode | 4000ms | 31.5s |
| 9. Preview/Charts | 4000ms | 35.5s |
| 10. Alpha Blend | 3500ms | 39s |
| 11. Hyperlinks | 3500ms | 42.5s |
| 12. Finale | 4000ms | 46.5s |

**Total tour duration:** ~47 seconds (unattended)

With user interaction (waiting for Enter), duration is variable.

---

## 7. Implementation Notes

### 7.1 Tour State

```rust
pub struct TourState {
    /// Current step index (0-based).
    pub current_step: usize,

    /// Time spent in current step (for progress/auto-advance).
    pub step_elapsed_ms: u64,

    /// Whether tour is paused (waiting for user input).
    pub paused: bool,

    /// Whether user has manually advanced (disables auto-advance).
    pub manual_mode: bool,
}
```

### 7.2 Integration Points

- **App state:** Tour mode sets `app.mode = AppMode::Tour`
- **Input routing:** Tour captures Enter/Backspace/Esc, passes others through
- **Render pass:** Tour HUD renders in `RenderPass::Overlay`
- **Exit handling:** `--exit-after-tour` sets `exit_reason = ExitReason::TourComplete`

### 7.3 Determinism Checklist

- [ ] All step durations are constants
- [ ] Animations use `seed + step_number` for variation
- [ ] No wall-clock time in content (use frame count)
- [ ] Synthetic events are injected at fixed points

---

## 8. Acceptance Criteria

- [x] **Tour model defined** — Section 2 defines Step struct and actions
- [x] **12 feature-proving steps defined** — Section 3 specifies each step
- [x] **User controls defined** — Section 4 lists keyboard controls
- [x] **HUD design defined** — Section 5 shows tour overlay layout
- [x] **Timing budget defined** — Section 6 provides duration table
- [x] **Determinism rules defined** — Section 2.4 and 7.3

---

## 9. Related Beads

| Bead | Dependency | Description |
|------|------------|-------------|
| bd-1gy | This bead | Tour script specification |
| bd-3o0 | Blocked by this | Tour mode driver implementation |
| bd-31gz | Blocked by this | README documentation |
| bd-jqv | Blocked by this | Tour overlay UI |
