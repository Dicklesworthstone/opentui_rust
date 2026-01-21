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

        let h = h % 360.0;
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
        if self.a >= 1.0 {
            return self;
        }
        if self.a <= 0.0 {
            return other;
        }

        let inv_alpha = 1.0 - self.a;
        let out_a = other.a.mul_add(inv_alpha, self.a);

        if out_a <= 0.0 {
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
        (
            (self.r * 255.0).clamp(0.0, 255.0) as u8,
            (self.g * 255.0).clamp(0.0, 255.0) as u8,
            (self.b * 255.0).clamp(0.0, 255.0) as u8,
        )
    }

    /// Convert to u8 RGBA tuple, clamping values to [0, 255].
    #[must_use]
    pub fn to_rgba_u8(self) -> (u8, u8, u8, u8) {
        let (r, g, b) = self.to_rgb_u8();
        (r, g, b, (self.a * 255.0).clamp(0.0, 255.0) as u8)
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
        let cube_values: [u8; 6] = [0, 95, 135, 175, 215, 255];

        let ri = Self::nearest_cube_index(r, cube_values);
        let gi = Self::nearest_cube_index(g, cube_values);
        let bi = Self::nearest_cube_index(b, cube_values);

        16 + 36 * ri + 6 * gi + bi
    }

    /// Find the nearest index in the 6x6x6 cube for a component value.
    fn nearest_cube_index(val: u8, cube_values: [u8; 6]) -> u8 {
        let mut best = 0;
        let mut best_dist = u16::MAX;
        for (i, &cv) in cube_values.iter().enumerate() {
            let dist = (val as i16 - cv as i16).unsigned_abs();
            if dist < best_dist {
                best_dist = dist;
                best = i as u8;
            }
        }
        best
    }

    /// Convert to nearest 16-color (basic ANSI) palette index.
    ///
    /// Returns a value 0-15 for the standard ANSI colors:
    /// 0-7: black, red, green, yellow, blue, magenta, cyan, white (normal)
    /// 8-15: bright versions of the above
    #[must_use]
    pub fn to_16_color(self) -> u8 {
        let (r, g, b) = self.to_rgb_u8();

        // Luminance for determining bright vs normal (using u32 to avoid overflow)
        let lum = (u32::from(r) * 299 + u32::from(g) * 587 + u32::from(b) * 114) / 1000;
        let is_bright = lum > 127;

        // Determine which of the 8 base colors is closest
        let r_bit = u8::from(r > 127);
        let g_bit = u8::from(g > 127);
        let b_bit = u8::from(b > 127);

        // ANSI color order: 0=black, 1=red, 2=green, 3=yellow, 4=blue, 5=magenta, 6=cyan, 7=white
        let base = r_bit | (g_bit << 1) | (b_bit << 2);

        // Remap from RGB bits to ANSI order
        // RGB bits: 0=000 (black), 1=001 (red), 2=010 (green), 3=011 (yellow),
        //           4=100 (blue), 5=101 (magenta), 6=110 (cyan), 7=111 (white)
        // This matches ANSI already since ANSI uses: red=1, green=2, blue=4
        let color = base;

        if is_bright { color + 8 } else { color }
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
        let (r, g, b) = self.to_rgb_u8();
        if self.a >= 1.0 {
            write!(f, "#{r:02X}{g:02X}{b:02X}")
        } else {
            let a = (self.a * 255.0).clamp(0.0, 255.0) as u8;
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
