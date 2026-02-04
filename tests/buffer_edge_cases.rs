//! Edge case tests for buffer operations (bd-1t6h).
//!
//! Tests boundary conditions, drawing edge cases, scissor edge cases,
//! and opacity edge cases that may not be covered by other tests.

use opentui::buffer::{ClipRect, OptimizedBuffer};
use opentui::cell::Cell;
use opentui::color::Rgba;
use opentui::style::Style;
use opentui_rust as opentui;

// ============================================================================
// Boundary Conditions
// ============================================================================

mod boundary_conditions {
    use super::*;

    #[test]
    fn zero_size_buffer_no_panic() {
        // Zero dimensions are clamped to 1 to prevent division-by-zero in iter_cells()

        // Zero-width buffer -> clamped to 1
        let buf = OptimizedBuffer::new(0, 10);
        assert_eq!(buf.size(), (1, 10));
        assert!(buf.get(0, 0).is_some()); // Has a valid cell now

        // Zero-height buffer -> clamped to 1
        let buf = OptimizedBuffer::new(10, 0);
        assert_eq!(buf.size(), (10, 1));
        assert!(buf.get(0, 0).is_some());

        // Completely zero buffer -> clamped to 1x1
        let buf = OptimizedBuffer::new(0, 0);
        assert_eq!(buf.size(), (1, 1));
        assert!(buf.get(0, 0).is_some());
    }

    #[test]
    fn single_cell_buffer() {
        let mut buf = OptimizedBuffer::new(1, 1);
        assert_eq!(buf.size(), (1, 1));

        // Can get the single cell
        assert!(buf.get(0, 0).is_some());

        // Out of bounds returns None
        assert!(buf.get(1, 0).is_none());
        assert!(buf.get(0, 1).is_none());
        assert!(buf.get(1, 1).is_none());

        // Can set the single cell
        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(0, 0, cell);
        assert_eq!(buf.get(0, 0).unwrap().content.as_char(), Some('X'));
    }

    #[test]
    fn very_wide_buffer() {
        let width = 10000u32;
        let buf = OptimizedBuffer::new(width, 1);
        assert_eq!(buf.size(), (width, 1));

        // Can access first and last cells
        assert!(buf.get(0, 0).is_some());
        assert!(buf.get(width - 1, 0).is_some());
        assert!(buf.get(width, 0).is_none());
    }

    #[test]
    fn very_tall_buffer() {
        let height = 10000u32;
        let buf = OptimizedBuffer::new(1, height);
        assert_eq!(buf.size(), (1, height));

        // Can access first and last cells
        assert!(buf.get(0, 0).is_some());
        assert!(buf.get(0, height - 1).is_some());
        assert!(buf.get(0, height).is_none());
    }

    #[test]
    fn large_buffer_allocation() {
        // Test a moderately large buffer (1000x1000 = 1M cells)
        let buf = OptimizedBuffer::new(1000, 1000);
        assert_eq!(buf.size(), (1000, 1000));

        // Verify byte_size is reasonable
        let byte_size = buf.byte_size();
        assert!(byte_size > 0);
        // Each cell should be at least a few bytes
        assert!(byte_size >= 1_000_000);
    }
}

// ============================================================================
// Drawing Edge Cases
// ============================================================================

mod drawing_edge_cases {
    use super::*;

    #[test]
    fn draw_at_exact_boundary() {
        let mut buf = OptimizedBuffer::new(10, 10);
        let cell = Cell::new('X', Style::fg(Rgba::RED));

        // Drawing at last valid position should work
        buf.set(9, 9, cell);
        assert_eq!(buf.get(9, 9).unwrap().content.as_char(), Some('X'));
    }

    #[test]
    fn draw_past_boundary_no_panic() {
        let mut buf = OptimizedBuffer::new(10, 10);
        let cell = Cell::new('X', Style::fg(Rgba::RED));

        // Drawing outside bounds should be silently ignored
        buf.set(10, 0, cell);
        buf.set(0, 10, cell);
        buf.set(100, 100, cell);
        buf.set(u32::MAX, u32::MAX, cell);

        // Buffer should be unchanged
        assert!(buf.get(9, 9).unwrap().content.is_empty());
    }

    #[test]
    fn fill_rect_past_boundary() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Fill starting inside but extending past boundary
        buf.fill_rect(5, 5, 100, 100, Rgba::RED);

        // Cells inside should be filled
        let cell = buf.get(9, 9).unwrap();
        assert!(!cell.bg.is_transparent());

        // Original cells should still work
        buf.fill_rect(0, 0, 5, 5, Rgba::BLUE);
    }

    #[test]
    fn fill_rect_zero_dimensions() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Zero width or height should do nothing
        buf.fill_rect(0, 0, 0, 10, Rgba::RED);
        buf.fill_rect(0, 0, 10, 0, Rgba::RED);
        buf.fill_rect(0, 0, 0, 0, Rgba::RED);

        // Buffer should remain white
        assert!(approx_eq_rgba(buf.get(0, 0).unwrap().bg, Rgba::WHITE));
    }

    #[test]
    fn fill_rect_completely_outside() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Fill completely outside buffer
        buf.fill_rect(100, 100, 10, 10, Rgba::RED);

        // Buffer should remain unchanged
        assert!(approx_eq_rgba(buf.get(0, 0).unwrap().bg, Rgba::WHITE));
    }

    #[test]
    fn draw_text_at_boundary() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Draw text at last row
        buf.draw_text(0, 9, "Hello", Style::fg(Rgba::RED));

        // Text should be visible up to buffer edge
        assert_eq!(buf.get(0, 9).unwrap().content.as_char(), Some('H'));

        // Draw text at last column - should clip
        buf.draw_text(9, 0, "Hello", Style::fg(Rgba::RED));
        // Only first char should be visible
        assert_eq!(buf.get(9, 0).unwrap().content.as_char(), Some('H'));
    }

    #[test]
    fn draw_text_completely_outside() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Draw text completely outside buffer
        buf.draw_text(100, 100, "Hello", Style::fg(Rgba::RED));

        // Buffer should remain unchanged
        assert!(buf.get(0, 0).unwrap().content.is_empty());
    }

    #[test]
    fn clear_on_zero_buffer() {
        // Zero dimensions are clamped to 1, so this is actually a 1x1 buffer
        let mut buf = OptimizedBuffer::new(0, 0);
        assert_eq!(buf.size(), (1, 1));
        // Should not panic
        buf.clear(Rgba::RED);
        // Verify clear worked on the 1x1 buffer
        assert!(approx_eq_rgba(buf.get(0, 0).unwrap().bg, Rgba::RED));
    }
}

// ============================================================================
// Scissor Edge Cases
// ============================================================================

mod scissor_edge_cases {
    use super::*;

    #[test]
    fn scissor_larger_than_buffer() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Scissor larger than buffer should work
        buf.push_scissor(ClipRect::new(0, 0, 100, 100));

        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(5, 5, cell);

        // Drawing should still work within buffer bounds
        assert_eq!(buf.get(5, 5).unwrap().content.as_char(), Some('X'));

        buf.pop_scissor();
    }

    #[test]
    fn scissor_with_negative_coords() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Scissor starting at negative coords
        buf.push_scissor(ClipRect::new(-5, -5, 15, 15));

        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(0, 0, cell);

        // Drawing at (0,0) should work since it's within the scissor
        assert_eq!(buf.get(0, 0).unwrap().content.as_char(), Some('X'));

        buf.pop_scissor();
    }

    #[test]
    fn deeply_nested_scissors() {
        let mut buf = OptimizedBuffer::new(100, 100);

        // Push 50 nested scissors, each slightly smaller (avoiding overflow)
        for i in 0..50u32 {
            let offset = i32::try_from(i).expect("loop index fits in i32");
            let size = 100 - (i * 2);
            buf.push_scissor(ClipRect::new(offset, offset, size, size));
        }

        // Drawing at center should work if within innermost scissor
        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(50, 50, cell);

        // Pop all scissors
        for _ in 0..50 {
            buf.pop_scissor();
        }

        // Buffer should be stable
        assert!(buf.get(50, 50).is_some());
    }

    #[test]
    fn scissor_empty_area() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Empty scissor (zero width/height)
        buf.push_scissor(ClipRect::new(5, 5, 0, 0));

        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(5, 5, cell);

        // Drawing should be clipped (not visible)
        assert!(buf.get(5, 5).unwrap().content.is_empty());

        buf.pop_scissor();
    }

    #[test]
    fn scissor_outside_buffer() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Scissor completely outside buffer
        buf.push_scissor(ClipRect::new(100, 100, 10, 10));

        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(5, 5, cell);

        // Drawing should be clipped (scissor doesn't overlap buffer)
        assert!(buf.get(5, 5).unwrap().content.is_empty());

        buf.pop_scissor();
    }

    #[test]
    fn pop_more_scissors_than_pushed() {
        let mut buf = OptimizedBuffer::new(10, 10);

        buf.push_scissor(ClipRect::new(0, 0, 10, 10));
        buf.pop_scissor();

        // Extra pops should not panic (protected by stack underflow check)
        buf.pop_scissor();
        buf.pop_scissor();

        // Buffer should still work
        let cell = Cell::new('X', Style::fg(Rgba::RED));
        buf.set(5, 5, cell);
        assert_eq!(buf.get(5, 5).unwrap().content.as_char(), Some('X'));
    }

    #[test]
    fn scissor_intersection() {
        let mut buf = OptimizedBuffer::new(20, 20);

        // Push overlapping scissors
        buf.push_scissor(ClipRect::new(0, 0, 15, 15));
        buf.push_scissor(ClipRect::new(5, 5, 15, 15));

        // Only cells in the intersection (5-14, 5-14) should be drawable
        let cell = Cell::new('X', Style::fg(Rgba::RED));

        // Inside intersection
        buf.set(10, 10, cell);
        assert_eq!(buf.get(10, 10).unwrap().content.as_char(), Some('X'));

        // Outside intersection (but inside first scissor)
        buf.set(2, 2, cell);
        assert!(buf.get(2, 2).unwrap().content.is_empty());

        buf.pop_scissor();
        buf.pop_scissor();
    }
}

// ============================================================================
// Opacity Edge Cases
// ============================================================================

mod opacity_edge_cases {
    use super::*;

    #[test]
    fn opacity_greater_than_one() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::BLACK);

        // Push opacity > 1.0 (should be clamped or handled gracefully)
        buf.push_opacity(2.0);

        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set(5, 5, cell);

        // Drawing should still work, alpha should be reasonable
        let result = buf.get(5, 5).unwrap();
        assert!(result.bg.a >= 0.0 && result.bg.a <= 1.0);

        buf.pop_opacity();
    }

    #[test]
    fn opacity_less_than_zero() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Push opacity < 0.0 (should be clamped or handled gracefully)
        buf.push_opacity(-0.5);

        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set(5, 5, cell);

        // Drawing should handle gracefully
        buf.pop_opacity();
    }

    #[test]
    fn opacity_zero() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Push zero opacity
        buf.push_opacity(0.0);

        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set(5, 5, cell);

        // With zero opacity, the cell should be fully transparent
        let result = buf.get(5, 5).unwrap();
        // The cell might still have content but with transparent colors
        assert!(result.bg.a <= 0.001 || approx_eq_rgba(result.bg, Rgba::WHITE));

        buf.pop_opacity();
    }

    #[test]
    fn very_small_opacity() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Push very small opacity
        buf.push_opacity(1e-10);

        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set(5, 5, cell);

        // Should handle gracefully without numerical issues
        buf.pop_opacity();
    }

    #[test]
    fn deeply_nested_opacity() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::WHITE);

        // Push 100 nested opacities of 0.99
        for _ in 0..100 {
            buf.push_opacity(0.99);
        }

        // Combined opacity should be very small (~0.99^100 ~ 0.366)
        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set(5, 5, cell);

        // Pop all opacities
        for _ in 0..100 {
            buf.pop_opacity();
        }

        // Buffer should be stable
        assert!(buf.get(5, 5).is_some());
    }

    #[test]
    fn pop_more_opacities_than_pushed() {
        let mut buf = OptimizedBuffer::new(10, 10);

        buf.push_opacity(0.5);
        buf.pop_opacity();

        // Extra pops should not panic
        buf.pop_opacity();
        buf.pop_opacity();

        // Buffer should still work with default opacity
        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set(5, 5, cell);

        let result = buf.get(5, 5).unwrap();
        // With default opacity (1.0), alpha should be preserved
        assert!(result.bg.a > 0.9);
    }

    #[test]
    fn opacity_interaction_with_blending() {
        let mut buf = OptimizedBuffer::new(10, 10);
        buf.clear(Rgba::BLACK);

        // Set up a background cell
        buf.set_blended(5, 5, Cell::new(' ', Style::NONE.with_bg(Rgba::WHITE)));

        // Now draw with 50% opacity
        buf.push_opacity(0.5);
        let cell = Cell::new('X', Style::fg(Rgba::RED).with_bg(Rgba::RED));
        buf.set_blended(5, 5, cell);
        buf.pop_opacity();

        // Result should be a blend of red and white
        let result = buf.get(5, 5).unwrap();
        // Should have some redness but not pure red
        assert!(result.bg.r > 0.3);
    }
}

// ============================================================================
// Wide Character Edge Cases
// ============================================================================

mod wide_char_edge_cases {
    use super::*;

    #[test]
    fn wide_char_at_right_edge() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Drawing a wide char at the last column
        // The continuation cell would be out of bounds
        buf.draw_text(9, 0, "中", Style::fg(Rgba::RED));

        // The wide char might be clipped or handled specially
        // Just ensure no panic
        let _ = buf.get(9, 0);
    }

    #[test]
    fn wide_char_at_second_to_last() {
        let mut buf = OptimizedBuffer::new(10, 10);

        // Drawing at second-to-last column should work
        buf.draw_text(8, 0, "中", Style::fg(Rgba::RED));

        // Wide char should be visible at (8,0)
        // Continuation at (9,0)
    }

    #[test]
    fn many_wide_chars() {
        let mut buf = OptimizedBuffer::new(100, 10);

        // Draw many wide chars
        buf.draw_text(0, 0, "中文日本語한국어", Style::fg(Rgba::RED));

        // Should handle without panic
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn approx_eq_rgba(a: Rgba, b: Rgba) -> bool {
    (a.r - b.r).abs() < 0.01
        && (a.g - b.g).abs() < 0.01
        && (a.b - b.b).abs() < 0.01
        && (a.a - b.a).abs() < 0.01
}
