//! Scissor (clipping) rectangle stack.

/// A clipping rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClipRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl ClipRect {
    /// Create a new clipping rectangle.
    #[must_use]
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is inside this rectangle.
    #[must_use]
    pub fn contains(&self, px: i32, py: i32) -> bool {
        if px < self.x || py < self.y {
            return false;
        }
        // Handle large dimensions to avoid i32 overflow
        let x_end = self.x.saturating_add_unsigned(self.width);
        let y_end = self.y.saturating_add_unsigned(self.height);
        px < x_end && py < y_end
    }

    /// Compute intersection with another rectangle.
    #[must_use]
    pub fn intersect(&self, other: &ClipRect) -> Option<ClipRect> {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        // Use saturating arithmetic to handle large dimensions
        let x2 = self
            .x
            .saturating_add_unsigned(self.width)
            .min(other.x.saturating_add_unsigned(other.width));
        let y2 = self
            .y
            .saturating_add_unsigned(self.height)
            .min(other.y.saturating_add_unsigned(other.height));

        if x2 > x1 && y2 > y1 {
            Some(ClipRect {
                x: x1,
                y: y1,
                width: (x2 - x1) as u32,
                height: (y2 - y1) as u32,
            })
        } else {
            None
        }
    }

    /// Check if this rectangle is empty (zero area).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

impl Default for ClipRect {
    fn default() -> Self {
        Self::new(0, 0, u32::MAX, u32::MAX)
    }
}

/// Stack of scissor rectangles with intersection.
#[derive(Clone, Debug, Default)]
pub struct ScissorStack {
    stack: Vec<ClipRect>,
    current: ClipRect,
}

impl ScissorStack {
    /// Create a new scissor stack with infinite bounds.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            current: ClipRect::default(),
        }
    }

    /// Push a new scissor rectangle, intersecting with current.
    pub fn push(&mut self, rect: ClipRect) {
        self.stack.push(self.current);
        self.current = self
            .current
            .intersect(&rect)
            .unwrap_or(ClipRect::new(0, 0, 0, 0));
    }

    /// Pop the top scissor rectangle.
    pub fn pop(&mut self) {
        if let Some(rect) = self.stack.pop() {
            self.current = rect;
        }
    }

    /// Clear the stack.
    pub fn clear(&mut self) {
        self.stack.clear();
        self.current = ClipRect::default();
    }

    /// Check if a point is within the current scissor region.
    #[must_use]
    pub fn contains(&self, x: i32, y: i32) -> bool {
        self.current.contains(x, y)
    }

    /// Get the current effective scissor rectangle.
    #[must_use]
    pub fn current(&self) -> ClipRect {
        self.current
    }

    /// Check if current scissor region is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clip_rect_contains() {
        let rect = ClipRect::new(10, 10, 20, 20);
        assert!(rect.contains(10, 10));
        assert!(rect.contains(29, 29));
        assert!(!rect.contains(30, 30));
        assert!(!rect.contains(9, 10));
    }

    #[test]
    fn test_clip_rect_intersect() {
        let a = ClipRect::new(0, 0, 20, 20);
        let b = ClipRect::new(10, 10, 20, 20);

        let c = a.intersect(&b).unwrap();
        assert_eq!(c.x, 10);
        assert_eq!(c.y, 10);
        assert_eq!(c.width, 10);
        assert_eq!(c.height, 10);
    }

    #[test]
    fn test_scissor_stack() {
        let mut stack = ScissorStack::new();

        // Default contains everything
        assert!(stack.contains(1000, 1000));

        stack.push(ClipRect::new(0, 0, 100, 100));
        assert!(stack.contains(50, 50));
        assert!(!stack.contains(150, 150));

        stack.push(ClipRect::new(25, 25, 50, 50));
        assert!(stack.contains(50, 50));
        assert!(!stack.contains(10, 10));

        stack.pop();
        assert!(stack.contains(10, 10));

        stack.pop();
        assert!(stack.contains(1000, 1000));
    }
}
