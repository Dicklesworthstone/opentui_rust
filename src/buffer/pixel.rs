//! Pixel-based rendering for high-resolution graphics.
//!
//! This module provides functions for rendering pixel data to the terminal
//! using Unicode block characters:
//!
//! - **Quadrant blocks**: 2x2 pixel blocks using Unicode 2580-259F characters
//! - **Grayscale**: Intensity mapping to ASCII/Unicode shade characters
//! - **Supersampling**: Averaging 2x2 pixel blocks for smoother rendering

use crate::buffer::OptimizedBuffer;
use crate::cell::Cell;
use crate::color::Rgba;
use crate::error::Error;
use crate::style::Style;

/// Unicode block characters for 2x2 quadrant rendering.
///
/// Quadrant blocks represent 4 pixels per terminal cell using 16 combinations:
/// - `' '` (0b0000): All empty
/// - `'▘'` (0b0001): Top-left only
/// - `'▝'` (0b0010): Top-right only
/// - `'▀'` (0b0011): Top row
/// - `'▖'` (0b0100): Bottom-left only
/// - `'▌'` (0b0101): Left column
/// - `'▞'` (0b0110): Diagonal (TL-BR)
/// - `'▛'` (0b0111): All except bottom-right
/// - `'▗'` (0b1000): Bottom-right only
/// - `'▚'` (0b1001): Anti-diagonal
/// - `'▐'` (0b1010): Right column
/// - `'▜'` (0b1011): All except bottom-left
/// - `'▄'` (0b1100): Bottom row
/// - `'▙'` (0b1101): All except top-right
/// - `'▟'` (0b1110): All except top-left
/// - `'█'` (0b1111): Full block
const QUADRANT_CHARS: [char; 16] = [
    ' ', '▘', '▝', '▀', '▖', '▌', '▞', '▛', '▗', '▚', '▐', '▜', '▄', '▙', '▟', '█',
];

/// ASCII grayscale characters from darkest to lightest.
const GRAYSCALE_ASCII: &[char] = &[' ', '.', ':', '-', '=', '+', '*', '#', '%', '@'];

/// Unicode shade characters for grayscale (4 levels + space).
const GRAYSCALE_UNICODE: &[char] = &[' ', '░', '▒', '▓', '█'];

/// A 2D pixel buffer for high-resolution rendering.
///
/// Each pixel has an RGBA color. The buffer can be rendered to an
/// `OptimizedBuffer` using various methods.
#[derive(Clone, Debug)]
pub struct PixelBuffer {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel data in row-major order (RGBA).
    pub pixels: Vec<Rgba>,
}

impl PixelBuffer {
    /// Create a new pixel buffer filled with transparent black.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            pixels: vec![Rgba::TRANSPARENT; size],
        }
    }

    /// Compute pixel index with overflow protection.
    ///
    /// Returns `None` if:
    /// - Coordinates are out of bounds
    /// - Index calculation would overflow
    #[inline]
    fn pixel_index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        // Use checked arithmetic to prevent overflow on large dimensions
        let row_offset = (y as usize).checked_mul(self.width as usize)?;
        let idx = row_offset.checked_add(x as usize)?;
        // Bounds check (should always pass given x/y bounds, but defense in depth)
        if idx < self.pixels.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Create from raw RGBA data.
    ///
    /// # Panics
    /// Panics if `pixels.len() != width * height` or if dimensions would overflow.
    ///
    /// # Note
    /// For a non-panicking alternative, use [`try_from_pixels()`](Self::try_from_pixels).
    #[must_use]
    pub fn from_pixels(width: u32, height: u32, pixels: Vec<Rgba>) -> Self {
        Self::try_from_pixels(width, height, pixels).expect("invalid pixel buffer dimensions")
    }

    /// Try to create from raw RGBA data.
    ///
    /// Returns an error if:
    /// - `width * height` would overflow `usize`
    /// - `pixels.len() != width * height`
    ///
    /// # Errors
    /// - [`Error::DimensionOverflow`] if dimensions overflow
    /// - [`Error::SizeMismatch`] if pixel count doesn't match dimensions
    pub fn try_from_pixels(width: u32, height: u32, pixels: Vec<Rgba>) -> Result<Self, Error> {
        let expected_size = (width as usize)
            .checked_mul(height as usize)
            .ok_or(Error::DimensionOverflow { width, height })?;

        if pixels.len() != expected_size {
            return Err(Error::SizeMismatch {
                expected: expected_size,
                actual: pixels.len(),
            });
        }

        Ok(Self {
            width,
            height,
            pixels,
        })
    }

    /// Get pixel at (x, y).
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<Rgba> {
        self.pixel_index(x, y).map(|idx| self.pixels[idx])
    }

    /// Set pixel at (x, y).
    pub fn set(&mut self, x: u32, y: u32, color: Rgba) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.pixels[idx] = color;
        }
    }

    /// Fill entire buffer with a color.
    pub fn fill(&mut self, color: Rgba) {
        self.pixels.fill(color);
    }
}

/// A grayscale buffer for intensity-based rendering.
#[derive(Clone, Debug)]
pub struct GrayscaleBuffer {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Intensity values (0.0 = black, 1.0 = white).
    pub values: Vec<f32>,
}

impl GrayscaleBuffer {
    /// Create a new grayscale buffer filled with black.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            values: vec![0.0; size],
        }
    }

    /// Compute pixel index with overflow protection.
    #[inline]
    fn pixel_index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let row_offset = (y as usize).checked_mul(self.width as usize)?;
        let idx = row_offset.checked_add(x as usize)?;
        if idx < self.values.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Get intensity at (x, y).
    #[must_use]
    pub fn get(&self, x: u32, y: u32) -> Option<f32> {
        self.pixel_index(x, y).map(|idx| self.values[idx])
    }

    /// Set intensity at (x, y).
    pub fn set(&mut self, x: u32, y: u32, value: f32) {
        if let Some(idx) = self.pixel_index(x, y) {
            self.values[idx] = value.clamp(0.0, 1.0);
        }
    }
}

impl OptimizedBuffer {
    /// Draw a pixel buffer using 2x2 quadrant blocks.
    ///
    /// Each terminal cell represents a 2x2 pixel block. Colors are averaged
    /// to determine foreground and background, with the block character
    /// chosen to best represent which quadrants are "lit".
    ///
    /// # Arguments
    /// * `x`, `y` - Destination position in terminal cells
    /// * `src` - Source pixel buffer
    /// * `threshold` - Brightness threshold (0.0-1.0) for considering a pixel "lit"
    #[allow(clippy::similar_names)]
    pub fn draw_supersample_buffer(&mut self, x: u32, y: u32, src: &PixelBuffer, threshold: f32) {
        // Each terminal cell = 2x2 pixels
        let cells_w = src.width / 2;
        let cells_h = src.height / 2;

        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let px = cx * 2;
                let py = cy * 2;

                // Get 2x2 pixel block
                let tl = src.get(px, py).unwrap_or(Rgba::TRANSPARENT);
                let tr = src.get(px + 1, py).unwrap_or(Rgba::TRANSPARENT);
                let bl = src.get(px, py + 1).unwrap_or(Rgba::TRANSPARENT);
                let br = src.get(px + 1, py + 1).unwrap_or(Rgba::TRANSPARENT);

                // Calculate brightness for each pixel
                let tl_bright = tl.luminance();
                let tr_bright = tr.luminance();
                let bl_bright = bl.luminance();
                let br_bright = br.luminance();

                // Build quadrant mask
                let mut mask = 0u8;
                if tl_bright >= threshold {
                    mask |= 0b0001;
                }
                if tr_bright >= threshold {
                    mask |= 0b0010;
                }
                if bl_bright >= threshold {
                    mask |= 0b0100;
                }
                if br_bright >= threshold {
                    mask |= 0b1000;
                }

                // Average colors for foreground (lit) and background (unlit)
                let lit_mask = [
                    tl_bright >= threshold,
                    tr_bright >= threshold,
                    bl_bright >= threshold,
                    br_bright >= threshold,
                ];
                let (fg, bg) = average_colors(&[tl, tr, bl, br], &lit_mask);

                let ch = QUADRANT_CHARS[mask as usize];
                let style = Style::builder().fg(fg).bg(bg).build();
                let cell = Cell::new(ch, style);
                self.set(x + cx, y + cy, cell);
            }
        }
    }

    /// Draw a grayscale buffer using ASCII shade characters.
    ///
    /// Maps intensity values to characters: ' ' `.` `:` `-` `=` `+` `*` `#` `%` `@`
    ///
    /// # Arguments
    /// * `x`, `y` - Destination position
    /// * `src` - Source grayscale buffer
    /// * `fg` - Foreground color for shade characters
    /// * `bg` - Background color
    pub fn draw_grayscale_buffer(
        &mut self,
        x: u32,
        y: u32,
        src: &GrayscaleBuffer,
        fg: Rgba,
        bg: Rgba,
    ) {
        self.draw_grayscale_buffer_with_chars(x, y, src, fg, bg, GRAYSCALE_ASCII);
    }

    /// Draw a grayscale buffer using Unicode shade blocks.
    ///
    /// Maps intensity values to: ' ' `░` `▒` `▓` `█`
    pub fn draw_grayscale_buffer_unicode(
        &mut self,
        x: u32,
        y: u32,
        src: &GrayscaleBuffer,
        fg: Rgba,
        bg: Rgba,
    ) {
        self.draw_grayscale_buffer_with_chars(x, y, src, fg, bg, GRAYSCALE_UNICODE);
    }

    /// Draw a grayscale buffer with custom character set.
    fn draw_grayscale_buffer_with_chars(
        &mut self,
        x: u32,
        y: u32,
        src: &GrayscaleBuffer,
        fg: Rgba,
        bg: Rgba,
        chars: &[char],
    ) {
        let num_chars = chars.len();
        let style = Style::builder().fg(fg).bg(bg).build();

        for py in 0..src.height {
            for px in 0..src.width {
                let intensity = src.get(px, py).unwrap_or(0.0);
                // Map intensity to character index
                let idx =
                    ((intensity * (num_chars - 1) as f32).round() as usize).min(num_chars - 1);
                let ch = chars[idx];
                let cell = Cell::new(ch, style);
                self.set(x + px, y + py, cell);
            }
        }
    }

    /// Draw a grayscale buffer with 2x2 supersampling.
    ///
    /// Each terminal cell represents a 2x2 pixel block, with intensities
    /// averaged for smoother rendering.
    pub fn draw_grayscale_buffer_supersampled(
        &mut self,
        x: u32,
        y: u32,
        src: &GrayscaleBuffer,
        fg: Rgba,
        bg: Rgba,
    ) {
        let cells_w = src.width / 2;
        let cells_h = src.height / 2;
        let num_chars = GRAYSCALE_ASCII.len();
        let style = Style::builder().fg(fg).bg(bg).build();

        for cy in 0..cells_h {
            for cx in 0..cells_w {
                let px = cx * 2;
                let py = cy * 2;

                // Average 2x2 block
                let tl = src.get(px, py).unwrap_or(0.0);
                let tr = src.get(px + 1, py).unwrap_or(0.0);
                let bl = src.get(px, py + 1).unwrap_or(0.0);
                let br = src.get(px + 1, py + 1).unwrap_or(0.0);
                let avg = (tl + tr + bl + br) / 4.0;

                let idx = ((avg * (num_chars - 1) as f32).round() as usize).min(num_chars - 1);
                let ch = GRAYSCALE_ASCII[idx];
                let cell = Cell::new(ch, style);
                self.set(x + cx, y + cy, cell);
            }
        }
    }

    /// Draw pre-computed packed cell data.
    ///
    /// This is useful for rendering output from compute shaders or other
    /// pre-processed rendering pipelines.
    ///
    /// # Arguments
    /// * `x`, `y` - Destination position
    /// * `width`, `height` - Dimensions of packed data
    /// * `cells` - Pre-computed cells in row-major order
    pub fn draw_packed_buffer(&mut self, x: u32, y: u32, width: u32, height: u32, cells: &[Cell]) {
        if cells.len() < (width as usize * height as usize) {
            return;
        }

        for py in 0..height {
            for px in 0..width {
                let idx = (py * width + px) as usize;
                self.set(x + px, y + py, cells[idx]);
            }
        }
    }
}

/// Convert sRGB gamma-encoded value to linear light.
///
/// sRGB uses a piecewise transfer function: linear near zero, then gamma 2.4.
#[inline]
fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert linear light value to sRGB gamma-encoded.
///
/// Inverse of `srgb_to_linear`.
#[inline]
fn linear_to_srgb(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

/// Helper function to average colors using gamma-correct linear blending.
///
/// Colors are converted to linear space before averaging, then converted
/// back to sRGB. This prevents the darkening bias that occurs when averaging
/// directly in sRGB space.
fn average_colors(colors: &[Rgba], mask: &[bool]) -> (Rgba, Rgba) {
    let mut fg_r = 0.0f32;
    let mut fg_g = 0.0f32;
    let mut fg_b = 0.0f32;
    let mut fg_count = 0u32;

    let mut bg_r = 0.0f32;
    let mut bg_g = 0.0f32;
    let mut bg_b = 0.0f32;
    let mut bg_count = 0u32;

    // Accumulate in linear space for correct blending
    for (i, &color) in colors.iter().enumerate() {
        let lin_r = srgb_to_linear(color.r);
        let lin_g = srgb_to_linear(color.g);
        let lin_b = srgb_to_linear(color.b);

        if mask[i] {
            fg_r += lin_r;
            fg_g += lin_g;
            fg_b += lin_b;
            fg_count += 1;
        } else {
            bg_r += lin_r;
            bg_g += lin_g;
            bg_b += lin_b;
            bg_count += 1;
        }
    }

    // Average in linear space, then convert back to sRGB
    let fg = if fg_count > 0 {
        Rgba::rgb(
            linear_to_srgb(fg_r / fg_count as f32),
            linear_to_srgb(fg_g / fg_count as f32),
            linear_to_srgb(fg_b / fg_count as f32),
        )
    } else {
        Rgba::WHITE
    };

    let bg = if bg_count > 0 {
        Rgba::rgb(
            linear_to_srgb(bg_r / bg_count as f32),
            linear_to_srgb(bg_g / bg_count as f32),
            linear_to_srgb(bg_b / bg_count as f32),
        )
    } else {
        Rgba::BLACK
    };

    (fg, bg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_buffer_creation() {
        let buf = PixelBuffer::new(10, 10);
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 10);
        assert_eq!(buf.pixels.len(), 100);
    }

    #[test]
    fn test_pixel_buffer_get_set() {
        let mut buf = PixelBuffer::new(10, 10);
        buf.set(5, 5, Rgba::RED);
        assert_eq!(buf.get(5, 5), Some(Rgba::RED));
    }

    #[test]
    fn test_grayscale_buffer_creation() {
        let buf = GrayscaleBuffer::new(10, 10);
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 10);
        assert_eq!(buf.values.len(), 100);
    }

    #[test]
    fn test_quadrant_chars() {
        // Verify quadrant character mapping
        assert_eq!(QUADRANT_CHARS[0b0000], ' ');
        assert_eq!(QUADRANT_CHARS[0b1111], '█');
        assert_eq!(QUADRANT_CHARS[0b0011], '▀'); // top row
        assert_eq!(QUADRANT_CHARS[0b1100], '▄'); // bottom row
    }

    #[test]
    fn test_draw_supersample_buffer() {
        let mut dest = OptimizedBuffer::new(10, 10);
        let mut src = PixelBuffer::new(4, 4);

        // Set top-left 2x2 to white
        src.set(0, 0, Rgba::WHITE);
        src.set(1, 0, Rgba::WHITE);
        src.set(0, 1, Rgba::WHITE);
        src.set(1, 1, Rgba::WHITE);

        dest.draw_supersample_buffer(0, 0, &src, 0.5);

        // First cell should be full block
        let cell = dest.get(0, 0).unwrap();
        assert_eq!(cell.content, crate::cell::CellContent::Char('█'));
    }

    #[test]
    fn test_draw_grayscale_buffer() {
        let mut dest = OptimizedBuffer::new(10, 10);
        let mut src = GrayscaleBuffer::new(5, 5);

        src.set(0, 0, 0.0); // darkest
        src.set(1, 0, 1.0); // brightest

        dest.draw_grayscale_buffer(0, 0, &src, Rgba::WHITE, Rgba::BLACK);

        let cell0 = dest.get(0, 0).unwrap();
        let cell1 = dest.get(1, 0).unwrap();

        // Darkest should be space, brightest should be @
        assert_eq!(cell0.content, crate::cell::CellContent::Char(' '));
        assert_eq!(cell1.content, crate::cell::CellContent::Char('@'));
    }

    #[test]
    fn test_luminance() {
        assert!((Rgba::WHITE.luminance() - 1.0).abs() < 0.01);
        assert!(Rgba::BLACK.luminance().abs() < 0.01);
        // Pure red has luminance ~0.299
        assert!((Rgba::RED.luminance() - 0.299).abs() < 0.01);
    }

    #[test]
    fn test_srgb_linear_roundtrip() {
        // Test that srgb_to_linear and linear_to_srgb are inverses
        for i in 0..=10 {
            let value = i as f32 / 10.0;
            let linear = super::srgb_to_linear(value);
            let back = super::linear_to_srgb(linear);
            assert!(
                (value - back).abs() < 0.0001,
                "Roundtrip failed for {value}: got {back}"
            );
        }
    }

    #[test]
    fn test_srgb_linear_boundary() {
        // Test at the boundary (0.04045)
        let below = super::srgb_to_linear(0.04);
        let above = super::srgb_to_linear(0.05);
        // Both should be positive and continuous
        assert!(below > 0.0);
        assert!(above > below);
    }

    #[test]
    fn test_gamma_correct_average_brighter_than_naive() {
        // Averaging red and blue in sRGB space causes darkening.
        // Gamma-correct averaging should produce a brighter result.
        let red = Rgba::rgb(1.0, 0.0, 0.0);
        let blue = Rgba::rgb(0.0, 0.0, 1.0);

        // Naive sRGB average: (0.5, 0.0, 0.5)
        let naive_avg_r = 0.5;
        let naive_avg_b = 0.5;

        // Gamma-correct average: should be brighter
        let (fg, _bg) = super::average_colors(&[red, blue], &[true, true]);

        // In gamma-correct averaging, the midpoint of 1.0 and 0.0 in linear space
        // is 0.5, which converts back to sRGB as ~0.735 (brighter than 0.5)
        assert!(
            fg.r > naive_avg_r,
            "Gamma-correct red {} should be brighter than naive {}",
            fg.r,
            naive_avg_r
        );
        assert!(
            fg.b > naive_avg_b,
            "Gamma-correct blue {} should be brighter than naive {}",
            fg.b,
            naive_avg_b
        );
    }

    #[test]
    fn test_gamma_correct_average_preserves_extremes() {
        // Averaging two whites should give white
        let (fg, _) = super::average_colors(&[Rgba::WHITE, Rgba::WHITE], &[true, true]);
        assert!((fg.r - 1.0).abs() < 0.001);
        assert!((fg.g - 1.0).abs() < 0.001);
        assert!((fg.b - 1.0).abs() < 0.001);

        // Averaging two blacks should give black
        let (fg, _) = super::average_colors(&[Rgba::BLACK, Rgba::BLACK], &[true, true]);
        assert!(fg.r.abs() < 0.001);
        assert!(fg.g.abs() < 0.001);
        assert!(fg.b.abs() < 0.001);
    }

    #[test]
    fn test_gamma_correct_fg_bg_separation() {
        // Test that foreground and background are properly separated
        let red = Rgba::RED;
        let green = Rgba::GREEN;
        let blue = Rgba::BLUE;
        let white = Rgba::WHITE;

        // red and green are foreground, blue and white are background
        let mask = [true, true, false, false];
        let (fg, bg) = super::average_colors(&[red, green, blue, white], &mask);

        // fg should be yellow-ish (red + green), bg should be cyan-ish (blue + white)
        assert!(fg.r > 0.5, "FG should have red component");
        assert!(fg.g > 0.5, "FG should have green component");
        assert!(fg.b < 0.3, "FG should not have blue component");

        assert!(bg.b > 0.5, "BG should have blue component");
    }

    #[test]
    fn test_try_from_pixels_success() {
        let pixels = vec![Rgba::RED; 100];
        let result = PixelBuffer::try_from_pixels(10, 10, pixels);
        assert!(result.is_ok());

        let buf = result.unwrap();
        assert_eq!(buf.width, 10);
        assert_eq!(buf.height, 10);
        assert_eq!(buf.pixels.len(), 100);
    }

    #[test]
    fn test_try_from_pixels_size_mismatch() {
        let pixels = vec![Rgba::RED; 50]; // Wrong size
        let result = PixelBuffer::try_from_pixels(10, 10, pixels);
        assert!(result.is_err());

        match result {
            Err(Error::SizeMismatch { expected, actual }) => {
                assert_eq!(expected, 100);
                assert_eq!(actual, 50);
            }
            _ => panic!("expected SizeMismatch error"),
        }
    }

    #[test]
    fn test_try_from_pixels_dimension_overflow() {
        // Very large dimensions that would overflow on 32-bit systems
        // or cause SizeMismatch on 64-bit systems (since empty vec can't match)
        let pixels = vec![];
        let result = PixelBuffer::try_from_pixels(u32::MAX, u32::MAX, pixels);
        assert!(result.is_err());

        // On 32-bit: DimensionOverflow (multiplication overflows)
        // On 64-bit: SizeMismatch (expected huge number, got 0)
        assert!(
            matches!(result, Err(Error::DimensionOverflow { .. }))
                || matches!(result, Err(Error::SizeMismatch { .. }))
        );
    }

    #[test]
    fn test_from_pixels_delegates_to_try() {
        // from_pixels should work for valid input
        let pixels = vec![Rgba::BLUE; 25];
        let buf = PixelBuffer::from_pixels(5, 5, pixels);
        assert_eq!(buf.width, 5);
        assert_eq!(buf.height, 5);
    }
}
