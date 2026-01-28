//! RGBA color type with alpha blending operations.
//!
//! This module provides the [`Rgba`] type, which represents colors using
//! floating-point RGBA components. It supports:
//!
//! - **Color creation**: From f32/u8 components, hex strings, or HSV values
//! - **Alpha blending**: Porter-Duff "over" compositing for layered rendering
//! - **Color conversion**: To/from 256-color and 16-color terminal palettes
//! - **Interpolation**: Linear interpolation between colors
//!
//! # Examples
//!
//! ```
//! use opentui::Rgba;
//!
//! // Create colors in various ways
//! let red = Rgba::RED;
//! let custom = Rgba::from_hex("#1a1a2e").unwrap();
//! let semi_transparent = Rgba::BLUE.with_alpha(0.5);
//!
//! // Blend colors using Porter-Duff "over"
//! let result = semi_transparent.blend_over(Rgba::WHITE);
//!
//! // Convert to terminal palette
//! let ansi_256 = red.to_256_color();
//! ```

use std::fmt;

/// RGBA color with f32 components in range [0.0, 1.0].
///
/// Colors are stored as floating-point values for precision during blending
/// operations. Terminal output converts to appropriate formats (true color,
/// 256-color, or 16-color) based on terminal capabilities.
///
/// # Examples
///
/// ```
/// use opentui::Rgba;
///
/// // Use predefined constants
/// let bg = Rgba::BLACK;
///
/// // Create from RGB (opaque)
/// let accent = Rgba::from_rgb_u8(100, 149, 237);
///
/// // Create with transparency
/// let overlay = Rgba::RED.with_alpha(0.5);
///
/// // Blend: overlay on top of background
/// let blended = overlay.blend_over(bg);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    /// Fully transparent black.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Opaque black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Opaque red.
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque green.
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };

    /// Opaque blue.
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };

    /// Create a new RGBA color from f32 components.
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create an opaque color from f32 RGB components.
    #[must_use]
    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// Create an opaque color from u8 RGB components.
    #[must_use]
    pub fn from_rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: 1.0,
        }
    }

    /// Create a color from u8 RGBA components.
    #[must_use]
    pub fn from_rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: f32::from(r) / 255.0,
            g: f32::from(g) / 255.0,
            b: f32::from(b) / 255.0,
            a: f32::from(a) / 255.0,
        }
    }

    /// Parse a hex color string (e.g., "#FF0000" or "FF0000").
    ///
    /// Supports 3-char (#RGB), 6-char (#RRGGBB), and 8-char (#RRGGBBAA) formats.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);

        match hex.len() {
            3 => {
                // #RGB -> #RRGGBB
                let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
                Some(Self::from_rgb_u8(r * 17, g * 17, b * 17))
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::from_rgb_u8(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::from_rgba_u8(r, g, b, a))
            }
            _ => None,
        }
    }

    /// Create a color from HSV values.
    ///
    /// - h: Hue in degrees [0, 360)
    /// - s: Saturation [0, 1]
    /// - v: Value [0, 1]
    #[must_use]
    #[allow(clippy::many_single_char_names)]
    pub fn from_hsv(h: f32, s: f32, v: f32) -> Self {
        if s <= 0.0 {
            return Self::rgb(v, v, v);
        }

        let h = h.rem_euclid(360.0);
        let h = h / 60.0;
        let i = h.floor() as i32;
        let f = h - i as f32;
        let p = v * (1.0 - s);
        let q = v * s.mul_add(-f, 1.0);
        let t = v * s.mul_add(f - 1.0, 1.0);

        match i {
            0 => Self::rgb(v, t, p),
            1 => Self::rgb(q, v, p),
            2 => Self::rgb(p, v, t),
            3 => Self::rgb(p, q, v),
            4 => Self::rgb(t, p, v),
            _ => Self::rgb(v, p, q),
        }
    }

    /// Blend this color over another using standard alpha compositing (Porter-Duff "over").
    ///
    /// `self` is the foreground (on top), `other` is the background.
    #[must_use]
    pub fn blend_over(self, other: Self) -> Self {
        // Epsilon for numerical stability - values below this threshold are
        // considered effectively zero to prevent division instability
        const ALPHA_EPSILON: f32 = 1e-6;

        if self.a >= 1.0 {
            return self;
        }
        if self.a <= 0.0 {
            return other;
        }

        let inv_alpha = 1.0 - self.a;
        let out_a = other.a.mul_add(inv_alpha, self.a);

        // Use epsilon threshold to prevent numerical instability from division
        // by very small numbers which could amplify floating-point errors
        if out_a <= ALPHA_EPSILON {
            return Self::TRANSPARENT;
        }

        Self {
            r: (other.r * other.a).mul_add(inv_alpha, self.r * self.a) / out_a,
            g: (other.g * other.a).mul_add(inv_alpha, self.g * self.a) / out_a,
            b: (other.b * other.a).mul_add(inv_alpha, self.b * self.a) / out_a,
            a: out_a,
        }
    }

    /// Return a new color with the specified alpha value.
    #[must_use]
    pub const fn with_alpha(self, alpha: f32) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: alpha,
        }
    }

    /// Multiply this color's alpha by the given factor.
    #[must_use]
    pub fn multiply_alpha(self, factor: f32) -> Self {
        self.with_alpha(self.a * factor)
    }

    /// Convert to u8 RGB tuple, clamping values to [0, 255].
    #[must_use]
    pub fn to_rgb_u8(self) -> (u8, u8, u8) {
        let to_u8 = |value: f32| (value * 255.0).round().clamp(0.0, 255.0) as u8;
        (to_u8(self.r), to_u8(self.g), to_u8(self.b))
    }

    /// Convert to u8 RGBA tuple, clamping values to [0, 255].
    #[must_use]
    pub fn to_rgba_u8(self) -> (u8, u8, u8, u8) {
        let (r, g, b) = self.to_rgb_u8();
        let a = (self.a * 255.0).round().clamp(0.0, 255.0) as u8;
        (r, g, b, a)
    }

    /// Check if this color is fully transparent.
    #[must_use]
    pub fn is_transparent(self) -> bool {
        self.a <= 0.0
    }

    /// Check if this color is fully opaque.
    #[must_use]
    pub fn is_opaque(self) -> bool {
        self.a >= 1.0
    }

    /// Convert to packed u64 for fast integer comparison.
    ///
    /// This packs all 4 f32 components into a single u128 by reinterpreting
    /// their bit patterns. This allows fast cell comparison during diff
    /// detection by comparing integers instead of floating-point.
    ///
    /// Note: This is for comparison purposes only, not for serialization.
    /// NaN values will compare as different even if logically equivalent.
    #[inline]
    #[must_use]
    pub const fn to_bits(self) -> u128 {
        let r = self.r.to_bits() as u128;
        let g = self.g.to_bits() as u128;
        let b = self.b.to_bits() as u128;
        let a = self.a.to_bits() as u128;
        r | (g << 32) | (b << 64) | (a << 96)
    }

    /// Fast bitwise equality check.
    ///
    /// This is faster than float comparison for cell diffing
    /// because it uses integer operations instead of floating-point.
    #[inline]
    #[must_use]
    pub const fn bits_eq(self, other: Self) -> bool {
        self.to_bits() == other.to_bits()
    }

    /// Calculate luminance (perceived brightness).
    ///
    /// Uses the standard luminance formula: 0.299*R + 0.587*G + 0.114*B
    /// This matches the ITU-R BT.601 standard for luminance.
    #[must_use]
    pub fn luminance(self) -> f32 {
        0.299 * self.r + 0.587 * self.g + 0.114 * self.b
    }

    /// Linearly interpolate between two colors.
    #[must_use]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: (other.r - self.r).mul_add(t, self.r),
            g: (other.g - self.g).mul_add(t, self.g),
            b: (other.b - self.b).mul_add(t, self.b),
            a: (other.a - self.a).mul_add(t, self.a),
        }
    }

    /// Convert to nearest 256-color palette index.
    ///
    /// Uses the 6x6x6 color cube (colors 16-231) or grayscale ramp (232-255)
    /// depending on which provides the closest match.
    #[must_use]
    pub fn to_256_color(self) -> u8 {
        let (r, g, b) = self.to_rgb_u8();

        // Check if grayscale would be a better match
        let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;
        let is_grayscale = (r as i16 - gray as i16).abs() < 10
            && (g as i16 - gray as i16).abs() < 10
            && (b as i16 - gray as i16).abs() < 10;

        if is_grayscale {
            // Use grayscale ramp (232-255 = 24 levels from dark to light)
            // Each level is 10 apart, starting at 8: 8, 18, 28, ..., 238
            let gray_idx = (gray as u16 * 24 / 256) as u8;
            return 232 + gray_idx.min(23);
        }

        // Use 6x6x6 color cube (colors 16-231)
        // Each component maps to 0-5: 0, 95, 135, 175, 215, 255
        let ri = Self::nearest_cube_index(r);
        let gi = Self::nearest_cube_index(g);
        let bi = Self::nearest_cube_index(b);

        16 + 36 * ri + 6 * gi + bi
    }

    /// Find the nearest index in the 6x6x6 cube for a component value.
    ///
    /// Uses a lookup table for O(1) mapping instead of linear search.
    /// The cube values are [0, 95, 135, 175, 215, 255] with boundaries
    /// at midpoints: 48, 115, 155, 195, 235.
    #[inline]
    fn nearest_cube_index(val: u8) -> u8 {
        // Boundaries between cube values (midpoints)
        // 0-47→0, 48-114→1, 115-154→2, 155-194→3, 195-234→4, 235-255→5
        if val < 48 {
            0
        } else if val < 115 {
            1
        } else if val < 155 {
            2
        } else if val < 195 {
            3
        } else if val < 235 {
            4
        } else {
            5
        }
    }

    /// Convert to nearest 16-color (basic ANSI) palette index.
    ///
    /// Returns a value 0-15 for the standard ANSI colors:
    /// 0-7: black, red, green, yellow, blue, magenta, cyan, white (normal)
    /// 8-15: bright versions of the above
    #[must_use]
    pub fn to_16_color(self) -> u8 {
        let (r, g, b) = self.to_rgb_u8();
        let r = i32::from(r);
        let g = i32::from(g);
        let b = i32::from(b);

        // Standard ANSI palette (approximate values)
        #[rustfmt::skip]
        const PALETTE: [(i32, i32, i32); 16] = [
            (0, 0, 0),       // 0 Black
            (128, 0, 0),     // 1 Red
            (0, 128, 0),     // 2 Green
            (128, 128, 0),   // 3 Yellow
            (0, 0, 128),     // 4 Blue
            (128, 0, 128),   // 5 Magenta
            (0, 128, 128),   // 6 Cyan
            (192, 192, 192), // 7 White
            (128, 128, 128), // 8 Bright Black
            (255, 0, 0),     // 9 Bright Red
            (0, 255, 0),     // 10 Bright Green
            (255, 255, 0),   // 11 Bright Yellow
            (0, 0, 255),     // 12 Bright Blue
            (255, 0, 255),   // 13 Bright Magenta
            (0, 255, 255),   // 14 Bright Cyan
            (255, 255, 255), // 15 Bright White
        ];

        let mut best_idx = 0;
        let mut min_dist = i32::MAX;

        for (i, &(pr, pg, pb)) in PALETTE.iter().enumerate() {
            let dr = r - pr;
            let dg = g - pg;
            let db = b - pb;
            // Squared Euclidean distance
            let dist = dr * dr + dg * dg + db * db;

            if dist < min_dist {
                min_dist = dist;
                best_idx = i;
            }
        }

        best_idx as u8
    }

    /// Create an Rgba from a 256-color palette index.
    #[must_use]
    pub fn from_256_color(index: u8) -> Self {
        match index {
            // Standard 16 colors (approximations)
            0 => Self::from_rgb_u8(0, 0, 0),        // Black
            1 => Self::from_rgb_u8(128, 0, 0),      // Red
            2 => Self::from_rgb_u8(0, 128, 0),      // Green
            3 => Self::from_rgb_u8(128, 128, 0),    // Yellow
            4 => Self::from_rgb_u8(0, 0, 128),      // Blue
            5 => Self::from_rgb_u8(128, 0, 128),    // Magenta
            6 => Self::from_rgb_u8(0, 128, 128),    // Cyan
            7 => Self::from_rgb_u8(192, 192, 192),  // White
            8 => Self::from_rgb_u8(128, 128, 128),  // Bright Black (Gray)
            9 => Self::from_rgb_u8(255, 0, 0),      // Bright Red
            10 => Self::from_rgb_u8(0, 255, 0),     // Bright Green
            11 => Self::from_rgb_u8(255, 255, 0),   // Bright Yellow
            12 => Self::from_rgb_u8(0, 0, 255),     // Bright Blue
            13 => Self::from_rgb_u8(255, 0, 255),   // Bright Magenta
            14 => Self::from_rgb_u8(0, 255, 255),   // Bright Cyan
            15 => Self::from_rgb_u8(255, 255, 255), // Bright White
            // 6x6x6 color cube (16-231)
            16..=231 => {
                let idx = index - 16;
                let r = (idx / 36) % 6;
                let g = (idx / 6) % 6;
                let b = idx % 6;
                let cube_values: [u8; 6] = [0, 95, 135, 175, 215, 255];
                Self::from_rgb_u8(
                    cube_values[r as usize],
                    cube_values[g as usize],
                    cube_values[b as usize],
                )
            }
            // Grayscale ramp (232-255)
            232..=255 => {
                let gray = 8 + (index - 232) * 10;
                Self::from_rgb_u8(gray, gray, gray)
            }
        }
    }

    /// Create an Rgba from a 16-color (basic ANSI) palette index.
    #[must_use]
    pub fn from_16_color(index: u8) -> Self {
        Self::from_256_color(index & 0x0F)
    }
}

impl fmt::Display for Rgba {
    #[allow(clippy::many_single_char_names)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let to_hex_u8 = |value: f32| (value.clamp(0.0, 1.0) * 255.0).floor() as u8;
        let r = to_hex_u8(self.r);
        let g = to_hex_u8(self.g);
        let b = to_hex_u8(self.b);
        if self.a >= 1.0 {
            write!(f, "#{r:02X}{g:02X}{b:02X}")
        } else {
            let a = to_hex_u8(self.a);
            write!(f, "#{r:02X}{g:02X}{b:02X}{a:02X}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_hex() {
        assert_eq!(Rgba::from_hex("#FF0000"), Some(Rgba::RED));
        assert_eq!(Rgba::from_hex("00FF00"), Some(Rgba::GREEN));
        assert_eq!(Rgba::from_hex("#00F"), Some(Rgba::BLUE));
        assert_eq!(Rgba::from_hex("#000000FF"), Some(Rgba::BLACK));
    }

    #[test]
    fn test_blend_over() {
        // Opaque over anything = opaque
        let result = Rgba::RED.blend_over(Rgba::BLUE);
        assert_eq!(result, Rgba::RED);

        // Transparent over anything = that thing
        let result = Rgba::TRANSPARENT.blend_over(Rgba::GREEN);
        assert_eq!(result, Rgba::GREEN);

        // 50% alpha blend: half_red over blue
        // Standard Porter-Duff "over": result = src*src_a + dst*dst_a*(1-src_a) / out_a
        // out_a = 0.5 + 1.0*0.5 = 1.0
        // out_r = (1.0*0.5 + 0.0*1.0*0.5) / 1.0 = 0.5
        // out_b = (0.0*0.5 + 1.0*1.0*0.5) / 1.0 = 0.5
        let half_red = Rgba::RED.with_alpha(0.5);
        let result = half_red.blend_over(Rgba::BLUE);
        assert!((result.r - 0.5).abs() < 0.01);
        assert!((result.b - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_to_rgb_u8() {
        assert_eq!(Rgba::RED.to_rgb_u8(), (255, 0, 0));
        assert_eq!(Rgba::WHITE.to_rgb_u8(), (255, 255, 255));
        assert_eq!(Rgba::BLACK.to_rgb_u8(), (0, 0, 0));
    }

    #[test]
    fn test_display() {
        assert_eq!(format!("{}", Rgba::RED), "#FF0000");
        assert_eq!(format!("{}", Rgba::BLACK.with_alpha(0.5)), "#0000007F");
    }

    #[test]
    fn test_from_hsv() {
        // Pure red at hue 0
        let red = Rgba::from_hsv(0.0, 1.0, 1.0);
        assert!((red.r - 1.0).abs() < 0.01);
        assert!(red.g < 0.01);
        assert!(red.b < 0.01);

        // Pure green at hue 120
        let green = Rgba::from_hsv(120.0, 1.0, 1.0);
        assert!(green.r < 0.01);
        assert!((green.g - 1.0).abs() < 0.01);
        assert!(green.b < 0.01);

        // Pure blue at hue 240
        let blue = Rgba::from_hsv(240.0, 1.0, 1.0);
        assert!(blue.r < 0.01);
        assert!(blue.g < 0.01);
        assert!((blue.b - 1.0).abs() < 0.01);

        // Negative hue should wrap around: -60 degrees = 300 degrees (magenta-ish)
        let neg_hue = Rgba::from_hsv(-60.0, 1.0, 1.0);
        let pos_hue = Rgba::from_hsv(300.0, 1.0, 1.0);
        assert!((neg_hue.r - pos_hue.r).abs() < 0.01);
        assert!((neg_hue.g - pos_hue.g).abs() < 0.01);
        assert!((neg_hue.b - pos_hue.b).abs() < 0.01);

        // Hue > 360 should wrap around: 420 degrees = 60 degrees (yellow)
        let large_hue = Rgba::from_hsv(420.0, 1.0, 1.0);
        let normal_hue = Rgba::from_hsv(60.0, 1.0, 1.0);
        assert!((large_hue.r - normal_hue.r).abs() < 0.01);
        assert!((large_hue.g - normal_hue.g).abs() < 0.01);
        assert!((large_hue.b - normal_hue.b).abs() < 0.01);

        // Gray (saturation 0)
        let gray = Rgba::from_hsv(0.0, 0.0, 0.5);
        assert!((gray.r - 0.5).abs() < 0.01);
        assert!((gray.g - 0.5).abs() < 0.01);
        assert!((gray.b - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_to_256_color() {
        // Pure red should map to bright red in cube
        let red_idx = Rgba::RED.to_256_color();
        assert!((16..=231).contains(&red_idx)); // In color cube

        // Gray should use grayscale ramp
        let gray = Rgba::from_rgb_u8(128, 128, 128);
        let gray_idx = gray.to_256_color();
        assert!((232..=255).contains(&gray_idx)); // In grayscale ramp
    }

    #[test]
    fn test_to_16_color() {
        // Red
        let red_idx = Rgba::RED.to_16_color();
        assert!(red_idx == 1 || red_idx == 9); // Red or bright red

        // White
        let white_idx = Rgba::WHITE.to_16_color();
        assert!(white_idx == 7 || white_idx == 15); // White or bright white

        // Black
        let black_idx = Rgba::BLACK.to_16_color();
        assert_eq!(black_idx, 0);
    }

    #[test]
    fn test_from_256_color_roundtrip() {
        // Standard colors
        let red = Rgba::from_256_color(9); // Bright red
        assert_eq!(red.to_rgb_u8(), (255, 0, 0));

        // Grayscale
        let gray = Rgba::from_256_color(240);
        let (r, g, b) = gray.to_rgb_u8();
        assert_eq!(r, g);
        assert_eq!(g, b);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy to generate valid RGBA colors with f32 in [0, 1].
    fn rgba_strategy() -> impl Strategy<Value = Rgba> {
        (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0)
            .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
    }

    /// Strategy to generate u8 RGB values.
    fn rgb_u8_strategy() -> impl Strategy<Value = (u8, u8, u8)> {
        (any::<u8>(), any::<u8>(), any::<u8>())
    }

    proptest! {
        /// Blending opaque color over anything returns the opaque color.
        #[test]
        fn blend_opaque_is_identity(fg in rgba_strategy(), bg in rgba_strategy()) {
            let opaque_fg = fg.with_alpha(1.0);
            let result = opaque_fg.blend_over(bg);
            prop_assert!((result.r - opaque_fg.r).abs() < 1e-6);
            prop_assert!((result.g - opaque_fg.g).abs() < 1e-6);
            prop_assert!((result.b - opaque_fg.b).abs() < 1e-6);
            prop_assert!((result.a - 1.0).abs() < 1e-6);
        }

        /// Blending transparent color over anything returns the background.
        #[test]
        fn blend_transparent_is_background(bg in rgba_strategy()) {
            let result = Rgba::TRANSPARENT.blend_over(bg);
            prop_assert!((result.r - bg.r).abs() < 1e-6);
            prop_assert!((result.g - bg.g).abs() < 1e-6);
            prop_assert!((result.b - bg.b).abs() < 1e-6);
            prop_assert!((result.a - bg.a).abs() < 1e-6);
        }

        /// lerp(0) returns self, lerp(1) returns other.
        #[test]
        fn lerp_endpoints(a in rgba_strategy(), b in rgba_strategy()) {
            let at_0 = a.lerp(b, 0.0);
            let at_1 = a.lerp(b, 1.0);

            prop_assert!((at_0.r - a.r).abs() < 1e-6);
            prop_assert!((at_0.g - a.g).abs() < 1e-6);
            prop_assert!((at_0.b - a.b).abs() < 1e-6);
            prop_assert!((at_0.a - a.a).abs() < 1e-6);

            prop_assert!((at_1.r - b.r).abs() < 1e-6);
            prop_assert!((at_1.g - b.g).abs() < 1e-6);
            prop_assert!((at_1.b - b.b).abs() < 1e-6);
            prop_assert!((at_1.a - b.a).abs() < 1e-6);
        }

        /// lerp(0.5) is the midpoint.
        #[test]
        fn lerp_midpoint(a in rgba_strategy(), b in rgba_strategy()) {
            let mid = a.lerp(b, 0.5);
            let expected_r = f32::midpoint(a.r, b.r);
            let expected_g = f32::midpoint(a.g, b.g);
            let expected_b = f32::midpoint(a.b, b.b);
            let expected_a = f32::midpoint(a.a, b.a);

            prop_assert!((mid.r - expected_r).abs() < 1e-5);
            prop_assert!((mid.g - expected_g).abs() < 1e-5);
            prop_assert!((mid.b - expected_b).abs() < 1e-5);
            prop_assert!((mid.a - expected_a).abs() < 1e-5);
        }

        /// u8 RGB round-trip preserves values.
        #[test]
        fn rgb_u8_roundtrip((r, g, b) in rgb_u8_strategy()) {
            let color = Rgba::from_rgb_u8(r, g, b);
            let (r2, g2, b2) = color.to_rgb_u8();
            prop_assert_eq!(r, r2);
            prop_assert_eq!(g, g2);
            prop_assert_eq!(b, b2);
        }

        /// blend_over result alpha is in [0, 1].
        #[test]
        fn blend_alpha_in_range(fg in rgba_strategy(), bg in rgba_strategy()) {
            let result = fg.blend_over(bg);
            prop_assert!(result.a >= 0.0);
            prop_assert!(result.a <= 1.0 + 1e-6);
        }

        /// with_alpha preserves RGB.
        #[test]
        fn with_alpha_preserves_rgb(color in rgba_strategy(), new_alpha in 0.0f32..=1.0) {
            let modified = color.with_alpha(new_alpha);
            prop_assert!((modified.r - color.r).abs() < 1e-6);
            prop_assert!((modified.g - color.g).abs() < 1e-6);
            prop_assert!((modified.b - color.b).abs() < 1e-6);
            prop_assert!((modified.a - new_alpha).abs() < 1e-6);
        }

        /// multiply_alpha(1.0) is identity.
        #[test]
        fn multiply_alpha_identity(color in rgba_strategy()) {
            let result = color.multiply_alpha(1.0);
            prop_assert!((result.a - color.a).abs() < 1e-6);
        }

        /// to_256_color always produces valid index (0-255).
        #[test]
        fn to_256_color_valid_range(color in rgba_strategy()) {
            let idx = color.to_256_color();
            // Valid range is 16-255 (skips first 16 standard colors for cube/gray)
            prop_assert!(idx >= 16);
        }

        /// to_16_color always produces valid index (0-15).
        #[test]
        fn to_16_color_valid_range(color in rgba_strategy()) {
            let idx = color.to_16_color();
            prop_assert!(idx < 16);
        }
    }
}

/// Explicit unit tests for Porter-Duff alpha blending.
/// These supplement the property-based tests above with specific,
/// documented test cases per bd-2fv0.
#[cfg(test)]
mod porter_duff_tests {
    use super::*;

    // =========================================================================
    // Basic Blending Tests
    // Porter-Duff "over" operator: result = fg + bg × (1 - fg.alpha)
    // =========================================================================

    #[test]
    fn test_blend_opaque_over_opaque() {
        // When fg.a = 1.0 and bg.a = 1.0, result should be fg completely
        // Porter-Duff: result = fg × 1.0 + bg × (1 - 1.0) = fg
        let fg = Rgba::new(1.0, 0.0, 0.0, 1.0); // Opaque red
        let bg = Rgba::new(0.0, 1.0, 0.0, 1.0); // Opaque green

        let result = fg.blend_over(bg);

        assert!((result.r - 1.0).abs() < 1e-6, "Red channel should be 1.0");
        assert!(result.g.abs() < 1e-6, "Green channel should be 0.0");
        assert!(result.b.abs() < 1e-6, "Blue channel should be 0.0");
        assert!((result.a - 1.0).abs() < 1e-6, "Alpha should be 1.0");
    }

    #[test]
    fn test_blend_transparent_over_opaque() {
        // When fg.a = 0.0, result should be bg completely
        // Porter-Duff: result = fg × 0.0 + bg × (1 - 0.0) = bg
        let fg = Rgba::TRANSPARENT;
        let bg = Rgba::new(0.3, 0.6, 0.9, 1.0); // Opaque custom color

        let result = fg.blend_over(bg);

        assert!((result.r - bg.r).abs() < 1e-6, "R should match background");
        assert!((result.g - bg.g).abs() < 1e-6, "G should match background");
        assert!((result.b - bg.b).abs() < 1e-6, "B should match background");
        assert!((result.a - bg.a).abs() < 1e-6, "A should match background");
    }

    #[test]
    fn test_blend_opaque_over_transparent() {
        // When bg.a = 0.0 and fg is opaque, result should be fg
        let fg = Rgba::new(0.7, 0.2, 0.5, 1.0); // Opaque custom color
        let bg = Rgba::TRANSPARENT;

        let result = fg.blend_over(bg);

        assert!((result.r - fg.r).abs() < 1e-6, "R should match foreground");
        assert!((result.g - fg.g).abs() < 1e-6, "G should match foreground");
        assert!((result.b - fg.b).abs() < 1e-6, "B should match foreground");
        assert!((result.a - 1.0).abs() < 1e-6, "A should be 1.0");
    }

    #[test]
    fn test_blend_both_transparent() {
        // When both fg.a = 0.0 and bg.a = 0.0, result is fully transparent
        let fg = Rgba::TRANSPARENT;
        let bg = Rgba::TRANSPARENT;

        let result = fg.blend_over(bg);

        assert_eq!(result.a, 0.0, "Result alpha should be 0.0");
        // RGB values are undefined when a=0, but should not be NaN
        assert!(!result.r.is_nan(), "R should not be NaN");
        assert!(!result.g.is_nan(), "G should not be NaN");
        assert!(!result.b.is_nan(), "B should not be NaN");
    }

    #[test]
    fn test_blend_semi_transparent_over_opaque() {
        // 50% transparent red over opaque blue
        // fg.a = 0.5, bg.a = 1.0
        // out_a = 0.5 + 1.0 × (1 - 0.5) = 0.5 + 0.5 = 1.0
        // out_r = (1.0×0.5 + 0.0×1.0×0.5) / 1.0 = 0.5
        // out_b = (0.0×0.5 + 1.0×1.0×0.5) / 1.0 = 0.5
        let fg = Rgba::RED.with_alpha(0.5);
        let bg = Rgba::BLUE;

        let result = fg.blend_over(bg);

        assert!(
            (result.r - 0.5).abs() < 0.01,
            "Red should be ~0.5, got {}",
            result.r
        );
        assert!(result.g.abs() < 0.01, "Green should be ~0.0");
        assert!(
            (result.b - 0.5).abs() < 0.01,
            "Blue should be ~0.5, got {}",
            result.b
        );
        assert!((result.a - 1.0).abs() < 1e-6, "Alpha should be 1.0");
    }

    #[test]
    fn test_blend_semi_transparent_over_semi_transparent() {
        // 50% red over 50% blue
        // fg.a = 0.5, bg.a = 0.5
        // out_a = 0.5 + 0.5 × (1 - 0.5) = 0.5 + 0.25 = 0.75
        let fg = Rgba::RED.with_alpha(0.5);
        let bg = Rgba::BLUE.with_alpha(0.5);

        let result = fg.blend_over(bg);

        assert!(
            (result.a - 0.75).abs() < 0.01,
            "Alpha should be ~0.75, got {}",
            result.a
        );
        // RGB values depend on formula: (fg_r×fg_a + bg_r×bg_a×(1-fg_a)) / out_a
        // out_r = (1.0×0.5 + 0.0×0.5×0.5) / 0.75 = 0.5/0.75 ≈ 0.667
        assert!(result.r > 0.5, "Red should be > 0.5");
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_blend_channel_clamping() {
        // Verify RGB channels stay in [0, 1] range after blending
        // Use extreme values to test potential overflow
        let bright = Rgba::new(1.0, 1.0, 1.0, 1.0);
        let also_bright = Rgba::new(1.0, 1.0, 1.0, 0.9);

        let result = also_bright.blend_over(bright);

        assert!(result.r <= 1.0, "R should not exceed 1.0");
        assert!(result.g <= 1.0, "G should not exceed 1.0");
        assert!(result.b <= 1.0, "B should not exceed 1.0");
        assert!(result.r >= 0.0, "R should not be negative");
        assert!(result.g >= 0.0, "G should not be negative");
        assert!(result.b >= 0.0, "B should not be negative");
    }

    #[test]
    fn test_blend_preserves_rgb_when_opaque() {
        // Opaque foreground should preserve its exact RGB values
        let fg = Rgba::new(0.123_456_7, 0.987_654_3, 0.555_555_5, 1.0);
        let bg = Rgba::new(0.999, 0.001, 0.500, 1.0);

        let result = fg.blend_over(bg);

        // Use exact comparison since opaque fg should pass through unchanged
        assert!(
            (result.r - fg.r).abs() < 1e-6,
            "R should be preserved exactly"
        );
        assert!(
            (result.g - fg.g).abs() < 1e-6,
            "G should be preserved exactly"
        );
        assert!(
            (result.b - fg.b).abs() < 1e-6,
            "B should be preserved exactly"
        );
    }

    #[test]
    fn test_blend_not_commutative() {
        // Verify that blend(a, b) ≠ blend(b, a) in general
        // This is fundamental to the "over" operator
        let red_semi = Rgba::RED.with_alpha(0.7);
        let blue_semi = Rgba::BLUE.with_alpha(0.7);

        let red_over_blue = red_semi.blend_over(blue_semi);
        let blue_over_red = blue_semi.blend_over(red_semi);

        // Both should have same alpha (symmetric calculation)
        assert!(
            (red_over_blue.a - blue_over_red.a).abs() < 1e-6,
            "Alpha should be same"
        );

        // But RGB values should differ
        let r_diff = (red_over_blue.r - blue_over_red.r).abs();
        let b_diff = (red_over_blue.b - blue_over_red.b).abs();

        assert!(
            r_diff > 0.1 || b_diff > 0.1,
            "Blending should not be commutative: red_over_blue={:?}, blue_over_red={:?}",
            red_over_blue,
            blue_over_red
        );
    }

    // =========================================================================
    // Numerical Stability
    // =========================================================================

    #[test]
    fn test_blend_rounding_consistency() {
        // Verify consistent results across similar inputs
        let fg1 = Rgba::new(0.333_333_3, 0.666_666_7, 0.5, 0.8);
        let fg2 = Rgba::new(0.333_333_3, 0.666_666_7, 0.5, 0.8);
        let bg = Rgba::new(0.1, 0.2, 0.3, 0.9);

        let result1 = fg1.blend_over(bg);
        let result2 = fg2.blend_over(bg);

        // Identical inputs should produce identical outputs
        assert_eq!(result1.r, result2.r, "R should be deterministic");
        assert_eq!(result1.g, result2.g, "G should be deterministic");
        assert_eq!(result1.b, result2.b, "B should be deterministic");
        assert_eq!(result1.a, result2.a, "A should be deterministic");
    }

    #[test]
    fn test_blend_chain_multiple() {
        // Test blending multiple layers: a over (b over c)
        // This simulates layered UI rendering
        let top = Rgba::RED.with_alpha(0.3);
        let middle = Rgba::GREEN.with_alpha(0.5);
        let bottom = Rgba::BLUE.with_alpha(1.0);

        // Blend bottom-up: middle over bottom, then top over result
        let mid_over_bot = middle.blend_over(bottom);
        let final_result = top.blend_over(mid_over_bot);

        // Result should be valid
        assert!(final_result.r >= 0.0 && final_result.r <= 1.0);
        assert!(final_result.g >= 0.0 && final_result.g <= 1.0);
        assert!(final_result.b >= 0.0 && final_result.b <= 1.0);
        assert!(final_result.a >= 0.0 && final_result.a <= 1.0);

        // Result should have all color contributions
        assert!(final_result.r > 0.0, "Should have some red from top layer");
        assert!(
            final_result.g > 0.0,
            "Should have some green from middle layer"
        );
        assert!(
            final_result.b > 0.0,
            "Should have some blue from bottom layer"
        );
    }

    #[test]
    fn test_blend_near_zero_alpha() {
        // Test very small alpha values for numerical stability
        let tiny_alpha = Rgba::WHITE.with_alpha(1e-7);
        let bg = Rgba::BLACK;

        let result = tiny_alpha.blend_over(bg);

        // Should not produce NaN or Inf
        assert!(!result.r.is_nan(), "R should not be NaN");
        assert!(!result.g.is_nan(), "G should not be NaN");
        assert!(!result.b.is_nan(), "B should not be NaN");
        assert!(!result.a.is_nan(), "A should not be NaN");
        assert!(!result.r.is_infinite(), "R should not be infinite");

        // Result should be very close to background
        assert!((result.r - bg.r).abs() < 0.001);
        assert!((result.g - bg.g).abs() < 0.001);
        assert!((result.b - bg.b).abs() < 0.001);
    }

    #[test]
    fn test_blend_near_one_alpha() {
        // Test alpha very close to 1.0
        let nearly_opaque = Rgba::WHITE.with_alpha(0.999_999);
        let bg = Rgba::BLACK;

        let result = nearly_opaque.blend_over(bg);

        // Should be very close to white
        assert!((result.r - 1.0).abs() < 0.001);
        assert!((result.g - 1.0).abs() < 0.001);
        assert!((result.b - 1.0).abs() < 0.001);
    }

    // =========================================================================
    // Porter-Duff Formula Verification
    // =========================================================================

    #[test]
    fn test_blend_formula_verification() {
        // Manually verify the Porter-Duff formula with known values
        // Formula: out_a = fg_a + bg_a × (1 - fg_a)
        //          out_rgb = (fg_rgb × fg_a + bg_rgb × bg_a × (1 - fg_a)) / out_a

        let fg = Rgba::new(1.0, 0.0, 0.0, 0.6); // 60% red
        let bg = Rgba::new(0.0, 0.0, 1.0, 0.8); // 80% blue

        let result = fg.blend_over(bg);

        // Calculate expected values manually
        let expected_a = 0.6 + 0.8 * (1.0 - 0.6); // 0.6 + 0.8*0.4 = 0.6 + 0.32 = 0.92
        let expected_r = (1.0 * 0.6 + 0.0 * 0.8 * 0.4) / expected_a; // 0.6 / 0.92 ≈ 0.652
        let expected_b = (0.0 * 0.6 + 1.0 * 0.8 * 0.4) / expected_a; // 0.32 / 0.92 ≈ 0.348

        assert!(
            (result.a - expected_a).abs() < 1e-5,
            "Alpha: expected {expected_a}, got {}",
            result.a
        );
        assert!(
            (result.r - expected_r).abs() < 1e-5,
            "Red: expected {expected_r}, got {}",
            result.r
        );
        assert!(
            (result.b - expected_b).abs() < 1e-5,
            "Blue: expected {expected_b}, got {}",
            result.b
        );
        assert!(
            result.g.abs() < 1e-5,
            "Green should be ~0, got {}",
            result.g
        );
    }
}
