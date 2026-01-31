//! Input simulation utilities for E2E testing.
//!
//! Provides utilities to simulate terminal input sequences for testing:
//! - Keyboard input (single keys, modifiers, special keys)
//! - Mouse input (clicks, drags, scroll)
//! - Paste events (bracketed paste mode)
//! - Timing simulation (instant, realistic WPM, stress testing)

#![allow(dead_code)] // Shared test helper; not every integration test uses every builder/mode

use opentui::input::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

/// Timing mode for input simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimingMode {
    /// All events at once (for stress testing).
    #[default]
    Instant,
    /// Realistic typing speed (50-200 WPM).
    Realistic { wpm: u32 },
    /// Rapid burst input (for race condition testing).
    Stress,
}

/// A single input event with optional delay.
#[derive(Clone, Debug)]
pub struct TimedInputEvent {
    /// The event to simulate.
    pub event: InputEvent,
    /// Delay before this event in milliseconds.
    pub delay_ms: u64,
}

/// Raw input event types for simulation.
#[derive(Clone, Debug)]
pub enum InputEvent {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Paste event.
    Paste(String),
    /// Focus gained.
    FocusGained,
    /// Focus lost.
    FocusLost,
    /// Resize event.
    Resize { width: u16, height: u16 },
}

impl From<InputEvent> for Event {
    fn from(event: InputEvent) -> Self {
        match event {
            InputEvent::Key(k) => Event::Key(k),
            InputEvent::Mouse(m) => Event::Mouse(m),
            InputEvent::Paste(s) => Event::Paste(opentui::input::PasteEvent::new(s)),
            InputEvent::FocusGained => Event::FocusGained,
            InputEvent::FocusLost => Event::FocusLost,
            InputEvent::Resize { width, height } => {
                Event::Resize(opentui::input::ResizeEvent::new(width, height))
            }
        }
    }
}

/// Builder for creating input sequences.
#[derive(Clone, Debug, Default)]
pub struct InputSequence {
    events: Vec<TimedInputEvent>,
    timing: TimingMode,
    default_delay_ms: u64,
}

impl InputSequence {
    /// Create a new empty input sequence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a sequence that types the given text.
    #[must_use]
    pub fn type_text(s: &str) -> Self {
        let mut seq = Self::new();
        for c in s.chars() {
            seq = seq.key(KeyCode::Char(c));
        }
        seq
    }

    /// Create a sequence with a single keystroke.
    #[must_use]
    pub fn keystroke(key: KeyCode, mods: KeyModifiers) -> Self {
        let mut seq = Self::new();
        seq.events.push(TimedInputEvent {
            event: InputEvent::Key(KeyEvent::new(key, mods)),
            delay_ms: 0,
        });
        seq
    }

    /// Create a sequence with a single mouse click.
    #[must_use]
    pub fn mouse_click(x: u32, y: u32, button: MouseButton) -> Self {
        let mut seq = Self::new();
        seq.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(x, y, button, MouseEventKind::Press)),
            delay_ms: 0,
        });
        seq.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(x, y, button, MouseEventKind::Release)),
            delay_ms: 10,
        });
        seq
    }

    /// Create a mouse drag sequence.
    #[must_use]
    pub fn mouse_drag(from: (u32, u32), to: (u32, u32), button: MouseButton) -> Self {
        let mut seq = Self::new();
        // Press at start position
        seq.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(
                from.0,
                from.1,
                button,
                MouseEventKind::Press,
            )),
            delay_ms: 0,
        });
        // Generate intermediate move events
        let steps = 5;
        for i in 1..=steps {
            let x = from.0 as f32 + ((to.0 as f32 - from.0 as f32) * i as f32 / steps as f32);
            let y = from.1 as f32 + ((to.1 as f32 - from.1 as f32) * i as f32 / steps as f32);
            seq.events.push(TimedInputEvent {
                event: InputEvent::Mouse(MouseEvent::new(
                    x as u32,
                    y as u32,
                    button,
                    MouseEventKind::Move,
                )),
                delay_ms: 10,
            });
        }
        // Release at end position
        seq.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(to.0, to.1, button, MouseEventKind::Release)),
            delay_ms: 10,
        });
        seq
    }

    /// Create a paste sequence.
    #[must_use]
    pub fn paste(content: &str) -> Self {
        let mut seq = Self::new();
        seq.events.push(TimedInputEvent {
            event: InputEvent::Paste(content.to_string()),
            delay_ms: 0,
        });
        seq
    }

    /// Add a key event to the sequence.
    #[must_use]
    pub fn key(mut self, code: KeyCode) -> Self {
        let delay = self.compute_delay();
        self.events.push(TimedInputEvent {
            event: InputEvent::Key(KeyEvent::key(code)),
            delay_ms: delay,
        });
        self
    }

    /// Add a key with modifiers.
    #[must_use]
    pub fn key_with_mods(mut self, code: KeyCode, mods: KeyModifiers) -> Self {
        let delay = self.compute_delay();
        self.events.push(TimedInputEvent {
            event: InputEvent::Key(KeyEvent::new(code, mods)),
            delay_ms: delay,
        });
        self
    }

    /// Add Ctrl+key.
    #[must_use]
    pub fn ctrl_key(self, code: KeyCode) -> Self {
        self.key_with_mods(code, KeyModifiers::CTRL)
    }

    /// Add Alt+key.
    #[must_use]
    pub fn alt_key(self, code: KeyCode) -> Self {
        self.key_with_mods(code, KeyModifiers::ALT)
    }

    /// Add Shift+key.
    #[must_use]
    pub fn shift_key(self, code: KeyCode) -> Self {
        self.key_with_mods(code, KeyModifiers::SHIFT)
    }

    /// Add a mouse click.
    #[must_use]
    pub fn click(mut self, x: u32, y: u32, button: MouseButton) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(x, y, button, MouseEventKind::Press)),
            delay_ms: self.default_delay_ms,
        });
        self.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(x, y, button, MouseEventKind::Release)),
            delay_ms: 10,
        });
        self
    }

    /// Add a left click.
    #[must_use]
    pub fn left_click(self, x: u32, y: u32) -> Self {
        self.click(x, y, MouseButton::Left)
    }

    /// Add a right click.
    #[must_use]
    pub fn right_click(self, x: u32, y: u32) -> Self {
        self.click(x, y, MouseButton::Right)
    }

    /// Add scroll up event.
    #[must_use]
    pub fn scroll_up(mut self, x: u32, y: u32) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(
                x,
                y,
                MouseButton::None,
                MouseEventKind::ScrollUp,
            )),
            delay_ms: self.default_delay_ms,
        });
        self
    }

    /// Add scroll down event.
    #[must_use]
    pub fn scroll_down(mut self, x: u32, y: u32) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::Mouse(MouseEvent::new(
                x,
                y,
                MouseButton::None,
                MouseEventKind::ScrollDown,
            )),
            delay_ms: self.default_delay_ms,
        });
        self
    }

    /// Add a drag from one point to another.
    #[must_use]
    pub fn drag(mut self, from: (u32, u32), to: (u32, u32)) -> Self {
        let drag_seq = Self::mouse_drag(from, to, MouseButton::Left);
        for event in drag_seq.events {
            self.events.push(event);
        }
        self
    }

    /// Add a paste event.
    #[must_use]
    pub fn add_paste(mut self, content: &str) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::Paste(content.to_string()),
            delay_ms: self.default_delay_ms,
        });
        self
    }

    /// Add focus gained event.
    #[must_use]
    pub fn focus_gained(mut self) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::FocusGained,
            delay_ms: self.default_delay_ms,
        });
        self
    }

    /// Add focus lost event.
    #[must_use]
    pub fn focus_lost(mut self) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::FocusLost,
            delay_ms: self.default_delay_ms,
        });
        self
    }

    /// Add resize event.
    #[must_use]
    pub fn resize(mut self, width: u16, height: u16) -> Self {
        self.events.push(TimedInputEvent {
            event: InputEvent::Resize { width, height },
            delay_ms: self.default_delay_ms,
        });
        self
    }

    /// Add a delay before the next event.
    #[must_use]
    pub fn with_delay(mut self, ms: u64) -> Self {
        if let Some(last) = self.events.last_mut() {
            last.delay_ms = ms;
        }
        self
    }

    /// Set the timing mode for the sequence.
    #[must_use]
    pub fn with_timing(mut self, timing: TimingMode) -> Self {
        self.timing = timing;
        self
    }

    /// Set realistic typing speed in WPM.
    #[must_use]
    pub fn with_wpm(mut self, wpm: u32) -> Self {
        self.timing = TimingMode::Realistic { wpm };
        self
    }

    /// Set stress testing mode (rapid input).
    #[must_use]
    pub fn stress_mode(mut self) -> Self {
        self.timing = TimingMode::Stress;
        self
    }

    /// Append another sequence.
    #[must_use]
    pub fn then(mut self, other: InputSequence) -> Self {
        self.events.extend(other.events);
        self
    }

    /// Get the events in this sequence.
    pub fn events(&self) -> &[TimedInputEvent] {
        &self.events
    }

    /// Get mutable access to events.
    pub fn events_mut(&mut self) -> &mut Vec<TimedInputEvent> {
        &mut self.events
    }

    /// Convert to a list of raw input events (without timing).
    pub fn to_events(&self) -> Vec<InputEvent> {
        self.events.iter().map(|e| e.event.clone()).collect()
    }

    /// Convert to high-level Event types.
    pub fn to_terminal_events(&self) -> Vec<Event> {
        self.events.iter().map(|e| e.event.clone().into()).collect()
    }

    /// Get total simulated time in milliseconds.
    pub fn total_time_ms(&self) -> u64 {
        self.events.iter().map(|e| e.delay_ms).sum()
    }

    /// Get the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the sequence is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get the timing mode.
    pub fn timing(&self) -> TimingMode {
        self.timing
    }

    /// Compute delay based on timing mode.
    fn compute_delay(&self) -> u64 {
        match self.timing {
            TimingMode::Instant => 0,
            TimingMode::Realistic { wpm } => {
                // Average word = 5 characters
                // WPM -> characters per minute -> ms per character
                if wpm > 0 {
                    60_000 / (wpm * 5) as u64
                } else {
                    0
                }
            }
            TimingMode::Stress => 0, // No delay in stress mode
        }
    }
}

/// Generate ANSI escape sequence bytes for a key event.
pub fn key_to_ansi(key: &KeyEvent) -> Vec<u8> {
    let mut result = Vec::new();

    // Handle modifiers for special keys
    let mod_suffix = if key.modifiers.is_empty() {
        String::new()
    } else {
        let mut n = 1u8;
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            n += 1;
        }
        if key.modifiers.contains(KeyModifiers::ALT) {
            n += 2;
        }
        if key.modifiers.contains(KeyModifiers::CTRL) {
            n += 4;
        }
        format!(";{n}")
    };

    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::ALT) {
                result.push(0x1b);
                let c = if key.modifiers.contains(KeyModifiers::CTRL) {
                    // Ctrl+Alt+letter
                    (c as u8).wrapping_sub(b'a').wrapping_add(1)
                } else {
                    c as u8
                };
                result.push(c);
            } else if key.modifiers.contains(KeyModifiers::CTRL) {
                // Ctrl+letter -> ASCII 1-26
                if c.is_ascii_lowercase() || c.is_ascii_uppercase() {
                    let base = c.to_ascii_lowercase() as u8;
                    result.push(base.wrapping_sub(b'a').wrapping_add(1));
                } else {
                    let mut buf = [0u8; 4];
                    result.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
            } else {
                let mut buf = [0u8; 4];
                result.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
            }
        }
        KeyCode::Enter => result.push(b'\r'),
        KeyCode::Tab => result.push(b'\t'),
        KeyCode::BackTab => {
            result.extend_from_slice(b"\x1b[Z");
        }
        KeyCode::Backspace => result.push(0x7f),
        KeyCode::Esc => result.push(0x1b),
        KeyCode::Up => {
            result.extend_from_slice(format!("\x1b[1{mod_suffix}A").as_bytes());
        }
        KeyCode::Down => {
            result.extend_from_slice(format!("\x1b[1{mod_suffix}B").as_bytes());
        }
        KeyCode::Right => {
            result.extend_from_slice(format!("\x1b[1{mod_suffix}C").as_bytes());
        }
        KeyCode::Left => {
            result.extend_from_slice(format!("\x1b[1{mod_suffix}D").as_bytes());
        }
        KeyCode::Home => {
            result.extend_from_slice(format!("\x1b[1{mod_suffix}H").as_bytes());
        }
        KeyCode::End => {
            result.extend_from_slice(format!("\x1b[1{mod_suffix}F").as_bytes());
        }
        KeyCode::PageUp => {
            result.extend_from_slice(format!("\x1b[5{mod_suffix}~").as_bytes());
        }
        KeyCode::PageDown => {
            result.extend_from_slice(format!("\x1b[6{mod_suffix}~").as_bytes());
        }
        KeyCode::Insert => {
            result.extend_from_slice(format!("\x1b[2{mod_suffix}~").as_bytes());
        }
        KeyCode::Delete => {
            result.extend_from_slice(format!("\x1b[3{mod_suffix}~").as_bytes());
        }
        KeyCode::F(n) => {
            let code = match n {
                1 => 11,
                2 => 12,
                3 => 13,
                4 => 14,
                5 => 15,
                6 => 17,
                7 => 18,
                8 => 19,
                9 => 20,
                10 => 21,
                11 => 23,
                12 => 24,
                _ => 15 + n as u8,
            };
            result.extend_from_slice(format!("\x1b[{code}{mod_suffix}~").as_bytes());
        }
        KeyCode::Null => result.push(0x00),
        _ => {}
    }

    result
}

/// Generate SGR mouse encoding for a mouse event.
pub fn mouse_to_sgr(event: &MouseEvent) -> Vec<u8> {
    let mut cb: u8 = match event.button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        MouseButton::None => 0,
    };

    match event.kind {
        MouseEventKind::Move => cb |= 32,
        MouseEventKind::ScrollUp => cb = 64,
        MouseEventKind::ScrollDown => cb = 65,
        MouseEventKind::ScrollLeft => cb = 66,
        MouseEventKind::ScrollRight => cb = 67,
        _ => {}
    }

    if event.shift {
        cb |= 4;
    }
    if event.alt {
        cb |= 8;
    }
    if event.ctrl {
        cb |= 16;
    }

    let term = if event.kind == MouseEventKind::Release {
        'm'
    } else {
        'M'
    };

    // SGR uses 1-indexed coordinates
    let x = event.x + 1;
    let y = event.y + 1;

    format!("\x1b[<{cb};{x};{y}{term}").into_bytes()
}

/// Generate bracketed paste sequence.
pub fn paste_to_ansi(content: &str) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(b"\x1b[200~");
    result.extend_from_slice(content.as_bytes());
    result.extend_from_slice(b"\x1b[201~");
    result
}

/// Generate focus event sequence.
pub fn focus_to_ansi(gained: bool) -> Vec<u8> {
    if gained {
        b"\x1b[I".to_vec()
    } else {
        b"\x1b[O".to_vec()
    }
}

/// Generate resize event sequence.
pub fn resize_to_ansi(width: u16, height: u16) -> Vec<u8> {
    format!("\x1b[8;{height};{width}t").into_bytes()
}

/// Convert an InputEvent to ANSI bytes.
pub fn event_to_ansi(event: &InputEvent) -> Vec<u8> {
    match event {
        InputEvent::Key(k) => key_to_ansi(k),
        InputEvent::Mouse(m) => mouse_to_sgr(m),
        InputEvent::Paste(s) => paste_to_ansi(s),
        InputEvent::FocusGained => focus_to_ansi(true),
        InputEvent::FocusLost => focus_to_ansi(false),
        InputEvent::Resize { width, height } => resize_to_ansi(*width, *height),
    }
}

/// Convert an entire InputSequence to ANSI bytes.
pub fn sequence_to_ansi(seq: &InputSequence) -> Vec<u8> {
    let mut result = Vec::new();
    for timed in seq.events() {
        result.extend(event_to_ansi(&timed.event));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_text() {
        let seq = InputSequence::type_text("hello");
        assert_eq!(seq.len(), 5);

        let events: Vec<_> = seq
            .to_events()
            .into_iter()
            .filter_map(|e| match e {
                InputEvent::Key(k) => Some(k.code),
                _ => None,
            })
            .collect();
        assert_eq!(
            events,
            vec![
                KeyCode::Char('h'),
                KeyCode::Char('e'),
                KeyCode::Char('l'),
                KeyCode::Char('l'),
                KeyCode::Char('o'),
            ]
        );
    }

    #[test]
    fn test_keystroke() {
        let seq = InputSequence::keystroke(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(seq.len(), 1);
    }

    #[test]
    fn test_mouse_click() {
        let seq = InputSequence::mouse_click(10, 5, MouseButton::Left);
        assert_eq!(seq.len(), 2); // Press + Release
    }

    #[test]
    fn test_mouse_drag() {
        let seq = InputSequence::mouse_drag((0, 0), (10, 10), MouseButton::Left);
        // Press + 5 moves + Release = 7
        assert_eq!(seq.len(), 7);
    }

    #[test]
    fn test_paste() {
        let seq = InputSequence::paste("test content");
        assert_eq!(seq.len(), 1);
        if let InputEvent::Paste(content) = &seq.events()[0].event {
            assert_eq!(content, "test content");
        } else {
            panic!("Expected paste event");
        }
    }

    #[test]
    fn test_builder_chaining() {
        let seq = InputSequence::new()
            .key(KeyCode::Char('a'))
            .ctrl_key(KeyCode::Char('c'))
            .left_click(10, 5)
            .key(KeyCode::Enter);

        assert_eq!(seq.len(), 5); // 'a', Ctrl+C, click press, click release, Enter
    }

    #[test]
    fn test_with_delay() {
        let seq = InputSequence::new()
            .key(KeyCode::Char('a'))
            .with_delay(100)
            .key(KeyCode::Char('b'));

        assert_eq!(seq.events()[0].delay_ms, 100);
    }

    #[test]
    fn test_realistic_timing() {
        let seq = InputSequence::type_text("hello").with_wpm(60);
        // 60 WPM = 300 chars/min = 200ms/char
        assert_eq!(seq.timing, TimingMode::Realistic { wpm: 60 });
    }

    #[test]
    fn test_key_to_ansi() {
        // Simple character
        assert_eq!(key_to_ansi(&KeyEvent::char('a')), vec![b'a']);

        // Ctrl+C
        let ctrl_c = KeyEvent::with_ctrl(KeyCode::Char('c'));
        assert_eq!(key_to_ansi(&ctrl_c), vec![3]); // ASCII ETX

        // Arrow key
        let up = KeyEvent::key(KeyCode::Up);
        assert_eq!(key_to_ansi(&up), b"\x1b[1A");

        // Enter
        assert_eq!(key_to_ansi(&KeyEvent::key(KeyCode::Enter)), vec![b'\r']);
    }

    #[test]
    fn test_mouse_to_sgr() {
        let click = MouseEvent::new(10, 5, MouseButton::Left, MouseEventKind::Press);
        let ansi = mouse_to_sgr(&click);
        // SGR: ESC[<0;11;6M (x+1, y+1)
        assert_eq!(ansi, b"\x1b[<0;11;6M");
    }

    #[test]
    fn test_paste_to_ansi() {
        let ansi = paste_to_ansi("hello");
        assert_eq!(ansi, b"\x1b[200~hello\x1b[201~");
    }

    #[test]
    fn test_sequence_to_ansi() {
        let seq = InputSequence::type_text("hi");
        let ansi = sequence_to_ansi(&seq);
        assert_eq!(ansi, b"hi");
    }

    #[test]
    fn test_then_combinator() {
        let seq1 = InputSequence::type_text("a");
        let seq2 = InputSequence::type_text("b");
        let combined = seq1.then(seq2);
        assert_eq!(combined.len(), 2);
    }

    #[test]
    fn test_scroll_events() {
        let seq = InputSequence::new().scroll_up(10, 5).scroll_down(10, 5);

        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn test_focus_events() {
        let seq = InputSequence::new().focus_gained().focus_lost();

        assert_eq!(seq.len(), 2);
    }

    #[test]
    fn test_resize_event() {
        let seq = InputSequence::new().resize(120, 50);
        assert_eq!(seq.len(), 1);
    }
}
