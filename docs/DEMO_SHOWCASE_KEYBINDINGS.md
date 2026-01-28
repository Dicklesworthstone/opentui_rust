# demo_showcase Keybindings & Interaction Model

> **Version:** 1.0
> **Status:** Draft
> **Bead:** bd-3l0

This document defines the keybindings and interaction model for `demo_showcase`.

---

## 1. Design Philosophy

1. **Discoverable** - Status bar and help overlay always tell you what to do
2. **Low cognitive load** - Small set of global chords; panel-local keys only when focused
3. **Mouse is first-class** - Click to focus; scroll to scroll; clickable buttons prove HitGrid
4. **Consistent** - Similar actions use similar keys across panels

---

## 2. Global Keybindings (Always Active)

These keybindings work regardless of which panel is focused or what mode is active:

| Key | Action | Notes |
|-----|--------|-------|
| `Ctrl+Q` | Quit | Clean exit with terminal restore |
| `Esc` | Quit / Close overlay | Quits if no overlay open |
| `F1` | Help overlay (toggle) | Shows all keybindings |
| `Ctrl+P` | Command palette (toggle) | Fuzzy action search |
| `Ctrl+T` | Tour mode (toggle) | Start/resume guided tour |
| `Ctrl+D` | Debug overlay (toggle) | FPS, cells, memory stats |
| `Ctrl+N` | Cycle theme | Synthwave → Paper → Solarized → High Contrast |
| `Tab` | Cycle focus forward | Sidebar → Editor → Preview → Logs → Sidebar |
| `Shift+Tab` | Cycle focus backward | Reverse of Tab |
| `Ctrl+R` | Force redraw | Calls `renderer.invalidate()` |
| `1`-`6` | Jump to section | Quick navigation to sections |

---

## 3. Panel Focus Model

### 3.1 Focus Rules

- There is always exactly **one focused panel**
- Focus is indicated by:
  - Border highlight (accent color)
  - Subtle glow effect (if terminal supports)
- Keyboard events route to the focused panel
- Mouse click inside a panel sets focus

### 3.2 Panel Order

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  [1] Sidebar   │  [2] Editor         │  [3] Preview                         │
│                │                     │                                      │
│                │                     │                                      │
│                ├─────────────────────┴──────────────────────────────────────┤
│                │  [4] Logs                                                   │
└────────────────┴─────────────────────────────────────────────────────────────┘
```

Focus cycles: Sidebar → Editor → Preview → Logs → Sidebar

### 3.3 Focus Indicators

| State | Visual |
|-------|--------|
| Focused | Accent-colored border, panel title highlighted |
| Unfocused | Dim border, normal panel title |
| Disabled | Grayed out, no border |

---

## 4. Mouse Interaction Model

### 4.1 Click Behavior

| Target | Action |
|--------|--------|
| Sidebar item | Select section, update editor content |
| Top bar button | Toggle associated overlay |
| Panel interior | Focus panel |
| Scrollbar | Scroll to position |
| Hyperlink | Show pressed feedback (terminal handles link) |

### 4.2 Scroll Behavior

| Target | Action |
|--------|--------|
| Sidebar | Scroll section list |
| Editor | Scroll document |
| Logs | Scroll log entries |
| Preview | Scroll preview content |

### 4.3 HitGrid Registration

The following hit areas must be registered:

- Each sidebar row (`hit_sidebar_item_N`)
- Top bar buttons (`hit_btn_help`, `hit_btn_palette`, `hit_btn_tour`)
- Panel regions (`hit_panel_sidebar`, `hit_panel_editor`, etc.)
- Scrollable regions (for wheel events)

---

## 5. Input Events to Surface

The demo should visibly react to these events:

### 5.1 Focus Events

| Event | Reaction |
|-------|----------|
| Focus lost | Pause animations, show "PAUSED" badge |
| Focus gained | Resume animations, remove badge |

### 5.2 Paste Events

| Event | Reaction |
|-------|----------|
| Bracketed paste | Insert text in editor, show toast "Pasted N chars" |

### 5.3 Resize Events

| Event | Reaction |
|-------|----------|
| Terminal resize | Recompute layout, show toast "Resized to WxH" |
| Layout mode change | Show toast "Layout: Full/Compact/Minimal" |

---

## 6. Panel-Specific Keybindings

### 6.1 Editor Panel (When Focused)

Based on `examples/editor.rs`:

| Key | Action |
|-----|--------|
| Arrow keys | Move cursor |
| `Ctrl+Left/Right` | Word navigation |
| `Home` / `End` | Line start/end |
| `Ctrl+Home/End` | Document start/end |
| `PageUp` / `PageDown` | Scroll by page |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` / `Ctrl+Shift+Z` | Redo |
| `Ctrl+W` | Cycle wrap mode (None → Word → Char) |
| `Ctrl+L` | Toggle line numbers |
| `Ctrl+G` | Go to line (shows mini-dialog) |
| `Ctrl+F` | Find (shows search overlay) |

### 6.2 Sidebar Panel (When Focused)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Navigate sections |
| `Enter` / `Space` | Select section |
| `Home` | Jump to first section |
| `End` | Jump to last section |

### 6.3 Logs Panel (When Focused)

| Key | Action |
|-----|--------|
| `Up` / `Down` | Scroll by line |
| `PageUp` / `PageDown` | Scroll by page |
| `Home` | Jump to oldest entry |
| `End` | Jump to newest entry |
| `Ctrl+C` | Copy selected log entry |

### 6.4 Preview Panel (When Focused)

| Key | Action |
|-----|--------|
| Arrow keys | Pan preview |
| `+` / `-` | Zoom in/out (if applicable) |
| `0` | Reset zoom |
| `Space` | Toggle auto-update |

---

## 7. Overlay Keybindings

### 7.1 Help Overlay (`F1`)

| Key | Action |
|-----|--------|
| `Esc` | Close help |
| `F1` | Close help (toggle) |
| `Up` / `Down` | Scroll help content |
| `PageUp` / `PageDown` | Scroll by page |

### 7.2 Command Palette (`Ctrl+P`)

| Key | Action |
|-----|--------|
| `Esc` | Close palette |
| `Enter` | Execute selected action |
| `Up` / `Down` | Navigate action list |
| Type | Filter actions |
| `Ctrl+Backspace` | Clear filter |

### 7.3 Tour Mode (`Ctrl+T`)

| Key | Action |
|-----|--------|
| `Enter` / `Space` | Next step |
| `Backspace` | Previous step |
| `Esc` | Exit tour |
| `Home` | Restart tour |

---

## 8. Mode System

### 8.1 Modes

The demo operates in one of these modes:

| Mode | Description |
|------|-------------|
| `Normal` | Standard operation, all panels interactive |
| `Tour` | Guided walkthrough, limited input |
| `Overlay` | Modal overlay open (Help/Palette) |
| `Paused` | Focus lost, animations paused |

### 8.2 Mode Transitions

```
           ┌──────────────────────────────┐
           │                              │
           v                              │
┌─────────────────┐  Ctrl+T    ┌─────────────────┐
│     Normal      │ ────────>  │      Tour       │
│                 │ <────────  │                 │
└─────────────────┘    Esc     └─────────────────┘
           │                              │
           │ F1/Ctrl+P                    │ Esc
           v                              │
┌─────────────────┐                       │
│     Overlay     │ ──────────────────────┘
│                 │
└─────────────────┘
           │
           │ Focus lost
           v
┌─────────────────┐
│     Paused      │
└─────────────────┘
```

---

## 9. Status Bar Content

The status bar shows context-sensitive hints:

### 9.1 Normal Mode

```
Ctrl+Q Quit  |  F1 Help  |  Ctrl+P Palette  |  Tab Focus  |  Frame: 1234
```

### 9.2 Tour Mode

```
Step 3/12: "The Editor Panel"  |  Enter: Next  |  Esc: Exit Tour
```

### 9.3 Overlay Mode

```
Esc: Close  |  Up/Down: Navigate  |  Enter: Select
```

### 9.4 Paused Mode

```
[PAUSED - Click window to resume]                            Frame: 1234
```

---

## 10. Accessibility Considerations

### 10.1 Keyboard-Only Operation

All functionality is accessible via keyboard:

- Focus indicators are always visible
- Tab order is logical and predictable
- Status bar shows available keys

### 10.2 Mouse-Only Operation

All functionality is accessible via mouse:

- Clickable sidebar items
- Top bar buttons for overlays
- Scrollable panels

### 10.3 Screen Reader Hints

When possible, use semantic structure:

- Announce focus changes
- Announce mode changes
- Announce toast messages

---

## 11. Acceptance Criteria Checklist

- [x] **Global keybindings defined** - Section 2
- [x] **Panel focus model defined** - Section 3
- [x] **Mouse interaction model defined** - Section 4
- [x] **Input events to surface defined** - Section 5
- [x] **Panel-specific keybindings defined** - Section 6
- [x] **Overlay keybindings defined** - Section 7
- [x] **Mode system defined** - Section 8

---

## 12. Related Beads

| Bead | Dependency | Description |
|------|------------|-------------|
| bd-3l0 | This bead | Keybindings + interaction model |
| bd-35g | Blocked by this | App state machine |
| bd-2tj | Blocked by this | Editor panel |
| bd-1al | Blocked by this | Overlay system |
| bd-1gy | Blocked by this | Tour script |
