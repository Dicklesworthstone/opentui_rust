//! Text styling with attributes and colors.
//!
//! This module provides types for styling text in the terminal:
//!
//! - [`TextAttributes`]: Bitflags for bold, italic, underline, etc.
//! - [`Style`]: Complete styling including colors, attributes, and hyperlinks
//! - [`StyleBuilder`]: Fluent builder for constructing styles
//!
//! # Examples
//!
//! ```
//! use opentui::{Style, TextAttributes, Rgba};
//!
//! // Quick style creation
//! let title_style = Style::fg(Rgba::WHITE).with_bold();
//!
//! // Builder pattern for complex styles
//! let highlight = Style::builder()
//!     .fg(Rgba::from_hex("#FFD700").unwrap())
//!     .bg(Rgba::from_hex("#1a1a2e").unwrap())
//!     .bold()
//!     .underline()
//!     .build();
//!
//! // Merge styles (overlay takes precedence)
//! let combined = Style::bold().merge(Style::fg(Rgba::RED));
//! ```

use crate::color::Rgba;
use bitflags::bitflags;

bitflags! {
    /// Text rendering attributes (bold, italic, etc.).
    ///
    /// Attributes are represented as bitflags and can be combined using
    /// bitwise OR. Not all terminals support all attributes.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
    pub struct TextAttributes: u8 {
        /// Bold/increased intensity.
        const BOLD          = 0x01;
        /// Dim/decreased intensity.
        const DIM           = 0x02;
        /// Italic (not widely supported).
        const ITALIC        = 0x04;
        /// Underlined text.
        const UNDERLINE     = 0x08;
        /// Blinking text (rarely supported).
        const BLINK         = 0x10;
        /// Swapped foreground/background.
        const INVERSE       = 0x20;
        /// Hidden/invisible text.
        const HIDDEN        = 0x40;
        /// Strikethrough text.
        const STRIKETHROUGH = 0x80;
    }
}

/// Complete text style including colors, attributes, and optional hyperlink.
///
/// Styles are immutable and cheap to copy. Use the builder methods to create
/// modified versions, or [`Style::merge`] to combine multiple styles.
///
/// # Default Values
///
/// `None` for colors means "use terminal default" rather than a specific color.
/// This allows styled text to respect the user's terminal theme.
///
/// # Hyperlinks
///
/// The `link_id` field references URLs stored in a [`LinkPool`](crate::LinkPool).
/// Terminals supporting OSC 8 will render these as clickable links.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    /// Foreground color (None = terminal default).
    pub fg: Option<Rgba>,
    /// Background color (None = terminal default).
    pub bg: Option<Rgba>,
    /// Text rendering attributes.
    pub attributes: TextAttributes,
    /// Optional hyperlink ID (for OSC 8 links).
    pub link_id: Option<u32>,
}

impl Style {
    /// Empty style with no colors or attributes.
    pub const NONE: Self = Self {
        fg: None,
        bg: None,
        attributes: TextAttributes::empty(),
        link_id: None,
    };

    /// Create a new style builder.
    #[must_use]
    pub fn builder() -> StyleBuilder {
        StyleBuilder::default()
    }

    /// Create a style with only foreground color.
    #[must_use]
    pub const fn fg(color: Rgba) -> Self {
        Self {
            fg: Some(color),
            bg: None,
            attributes: TextAttributes::empty(),
            link_id: None,
        }
    }

    /// Create a style with only background color.
    #[must_use]
    pub const fn bg(color: Rgba) -> Self {
        Self {
            fg: None,
            bg: Some(color),
            attributes: TextAttributes::empty(),
            link_id: None,
        }
    }

    /// Create a bold style.
    #[must_use]
    pub const fn bold() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::BOLD,
            link_id: None,
        }
    }

    /// Create an italic style.
    #[must_use]
    pub const fn italic() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::ITALIC,
            link_id: None,
        }
    }

    /// Create an underline style.
    #[must_use]
    pub const fn underline() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::UNDERLINE,
            link_id: None,
        }
    }

    /// Create a dim style.
    #[must_use]
    pub const fn dim() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::DIM,
            link_id: None,
        }
    }

    /// Create an inverse (swapped fg/bg) style.
    #[must_use]
    pub const fn inverse() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::INVERSE,
            link_id: None,
        }
    }

    /// Create a strikethrough style.
    #[must_use]
    pub const fn strikethrough() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::STRIKETHROUGH,
            link_id: None,
        }
    }

    /// Return a new style with the specified foreground color.
    #[must_use]
    pub const fn with_fg(self, color: Rgba) -> Self {
        Self {
            fg: Some(color),
            ..self
        }
    }

    /// Return a new style with the specified background color.
    #[must_use]
    pub const fn with_bg(self, color: Rgba) -> Self {
        Self {
            bg: Some(color),
            ..self
        }
    }

    /// Return a new style with the specified attributes added.
    #[must_use]
    pub const fn with_attributes(self, attrs: TextAttributes) -> Self {
        Self {
            attributes: self.attributes.union(attrs),
            ..self
        }
    }

    /// Return a new style with the bold attribute added.
    #[must_use]
    pub const fn with_bold(self) -> Self {
        self.with_attributes(TextAttributes::BOLD)
    }

    /// Return a new style with the italic attribute added.
    #[must_use]
    pub const fn with_italic(self) -> Self {
        self.with_attributes(TextAttributes::ITALIC)
    }

    /// Return a new style with the underline attribute added.
    #[must_use]
    pub const fn with_underline(self) -> Self {
        self.with_attributes(TextAttributes::UNDERLINE)
    }

    /// Return a new style with a hyperlink ID.
    #[must_use]
    pub const fn with_link(self, link_id: u32) -> Self {
        Self {
            link_id: Some(link_id),
            ..self
        }
    }

    /// Check if this style has any non-default properties.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fg.is_none()
            && self.bg.is_none()
            && self.attributes.is_empty()
            && self.link_id.is_none()
    }

    /// Merge two styles, with `other` taking precedence for set values.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            attributes: self.attributes | other.attributes,
            link_id: other.link_id.or(self.link_id),
        }
    }
}

/// Builder for creating styles fluently.
#[derive(Clone, Debug, Default)]
pub struct StyleBuilder {
    style: Style,
}

impl StyleBuilder {
    /// Set foreground color.
    #[must_use]
    pub fn fg(mut self, color: Rgba) -> Self {
        self.style.fg = Some(color);
        self
    }

    /// Set background color.
    #[must_use]
    pub fn bg(mut self, color: Rgba) -> Self {
        self.style.bg = Some(color);
        self
    }

    /// Add bold attribute.
    #[must_use]
    pub fn bold(mut self) -> Self {
        self.style.attributes |= TextAttributes::BOLD;
        self
    }

    /// Add dim attribute.
    #[must_use]
    pub fn dim(mut self) -> Self {
        self.style.attributes |= TextAttributes::DIM;
        self
    }

    /// Add italic attribute.
    #[must_use]
    pub fn italic(mut self) -> Self {
        self.style.attributes |= TextAttributes::ITALIC;
        self
    }

    /// Add underline attribute.
    #[must_use]
    pub fn underline(mut self) -> Self {
        self.style.attributes |= TextAttributes::UNDERLINE;
        self
    }

    /// Add blink attribute.
    #[must_use]
    pub fn blink(mut self) -> Self {
        self.style.attributes |= TextAttributes::BLINK;
        self
    }

    /// Add inverse attribute.
    #[must_use]
    pub fn inverse(mut self) -> Self {
        self.style.attributes |= TextAttributes::INVERSE;
        self
    }

    /// Add hidden attribute.
    #[must_use]
    pub fn hidden(mut self) -> Self {
        self.style.attributes |= TextAttributes::HIDDEN;
        self
    }

    /// Add strikethrough attribute.
    #[must_use]
    pub fn strikethrough(mut self) -> Self {
        self.style.attributes |= TextAttributes::STRIKETHROUGH;
        self
    }

    /// Set hyperlink ID.
    #[must_use]
    pub fn link(mut self, link_id: u32) -> Self {
        self.style.link_id = Some(link_id);
        self
    }

    /// Build the final style.
    #[must_use]
    pub fn build(self) -> Style {
        self.style
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_builder() {
        let style = Style::builder()
            .fg(Rgba::RED)
            .bg(Rgba::BLACK)
            .bold()
            .underline()
            .build();

        assert_eq!(style.fg, Some(Rgba::RED));
        assert_eq!(style.bg, Some(Rgba::BLACK));
        assert!(style.attributes.contains(TextAttributes::BOLD));
        assert!(style.attributes.contains(TextAttributes::UNDERLINE));
    }

    #[test]
    fn test_style_merge() {
        let base = Style::fg(Rgba::RED).with_bold();
        let overlay = Style::bg(Rgba::BLUE).with_italic();

        let merged = base.merge(overlay);

        assert_eq!(merged.fg, Some(Rgba::RED));
        assert_eq!(merged.bg, Some(Rgba::BLUE));
        assert!(merged.attributes.contains(TextAttributes::BOLD));
        assert!(merged.attributes.contains(TextAttributes::ITALIC));
    }

    #[test]
    fn test_const_styles() {
        assert!(Style::bold().attributes.contains(TextAttributes::BOLD));
        assert!(Style::italic().attributes.contains(TextAttributes::ITALIC));
        assert!(
            Style::underline()
                .attributes
                .contains(TextAttributes::UNDERLINE)
        );
    }
}
