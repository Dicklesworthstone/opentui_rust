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
//! use opentui_rust::{Style, TextAttributes, Rgba};
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
    /// Text rendering attributes (bold, italic, underline, etc.).
    ///
    /// Attributes are represented as bitflags and can be combined using
    /// bitwise OR. Not all terminals support all attributes.
    ///
    /// Link IDs are packed into the upper 24 bits to match the Zig spec.
    #[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Hash)]
    pub struct TextAttributes: u32 {
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

impl TextAttributes {
    /// Mask for the lower 8 bits containing style flags.
    pub const FLAGS_MASK: u32 = 0x0000_00FF;
    /// Mask for the upper 24 bits containing link ID.
    pub const LINK_ID_MASK: u32 = 0xFFFF_FF00;
    /// Bit shift for link ID storage.
    pub const LINK_ID_SHIFT: u32 = 8;
    /// Maximum link ID that fits in 24 bits.
    pub const MAX_LINK_ID: u32 = 0x00FF_FFFF;

    /// Extract the link ID (if any).
    #[must_use]
    pub const fn link_id(self) -> Option<u32> {
        let id = (self.bits() & Self::LINK_ID_MASK) >> Self::LINK_ID_SHIFT;
        if id == 0 { None } else { Some(id) }
    }

    /// Return attributes with a link ID set (masked to 24 bits).
    #[must_use]
    pub const fn with_link_id(self, link_id: u32) -> Self {
        let id = link_id & Self::MAX_LINK_ID;
        let bits = (self.bits() & Self::FLAGS_MASK) | (id << Self::LINK_ID_SHIFT);
        Self::from_bits_retain(bits)
    }

    /// Clear the link ID, preserving style flags.
    #[must_use]
    pub const fn clear_link_id(self) -> Self {
        Self::from_bits_retain(self.bits() & Self::FLAGS_MASK)
    }

    /// Return only the style flags (link ID cleared).
    #[must_use]
    pub const fn flags_only(self) -> Self {
        Self::from_bits_retain(self.bits() & Self::FLAGS_MASK)
    }

    /// Merge attributes: OR flags, prefer `other` link ID when set.
    #[must_use]
    pub const fn merge(self, other: Self) -> Self {
        let flags = (self.bits() | other.bits()) & Self::FLAGS_MASK;
        let link_bits = if (other.bits() & Self::LINK_ID_MASK) != 0 {
            other.bits() & Self::LINK_ID_MASK
        } else {
            self.bits() & Self::LINK_ID_MASK
        };
        Self::from_bits_retain(flags | link_bits)
    }

    /// Set the link ID in place.
    pub fn set_link_id(&mut self, link_id: u32) {
        *self = self.with_link_id(link_id);
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
/// Link IDs are packed into [`TextAttributes`] (bits 8-31) and reference
/// URLs stored in a [`LinkPool`](crate::LinkPool).
/// Terminals supporting OSC 8 will render these as clickable links.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    /// Foreground color (None = terminal default).
    pub fg: Option<Rgba>,
    /// Background color (None = terminal default).
    pub bg: Option<Rgba>,
    /// Text rendering attributes.
    pub attributes: TextAttributes,
}

impl Style {
    /// Empty style with no colors or attributes.
    pub const NONE: Self = Self {
        fg: None,
        bg: None,
        attributes: TextAttributes::empty(),
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
        }
    }

    /// Create a style with only background color.
    #[must_use]
    pub const fn bg(color: Rgba) -> Self {
        Self {
            fg: None,
            bg: Some(color),
            attributes: TextAttributes::empty(),
        }
    }

    /// Create a bold style.
    #[must_use]
    pub const fn bold() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::BOLD,
        }
    }

    /// Create an italic style.
    #[must_use]
    pub const fn italic() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::ITALIC,
        }
    }

    /// Create an underline style.
    #[must_use]
    pub const fn underline() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::UNDERLINE,
        }
    }

    /// Create a dim style.
    #[must_use]
    pub const fn dim() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::DIM,
        }
    }

    /// Create an inverse (swapped fg/bg) style.
    #[must_use]
    pub const fn inverse() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::INVERSE,
        }
    }

    /// Create a strikethrough style.
    #[must_use]
    pub const fn strikethrough() -> Self {
        Self {
            fg: None,
            bg: None,
            attributes: TextAttributes::STRIKETHROUGH,
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
            attributes: self.attributes.merge(attrs),
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
            attributes: self.attributes.with_link_id(link_id),
            ..self
        }
    }

    /// Check if this style has any non-default properties.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fg.is_none() && self.bg.is_none() && self.attributes.is_empty()
    }

    /// Merge two styles, with `other` taking precedence for set values.
    #[must_use]
    pub fn merge(self, other: Self) -> Self {
        Self {
            fg: other.fg.or(self.fg),
            bg: other.bg.or(self.bg),
            attributes: self.attributes.merge(other.attributes),
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
        self.style.attributes = self.style.attributes.with_link_id(link_id);
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

    #[test]
    fn test_text_attributes_link_id_packing() {
        let attrs = TextAttributes::BOLD.with_link_id(0x12_3456);
        assert!(attrs.contains(TextAttributes::BOLD));
        assert_eq!(attrs.link_id(), Some(0x12_3456));
        assert_eq!(attrs.flags_only(), TextAttributes::BOLD);
    }

    #[test]
    fn test_text_attributes_merge_link_id_preference() {
        let base = TextAttributes::BOLD.with_link_id(1);
        let overlay_no_link = TextAttributes::ITALIC;
        let merged = base.merge(overlay_no_link);
        assert_eq!(merged.link_id(), Some(1));
        assert!(merged.contains(TextAttributes::BOLD));
        assert!(merged.contains(TextAttributes::ITALIC));

        let overlay_with_link = TextAttributes::UNDERLINE.with_link_id(2);
        let merged_with_link = base.merge(overlay_with_link);
        assert_eq!(merged_with_link.link_id(), Some(2));
        assert!(merged_with_link.contains(TextAttributes::BOLD));
        assert!(merged_with_link.contains(TextAttributes::UNDERLINE));
    }

    #[test]
    fn test_text_attributes_link_id_masking() {
        let attrs = TextAttributes::empty().with_link_id(0x1FF_FFFF);
        assert_eq!(attrs.link_id(), Some(TextAttributes::MAX_LINK_ID));
    }
}
