//! Property-based tests for color operations (bd-362e).
//!
//! Uses proptest to verify invariants of Rgba operations including
//! alpha blending, color interpolation, HSV conversion, and hex parsing.

#![allow(clippy::float_cmp)] // Exact float comparison is intentional in tests

use opentui::color::Rgba;
use proptest::prelude::*;

// ============================================================================
// Strategies
// ============================================================================

/// Generate an RGBA color with components in [0, 1].
fn rgba_strategy() -> impl Strategy<Value = Rgba> {
    (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0)
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Generate an opaque RGBA color (alpha = 1.0).
fn opaque_rgba_strategy() -> impl Strategy<Value = Rgba> {
    (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(r, g, b)| Rgba::rgb(r, g, b))
}

/// Generate a fully transparent color (alpha = 0.0).
fn transparent_rgba_strategy() -> impl Strategy<Value = Rgba> {
    (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0).prop_map(|(r, g, b)| Rgba::new(r, g, b, 0.0))
}

/// Generate a valid 6-char hex string.
fn hex_6_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A',
            'B', 'C', 'D', 'E', 'F',
        ]),
        6,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

/// Generate a valid 3-char hex string.
fn hex_3_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A',
            'B', 'C', 'D', 'E', 'F',
        ]),
        3,
    )
    .prop_map(|chars| chars.into_iter().collect::<String>())
}

// ============================================================================
// Alpha Blending Properties (Porter-Duff "over")
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Blending with transparent foreground returns the background.
    #[test]
    fn blend_transparent_fg_returns_bg(fg in transparent_rgba_strategy(), bg in rgba_strategy()) {
        let result = fg.blend_over(bg);
        // When foreground is transparent, result should equal background
        prop_assert!(
            approx_eq(result.r, bg.r) &&
            approx_eq(result.g, bg.g) &&
            approx_eq(result.b, bg.b) &&
            approx_eq(result.a, bg.a),
            "blending transparent foreground over {:?} produced {:?}, expected ~{:?}",
            bg, result, bg
        );
    }

    /// Blending opaque foreground over anything returns the foreground.
    #[test]
    fn blend_opaque_fg_returns_fg(fg in opaque_rgba_strategy(), bg in rgba_strategy()) {
        let result = fg.blend_over(bg);
        prop_assert!(
            approx_eq(result.r, fg.r) &&
            approx_eq(result.g, fg.g) &&
            approx_eq(result.b, fg.b) &&
            approx_eq(result.a, fg.a),
            "blending opaque {:?} over {:?} produced {:?}, expected ~{:?}",
            fg, bg, result, fg
        );
    }

    /// Result alpha is always in [0, 1].
    #[test]
    fn blend_result_alpha_bounded(fg in rgba_strategy(), bg in rgba_strategy()) {
        let result = fg.blend_over(bg);
        prop_assert!(result.a >= 0.0 && result.a <= 1.0,
            "blend result alpha {} out of bounds [0, 1]", result.a);
    }

    /// Result RGB components are in [0, 1].
    #[test]
    fn blend_result_rgb_bounded(fg in rgba_strategy(), bg in rgba_strategy()) {
        let result = fg.blend_over(bg);
        prop_assert!(result.r >= 0.0 && result.r <= 1.0,
            "blend result r {} out of bounds", result.r);
        prop_assert!(result.g >= 0.0 && result.g <= 1.0,
            "blend result g {} out of bounds", result.g);
        prop_assert!(result.b >= 0.0 && result.b <= 1.0,
            "blend result b {} out of bounds", result.b);
    }

    /// Blending a color over itself returns the same color (for opaque colors).
    #[test]
    fn blend_opaque_over_self_is_self(c in opaque_rgba_strategy()) {
        let result = c.blend_over(c);
        prop_assert!(
            approx_eq(result.r, c.r) &&
            approx_eq(result.g, c.g) &&
            approx_eq(result.b, c.b) &&
            approx_eq(result.a, c.a),
            "blending {:?} over itself produced {:?}", c, result
        );
    }

    /// Blending over transparent background returns foreground with original alpha.
    #[test]
    fn blend_over_transparent_bg(fg in rgba_strategy()) {
        let bg = Rgba::TRANSPARENT;
        let result = fg.blend_over(bg);
        // Result should be foreground scaled by its own alpha
        prop_assert!(approx_eq(result.a, fg.a),
            "alpha should match foreground: {} vs {}", result.a, fg.a);
    }
}

// ============================================================================
// Color Interpolation (lerp) Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// lerp(0.0) returns the first color.
    #[test]
    fn lerp_at_zero_is_first(a in rgba_strategy(), b in rgba_strategy()) {
        let result = a.lerp(b, 0.0);
        prop_assert!(
            approx_eq(result.r, a.r) &&
            approx_eq(result.g, a.g) &&
            approx_eq(result.b, a.b) &&
            approx_eq(result.a, a.a),
            "lerp at 0.0: {:?} != {:?}", result, a
        );
    }

    /// lerp(1.0) returns the second color.
    #[test]
    fn lerp_at_one_is_second(a in rgba_strategy(), b in rgba_strategy()) {
        let result = a.lerp(b, 1.0);
        prop_assert!(
            approx_eq(result.r, b.r) &&
            approx_eq(result.g, b.g) &&
            approx_eq(result.b, b.b) &&
            approx_eq(result.a, b.a),
            "lerp at 1.0: {:?} != {:?}", result, b
        );
    }

    /// lerp result is always within [0, 1] bounds for all components.
    #[test]
    fn lerp_result_bounded(a in rgba_strategy(), b in rgba_strategy(), t in 0.0f32..=1.0) {
        let result = a.lerp(b, t);
        prop_assert!(result.r >= 0.0 && result.r <= 1.0, "r out of bounds: {}", result.r);
        prop_assert!(result.g >= 0.0 && result.g <= 1.0, "g out of bounds: {}", result.g);
        prop_assert!(result.b >= 0.0 && result.b <= 1.0, "b out of bounds: {}", result.b);
        prop_assert!(result.a >= 0.0 && result.a <= 1.0, "a out of bounds: {}", result.a);
    }

    /// lerp(0.5) is approximately the midpoint.
    #[test]
    fn lerp_half_is_midpoint(a in rgba_strategy(), b in rgba_strategy()) {
        let result = a.lerp(b, 0.5);
        let expected_r = (a.r + b.r) / 2.0;
        let expected_g = (a.g + b.g) / 2.0;
        let expected_b = (a.b + b.b) / 2.0;
        let expected_a = (a.a + b.a) / 2.0;
        prop_assert!(approx_eq(result.r, expected_r), "r: {} != {}", result.r, expected_r);
        prop_assert!(approx_eq(result.g, expected_g), "g: {} != {}", result.g, expected_g);
        prop_assert!(approx_eq(result.b, expected_b), "b: {} != {}", result.b, expected_b);
        prop_assert!(approx_eq(result.a, expected_a), "a: {} != {}", result.a, expected_a);
    }

    /// lerp is monotonic: result component is between a and b.
    #[test]
    fn lerp_is_monotonic(a in rgba_strategy(), b in rgba_strategy(), t in 0.0f32..=1.0) {
        let result = a.lerp(b, t);
        // Each component should be between a and b (or equal to one of them)
        prop_assert!(between_inclusive(result.r, a.r, b.r),
            "r={} not between {} and {}", result.r, a.r, b.r);
        prop_assert!(between_inclusive(result.g, a.g, b.g),
            "g={} not between {} and {}", result.g, a.g, b.g);
        prop_assert!(between_inclusive(result.b, a.b, b.b),
            "b={} not between {} and {}", result.b, a.b, b.b);
        prop_assert!(between_inclusive(result.a, a.a, b.a),
            "a={} not between {} and {}", result.a, a.a, b.a);
    }

    /// lerp clamps t values outside [0, 1].
    #[test]
    fn lerp_clamps_t(a in rgba_strategy(), b in rgba_strategy()) {
        // t < 0 should clamp to 0
        let result_neg = a.lerp(b, -0.5);
        prop_assert!(approx_eq(result_neg.r, a.r), "t=-0.5 should return a.r");

        // t > 1 should clamp to 1
        let result_pos = a.lerp(b, 1.5);
        prop_assert!(approx_eq(result_pos.r, b.r), "t=1.5 should return b.r");
    }
}

// ============================================================================
// HSV Conversion Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// from_hsv produces colors with components in [0, 1].
    #[test]
    fn hsv_result_bounded(h in 0.0f32..360.0, s in 0.0f32..=1.0, v in 0.0f32..=1.0) {
        let c = Rgba::from_hsv(h, s, v);
        prop_assert!(c.r >= 0.0 && c.r <= 1.0, "r out of bounds: {}", c.r);
        prop_assert!(c.g >= 0.0 && c.g <= 1.0, "g out of bounds: {}", c.g);
        prop_assert!(c.b >= 0.0 && c.b <= 1.0, "b out of bounds: {}", c.b);
        prop_assert!(c.a == 1.0, "from_hsv should produce opaque color");
    }

    /// HSV with s=0 produces grayscale (r=g=b=v).
    #[test]
    fn hsv_zero_saturation_is_gray(h in 0.0f32..360.0, v in 0.0f32..=1.0) {
        let c = Rgba::from_hsv(h, 0.0, v);
        prop_assert!(approx_eq(c.r, v), "r={} should equal v={}", c.r, v);
        prop_assert!(approx_eq(c.g, v), "g={} should equal v={}", c.g, v);
        prop_assert!(approx_eq(c.b, v), "b={} should equal v={}", c.b, v);
    }

    /// HSV with v=0 produces black.
    #[test]
    fn hsv_zero_value_is_black(h in 0.0f32..360.0, s in 0.0f32..=1.0) {
        let c = Rgba::from_hsv(h, s, 0.0);
        prop_assert!(approx_eq(c.r, 0.0), "r should be 0: {}", c.r);
        prop_assert!(approx_eq(c.g, 0.0), "g should be 0: {}", c.g);
        prop_assert!(approx_eq(c.b, 0.0), "b should be 0: {}", c.b);
    }

    /// HSV is periodic in hue: h and h+360 produce the same color.
    #[test]
    fn hsv_hue_periodic(h in 0.0f32..360.0, s in 0.0f32..=1.0, v in 0.0f32..=1.0) {
        let c1 = Rgba::from_hsv(h, s, v);
        let c2 = Rgba::from_hsv(h + 360.0, s, v);
        prop_assert!(approx_eq(c1.r, c2.r), "r: {} != {}", c1.r, c2.r);
        prop_assert!(approx_eq(c1.g, c2.g), "g: {} != {}", c1.g, c2.g);
        prop_assert!(approx_eq(c1.b, c2.b), "b: {} != {}", c1.b, c2.b);
    }

    /// Primary colors at HSV boundaries.
    #[test]
    fn hsv_primary_colors(_dummy in Just(())) {
        // Red at h=0, s=1, v=1
        let red = Rgba::from_hsv(0.0, 1.0, 1.0);
        prop_assert!(approx_eq(red.r, 1.0) && approx_eq(red.g, 0.0) && approx_eq(red.b, 0.0),
            "h=0 should be red: {:?}", red);

        // Green at h=120, s=1, v=1
        let green = Rgba::from_hsv(120.0, 1.0, 1.0);
        prop_assert!(approx_eq(green.r, 0.0) && approx_eq(green.g, 1.0) && approx_eq(green.b, 0.0),
            "h=120 should be green: {:?}", green);

        // Blue at h=240, s=1, v=1
        let blue = Rgba::from_hsv(240.0, 1.0, 1.0);
        prop_assert!(approx_eq(blue.r, 0.0) && approx_eq(blue.g, 0.0) && approx_eq(blue.b, 1.0),
            "h=240 should be blue: {:?}", blue);
    }
}

// ============================================================================
// Hex Parsing Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Valid 6-char hex always parses successfully.
    #[test]
    fn hex_6_always_parses(hex in hex_6_strategy()) {
        prop_assert!(Rgba::from_hex(&hex).is_some(),
            "valid hex {} should parse", hex);

        // With # prefix
        let with_hash = format!("#{hex}");
        prop_assert!(Rgba::from_hex(&with_hash).is_some(),
            "valid hex #{} should parse", hex);
    }

    /// Valid 3-char hex always parses successfully.
    #[test]
    fn hex_3_always_parses(hex in hex_3_strategy()) {
        prop_assert!(Rgba::from_hex(&hex).is_some(),
            "valid 3-char hex {} should parse", hex);
    }

    /// Parsed hex color has components in [0, 1].
    #[test]
    fn hex_parsed_bounded(hex in hex_6_strategy()) {
        if let Some(c) = Rgba::from_hex(&hex) {
            prop_assert!(c.r >= 0.0 && c.r <= 1.0, "r out of bounds");
            prop_assert!(c.g >= 0.0 && c.g <= 1.0, "g out of bounds");
            prop_assert!(c.b >= 0.0 && c.b <= 1.0, "b out of bounds");
            prop_assert!(c.a == 1.0, "6-char hex should be opaque");
        }
    }

    /// Hex parsing is case-insensitive.
    #[test]
    fn hex_case_insensitive(hex in hex_6_strategy()) {
        let lower = Rgba::from_hex(&hex.to_lowercase());
        let upper = Rgba::from_hex(&hex.to_uppercase());
        prop_assert_eq!(lower, upper, "hex parsing should be case-insensitive");
    }

    /// Empty and invalid length hex returns None.
    #[test]
    fn hex_invalid_length_fails(len in (0usize..10).prop_filter("not 3, 6, or 8", |l| *l != 3 && *l != 6 && *l != 8)) {
        let hex: String = std::iter::repeat('a').take(len).collect();
        prop_assert!(Rgba::from_hex(&hex).is_none(),
            "hex of length {} should fail: {}", len, hex);
    }

    /// Known hex values produce expected colors.
    #[test]
    fn hex_known_values(_dummy in Just(())) {
        // Black
        let black = Rgba::from_hex("000000").unwrap();
        prop_assert!(approx_eq(black.r, 0.0) && approx_eq(black.g, 0.0) && approx_eq(black.b, 0.0));

        // White
        let white = Rgba::from_hex("FFFFFF").unwrap();
        prop_assert!(approx_eq(white.r, 1.0) && approx_eq(white.g, 1.0) && approx_eq(white.b, 1.0));

        // Red
        let red = Rgba::from_hex("FF0000").unwrap();
        prop_assert!(approx_eq(red.r, 1.0) && approx_eq(red.g, 0.0) && approx_eq(red.b, 0.0));
    }
}

// ============================================================================
// Luminance Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Luminance is in [0, 1] for valid colors.
    #[test]
    fn luminance_bounded(c in rgba_strategy()) {
        let lum = c.luminance();
        prop_assert!(lum >= 0.0 && lum <= 1.0, "luminance {} out of bounds", lum);
    }

    /// Black has luminance 0.
    #[test]
    fn black_luminance_zero(_dummy in Just(())) {
        prop_assert!(approx_eq(Rgba::BLACK.luminance(), 0.0));
    }

    /// White has luminance 1.
    #[test]
    fn white_luminance_one(_dummy in Just(())) {
        prop_assert!(approx_eq(Rgba::WHITE.luminance(), 1.0));
    }

    /// Luminance is deterministic.
    #[test]
    fn luminance_deterministic(c in rgba_strategy()) {
        let l1 = c.luminance();
        let l2 = c.luminance();
        prop_assert_eq!(l1, l2);
    }
}

// ============================================================================
// to_256_color / to_16_color Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// to_16_color returns value in [0, 15].
    #[test]
    fn to_16_color_in_range(c in rgba_strategy()) {
        let idx = c.to_16_color();
        prop_assert!(idx <= 15, "16-color index {} out of range", idx);
    }

    /// to_256_color is deterministic.
    #[test]
    fn to_256_deterministic(c in rgba_strategy()) {
        let idx1 = c.to_256_color();
        let idx2 = c.to_256_color();
        prop_assert_eq!(idx1, idx2);
    }
}

// ============================================================================
// Alpha Operations Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// with_alpha preserves RGB.
    #[test]
    fn with_alpha_preserves_rgb(c in rgba_strategy(), new_alpha in 0.0f32..=1.0) {
        let result = c.with_alpha(new_alpha);
        prop_assert_eq!(result.r, c.r);
        prop_assert_eq!(result.g, c.g);
        prop_assert_eq!(result.b, c.b);
        prop_assert_eq!(result.a, new_alpha);
    }

    /// multiply_alpha with factor 1.0 is identity.
    #[test]
    fn multiply_alpha_one_is_identity(c in rgba_strategy()) {
        let result = c.multiply_alpha(1.0);
        prop_assert_eq!(result.a, c.a);
    }

    /// multiply_alpha with factor 0.0 produces transparent.
    #[test]
    fn multiply_alpha_zero_is_transparent(c in rgba_strategy()) {
        let result = c.multiply_alpha(0.0);
        prop_assert_eq!(result.a, 0.0);
    }

    /// is_transparent and is_opaque are mutually exclusive for valid alphas.
    #[test]
    fn transparent_opaque_exclusive(c in rgba_strategy()) {
        // They can't both be true
        prop_assert!(!(c.is_transparent() && c.is_opaque()),
            "color cannot be both transparent and opaque");
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Approximate equality for floating-point comparison.
fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-5
}

/// Check if value is between min and max (inclusive), accounting for either order.
fn between_inclusive(value: f32, a: f32, b: f32) -> bool {
    let (min, max) = if a <= b { (a, b) } else { (b, a) };
    value >= min - 1e-5 && value <= max + 1e-5
}
