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

    // ============================================
    // ClipRect Creation & Properties (bd-2mlr)
    // ============================================

    #[test]
    fn test_clip_rect_new_stores_values() {
        let r = ClipRect::new(5, 10, 20, 30);
        assert_eq!(r.x, 5);
        assert_eq!(r.y, 10);
        assert_eq!(r.width, 20);
        assert_eq!(r.height, 30);
    }

    #[test]
    fn test_clip_rect_default_is_max_bounds() {
        let r = ClipRect::default();
        assert_eq!(r.x, 0);
        assert_eq!(r.y, 0);
        assert_eq!(r.width, u32::MAX);
        assert_eq!(r.height, u32::MAX);
    }

    #[test]
    fn test_clip_rect_is_empty_zero_width() {
        assert!(ClipRect::new(0, 0, 0, 10).is_empty());
    }

    #[test]
    fn test_clip_rect_is_empty_zero_height() {
        assert!(ClipRect::new(0, 0, 10, 0).is_empty());
    }

    #[test]
    fn test_clip_rect_is_empty_both_zero() {
        assert!(ClipRect::new(0, 0, 0, 0).is_empty());
    }

    #[test]
    fn test_clip_rect_not_empty() {
        assert!(!ClipRect::new(0, 0, 1, 1).is_empty());
    }

    // ============================================
    // ClipRect contains() Tests
    // ============================================

    #[test]
    fn test_contains_point_at_origin() {
        let r = ClipRect::new(0, 0, 10, 10);
        assert!(r.contains(0, 0));
    }

    #[test]
    fn test_contains_point_at_corners() {
        let r = ClipRect::new(10, 20, 30, 40);
        assert!(r.contains(10, 20)); // top-left (inclusive)
        assert!(r.contains(39, 59)); // bottom-right (last valid)
        assert!(!r.contains(40, 60)); // just outside
    }

    #[test]
    fn test_contains_point_on_edges() {
        let r = ClipRect::new(10, 10, 20, 20);
        // Top edge
        assert!(r.contains(15, 10));
        // Bottom edge (exclusive)
        assert!(!r.contains(15, 30));
        // Left edge
        assert!(r.contains(10, 15));
        // Right edge (exclusive)
        assert!(!r.contains(30, 15));
    }

    #[test]
    fn test_contains_negative_coordinates() {
        let r = ClipRect::new(-10, -10, 20, 20);
        assert!(r.contains(-10, -10)); // top-left
        assert!(r.contains(0, 0)); // center
        assert!(r.contains(9, 9)); // last valid
        assert!(!r.contains(10, 10)); // outside
        assert!(!r.contains(-11, 0)); // before left edge
    }

    #[test]
    fn test_contains_point_below_rect() {
        let r = ClipRect::new(0, 0, 10, 10);
        assert!(!r.contains(5, -1));
        assert!(!r.contains(-1, 5));
    }

    // ============================================
    // ClipRect intersect() Tests
    // ============================================

    #[test]
    fn test_intersect_full_overlap() {
        let a = ClipRect::new(0, 0, 20, 20);
        let b = ClipRect::new(0, 0, 20, 20);
        let c = a.intersect(&b).unwrap();
        assert_eq!(c, ClipRect::new(0, 0, 20, 20));
    }

    #[test]
    fn test_intersect_partial_overlap() {
        let a = ClipRect::new(0, 0, 20, 20);
        let b = ClipRect::new(10, 10, 20, 20);
        let c = a.intersect(&b).unwrap();
        assert_eq!(c, ClipRect::new(10, 10, 10, 10));
    }

    #[test]
    fn test_intersect_no_overlap() {
        let a = ClipRect::new(0, 0, 10, 10);
        let b = ClipRect::new(20, 20, 10, 10);
        assert_eq!(a.intersect(&b), None);
    }

    #[test]
    fn test_intersect_touching_edges() {
        // Rects share an edge but don't overlap
        let a = ClipRect::new(0, 0, 10, 10);
        let b = ClipRect::new(10, 0, 10, 10);
        assert_eq!(a.intersect(&b), None);
    }

    #[test]
    fn test_intersect_contained_rect() {
        // b is entirely inside a
        let a = ClipRect::new(0, 0, 100, 100);
        let b = ClipRect::new(20, 20, 30, 30);
        let c = a.intersect(&b).unwrap();
        assert_eq!(c, ClipRect::new(20, 20, 30, 30));
    }

    #[test]
    fn test_intersect_with_negative_coords() {
        let a = ClipRect::new(-10, -10, 30, 30);
        let b = ClipRect::new(0, 0, 30, 30);
        let c = a.intersect(&b).unwrap();
        assert_eq!(c, ClipRect::new(0, 0, 20, 20));
    }

    #[test]
    fn test_intersect_returns_none_for_zero_area() {
        // Rects share only a single point (zero-area intersection)
        let a = ClipRect::new(0, 0, 10, 10);
        let b = ClipRect::new(5, 10, 10, 10); // starts at y=10 where a ends
        assert_eq!(a.intersect(&b), None);
    }

    #[test]
    fn test_intersect_commutative() {
        let a = ClipRect::new(0, 0, 20, 20);
        let b = ClipRect::new(5, 5, 25, 25);
        assert_eq!(a.intersect(&b), b.intersect(&a));
    }

    // ============================================
    // ScissorStack Operations
    // ============================================

    #[test]
    fn test_stack_new_contains_everything() {
        let s = ScissorStack::new();
        assert!(s.contains(0, 0));
        assert!(s.contains(1000, 1000));
        assert!(s.contains(i32::MAX - 1, i32::MAX - 1));
    }

    #[test]
    fn test_stack_push_intersects_with_current() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(10, 10, 50, 50));
        // Current should be intersection of default and pushed rect
        let c = s.current();
        assert_eq!(c.x, 10);
        assert_eq!(c.y, 10);
        assert_eq!(c.width, 50);
        assert_eq!(c.height, 50);
    }

    #[test]
    fn test_stack_pop_restores_previous() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(10, 10, 50, 50));
        assert!(!s.contains(5, 5));

        s.pop();
        assert!(s.contains(5, 5)); // Restored to default (infinite)
    }

    #[test]
    fn test_stack_nested_scissors_intersect() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(0, 0, 100, 100));
        s.push(ClipRect::new(50, 50, 100, 100));

        let c = s.current();
        assert_eq!(c.x, 50);
        assert_eq!(c.y, 50);
        assert_eq!(c.width, 50); // min(100, 150) - 50 = 50
        assert_eq!(c.height, 50);
    }

    #[test]
    fn test_stack_triple_nested() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(0, 0, 100, 100));
        s.push(ClipRect::new(20, 20, 60, 60));
        s.push(ClipRect::new(30, 30, 20, 20));

        // Should be intersection of all three
        assert!(s.contains(35, 35));
        assert!(!s.contains(25, 25)); // In second but not third

        // Pop innermost
        s.pop();
        assert!(s.contains(25, 25)); // In second now active
        assert!(!s.contains(15, 15)); // Outside second

        // Pop second
        s.pop();
        assert!(s.contains(15, 15)); // In first now active
    }

    #[test]
    fn test_stack_push_outside_current_makes_empty() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(0, 0, 10, 10));
        s.push(ClipRect::new(50, 50, 10, 10)); // No overlap

        assert!(s.is_empty());
        assert!(!s.contains(5, 5));
        assert!(!s.contains(55, 55));

        // Pop should restore first scissor
        s.pop();
        assert!(!s.is_empty());
        assert!(s.contains(5, 5));
    }

    #[test]
    fn test_stack_pop_on_empty_stack_is_noop() {
        let mut s = ScissorStack::new();
        s.pop(); // Pop with nothing pushed
        // Should still work with default bounds
        assert!(s.contains(100, 100));
    }

    #[test]
    fn test_stack_clear_resets_to_default() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(10, 10, 5, 5));
        s.push(ClipRect::new(11, 11, 3, 3));

        s.clear();

        // Should be back to infinite bounds
        assert!(s.contains(0, 0));
        assert!(s.contains(1000, 1000));
        assert!(!s.is_empty());
    }

    #[test]
    fn test_stack_is_empty_after_disjoint_push() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(0, 0, 10, 10));
        assert!(!s.is_empty());

        s.push(ClipRect::new(100, 100, 10, 10)); // disjoint
        assert!(s.is_empty());
    }

    #[test]
    fn test_stack_current_reflects_active_scissor() {
        let mut s = ScissorStack::new();
        s.push(ClipRect::new(5, 5, 30, 30));

        let c = s.current();
        assert_eq!(c.x, 5);
        assert_eq!(c.y, 5);
        assert_eq!(c.width, 30);
        assert_eq!(c.height, 30);
    }

    #[test]
    fn test_stack_deep_nesting() {
        let mut s = ScissorStack::new();

        // Push 50 nested scissors, each shrinking by 1px on each side
        for i in 0..50 {
            s.push(ClipRect::new(i, i, 100 - 2 * i as u32, 100 - 2 * i as u32));
        }

        // Current should be the innermost: (49, 49, 2, 2)
        let c = s.current();
        assert_eq!(c.x, 49);
        assert_eq!(c.y, 49);
        assert_eq!(c.width, 2);
        assert_eq!(c.height, 2);

        // Pop all and verify restoration
        for _ in 0..50 {
            s.pop();
        }
        assert!(s.contains(1000, 1000)); // Back to default
    }

    // ============================================
    // ClipRect Eq & Clone
    // ============================================

    #[test]
    fn test_clip_rect_equality() {
        let a = ClipRect::new(1, 2, 3, 4);
        let b = ClipRect::new(1, 2, 3, 4);
        let c = ClipRect::new(1, 2, 3, 5);
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_clip_rect_clone() {
        let a = ClipRect::new(10, 20, 30, 40);
        let b = a;
        assert_eq!(a, b);
    }
}
