use crate::color::Rgba;
use crate::highlight::token::TokenKind;
use crate::style::Style;
use std::collections::HashMap;

/// A syntax highlighting theme that maps token kinds to styles and editor chrome colors.
#[derive(Clone, Debug)]
pub struct Theme {
    name: String,
    styles: [Option<Style>; TokenKind::COUNT],
    default_style: Style,

    background: Rgba,
    foreground: Rgba,
    selection: Rgba,
    cursor: Rgba,
    line_number: Rgba,
    line_number_active: Rgba,
    gutter: Rgba,
}

impl Theme {
    /// Create a new theme with sensible defaults.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            styles: [None; TokenKind::COUNT],
            default_style: Style::default(),
            background: Rgba::BLACK,
            foreground: Rgba::WHITE,
            selection: Rgba::from_rgb_u8(80, 80, 80),
            cursor: Rgba::WHITE,
            line_number: Rgba::from_rgb_u8(120, 120, 120),
            line_number_active: Rgba::WHITE,
            gutter: Rgba::from_rgb_u8(24, 24, 24),
        }
    }

    /// Theme name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the style for a token kind (falls back to default style).
    #[must_use]
    pub fn style_for(&self, kind: TokenKind) -> &Style {
        self.styles[kind.as_usize()]
            .as_ref()
            .unwrap_or(&self.default_style)
    }

    /// Theme default style.
    #[must_use]
    pub const fn default_style(&self) -> Style {
        self.default_style
    }

    /// Set a style for a token kind.
    pub fn set_style(&mut self, kind: TokenKind, style: Style) -> &mut Self {
        self.styles[kind.as_usize()] = Some(style);
        self
    }

    /// Builder-style style setter.
    #[must_use]
    pub fn with_style(mut self, kind: TokenKind, style: Style) -> Self {
        self.set_style(kind, style);
        self
    }

    /// Builder-style default style setter.
    #[must_use]
    pub fn with_default_style(mut self, style: Style) -> Self {
        self.default_style = style;
        self
    }

    /// Builder-style background setter.
    #[must_use]
    pub fn with_background(mut self, color: Rgba) -> Self {
        self.background = color;
        self
    }

    /// Builder-style foreground setter.
    #[must_use]
    pub fn with_foreground(mut self, color: Rgba) -> Self {
        self.foreground = color;
        self.default_style.fg = Some(color);
        self
    }

    /// Builder-style selection color setter.
    #[must_use]
    pub fn with_selection(mut self, color: Rgba) -> Self {
        self.selection = color;
        self
    }

    /// Builder-style cursor color setter.
    #[must_use]
    pub fn with_cursor(mut self, color: Rgba) -> Self {
        self.cursor = color;
        self
    }

    /// Builder-style line number color setter.
    #[must_use]
    pub fn with_line_number(mut self, color: Rgba) -> Self {
        self.line_number = color;
        self
    }

    /// Builder-style active line number color setter.
    #[must_use]
    pub fn with_line_number_active(mut self, color: Rgba) -> Self {
        self.line_number_active = color;
        self
    }

    /// Builder-style gutter color setter.
    #[must_use]
    pub fn with_gutter(mut self, color: Rgba) -> Self {
        self.gutter = color;
        self
    }

    /// Theme background color.
    #[must_use]
    pub const fn background(&self) -> Rgba {
        self.background
    }

    /// Theme foreground color.
    #[must_use]
    pub const fn foreground(&self) -> Rgba {
        self.foreground
    }

    /// Theme selection color.
    #[must_use]
    pub const fn selection(&self) -> Rgba {
        self.selection
    }

    /// Theme cursor color.
    #[must_use]
    pub const fn cursor(&self) -> Rgba {
        self.cursor
    }

    /// Theme line number color.
    #[must_use]
    pub const fn line_number(&self) -> Rgba {
        self.line_number
    }

    /// Theme active line number color.
    #[must_use]
    pub const fn line_number_active(&self) -> Rgba {
        self.line_number_active
    }

    /// Theme gutter color.
    #[must_use]
    pub const fn gutter(&self) -> Rgba {
        self.gutter
    }

    /// Dark theme inspired by popular editor palettes.
    #[must_use]
    pub fn dark() -> Self {
        let background = Rgba::from_hex("#282a36").unwrap();
        let foreground = Rgba::from_hex("#f8f8f2").unwrap();
        let comment = Rgba::from_hex("#6272a4").unwrap();
        let keyword = Rgba::from_hex("#ff79c6").unwrap();
        let types = Rgba::from_hex("#8be9fd").unwrap();
        let string = Rgba::from_hex("#f1fa8c").unwrap();
        let number = Rgba::from_hex("#bd93f9").unwrap();
        let function = Rgba::from_hex("#50fa7b").unwrap();
        let selection = Rgba::from_hex("#44475a").unwrap();
        let gutter = Rgba::from_hex("#21222c").unwrap();

        Self::new("Dark")
            .with_background(background)
            .with_foreground(foreground)
            .with_selection(selection)
            .with_cursor(foreground)
            .with_line_number(comment)
            .with_line_number_active(foreground)
            .with_gutter(gutter)
            .with_style(TokenKind::Keyword, Style::fg(keyword))
            .with_style(TokenKind::KeywordControl, Style::fg(keyword))
            .with_style(TokenKind::KeywordModifier, Style::fg(keyword))
            .with_style(TokenKind::KeywordType, Style::fg(types).with_italic())
            .with_style(TokenKind::Type, Style::fg(types))
            .with_style(TokenKind::Function, Style::fg(function))
            .with_style(TokenKind::String, Style::fg(string))
            .with_style(TokenKind::StringEscape, Style::fg(string).with_bold())
            .with_style(TokenKind::Number, Style::fg(number))
            .with_style(TokenKind::Boolean, Style::fg(number))
            .with_style(TokenKind::Comment, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentBlock, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentDoc, Style::fg(comment).with_italic())
            .with_style(TokenKind::Attribute, Style::fg(function))
            .with_style(TokenKind::Macro, Style::fg(function))
            .with_style(TokenKind::Operator, Style::fg(keyword))
            .with_style(TokenKind::Punctuation, Style::fg(foreground))
            .with_style(TokenKind::Lifetime, Style::fg(types))
            .with_style(TokenKind::Label, Style::fg(function))
            .with_style(TokenKind::Error, Style::fg(Rgba::RED).with_bold())
    }

    /// Light theme for bright environments.
    #[must_use]
    pub fn light() -> Self {
        let background = Rgba::from_hex("#ffffff").unwrap();
        let foreground = Rgba::from_hex("#24292e").unwrap();
        let comment = Rgba::from_hex("#6a737d").unwrap();
        let keyword = Rgba::from_hex("#d73a49").unwrap();
        let types = Rgba::from_hex("#005cc5").unwrap();
        let string = Rgba::from_hex("#032f62").unwrap();
        let number = Rgba::from_hex("#6f42c1").unwrap();
        let function = Rgba::from_hex("#22863a").unwrap();
        let selection = Rgba::from_hex("#cce5ff").unwrap();
        let gutter = Rgba::from_hex("#f6f8fa").unwrap();

        Self::new("Light")
            .with_background(background)
            .with_foreground(foreground)
            .with_selection(selection)
            .with_cursor(foreground)
            .with_line_number(comment)
            .with_line_number_active(foreground)
            .with_gutter(gutter)
            .with_style(TokenKind::Keyword, Style::fg(keyword))
            .with_style(TokenKind::KeywordControl, Style::fg(keyword))
            .with_style(TokenKind::KeywordModifier, Style::fg(keyword))
            .with_style(TokenKind::KeywordType, Style::fg(types))
            .with_style(TokenKind::Type, Style::fg(types))
            .with_style(TokenKind::Function, Style::fg(function))
            .with_style(TokenKind::String, Style::fg(string))
            .with_style(TokenKind::StringEscape, Style::fg(string).with_bold())
            .with_style(TokenKind::Number, Style::fg(number))
            .with_style(TokenKind::Boolean, Style::fg(number))
            .with_style(TokenKind::Comment, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentBlock, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentDoc, Style::fg(comment).with_italic())
            .with_style(TokenKind::Attribute, Style::fg(function))
            .with_style(TokenKind::Macro, Style::fg(function))
            .with_style(TokenKind::Operator, Style::fg(keyword))
            .with_style(TokenKind::Punctuation, Style::fg(foreground))
            .with_style(TokenKind::Lifetime, Style::fg(types))
            .with_style(TokenKind::Label, Style::fg(function))
            .with_style(TokenKind::Error, Style::fg(Rgba::RED).with_bold())
    }

    /// High-contrast theme for accessibility.
    #[must_use]
    pub fn high_contrast() -> Self {
        let background = Rgba::BLACK;
        let foreground = Rgba::WHITE;
        let accent = Rgba::from_hex("#00ffff").unwrap();
        let warning = Rgba::from_hex("#ffff00").unwrap();
        let selection = Rgba::from_hex("#333333").unwrap();

        Self::new("High Contrast")
            .with_background(background)
            .with_foreground(foreground)
            .with_selection(selection)
            .with_cursor(foreground)
            .with_line_number(accent)
            .with_line_number_active(foreground)
            .with_gutter(Rgba::from_hex("#111111").unwrap())
            .with_style(TokenKind::Keyword, Style::fg(accent).with_bold())
            .with_style(TokenKind::KeywordControl, Style::fg(accent).with_bold())
            .with_style(TokenKind::KeywordModifier, Style::fg(accent).with_bold())
            .with_style(TokenKind::KeywordType, Style::fg(accent))
            .with_style(TokenKind::Type, Style::fg(accent))
            .with_style(
                TokenKind::Function,
                Style::fg(Rgba::from_hex("#00ff00").unwrap()),
            )
            .with_style(TokenKind::String, Style::fg(warning))
            .with_style(
                TokenKind::Number,
                Style::fg(Rgba::from_hex("#ff00ff").unwrap()),
            )
            .with_style(
                TokenKind::Boolean,
                Style::fg(Rgba::from_hex("#ff00ff").unwrap()),
            )
            .with_style(
                TokenKind::Comment,
                Style::fg(Rgba::from_hex("#888888").unwrap()),
            )
            .with_style(
                TokenKind::CommentBlock,
                Style::fg(Rgba::from_hex("#888888").unwrap()),
            )
            .with_style(
                TokenKind::CommentDoc,
                Style::fg(Rgba::from_hex("#888888").unwrap()),
            )
            .with_style(
                TokenKind::Attribute,
                Style::fg(Rgba::from_hex("#00ff00").unwrap()),
            )
            .with_style(
                TokenKind::Macro,
                Style::fg(Rgba::from_hex("#00ff00").unwrap()),
            )
            .with_style(TokenKind::Operator, Style::fg(accent))
            .with_style(TokenKind::Punctuation, Style::fg(foreground))
            .with_style(TokenKind::Lifetime, Style::fg(accent))
            .with_style(TokenKind::Label, Style::fg(accent))
            .with_style(TokenKind::Error, Style::fg(Rgba::RED).with_bold())
    }

    /// Monochrome theme using styles instead of colors.
    #[must_use]
    pub fn monochrome() -> Self {
        let background = Rgba::BLACK;
        let foreground = Rgba::from_hex("#e0e0e0").unwrap();
        let selection = Rgba::from_hex("#444444").unwrap();

        Self::new("Monochrome")
            .with_background(background)
            .with_foreground(foreground)
            .with_selection(selection)
            .with_cursor(foreground)
            .with_line_number(Rgba::from_hex("#666666").unwrap())
            .with_line_number_active(foreground)
            .with_gutter(Rgba::from_hex("#222222").unwrap())
            .with_style(TokenKind::Keyword, Style::fg(foreground).with_bold())
            .with_style(TokenKind::KeywordControl, Style::fg(foreground).with_bold())
            .with_style(
                TokenKind::KeywordModifier,
                Style::fg(foreground).with_bold(),
            )
            .with_style(TokenKind::KeywordType, Style::fg(foreground))
            .with_style(TokenKind::Type, Style::fg(foreground))
            .with_style(TokenKind::Function, Style::fg(foreground))
            .with_style(TokenKind::String, Style::fg(foreground).with_underline())
            .with_style(TokenKind::Number, Style::fg(foreground))
            .with_style(TokenKind::Boolean, Style::fg(foreground))
            .with_style(TokenKind::Comment, Style::fg(foreground).with_italic())
            .with_style(TokenKind::CommentBlock, Style::fg(foreground).with_italic())
            .with_style(TokenKind::CommentDoc, Style::fg(foreground).with_italic())
            .with_style(TokenKind::Attribute, Style::fg(foreground))
            .with_style(TokenKind::Macro, Style::fg(foreground))
            .with_style(TokenKind::Operator, Style::fg(foreground))
            .with_style(TokenKind::Punctuation, Style::fg(foreground))
            .with_style(TokenKind::Lifetime, Style::fg(foreground))
            .with_style(TokenKind::Label, Style::fg(foreground))
            .with_style(TokenKind::Error, Style::fg(Rgba::RED).with_bold())
    }

    /// Solarized dark theme.
    #[must_use]
    pub fn solarized_dark() -> Self {
        let background = Rgba::from_hex("#002b36").unwrap();
        let foreground = Rgba::from_hex("#839496").unwrap();
        let comment = Rgba::from_hex("#586e75").unwrap();
        let keyword = Rgba::from_hex("#268bd2").unwrap();
        let types = Rgba::from_hex("#b58900").unwrap();
        let string = Rgba::from_hex("#2aa198").unwrap();
        let number = Rgba::from_hex("#d33682").unwrap();
        let selection = Rgba::from_hex("#073642").unwrap();
        let gutter = Rgba::from_hex("#073642").unwrap();

        Self::new("Solarized Dark")
            .with_background(background)
            .with_foreground(foreground)
            .with_selection(selection)
            .with_cursor(foreground)
            .with_line_number(comment)
            .with_line_number_active(foreground)
            .with_gutter(gutter)
            .with_style(TokenKind::Keyword, Style::fg(keyword))
            .with_style(TokenKind::KeywordControl, Style::fg(keyword))
            .with_style(TokenKind::KeywordModifier, Style::fg(keyword))
            .with_style(TokenKind::KeywordType, Style::fg(types))
            .with_style(TokenKind::Type, Style::fg(types))
            .with_style(
                TokenKind::Function,
                Style::fg(Rgba::from_hex("#859900").unwrap()),
            )
            .with_style(TokenKind::String, Style::fg(string))
            .with_style(TokenKind::Number, Style::fg(number))
            .with_style(TokenKind::Boolean, Style::fg(number))
            .with_style(TokenKind::Comment, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentBlock, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentDoc, Style::fg(comment).with_italic())
            .with_style(TokenKind::Attribute, Style::fg(keyword))
            .with_style(TokenKind::Macro, Style::fg(keyword))
            .with_style(TokenKind::Operator, Style::fg(keyword))
            .with_style(TokenKind::Punctuation, Style::fg(foreground))
            .with_style(TokenKind::Lifetime, Style::fg(types))
            .with_style(TokenKind::Label, Style::fg(types))
            .with_style(TokenKind::Error, Style::fg(Rgba::RED).with_bold())
    }

    /// Solarized light theme.
    #[must_use]
    pub fn solarized_light() -> Self {
        let background = Rgba::from_hex("#fdf6e3").unwrap();
        let foreground = Rgba::from_hex("#657b83").unwrap();
        let comment = Rgba::from_hex("#93a1a1").unwrap();
        let keyword = Rgba::from_hex("#268bd2").unwrap();
        let types = Rgba::from_hex("#b58900").unwrap();
        let string = Rgba::from_hex("#2aa198").unwrap();
        let number = Rgba::from_hex("#d33682").unwrap();
        let selection = Rgba::from_hex("#eee8d5").unwrap();
        let gutter = Rgba::from_hex("#eee8d5").unwrap();

        Self::new("Solarized Light")
            .with_background(background)
            .with_foreground(foreground)
            .with_selection(selection)
            .with_cursor(foreground)
            .with_line_number(comment)
            .with_line_number_active(foreground)
            .with_gutter(gutter)
            .with_style(TokenKind::Keyword, Style::fg(keyword))
            .with_style(TokenKind::KeywordControl, Style::fg(keyword))
            .with_style(TokenKind::KeywordModifier, Style::fg(keyword))
            .with_style(TokenKind::KeywordType, Style::fg(types))
            .with_style(TokenKind::Type, Style::fg(types))
            .with_style(
                TokenKind::Function,
                Style::fg(Rgba::from_hex("#859900").unwrap()),
            )
            .with_style(TokenKind::String, Style::fg(string))
            .with_style(TokenKind::Number, Style::fg(number))
            .with_style(TokenKind::Boolean, Style::fg(number))
            .with_style(TokenKind::Comment, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentBlock, Style::fg(comment).with_italic())
            .with_style(TokenKind::CommentDoc, Style::fg(comment).with_italic())
            .with_style(TokenKind::Attribute, Style::fg(keyword))
            .with_style(TokenKind::Macro, Style::fg(keyword))
            .with_style(TokenKind::Operator, Style::fg(keyword))
            .with_style(TokenKind::Punctuation, Style::fg(foreground))
            .with_style(TokenKind::Lifetime, Style::fg(types))
            .with_style(TokenKind::Label, Style::fg(types))
            .with_style(TokenKind::Error, Style::fg(Rgba::RED).with_bold())
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

#[derive(Default)]
pub struct ThemeRegistry {
    themes: HashMap<String, Theme>,
    current: String,
}

impl ThemeRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a registry with built-in themes ("dark" default).
    #[must_use]
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        registry.register(Theme::dark());
        registry.register(Theme::light());
        registry.register(Theme::high_contrast());
        registry.register(Theme::solarized_dark());
        registry.register(Theme::solarized_light());
        registry.register(Theme::monochrome());
        registry.current = "dark".to_string();
        registry
    }

    /// Register a theme by name (case-insensitive).
    pub fn register(&mut self, theme: Theme) {
        let key = theme.name.to_ascii_lowercase();
        if self.current.is_empty() {
            self.current.clone_from(&key);
        }
        self.themes.insert(key, theme);
    }

    /// Get a theme by name (case-insensitive).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&Theme> {
        self.themes.get(&name.to_ascii_lowercase())
    }

    /// Get the current theme.
    #[must_use]
    pub fn current(&self) -> &Theme {
        self.themes
            .get(&self.current)
            .expect("current theme missing")
    }

    /// Set the current theme.
    pub fn set_current(&mut self, name: &str) -> Result<(), &'static str> {
        let key = name.to_ascii_lowercase();
        if self.themes.contains_key(&key) {
            self.current = key;
            Ok(())
        } else {
            Err("theme not found")
        }
    }

    /// List registered theme names.
    pub fn list(&self) -> impl Iterator<Item = &str> {
        self.themes.keys().map(String::as_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_fallback_and_override() {
        let theme = Theme::new("Test").with_foreground(Rgba::WHITE);
        let default_style = *theme.style_for(TokenKind::Text);
        let keyword_style = *theme.style_for(TokenKind::Keyword);
        assert_eq!(default_style, keyword_style);

        let custom = Style::fg(Rgba::RED).with_bold();
        let themed = theme.with_style(TokenKind::Keyword, custom);
        assert_eq!(*themed.style_for(TokenKind::Keyword), custom);
        assert_eq!(*themed.style_for(TokenKind::String), default_style);
    }

    #[test]
    fn builtins_define_core_styles() {
        let theme = Theme::dark();
        assert!(theme.style_for(TokenKind::Keyword).fg.is_some());
        assert!(theme.style_for(TokenKind::String).fg.is_some());
        assert!(theme.style_for(TokenKind::Comment).fg.is_some());
    }

    #[test]
    fn registry_switching() {
        let mut registry = ThemeRegistry::with_builtins();
        assert!(registry.get("dark").is_some());
        assert_eq!(registry.current().name(), "Dark");
        assert!(registry.set_current("light").is_ok());
        assert_eq!(registry.current().name(), "Light");
    }
}
