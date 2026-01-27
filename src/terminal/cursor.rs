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
    use super::*;

    #[test]
    fn test_cursor_state() {
        let mut cursor = CursorState::new();
        assert!(cursor.visible);
        assert_eq!(cursor.style, CursorStyle::Block);

        cursor.set_position(10, 5);
        assert_eq!(cursor.position(), (10, 5));
    }
}
