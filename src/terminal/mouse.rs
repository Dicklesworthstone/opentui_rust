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
    #![allow(clippy::uninlined_format_args)]
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

    // =============================================
    // Comprehensive Mouse Tests (bd-30ga)
    // =============================================

    // --- MouseButton variants ---

    #[test]
    fn test_mouse_button_variants_distinct() {
        assert_ne!(MouseButton::Left, MouseButton::Right);
        assert_ne!(MouseButton::Left, MouseButton::Middle);
        assert_ne!(MouseButton::Left, MouseButton::None);
        assert_ne!(MouseButton::Right, MouseButton::Middle);
        assert_ne!(MouseButton::Right, MouseButton::None);
        assert_ne!(MouseButton::Middle, MouseButton::None);
    }

    #[test]
    fn test_mouse_button_debug() {
        assert_eq!(format!("{:?}", MouseButton::Left), "Left");
        assert_eq!(format!("{:?}", MouseButton::Middle), "Middle");
        assert_eq!(format!("{:?}", MouseButton::Right), "Right");
        assert_eq!(format!("{:?}", MouseButton::None), "None");
    }

    // --- MouseEventKind variants ---

    #[test]
    fn test_event_kind_variants_distinct() {
        let kinds = [
            MouseEventKind::Press,
            MouseEventKind::Release,
            MouseEventKind::Move,
            MouseEventKind::ScrollUp,
            MouseEventKind::ScrollDown,
            MouseEventKind::ScrollLeft,
            MouseEventKind::ScrollRight,
        ];
        for i in 0..kinds.len() {
            for j in (i + 1)..kinds.len() {
                assert_ne!(kinds[i], kinds[j], "kinds[{i}] == kinds[{j}]");
            }
        }
    }

    // --- Constructor: MouseEvent::new ---

    #[test]
    fn test_new_event_fields() {
        let e = MouseEvent::new(5, 10, MouseButton::Right, MouseEventKind::Release);
        assert_eq!(e.x, 5);
        assert_eq!(e.y, 10);
        assert_eq!(e.button, MouseButton::Right);
        assert_eq!(e.kind, MouseEventKind::Release);
        assert!(!e.shift);
        assert!(!e.ctrl);
        assert!(!e.alt);
    }

    #[test]
    fn test_new_event_no_modifiers_by_default() {
        let e = MouseEvent::new(0, 0, MouseButton::Left, MouseEventKind::Press);
        assert!(!e.shift);
        assert!(!e.ctrl);
        assert!(!e.alt);
    }

    // --- Factory: press ---

    #[test]
    fn test_press_left() {
        let e = MouseEvent::press(1, 2, MouseButton::Left);
        assert_eq!(e.kind, MouseEventKind::Press);
        assert_eq!(e.button, MouseButton::Left);
        assert_eq!(e.x, 1);
        assert_eq!(e.y, 2);
    }

    #[test]
    fn test_press_right() {
        let e = MouseEvent::press(3, 4, MouseButton::Right);
        assert_eq!(e.kind, MouseEventKind::Press);
        assert_eq!(e.button, MouseButton::Right);
    }

    #[test]
    fn test_press_middle() {
        let e = MouseEvent::press(0, 0, MouseButton::Middle);
        assert_eq!(e.kind, MouseEventKind::Press);
        assert_eq!(e.button, MouseButton::Middle);
    }

    // --- Factory: release ---

    #[test]
    fn test_release_event() {
        let e = MouseEvent::release(7, 8, MouseButton::Left);
        assert_eq!(e.kind, MouseEventKind::Release);
        assert_eq!(e.button, MouseButton::Left);
        assert_eq!(e.x, 7);
        assert_eq!(e.y, 8);
    }

    #[test]
    fn test_release_not_press() {
        let e = MouseEvent::release(0, 0, MouseButton::Left);
        assert!(!e.is_press());
        assert!(!e.is_scroll());
    }

    // --- Factory: move_to ---

    #[test]
    fn test_move_to_event() {
        let e = MouseEvent::move_to(100, 200);
        assert_eq!(e.kind, MouseEventKind::Move);
        assert_eq!(e.button, MouseButton::None);
        assert_eq!(e.x, 100);
        assert_eq!(e.y, 200);
    }

    #[test]
    fn test_move_is_not_press_or_scroll() {
        let e = MouseEvent::move_to(0, 0);
        assert!(!e.is_press());
        assert!(!e.is_scroll());
    }

    // --- Factory: scroll_up / scroll_down ---

    #[test]
    fn test_scroll_up_event() {
        let e = MouseEvent::scroll_up(5, 10);
        assert_eq!(e.kind, MouseEventKind::ScrollUp);
        assert_eq!(e.button, MouseButton::None);
        assert_eq!(e.x, 5);
        assert_eq!(e.y, 10);
        assert!(e.is_scroll());
    }

    #[test]
    fn test_scroll_down_event() {
        let e = MouseEvent::scroll_down(3, 7);
        assert_eq!(e.kind, MouseEventKind::ScrollDown);
        assert_eq!(e.button, MouseButton::None);
        assert!(e.is_scroll());
    }

    // --- is_scroll covers all scroll variants ---

    #[test]
    fn test_is_scroll_all_directions() {
        let up = MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::ScrollUp);
        let down = MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::ScrollDown);
        let left = MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::ScrollLeft);
        let right = MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::ScrollRight);
        assert!(up.is_scroll());
        assert!(down.is_scroll());
        assert!(left.is_scroll());
        assert!(right.is_scroll());
    }

    #[test]
    fn test_non_scroll_kinds() {
        let press = MouseEvent::new(0, 0, MouseButton::Left, MouseEventKind::Press);
        let release = MouseEvent::new(0, 0, MouseButton::Left, MouseEventKind::Release);
        let mov = MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::Move);
        assert!(!press.is_scroll());
        assert!(!release.is_scroll());
        assert!(!mov.is_scroll());
    }

    // --- is_press ---

    #[test]
    fn test_is_press_only_for_press() {
        assert!(MouseEvent::new(0, 0, MouseButton::Left, MouseEventKind::Press).is_press());
        assert!(!MouseEvent::new(0, 0, MouseButton::Left, MouseEventKind::Release).is_press());
        assert!(!MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::Move).is_press());
        assert!(!MouseEvent::new(0, 0, MouseButton::None, MouseEventKind::ScrollUp).is_press());
    }

    // --- with_modifiers ---

    #[test]
    fn test_with_modifiers_all_true() {
        let e = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(true, true, true);
        assert!(e.shift);
        assert!(e.ctrl);
        assert!(e.alt);
    }

    #[test]
    fn test_with_modifiers_all_false() {
        let e = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(false, false, false);
        assert!(!e.shift);
        assert!(!e.ctrl);
        assert!(!e.alt);
    }

    #[test]
    fn test_with_modifiers_shift_only() {
        let e = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(true, false, false);
        assert!(e.shift);
        assert!(!e.ctrl);
        assert!(!e.alt);
    }

    #[test]
    fn test_with_modifiers_ctrl_only() {
        let e = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(false, true, false);
        assert!(!e.shift);
        assert!(e.ctrl);
        assert!(!e.alt);
    }

    #[test]
    fn test_with_modifiers_alt_only() {
        let e = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(false, false, true);
        assert!(!e.shift);
        assert!(!e.ctrl);
        assert!(e.alt);
    }

    #[test]
    fn test_with_modifiers_preserves_event_data() {
        let e = MouseEvent::press(50, 75, MouseButton::Right).with_modifiers(true, true, false);
        assert_eq!(e.x, 50);
        assert_eq!(e.y, 75);
        assert_eq!(e.button, MouseButton::Right);
        assert_eq!(e.kind, MouseEventKind::Press);
    }

    // --- Coordinate edge cases ---

    #[test]
    fn test_event_at_origin() {
        let e = MouseEvent::press(0, 0, MouseButton::Left);
        assert_eq!(e.x, 0);
        assert_eq!(e.y, 0);
    }

    #[test]
    fn test_event_at_large_coordinates() {
        let e = MouseEvent::press(u32::MAX, u32::MAX, MouseButton::Left);
        assert_eq!(e.x, u32::MAX);
        assert_eq!(e.y, u32::MAX);
    }

    #[test]
    fn test_event_asymmetric_coordinates() {
        let e = MouseEvent::move_to(1000, 0);
        assert_eq!(e.x, 1000);
        assert_eq!(e.y, 0);
    }

    // --- Equality ---

    #[test]
    fn test_event_equality() {
        let a = MouseEvent::press(10, 20, MouseButton::Left);
        let b = MouseEvent::press(10, 20, MouseButton::Left);
        assert_eq!(a, b);
    }

    #[test]
    fn test_event_inequality_position() {
        let a = MouseEvent::press(10, 20, MouseButton::Left);
        let b = MouseEvent::press(11, 20, MouseButton::Left);
        assert_ne!(a, b);
    }

    #[test]
    fn test_event_inequality_button() {
        let a = MouseEvent::press(10, 20, MouseButton::Left);
        let b = MouseEvent::press(10, 20, MouseButton::Right);
        assert_ne!(a, b);
    }

    #[test]
    fn test_event_inequality_kind() {
        let a = MouseEvent::new(10, 20, MouseButton::Left, MouseEventKind::Press);
        let b = MouseEvent::new(10, 20, MouseButton::Left, MouseEventKind::Release);
        assert_ne!(a, b);
    }

    #[test]
    fn test_event_inequality_modifiers() {
        let a = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(true, false, false);
        let b = MouseEvent::press(0, 0, MouseButton::Left).with_modifiers(false, false, false);
        assert_ne!(a, b);
    }

    // --- Clone ---

    #[test]
    fn test_event_clone() {
        let e = MouseEvent::press(5, 10, MouseButton::Middle).with_modifiers(true, true, false);
        let cloned = e;
        assert_eq!(e, cloned);
    }

    // --- Debug ---

    #[test]
    fn test_event_debug() {
        let e = MouseEvent::press(1, 2, MouseButton::Left);
        let s = format!("{:?}", e);
        assert!(s.contains("MouseEvent"));
        assert!(s.contains("Press"));
        assert!(s.contains("Left"));
    }
}
