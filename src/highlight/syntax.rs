//! Syntax style definitions and registry.

use crate::style::Style;
use std::collections::HashMap;

/// Named style for syntax highlighting.
#[derive(Clone, Debug)]
pub struct SyntaxStyle {
    /// Unique identifier.
    pub id: u32,
    /// Human-readable name.
    pub name: String,
    /// The style to apply.
    pub style: Style,
}

impl SyntaxStyle {
    /// Create a new syntax style.
    #[must_use]
    pub fn new(id: u32, name: impl Into<String>, style: Style) -> Self {
        Self {
            id,
            name: name.into(),
            style,
        }
    }
}

/// Registry of syntax styles for a theme or language.
#[derive(Clone, Debug, Default)]
pub struct SyntaxStyleRegistry {
    styles: HashMap<u32, SyntaxStyle>,
    by_name: HashMap<String, u32>,
    next_id: u32,
}

impl SyntaxStyleRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a style with auto-generated ID.
    pub fn register(&mut self, name: impl Into<String>, style: Style) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let name = name.into();
        self.by_name.insert(name.clone(), id);
        self.styles.insert(id, SyntaxStyle::new(id, name, style));

        id
    }

    /// Register a style with a specific ID.
    pub fn register_with_id(&mut self, id: u32, name: impl Into<String>, style: Style) {
        let name = name.into();
        self.by_name.insert(name.clone(), id);
        self.styles.insert(id, SyntaxStyle::new(id, name, style));
        self.next_id = self.next_id.max(id.saturating_add(1));
    }

    /// Get a style by ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&SyntaxStyle> {
        self.styles.get(&id)
    }

    /// Get a style by name.
    #[must_use]
    pub fn get_by_name(&self, name: &str) -> Option<&SyntaxStyle> {
        self.by_name.get(name).and_then(|id| self.styles.get(id))
    }

    /// Get style ID by name.
    #[must_use]
    pub fn id_for_name(&self, name: &str) -> Option<u32> {
        self.by_name.get(name).copied()
    }

    /// Get the style (Style struct) by ID.
    #[must_use]
    pub fn style(&self, id: u32) -> Option<Style> {
        self.styles.get(&id).map(|s| s.style)
    }

    /// Check if a style with the given ID exists.
    #[must_use]
    pub fn contains(&self, id: u32) -> bool {
        self.styles.contains_key(&id)
    }

    /// Get the number of registered styles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Check if empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }

    /// Iterate over all styles.
    pub fn iter(&self) -> impl Iterator<Item = &SyntaxStyle> {
        self.styles.values()
    }

    /// Clear all styles.
    pub fn clear(&mut self) {
        self.styles.clear();
        self.by_name.clear();
        self.next_id = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Rgba;

    #[test]
    fn test_registry_basic() {
        let mut registry = SyntaxStyleRegistry::new();
        let id = registry.register("keyword", Style::fg(Rgba::BLUE).with_bold());

        assert_eq!(registry.len(), 1);
        assert!(registry.contains(id));
        assert_eq!(registry.get(id).unwrap().name, "keyword");
    }

    #[test]
    fn test_registry_by_name() {
        let mut registry = SyntaxStyleRegistry::new();
        registry.register("string", Style::fg(Rgba::GREEN));

        let style = registry.get_by_name("string").unwrap();
        assert_eq!(style.name, "string");
        assert_eq!(style.style.fg, Some(Rgba::GREEN));
    }

    #[test]
    fn test_registry_with_id() {
        let mut registry = SyntaxStyleRegistry::new();
        registry.register_with_id(100, "comment", Style::dim());

        assert!(registry.contains(100));
        assert_eq!(registry.id_for_name("comment"), Some(100));
    }
}
