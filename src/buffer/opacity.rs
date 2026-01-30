//! Opacity stack for layered rendering.

/// Stack of opacity values that multiply together.
#[derive(Clone, Debug)]
pub struct OpacityStack {
    stack: Vec<f32>,
    current: f32,
}

impl OpacityStack {
    /// Create a new opacity stack with full opacity.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            current: 1.0,
        }
    }

    /// Push an opacity value onto the stack.
    ///
    /// The effective opacity is the product of all values on the stack.
    pub fn push(&mut self, opacity: f32) {
        self.stack.push(self.current);
        self.current *= opacity.clamp(0.0, 1.0);
    }

    /// Pop the top opacity value from the stack.
    pub fn pop(&mut self) {
        if let Some(prev) = self.stack.pop() {
            self.current = prev;
        }
    }

    /// Clear the stack, resetting to full opacity.
    pub fn clear(&mut self) {
        self.stack.clear();
        self.current = 1.0;
    }

    /// Get the current combined opacity value.
    #[must_use]
    pub fn current(&self) -> f32 {
        self.current
    }

    /// Check if current opacity is fully opaque.
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.current >= 1.0
    }

    /// Check if current opacity is fully transparent.
    #[must_use]
    pub fn is_transparent(&self) -> bool {
        self.current <= 0.0
    }
}

impl Default for OpacityStack {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparison is intentional in tests
    use super::*;

    #[test]
    fn test_opacity_default() {
        let stack = OpacityStack::new();
        assert!((stack.current() - 1.0).abs() < f32::EPSILON);
        assert!(stack.is_opaque());
    }

    #[test]
    fn test_opacity_multiply() {
        let mut stack = OpacityStack::new();

        stack.push(0.5);
        assert!((stack.current() - 0.5).abs() < f32::EPSILON);

        stack.push(0.5);
        assert!((stack.current() - 0.25).abs() < f32::EPSILON);

        stack.pop();
        assert!((stack.current() - 0.5).abs() < f32::EPSILON);

        stack.pop();
        assert!((stack.current() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_opacity_clamp() {
        let mut stack = OpacityStack::new();

        stack.push(2.0); // Should clamp to 1.0
        assert!((stack.current() - 1.0).abs() < f32::EPSILON);

        stack.push(-0.5); // Should clamp to 0.0
        assert!(stack.is_transparent());
    }

    #[test]
    fn test_opacity_clear() {
        let mut stack = OpacityStack::new();
        stack.push(0.5);
        stack.push(0.5);
        stack.clear();
        assert!((stack.current() - 1.0).abs() < f32::EPSILON);
    }

    // ============================================
    // Comprehensive Opacity Stack Tests (bd-3aps)
    // ============================================

    // --- Basic stack operations ---

    #[test]
    fn test_new_starts_opaque() {
        let s = OpacityStack::new();
        assert_eq!(s.current(), 1.0);
        assert!(s.is_opaque());
        assert!(!s.is_transparent());
    }

    #[test]
    fn test_default_same_as_new() {
        let s = OpacityStack::default();
        assert_eq!(s.current(), 1.0);
        assert!(s.is_opaque());
    }

    #[test]
    fn test_push_reduces_opacity() {
        let mut s = OpacityStack::new();
        s.push(0.8);
        assert!((s.current() - 0.8).abs() < 1e-6);
        assert!(!s.is_opaque());
        assert!(!s.is_transparent());
    }

    #[test]
    fn test_pop_restores_opacity() {
        let mut s = OpacityStack::new();
        s.push(0.3);
        assert!((s.current() - 0.3).abs() < 1e-6);

        s.pop();
        assert!((s.current() - 1.0).abs() < 1e-6);
    }

    // --- Multiplicative behavior ---

    #[test]
    fn test_two_half_opacities_make_quarter() {
        let mut s = OpacityStack::new();
        s.push(0.5);
        s.push(0.5);
        assert!((s.current() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_push_order_commutative() {
        // 0.3 then 0.7 should equal 0.7 then 0.3
        let mut s1 = OpacityStack::new();
        s1.push(0.3);
        s1.push(0.7);

        let mut s2 = OpacityStack::new();
        s2.push(0.7);
        s2.push(0.3);

        assert!((s1.current() - s2.current()).abs() < 1e-6);
    }

    #[test]
    fn test_three_layers_multiply() {
        let mut s = OpacityStack::new();
        s.push(0.5);
        s.push(0.5);
        s.push(0.5);
        // 0.5^3 = 0.125
        assert!((s.current() - 0.125).abs() < 1e-6);
    }

    #[test]
    fn test_pop_restores_each_level() {
        let mut s = OpacityStack::new();
        s.push(0.8); // 0.8
        s.push(0.5); // 0.4
        s.push(0.5); // 0.2

        assert!((s.current() - 0.2).abs() < 1e-5);
        s.pop();
        assert!((s.current() - 0.4).abs() < 1e-5);
        s.pop();
        assert!((s.current() - 0.8).abs() < 1e-6);
        s.pop();
        assert!((s.current() - 1.0).abs() < 1e-6);
    }

    // --- Edge cases ---

    #[test]
    fn test_push_zero_makes_transparent() {
        let mut s = OpacityStack::new();
        s.push(0.0);
        assert!(s.is_transparent());
        assert_eq!(s.current(), 0.0);
    }

    #[test]
    fn test_push_one_no_change() {
        let mut s = OpacityStack::new();
        s.push(1.0);
        assert!((s.current() - 1.0).abs() < 1e-6);
        assert!(s.is_opaque());
    }

    #[test]
    fn test_push_negative_clamps_to_zero() {
        let mut s = OpacityStack::new();
        s.push(-0.5);
        assert!(s.current() >= 0.0);
        assert!(s.is_transparent());
    }

    #[test]
    fn test_push_above_one_clamps_to_one() {
        let mut s = OpacityStack::new();
        s.push(2.0);
        assert!((s.current() - 1.0).abs() < 1e-6);
        assert!(s.is_opaque());
    }

    #[test]
    fn test_push_large_negative_clamps() {
        let mut s = OpacityStack::new();
        s.push(-100.0);
        assert_eq!(s.current(), 0.0);
    }

    #[test]
    fn test_pop_on_empty_stack_is_noop() {
        let mut s = OpacityStack::new();
        s.pop(); // Nothing pushed
        assert!((s.current() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_pop_multiple_on_empty_is_safe() {
        let mut s = OpacityStack::new();
        s.pop();
        s.pop();
        s.pop();
        assert!((s.current() - 1.0).abs() < 1e-6);
    }

    // --- Deep nesting ---

    #[test]
    fn test_deep_nesting_100_levels() {
        let mut s = OpacityStack::new();

        for _ in 0..100 {
            s.push(0.99);
        }

        // 0.99^100 ≈ 0.366
        let expected = 0.99_f32.powi(100);
        assert!((s.current() - expected).abs() < 0.01);

        // Pop all and verify restoration
        for _ in 0..100 {
            s.pop();
        }
        assert!((s.current() - 1.0).abs() < 1e-6);
    }

    // --- Precision ---

    #[test]
    fn test_many_small_opacities_precision() {
        let mut s = OpacityStack::new();

        // 10 layers of 0.9 = 0.9^10 ≈ 0.3486784401
        for _ in 0..10 {
            s.push(0.9);
        }

        let expected = 0.9_f32.powi(10);
        assert!(
            (s.current() - expected).abs() < 1e-4,
            "Expected ~{expected}, got {}",
            s.current()
        );
    }

    #[test]
    fn test_zero_opacity_stays_zero() {
        // Once opacity reaches 0, further pushes should keep it at 0
        let mut s = OpacityStack::new();
        s.push(0.0);
        s.push(0.5); // 0.0 * 0.5 = 0.0
        assert_eq!(s.current(), 0.0);
    }

    // --- is_opaque / is_transparent ---

    #[test]
    fn test_is_opaque_after_push_one() {
        let mut s = OpacityStack::new();
        s.push(1.0);
        s.push(1.0);
        assert!(s.is_opaque());
    }

    #[test]
    fn test_is_transparent_after_zero() {
        let mut s = OpacityStack::new();
        s.push(0.5);
        s.push(0.0);
        assert!(s.is_transparent());
    }

    #[test]
    fn test_neither_opaque_nor_transparent() {
        let mut s = OpacityStack::new();
        s.push(0.5);
        assert!(!s.is_opaque());
        assert!(!s.is_transparent());
    }

    // --- Clear ---

    #[test]
    fn test_clear_after_deep_nesting() {
        let mut s = OpacityStack::new();
        for _ in 0..50 {
            s.push(0.5);
        }
        s.clear();
        assert!((s.current() - 1.0).abs() < 1e-6);
        assert!(s.is_opaque());
    }

    #[test]
    fn test_clear_then_push_works() {
        let mut s = OpacityStack::new();
        s.push(0.1);
        s.clear();
        s.push(0.8);
        assert!((s.current() - 0.8).abs() < 1e-6);
    }
}
