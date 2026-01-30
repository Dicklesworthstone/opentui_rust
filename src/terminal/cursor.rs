//! Cursor state and styles.

use crate::color::Rgba;

/// Cursor shape style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorStyle {
    /// Block cursor (█).
    #[default]
    Block,
    /// Underline cursor (_).
    Underline,
    /// Vertical bar cursor (|).
    Bar,
}

/// Cursor state.
#[derive(Clone, Copy, Debug)]
pub struct CursorState {
    /// X position (column).
    pub x: u32,
    /// Y position (row).
    pub y: u32,
    /// Whether cursor is visible.
    pub visible: bool,
    /// Cursor style.
    pub style: CursorStyle,
    /// Whether cursor is blinking.
    pub blinking: bool,
    /// Cursor color (None = terminal default).
    pub color: Option<Rgba>,
}

impl Default for CursorState {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            visible: true,
            style: CursorStyle::Block,
            blinking: true,
            color: None,
        }
    }
}

impl CursorState {
    /// Create a new cursor state at origin.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a cursor at a specific position.
    #[must_use]
    pub fn at(x: u32, y: u32) -> Self {
        Self {
            x,
            y,
            visible: true,
            style: CursorStyle::Block,
            blinking: true,
            color: None,
        }
    }

    /// Set cursor color.
    pub fn set_color(&mut self, color: Option<Rgba>) {
        self.color = color;
    }

    /// Set position.
    pub fn set_position(&mut self, x: u32, y: u32) {
        self.x = x;
        self.y = y;
    }

    /// Get position as tuple.
    #[must_use]
    pub fn position(&self) -> (u32, u32) {
        (self.x, self.y)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::uninlined_format_args)]
    use super::*;

    #[test]
    fn test_cursor_state() {
        let mut cursor = CursorState::new();
        assert!(cursor.visible);
        assert_eq!(cursor.style, CursorStyle::Block);

        cursor.set_position(10, 5);
        assert_eq!(cursor.position(), (10, 5));
    }

    // =============================================
    // Comprehensive Cursor Tests (bd-30ga)
    // =============================================

    // --- CursorStyle ---

    #[test]
    fn test_cursor_style_default_is_block() {
        assert_eq!(CursorStyle::default(), CursorStyle::Block);
    }

    #[test]
    fn test_cursor_style_variants_are_distinct() {
        assert_ne!(CursorStyle::Block, CursorStyle::Underline);
        assert_ne!(CursorStyle::Block, CursorStyle::Bar);
        assert_ne!(CursorStyle::Underline, CursorStyle::Bar);
    }

    #[test]
    fn test_cursor_style_clone() {
        let style = CursorStyle::Bar;
        let cloned = style;
        assert_eq!(style, cloned);
    }

    #[test]
    fn test_cursor_style_debug() {
        let s = format!("{:?}", CursorStyle::Underline);
        assert_eq!(s, "Underline");
    }

    // --- CursorState::new / default ---

    #[test]
    fn test_cursor_new_defaults() {
        let c = CursorState::new();
        assert_eq!(c.x, 0);
        assert_eq!(c.y, 0);
        assert!(c.visible);
        assert_eq!(c.style, CursorStyle::Block);
        assert!(c.blinking);
        assert!(c.color.is_none());
    }

    #[test]
    fn test_cursor_default_matches_new() {
        let a = CursorState::new();
        let b = CursorState::default();
        assert_eq!(a.x, b.x);
        assert_eq!(a.y, b.y);
        assert_eq!(a.visible, b.visible);
        assert_eq!(a.style, b.style);
        assert_eq!(a.blinking, b.blinking);
        assert_eq!(a.color.is_none(), b.color.is_none());
    }

    // --- CursorState::at ---

    #[test]
    fn test_cursor_at_origin() {
        let c = CursorState::at(0, 0);
        assert_eq!(c.position(), (0, 0));
        assert!(c.visible);
        assert_eq!(c.style, CursorStyle::Block);
        assert!(c.blinking);
        assert!(c.color.is_none());
    }

    #[test]
    fn test_cursor_at_arbitrary_position() {
        let c = CursorState::at(42, 99);
        assert_eq!(c.x, 42);
        assert_eq!(c.y, 99);
    }

    #[test]
    fn test_cursor_at_large_coordinates() {
        let c = CursorState::at(u32::MAX, u32::MAX);
        assert_eq!(c.position(), (u32::MAX, u32::MAX));
    }

    // --- set_position / position ---

    #[test]
    fn test_set_position_to_origin() {
        let mut c = CursorState::at(10, 20);
        c.set_position(0, 0);
        assert_eq!(c.position(), (0, 0));
    }

    #[test]
    fn test_set_position_multiple_times() {
        let mut c = CursorState::new();
        c.set_position(1, 2);
        assert_eq!(c.position(), (1, 2));
        c.set_position(100, 200);
        assert_eq!(c.position(), (100, 200));
        c.set_position(0, 0);
        assert_eq!(c.position(), (0, 0));
    }

    #[test]
    fn test_set_position_preserves_other_fields() {
        let mut c = CursorState::new();
        c.visible = false;
        c.style = CursorStyle::Bar;
        c.blinking = false;
        c.set_position(5, 10);
        assert!(!c.visible);
        assert_eq!(c.style, CursorStyle::Bar);
        assert!(!c.blinking);
    }

    // --- set_color ---

    #[test]
    fn test_set_color_some() {
        let mut c = CursorState::new();
        let color = Rgba::new(1.0, 0.0, 0.0, 1.0);
        c.set_color(Some(color));
        assert!(c.color.is_some());
    }

    #[test]
    fn test_set_color_none() {
        let mut c = CursorState::new();
        c.set_color(Some(Rgba::new(0.0, 1.0, 0.0, 1.0)));
        c.set_color(None);
        assert!(c.color.is_none());
    }

    #[test]
    fn test_set_color_preserves_position() {
        let mut c = CursorState::at(7, 3);
        c.set_color(Some(Rgba::new(0.0, 0.0, 1.0, 1.0)));
        assert_eq!(c.position(), (7, 3));
    }

    // --- Visibility ---

    #[test]
    fn test_visibility_toggle() {
        let mut c = CursorState::new();
        assert!(c.visible);
        c.visible = false;
        assert!(!c.visible);
        c.visible = true;
        assert!(c.visible);
    }

    // --- Style mutation ---

    #[test]
    fn test_style_change_to_all_variants() {
        let mut c = CursorState::new();
        assert_eq!(c.style, CursorStyle::Block);

        c.style = CursorStyle::Underline;
        assert_eq!(c.style, CursorStyle::Underline);

        c.style = CursorStyle::Bar;
        assert_eq!(c.style, CursorStyle::Bar);

        c.style = CursorStyle::Block;
        assert_eq!(c.style, CursorStyle::Block);
    }

    // --- Blinking ---

    #[test]
    fn test_blinking_default_true() {
        let c = CursorState::new();
        assert!(c.blinking);
    }

    #[test]
    fn test_blinking_toggle() {
        let mut c = CursorState::new();
        c.blinking = false;
        assert!(!c.blinking);
        c.blinking = true;
        assert!(c.blinking);
    }

    // --- Clone ---

    #[test]
    fn test_cursor_state_clone() {
        let mut c = CursorState::at(15, 25);
        c.visible = false;
        c.style = CursorStyle::Bar;
        c.blinking = false;
        c.set_color(Some(Rgba::new(1.0, 1.0, 0.0, 0.5)));

        let cloned = c;
        assert_eq!(cloned.x, 15);
        assert_eq!(cloned.y, 25);
        assert!(!cloned.visible);
        assert_eq!(cloned.style, CursorStyle::Bar);
        assert!(!cloned.blinking);
        assert!(cloned.color.is_some());
    }

    // --- Debug ---

    #[test]
    fn test_cursor_state_debug() {
        let c = CursorState::new();
        let s = format!("{:?}", c);
        assert!(s.contains("CursorState"));
        assert!(s.contains("visible"));
    }
}
