//! Mock input provider for scripted test scenarios.
//!
//! This module provides [`MockInput`] which queues input events and bytes
//! for controlled test scenarios. Supports keyboard events, mouse events,
//! and raw byte sequences.

#![allow(dead_code)] // Shared test helpers; not every integration test uses every input helper

use opentui::input::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind, PasteEvent,
};
use opentui_rust as opentui;
use std::collections::VecDeque;

/// A mock input provider that delivers scripted input events.
///
/// Events are queued and delivered in order. Supports both high-level
/// events and raw byte sequences for testing parser behavior.
///
/// # Example
///
/// ```ignore
/// let mut input = MockInput::new();
/// input.queue_key('a');
/// input.queue_key_with_modifiers('c', KeyModifiers::CTRL);
/// input.queue_mouse_click(10, 5, MouseButton::Left);
///
/// while let Some(event) = input.next_event() {
///     // Process events...
/// }
/// ```
#[derive(Debug, Default)]
pub struct MockInput {
    /// Queue of events to deliver.
    events: VecDeque<Event>,
    /// Queue of raw bytes (alternative to events).
    raw_bytes: VecDeque<u8>,
    /// Whether to use raw bytes mode.
    raw_mode: bool,
}

impl MockInput {
    /// Create a new empty mock input.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a mock input in raw bytes mode.
    pub fn raw() -> Self {
        Self {
            raw_mode: true,
            ..Self::default()
        }
    }

    /// Queue a single character key press.
    pub fn queue_key(&mut self, c: char) {
        self.events.push_back(Event::Key(KeyEvent::new(
            KeyCode::Char(c),
            KeyModifiers::empty(),
        )));
    }

    /// Queue a key press with modifiers.
    pub fn queue_key_with_modifiers(&mut self, c: char, modifiers: KeyModifiers) {
        self.events
            .push_back(Event::Key(KeyEvent::new(KeyCode::Char(c), modifiers)));
    }

    /// Queue a special key press (Enter, Esc, etc.).
    pub fn queue_special_key(&mut self, key: KeyCode) {
        self.events
            .push_back(Event::Key(KeyEvent::new(key, KeyModifiers::empty())));
    }

    /// Queue a special key with modifiers.
    pub fn queue_special_key_with_modifiers(&mut self, key: KeyCode, modifiers: KeyModifiers) {
        self.events
            .push_back(Event::Key(KeyEvent::new(key, modifiers)));
    }

    /// Queue a mouse click event.
    pub fn queue_mouse_click(&mut self, x: u32, y: u32, button: MouseButton) {
        self.events.push_back(Event::Mouse(MouseEvent {
            x,
            y,
            button,
            kind: MouseEventKind::Press,
            shift: false,
            ctrl: false,
            alt: false,
        }));
    }

    /// Queue a mouse release event.
    pub fn queue_mouse_release(&mut self, x: u32, y: u32, button: MouseButton) {
        self.events.push_back(Event::Mouse(MouseEvent {
            x,
            y,
            button,
            kind: MouseEventKind::Release,
            shift: false,
            ctrl: false,
            alt: false,
        }));
    }

    /// Queue a mouse move event.
    pub fn queue_mouse_move(&mut self, x: u32, y: u32) {
        self.events.push_back(Event::Mouse(MouseEvent {
            x,
            y,
            button: MouseButton::None,
            kind: MouseEventKind::Move,
            shift: false,
            ctrl: false,
            alt: false,
        }));
    }

    /// Queue a mouse scroll event.
    pub fn queue_mouse_scroll(&mut self, x: u32, y: u32, direction: ScrollDirection) {
        let kind = match direction {
            ScrollDirection::Up => MouseEventKind::ScrollUp,
            ScrollDirection::Down => MouseEventKind::ScrollDown,
        };
        self.events.push_back(Event::Mouse(MouseEvent {
            x,
            y,
            button: MouseButton::None,
            kind,
            shift: false,
            ctrl: false,
            alt: false,
        }));
    }

    /// Queue a focus gained event.
    pub fn queue_focus_gained(&mut self) {
        self.events.push_back(Event::FocusGained);
    }

    /// Queue a focus lost event.
    pub fn queue_focus_lost(&mut self) {
        self.events.push_back(Event::FocusLost);
    }

    /// Queue a paste event.
    pub fn queue_paste(&mut self, text: impl Into<String>) {
        self.events
            .push_back(Event::Paste(PasteEvent::new(text.into())));
    }

    /// Queue a string as individual key events.
    pub fn queue_string(&mut self, s: &str) {
        for c in s.chars() {
            self.queue_key(c);
        }
    }

    /// Queue raw bytes for parser testing.
    pub fn queue_raw_bytes(&mut self, bytes: &[u8]) {
        self.raw_bytes.extend(bytes);
    }

    /// Queue a raw ANSI escape sequence.
    pub fn queue_raw_escape(&mut self, params: &str, final_byte: u8) {
        self.raw_bytes.push_back(0x1b);
        self.raw_bytes.push_back(b'[');
        self.raw_bytes.extend(params.as_bytes());
        self.raw_bytes.push_back(final_byte);
    }

    /// Get the next event from the queue.
    pub fn next_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Get the next raw byte from the queue.
    pub fn next_byte(&mut self) -> Option<u8> {
        self.raw_bytes.pop_front()
    }

    /// Read available raw bytes into a buffer.
    pub fn read_raw(&mut self, buf: &mut [u8]) -> usize {
        let mut count = 0;
        while count < buf.len() {
            if let Some(b) = self.raw_bytes.pop_front() {
                buf[count] = b;
                count += 1;
            } else {
                break;
            }
        }
        count
    }

    /// Check if there are more events.
    pub fn has_events(&self) -> bool {
        !self.events.is_empty()
    }

    /// Check if there are more raw bytes.
    pub fn has_raw_bytes(&self) -> bool {
        !self.raw_bytes.is_empty()
    }

    /// Get the number of queued events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get the number of queued raw bytes.
    pub fn raw_byte_count(&self) -> usize {
        self.raw_bytes.len()
    }

    /// Clear all queued events.
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Clear all queued raw bytes.
    pub fn clear_raw(&mut self) {
        self.raw_bytes.clear();
    }

    /// Clear everything.
    pub fn clear(&mut self) {
        self.events.clear();
        self.raw_bytes.clear();
    }

    /// Collect all remaining events into a vector.
    pub fn drain_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    /// Collect all remaining raw bytes into a vector.
    pub fn drain_raw(&mut self) -> Vec<u8> {
        self.raw_bytes.drain(..).collect()
    }
}

/// Mouse scroll direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// Builder for creating complex input sequences.
#[derive(Debug, Default)]
pub struct InputSequenceBuilder {
    input: MockInput,
}

impl InputSequenceBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a single character.
    pub fn char(mut self, c: char) -> Self {
        self.input.queue_key(c);
        self
    }

    /// Add a string of characters.
    pub fn string(mut self, s: &str) -> Self {
        self.input.queue_string(s);
        self
    }

    /// Add Enter key.
    pub fn enter(mut self) -> Self {
        self.input.queue_special_key(KeyCode::Enter);
        self
    }

    /// Add Escape key.
    pub fn escape(mut self) -> Self {
        self.input.queue_special_key(KeyCode::Esc);
        self
    }

    /// Add Tab key.
    pub fn tab(mut self) -> Self {
        self.input.queue_special_key(KeyCode::Tab);
        self
    }

    /// Add Backspace key.
    pub fn backspace(mut self) -> Self {
        self.input.queue_special_key(KeyCode::Backspace);
        self
    }

    /// Add Delete key.
    pub fn delete(mut self) -> Self {
        self.input.queue_special_key(KeyCode::Delete);
        self
    }

    /// Add arrow key.
    pub fn arrow(mut self, direction: ArrowDirection) -> Self {
        let key = match direction {
            ArrowDirection::Up => KeyCode::Up,
            ArrowDirection::Down => KeyCode::Down,
            ArrowDirection::Left => KeyCode::Left,
            ArrowDirection::Right => KeyCode::Right,
        };
        self.input.queue_special_key(key);
        self
    }

    /// Add Ctrl+key combination.
    pub fn ctrl(mut self, c: char) -> Self {
        self.input.queue_key_with_modifiers(c, KeyModifiers::CTRL);
        self
    }

    /// Add Alt+key combination.
    pub fn alt(mut self, c: char) -> Self {
        self.input.queue_key_with_modifiers(c, KeyModifiers::ALT);
        self
    }

    /// Add Shift+key combination.
    pub fn shift(mut self, c: char) -> Self {
        self.input.queue_key_with_modifiers(c, KeyModifiers::SHIFT);
        self
    }

    /// Add mouse click.
    pub fn click(mut self, x: u32, y: u32) -> Self {
        self.input.queue_mouse_click(x, y, MouseButton::Left);
        self
    }

    /// Add right click.
    pub fn right_click(mut self, x: u32, y: u32) -> Self {
        self.input.queue_mouse_click(x, y, MouseButton::Right);
        self
    }

    /// Add mouse drag from one point to another (press, move, release).
    pub fn drag(mut self, from: (u32, u32), to: (u32, u32)) -> Self {
        self.input
            .queue_mouse_click(from.0, from.1, MouseButton::Left);
        // Simulate drag with move events
        self.input.queue_mouse_move(to.0, to.1);
        self.input
            .queue_mouse_release(to.0, to.1, MouseButton::Left);
        self
    }

    /// Add scroll.
    pub fn scroll(mut self, x: u32, y: u32, direction: ScrollDirection) -> Self {
        self.input.queue_mouse_scroll(x, y, direction);
        self
    }

    /// Add paste.
    pub fn paste(mut self, text: &str) -> Self {
        self.input.queue_paste(text);
        self
    }

    /// Build the MockInput.
    pub fn build(self) -> MockInput {
        self.input
    }
}

/// Arrow key direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_input_key_events() {
        let mut input = MockInput::new();
        input.queue_key('a');
        input.queue_key('b');

        let event1 = input.next_event().unwrap();
        let event2 = input.next_event().unwrap();

        assert!(matches!(
            event1,
            Event::Key(KeyEvent {
                code: KeyCode::Char('a'),
                ..
            })
        ));
        assert!(matches!(
            event2,
            Event::Key(KeyEvent {
                code: KeyCode::Char('b'),
                ..
            })
        ));
        assert!(input.next_event().is_none());
    }

    #[test]
    fn test_mock_input_string() {
        let mut input = MockInput::new();
        input.queue_string("hi");

        assert_eq!(input.event_count(), 2);
    }

    #[test]
    fn test_mock_input_mouse() {
        let mut input = MockInput::new();
        input.queue_mouse_click(10, 20, MouseButton::Left);

        let event = input.next_event().unwrap();
        assert!(matches!(
            event,
            Event::Mouse(MouseEvent {
                x: 10,
                y: 20,
                button: MouseButton::Left,
                kind: MouseEventKind::Press,
                ..
            })
        ));
    }

    #[test]
    fn test_mock_input_raw_bytes() {
        let mut input = MockInput::raw();
        input.queue_raw_bytes(b"\x1b[A");

        assert_eq!(input.raw_byte_count(), 3);

        let mut buf = [0u8; 10];
        let n = input.read_raw(&mut buf);
        assert_eq!(n, 3);
        assert_eq!(&buf[..3], b"\x1b[A");
    }

    #[test]
    fn test_input_sequence_builder() {
        let input = InputSequenceBuilder::new()
            .string("hello")
            .enter()
            .ctrl('c')
            .build();

        assert_eq!(input.event_count(), 7); // 5 chars + enter + ctrl-c
    }

    #[test]
    fn test_mock_input_paste() {
        let mut input = MockInput::new();
        input.queue_paste("clipboard content");

        let event = input.next_event().unwrap();
        assert!(matches!(event, Event::Paste(ref p) if p.content == "clipboard content"));
    }

    #[test]
    fn test_mock_input_focus() {
        let mut input = MockInput::new();
        input.queue_focus_gained();
        input.queue_focus_lost();

        assert!(matches!(input.next_event(), Some(Event::FocusGained)));
        assert!(matches!(input.next_event(), Some(Event::FocusLost)));
    }

    #[test]
    fn test_mock_input_drain() {
        let mut input = MockInput::new();
        input.queue_key('a');
        input.queue_key('b');

        let events = input.drain_events();
        assert_eq!(events.len(), 2);
        assert!(!input.has_events());
    }
}
