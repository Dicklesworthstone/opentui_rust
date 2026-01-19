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
}
