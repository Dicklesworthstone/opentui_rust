//! Mouse event handling.

/// Mouse button.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Middle mouse button (scroll wheel click).
    Middle,
    /// Right mouse button.
    Right,
    /// No button (for move events).
    None,
}

/// Kind of mouse event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseEventKind {
    /// Button pressed.
    Press,
    /// Button released.
    Release,
    /// Mouse moved.
    Move,
    /// Scroll wheel up.
    ScrollUp,
    /// Scroll wheel down.
    ScrollDown,
    /// Scroll wheel left (horizontal).
    ScrollLeft,
    /// Scroll wheel right (horizontal).
    ScrollRight,
}

/// A mouse event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MouseEvent {
    /// X position (column).
    pub x: u32,
    /// Y position (row).
    pub y: u32,
    /// Button involved.
    pub button: MouseButton,
    /// Kind of event.
    pub kind: MouseEventKind,
    /// Shift key held.
    pub shift: bool,
    /// Control key held.
    pub ctrl: bool,
    /// Alt key held.
    pub alt: bool,
}

impl MouseEvent {
    /// Create a new mouse event.
    #[must_use]
    pub fn new(x: u32, y: u32, button: MouseButton, kind: MouseEventKind) -> Self {
        Self {
            x,
            y,
            button,
            kind,
            shift: false,
            ctrl: false,
            alt: false,
        }
    }

    /// Create a press event.
    #[must_use]
    pub fn press(x: u32, y: u32, button: MouseButton) -> Self {
        Self::new(x, y, button, MouseEventKind::Press)
    }

    /// Create a release event.
    #[must_use]
    pub fn release(x: u32, y: u32, button: MouseButton) -> Self {
        Self::new(x, y, button, MouseEventKind::Release)
    }

    /// Create a move event.
    #[must_use]
    pub fn move_to(x: u32, y: u32) -> Self {
        Self::new(x, y, MouseButton::None, MouseEventKind::Move)
    }

    /// Create a scroll up event.
    #[must_use]
    pub fn scroll_up(x: u32, y: u32) -> Self {
        Self::new(x, y, MouseButton::None, MouseEventKind::ScrollUp)
    }

    /// Create a scroll down event.
    #[must_use]
    pub fn scroll_down(x: u32, y: u32) -> Self {
        Self::new(x, y, MouseButton::None, MouseEventKind::ScrollDown)
    }

    /// Set modifier keys.
    #[must_use]
    pub fn with_modifiers(mut self, shift: bool, ctrl: bool, alt: bool) -> Self {
        self.shift = shift;
        self.ctrl = ctrl;
        self.alt = alt;
        self
    }

    /// Check if this is a click (press) event.
    #[must_use]
    pub fn is_press(&self) -> bool {
        self.kind == MouseEventKind::Press
    }

    /// Check if this is a scroll event.
    #[must_use]
    pub fn is_scroll(&self) -> bool {
        matches!(
            self.kind,
            MouseEventKind::ScrollUp
                | MouseEventKind::ScrollDown
                | MouseEventKind::ScrollLeft
                | MouseEventKind::ScrollRight
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_event() {
        let event = MouseEvent::press(10, 5, MouseButton::Left);
        assert_eq!(event.x, 10);
        assert_eq!(event.y, 5);
        assert!(event.is_press());
        assert!(!event.is_scroll());
    }

    #[test]
    fn test_mouse_scroll() {
        let event = MouseEvent::scroll_up(0, 0);
        assert!(event.is_scroll());
        assert!(!event.is_press());
    }

    #[test]
    fn test_mouse_modifiers() {
        let event = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(true, false, true);
        assert!(event.shift);
        assert!(!event.ctrl);
        assert!(event.alt);
    }
}
