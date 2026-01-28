//! Memory safety regression tests.
//!
//! These tests ensure that memory safety fixes cannot regress.
//! They cover integer overflow protection across the codebase:
//! - Buffer index calculations (OptimizedBuffer, PixelBuffer, HitGrid)
//! - GraphemeId width encoding
//! - GraphemePool ID limits

use opentui::buffer::{OptimizedBuffer, PixelBuffer, GrayscaleBuffer};
use opentui::cell::GraphemeId;
use opentui::color::Rgba;
use opentui::grapheme_pool::GraphemePool;
use opentui::renderer::HitGrid;

// ============================================
// Buffer Index Overflow Protection
// ============================================

mod buffer_overflow {
    use super::*;

    #[test]
    fn optimized_buffer_out_of_bounds_returns_none() {
        let buffer = OptimizedBuffer::new(100, 100);

        // Out of bounds should return None, not panic
        assert!(buffer.get(100, 0).is_none());
        assert!(buffer.get(0, 100).is_none());
        assert!(buffer.get(1000, 1000).is_none());
        assert!(buffer.get(u32::MAX, 0).is_none());
        assert!(buffer.get(0, u32::MAX).is_none());
        assert!(buffer.get(u32::MAX, u32::MAX).is_none());
    }

    #[test]
    fn optimized_buffer_large_dimensions() {
        // Test with dimensions that could cause overflow if not handled
        let buffer = OptimizedBuffer::new(1000, 1000);

        // Should be able to access corners without overflow
        assert!(buffer.get(0, 0).is_some());
        assert!(buffer.get(999, 999).is_some());
        assert!(buffer.get(1000, 1000).is_none());
    }

    #[test]
    fn pixel_buffer_out_of_bounds_returns_none() {
        let buffer = PixelBuffer::new(100, 100);

        // Out of bounds should return None
        assert!(buffer.get(100, 0).is_none());
        assert!(buffer.get(0, 100).is_none());
        assert!(buffer.get(u32::MAX, u32::MAX).is_none());
    }

    #[test]
    fn pixel_buffer_set_out_of_bounds_no_panic() {
        let mut buffer = PixelBuffer::new(100, 100);

        // Setting out of bounds should silently do nothing
        buffer.set(100, 0, Rgba::RED);
        buffer.set(0, 100, Rgba::RED);
        buffer.set(u32::MAX, u32::MAX, Rgba::RED);

        // Verify no corruption of valid data
        assert_eq!(buffer.get(0, 0), Some(Rgba::TRANSPARENT));
    }

    #[test]
    fn grayscale_buffer_out_of_bounds_returns_none() {
        let buffer = GrayscaleBuffer::new(100, 100);

        assert!(buffer.get(100, 0).is_none());
        assert!(buffer.get(0, 100).is_none());
        assert!(buffer.get(u32::MAX, u32::MAX).is_none());
    }

    #[test]
    fn grayscale_buffer_set_out_of_bounds_no_panic() {
        let mut buffer = GrayscaleBuffer::new(100, 100);

        // Setting out of bounds should silently do nothing
        buffer.set(100, 0, 1.0);
        buffer.set(u32::MAX, u32::MAX, 1.0);

        // Verify no corruption
        assert_eq!(buffer.get(0, 0), Some(0.0));
    }

    #[test]
    fn hit_grid_out_of_bounds_returns_none() {
        let grid = HitGrid::new(100, 100);

        assert!(grid.test(100, 0).is_none());
        assert!(grid.test(0, 100).is_none());
        assert!(grid.test(u32::MAX, u32::MAX).is_none());
    }

    #[test]
    fn hit_grid_register_out_of_bounds_no_panic() {
        let mut grid = HitGrid::new(100, 100);

        // Registering out of bounds should be handled gracefully
        grid.register(90, 90, 20, 20, 1); // Extends beyond bounds
        grid.register(1000, 1000, 10, 10, 2); // Completely out of bounds

        // Valid registration should work
        assert_eq!(grid.test(95, 95), Some(1));
    }
}

// ============================================
// GraphemeId Width Saturation
// ============================================

mod grapheme_id_width {
    use super::*;

    #[test]
    fn width_saturation_at_128() {
        let id = GraphemeId::new(1, 128);
        assert_eq!(id.width(), 127, "width 128 should saturate to 127");
    }

    #[test]
    fn width_saturation_at_max() {
        let id = GraphemeId::new(1, 255);
        assert_eq!(id.width(), 127, "width 255 should saturate to 127");
    }

    #[test]
    fn width_at_boundary() {
        let id = GraphemeId::new(1, 127);
        assert_eq!(id.width(), 127, "width 127 should be preserved");
    }

    #[test]
    fn width_zero() {
        let id = GraphemeId::new(1, 0);
        assert_eq!(id.width(), 0, "width 0 should be preserved");
    }

    #[test]
    fn pool_id_preserved_with_saturated_width() {
        let id = GraphemeId::new(12345, 200);
        assert_eq!(id.pool_id(), 12345, "pool_id should be preserved");
        assert_eq!(id.width(), 127, "width should be saturated");
    }
}

// ============================================
// GraphemePool Capacity
// ============================================

mod grapheme_pool_capacity {
    use super::*;

    #[test]
    fn capacity_decreases_with_allocation() {
        let mut pool = GraphemePool::new();
        let initial = pool.capacity_remaining();

        let _ = pool.alloc("test");
        assert_eq!(
            pool.capacity_remaining(),
            initial - 1,
            "capacity should decrease"
        );
    }

    #[test]
    fn capacity_restored_on_deallocation() {
        let mut pool = GraphemePool::new();
        let initial = pool.capacity_remaining();

        let id = pool.alloc("test");
        pool.decref(id);

        assert_eq!(
            pool.capacity_remaining(),
            initial,
            "capacity should be restored"
        );
    }

    #[test]
    fn is_full_on_empty_pool() {
        let pool = GraphemePool::new();
        assert!(!pool.is_full(), "empty pool should not be full");
    }

    #[test]
    fn intern_reuses_existing() {
        let mut pool = GraphemePool::new();
        let initial = pool.capacity_remaining();

        let id1 = pool.intern("test");
        let id2 = pool.intern("test");

        assert_eq!(id1, id2, "intern should return same ID");
        assert_eq!(
            pool.capacity_remaining(),
            initial - 1,
            "should only use one slot"
        );
    }
}

// ============================================
// Combined Safety Invariants
// ============================================

mod safety_invariants {
    use super::*;

    #[test]
    fn buffer_operations_preserve_data() {
        let mut buffer = OptimizedBuffer::new(50, 50);

        // Set some valid data
        use opentui::cell::Cell;
        use opentui::style::Style;
        let cell = Cell::new('X', Style::default());
        buffer.set(25, 25, cell);

        // Attempt invalid operations
        buffer.set(100, 100, cell); // Out of bounds
        buffer.set(u32::MAX, 0, cell); // Overflow coords

        // Original data should be preserved
        let retrieved = buffer.get(25, 25);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().content.as_char(), Some('X'));
    }

    #[test]
    fn grapheme_pool_reuse_works_correctly() {
        let mut pool = GraphemePool::new();

        // Allocate and free
        let id1 = pool.alloc("first");
        let slot1 = id1.pool_id();
        pool.decref(id1);

        // New allocation should reuse the freed slot
        let id2 = pool.alloc("second");
        let slot2 = id2.pool_id();

        assert_eq!(slot1, slot2, "should reuse freed slot");
        assert_eq!(pool.get(id2), Some("second"), "should get new content");
    }
}
