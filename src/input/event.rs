//! Terminal event types.

use crate::input::keyboard::KeyEvent;
use crate::terminal::MouseEvent;

/// A terminal event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Terminal resize event.
    Resize(ResizeEvent),
    /// Focus gained event.
    FocusGained,
    /// Focus lost event.
    FocusLost,
    /// Paste event (bracketed paste mode).
    Paste(PasteEvent),
}

impl Event {
    /// Check if this is a key event.
    #[must_use]
    pub fn is_key(&self) -> bool {
        matches!(self, Self::Key(_))
    }

    /// Check if this is a mouse event.
    #[must_use]
    pub fn is_mouse(&self) -> bool {
        matches!(self, Self::Mouse(_))
    }

    /// Check if this is a resize event.
    #[must_use]
    pub fn is_resize(&self) -> bool {
        matches!(self, Self::Resize(_))
    }

    /// Get the key event if this is one.
    #[must_use]
    pub fn key(&self) -> Option<&KeyEvent> {
        match self {
            Self::Key(e) => Some(e),
            _ => None,
        }
    }

    /// Get the mouse event if this is one.
    #[must_use]
    pub fn mouse(&self) -> Option<&MouseEvent> {
        match self {
            Self::Mouse(e) => Some(e),
            _ => None,
        }
    }

    /// Get the resize event if this is one.
    #[must_use]
    pub fn resize(&self) -> Option<&ResizeEvent> {
        match self {
            Self::Resize(e) => Some(e),
            _ => None,
        }
    }

    /// Get the paste event if this is one.
    #[must_use]
    pub fn paste(&self) -> Option<&PasteEvent> {
        match self {
            Self::Paste(e) => Some(e),
            _ => None,
        }
    }

    /// Check if this is a paste event.
    #[must_use]
    pub fn is_paste(&self) -> bool {
        matches!(self, Self::Paste(_))
    }
}

impl From<KeyEvent> for Event {
    fn from(e: KeyEvent) -> Self {
        Self::Key(e)
    }
}

impl From<MouseEvent> for Event {
    fn from(e: MouseEvent) -> Self {
        Self::Mouse(e)
    }
}

impl From<ResizeEvent> for Event {
    fn from(e: ResizeEvent) -> Self {
        Self::Resize(e)
    }
}

/// Terminal resize event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResizeEvent {
    /// New width in columns.
    pub width: u16,
    /// New height in rows.
    pub height: u16,
}

impl ResizeEvent {
    /// Create a new resize event.
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

/// Focus event (gained or lost).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusEvent {
    /// Terminal window gained focus.
    Gained,
    /// Terminal window lost focus.
    Lost,
}

/// Paste event from bracketed paste mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PasteEvent {
    /// The pasted text content.
    pub content: String,
}

impl PasteEvent {
    /// Create a new paste event.
    #[must_use]
    pub fn new(content: String) -> Self {
        Self { content }
    }

    /// Get the pasted content.
    #[must_use]
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Check if the paste is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Get the length of the pasted content.
    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{KeyCode, KeyModifiers};
    use crate::terminal::MouseButton;

    #[test]
    fn test_event_key() {
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        let event = Event::Key(key);
        assert!(event.is_key());
        assert!(!event.is_mouse());
        assert_eq!(event.key(), Some(&key));
    }

    #[test]
    fn test_event_mouse() {
        let mouse = MouseEvent::press(10, 5, MouseButton::Left);
        let event = Event::Mouse(mouse);
        assert!(event.is_mouse());
        assert!(!event.is_key());
        assert_eq!(event.mouse(), Some(&mouse));
    }

    #[test]
    fn test_resize_event() {
        let resize = ResizeEvent::new(80, 24);
        assert_eq!(resize.width, 80);
        assert_eq!(resize.height, 24);
    }

    #[test]
    fn test_paste_event() {
        let paste = PasteEvent::new("Hello, World!".to_string());
        assert_eq!(paste.content(), "Hello, World!");
        assert!(!paste.is_empty());
        assert_eq!(paste.len(), 13);
    }

    #[test]
    fn test_event_from_conversions() {
        let key = KeyEvent::char('a');
        let event: Event = key.into();
        assert!(event.is_key());

        let resize = ResizeEvent::new(100, 50);
        let event: Event = resize.into();
        assert!(event.is_resize());
    }
}
