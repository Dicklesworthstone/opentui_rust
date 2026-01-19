//! Rope wrapper using the ropey crate.

use ropey::{Rope, RopeSlice};

/// Wrapper around ropey::Rope with convenience methods.
#[derive(Clone, Debug, Default)]
pub struct RopeWrapper {
    rope: Rope,
}

impl RopeWrapper {
    /// Create an empty rope.
    #[must_use]
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    /// Create a rope from a string.
    #[must_use]
    pub fn from_str(s: &str) -> Self {
        Self {
            rope: Rope::from_str(s),
        }
    }

    /// Get the number of bytes.
    #[must_use]
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Get the number of characters.
    #[must_use]
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Get the number of lines.
    #[must_use]
    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rope.len_bytes() == 0
    }

    /// Get a line by index.
    #[must_use]
    pub fn line(&self, idx: usize) -> Option<RopeSlice<'_>> {
        if idx < self.rope.len_lines() {
            Some(self.rope.line(idx))
        } else {
            None
        }
    }

    /// Iterate over all lines.
    pub fn lines(&self) -> impl Iterator<Item = RopeSlice<'_>> {
        self.rope.lines()
    }

    /// Get a slice of the rope.
    #[must_use]
    pub fn slice<R>(&self, range: R) -> RopeSlice<'_>
    where
        R: std::ops::RangeBounds<usize>,
    {
        self.rope
            .get_slice(range)
            .unwrap_or_else(|| self.rope.slice(..0))
    }

    /// Insert text at a character position.
    pub fn insert(&mut self, char_idx: usize, text: &str) {
        if char_idx <= self.len_chars() {
            self.rope.insert(char_idx, text);
        }
    }

    /// Remove a range of characters.
    pub fn remove<R>(&mut self, range: R)
    where
        R: std::ops::RangeBounds<usize>,
    {
        self.rope.remove(range);
    }

    /// Replace the entire contents.
    pub fn replace(&mut self, text: &str) {
        self.rope = Rope::from_str(text);
    }

    /// Append text to the end.
    pub fn append(&mut self, text: &str) {
        let len = self.len_chars();
        self.rope.insert(len, text);
    }

    /// Clear all content.
    pub fn clear(&mut self) {
        self.rope = Rope::new();
    }

    /// Convert to string.
    #[must_use]
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// Convert char index to byte index.
    #[must_use]
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.len_chars()))
    }

    /// Convert byte index to char index.
    #[must_use]
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx.min(self.len_bytes()))
    }

    /// Convert char index to line index.
    #[must_use]
    pub fn char_to_line(&self, char_idx: usize) -> usize {
        self.rope.char_to_line(char_idx.min(self.len_chars()))
    }

    /// Get the char index at the start of a line.
    #[must_use]
    pub fn line_to_char(&self, line_idx: usize) -> usize {
        if line_idx >= self.len_lines() {
            self.len_chars()
        } else {
            self.rope.line_to_char(line_idx)
        }
    }

    /// Get access to the underlying rope.
    #[must_use]
    pub fn inner(&self) -> &Rope {
        &self.rope
    }
}

impl From<&str> for RopeWrapper {
    fn from(s: &str) -> Self {
        Self::from_str(s)
    }
}

impl From<String> for RopeWrapper {
    fn from(s: String) -> Self {
        Self::from_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rope_basic() {
        let rope = RopeWrapper::from_str("Hello, world!");
        assert_eq!(rope.len_chars(), 13);
        assert_eq!(rope.len_lines(), 1);
    }

    #[test]
    fn test_rope_multiline() {
        let rope = RopeWrapper::from_str("Line 1\nLine 2\nLine 3");
        assert_eq!(rope.len_lines(), 3);
        assert_eq!(rope.line(0).unwrap().to_string(), "Line 1\n");
        assert_eq!(rope.line(2).unwrap().to_string(), "Line 3");
    }

    #[test]
    fn test_rope_insert() {
        let mut rope = RopeWrapper::from_str("Hello!");
        rope.insert(5, ", world");
        assert_eq!(rope.to_string(), "Hello, world!");
    }

    #[test]
    fn test_rope_remove() {
        let mut rope = RopeWrapper::from_str("Hello, world!");
        rope.remove(5..12);
        assert_eq!(rope.to_string(), "Hello!");
    }
}
