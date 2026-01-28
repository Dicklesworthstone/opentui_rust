//! Hit testing grid for mouse events.

/// A hit testing grid that maps screen positions to widget IDs.
#[derive(Clone, Debug)]
pub struct HitGrid {
    width: u32,
    height: u32,
    cells: Vec<Option<u32>>,
}

impl HitGrid {
    /// Create a new hit grid with the given dimensions.
    ///
    /// Uses saturating multiplication to prevent overflow for extremely large dimensions.
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width as usize).saturating_mul(height as usize);
        Self {
            width,
            height,
            cells: vec![None; size],
        }
    }

    /// Compute cell index with overflow protection.
    #[inline]
    fn cell_index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let row_offset = (y as usize).checked_mul(self.width as usize)?;
        let idx = row_offset.checked_add(x as usize)?;
        if idx < self.cells.len() {
            Some(idx)
        } else {
            None
        }
    }

    /// Clear all hit areas.
    pub fn clear(&mut self) {
        self.cells.fill(None);
    }

    /// Register a hit area.
    pub fn register(&mut self, x: u32, y: u32, width: u32, height: u32, id: u32) {
        for row in y..y.saturating_add(height).min(self.height) {
            for col in x..x.saturating_add(width).min(self.width) {
                if let Some(idx) = self.cell_index(col, row) {
                    self.cells[idx] = Some(id);
                }
            }
        }
    }

    /// Test which ID is at a position.
    #[must_use]
    pub fn test(&self, x: u32, y: u32) -> Option<u32> {
        self.cell_index(x, y).and_then(|idx| self.cells[idx])
    }

    /// Resize the grid, clearing all hit areas.
    ///
    /// Uses saturating multiplication to prevent overflow for extremely large dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let size = (width as usize).saturating_mul(height as usize);
        self.cells = vec![None; size];
    }

    /// Get dimensions.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Estimated byte size of the hit grid storage.
    #[must_use]
    pub fn byte_size(&self) -> usize {
        self.cells.len() * std::mem::size_of::<Option<u32>>()
    }
}

impl Default for HitGrid {
    fn default() -> Self {
        Self::new(80, 24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Basic Hit Grid Tests
    // ============================================

    #[test]
    fn test_hit_grid_new() {
        let grid = HitGrid::new(80, 24);
        assert_eq!(grid.size(), (80, 24));
    }

    #[test]
    fn test_hit_grid_default() {
        let grid = HitGrid::default();
        assert_eq!(grid.size(), (80, 24));
    }

    #[test]
    fn test_hit_grid_basic() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(10, 10, 20, 10, 42);

        assert_eq!(grid.test(15, 15), Some(42));
        assert_eq!(grid.test(29, 19), Some(42));
        assert_eq!(grid.test(30, 20), None);
        assert_eq!(grid.test(5, 5), None);
    }

    #[test]
    fn test_hit_grid_single_cell() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(50, 25, 1, 1, 100);

        assert_eq!(grid.test(50, 25), Some(100));
        assert_eq!(grid.test(49, 25), None);
        assert_eq!(grid.test(51, 25), None);
        assert_eq!(grid.test(50, 24), None);
        assert_eq!(grid.test(50, 26), None);
    }

    // ============================================
    // Overlap Tests
    // ============================================

    #[test]
    fn test_hit_grid_overlap() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 20, 20, 1);
        grid.register(10, 10, 20, 20, 2);

        // Later registration wins in overlap area
        assert_eq!(grid.test(5, 5), Some(1));
        assert_eq!(grid.test(15, 15), Some(2));
    }

    #[test]
    fn test_hit_grid_complete_overlap() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 50, 50, 1);
        grid.register(0, 0, 50, 50, 2);

        // Second registration completely overwrites
        assert_eq!(grid.test(0, 0), Some(2));
        assert_eq!(grid.test(25, 25), Some(2));
        assert_eq!(grid.test(49, 49), Some(2));
    }

    #[test]
    fn test_hit_grid_nested_regions() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 30, 30, 1); // Outer
        grid.register(10, 10, 10, 10, 2); // Inner

        assert_eq!(grid.test(5, 5), Some(1)); // Outer only
        assert_eq!(grid.test(15, 15), Some(2)); // Inner overwrites
        assert_eq!(grid.test(25, 25), Some(1)); // Outer only
    }

    #[test]
    fn test_hit_grid_multiple_non_overlapping() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 10, 10, 1);
        grid.register(20, 0, 10, 10, 2);
        grid.register(40, 0, 10, 10, 3);

        assert_eq!(grid.test(5, 5), Some(1));
        assert_eq!(grid.test(25, 5), Some(2));
        assert_eq!(grid.test(45, 5), Some(3));
        assert_eq!(grid.test(15, 5), None); // Gap
    }

    // ============================================
    // Clear Tests
    // ============================================

    #[test]
    fn test_hit_grid_clear() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 50, 50, 1);
        assert_eq!(grid.test(25, 25), Some(1));

        grid.clear();
        assert_eq!(grid.test(25, 25), None);
    }

    #[test]
    fn test_hit_grid_clear_then_register() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 50, 50, 1);
        grid.clear();
        grid.register(0, 0, 50, 50, 2);

        assert_eq!(grid.test(25, 25), Some(2));
    }

    // ============================================
    // Bounds Tests
    // ============================================

    #[test]
    fn test_hit_grid_bounds() {
        let grid = HitGrid::new(100, 50);
        assert_eq!(grid.test(100, 50), None);
        assert_eq!(grid.test(1000, 1000), None);
    }

    #[test]
    fn test_hit_grid_register_at_edge() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(99, 49, 1, 1, 1);

        assert_eq!(grid.test(99, 49), Some(1));
        assert_eq!(grid.test(100, 49), None); // Out of bounds
        assert_eq!(grid.test(99, 50), None); // Out of bounds
    }

    #[test]
    fn test_hit_grid_register_extends_beyond() {
        let mut grid = HitGrid::new(100, 50);
        // Register area that extends beyond grid
        grid.register(90, 40, 20, 20, 1);

        // Should only be registered within bounds
        assert_eq!(grid.test(95, 45), Some(1));
        assert_eq!(grid.test(99, 49), Some(1));
        assert_eq!(grid.test(100, 50), None);
    }

    #[test]
    fn test_hit_grid_register_completely_out_of_bounds() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(200, 200, 10, 10, 1);

        // Nothing should be registered
        assert_eq!(grid.test(200, 200), None);
        assert_eq!(grid.test(0, 0), None);
    }

    // ============================================
    // Resize Tests
    // ============================================

    #[test]
    fn test_hit_grid_resize_clears() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 50, 50, 1);
        assert_eq!(grid.test(25, 25), Some(1));

        grid.resize(80, 24);
        assert_eq!(grid.size(), (80, 24));
        assert_eq!(grid.test(25, 25), None); // Cleared
    }

    #[test]
    fn test_hit_grid_resize_larger() {
        let mut grid = HitGrid::new(10, 10);
        grid.resize(100, 100);

        assert_eq!(grid.size(), (100, 100));
        assert_eq!(grid.test(50, 50), None);

        grid.register(50, 50, 10, 10, 1);
        assert_eq!(grid.test(55, 55), Some(1));
    }

    #[test]
    fn test_hit_grid_resize_smaller() {
        let mut grid = HitGrid::new(100, 100);
        grid.resize(10, 10);

        assert_eq!(grid.size(), (10, 10));
        // Out of bounds after resize
        assert_eq!(grid.test(50, 50), None);
    }

    // ============================================
    // Edge Case Tests
    // ============================================

    #[test]
    fn test_hit_grid_zero_size_region() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(50, 25, 0, 0, 1);

        // Zero-size region should register nothing
        assert_eq!(grid.test(50, 25), None);
    }

    #[test]
    fn test_hit_grid_origin() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 5, 5, 1);

        assert_eq!(grid.test(0, 0), Some(1));
    }

    #[test]
    fn test_hit_grid_byte_size() {
        let grid = HitGrid::new(100, 50);
        let expected = 100 * 50 * std::mem::size_of::<Option<u32>>();
        assert_eq!(grid.byte_size(), expected);
    }

    #[test]
    fn test_hit_grid_many_widgets() {
        let mut grid = HitGrid::new(100, 100);

        // Register 100 small widgets
        for i in 0..10 {
            for j in 0..10 {
                let id = (i * 10 + j) as u32;
                grid.register(i * 10, j * 10, 8, 8, id);
            }
        }

        // Test a few
        assert_eq!(grid.test(5, 5), Some(0));
        assert_eq!(grid.test(15, 15), Some(11));
        assert_eq!(grid.test(95, 95), Some(99));
    }

    #[test]
    fn test_hit_grid_border_cells() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(10, 10, 20, 10, 1);

        // Test exact border cells
        assert_eq!(grid.test(10, 10), Some(1)); // Top-left
        assert_eq!(grid.test(29, 10), Some(1)); // Top-right
        assert_eq!(grid.test(10, 19), Some(1)); // Bottom-left
        assert_eq!(grid.test(29, 19), Some(1)); // Bottom-right

        // Just outside
        assert_eq!(grid.test(9, 10), None);
        assert_eq!(grid.test(30, 10), None);
        assert_eq!(grid.test(10, 9), None);
        assert_eq!(grid.test(10, 20), None);
    }

    // ============================================
    // Performance/Stress Tests
    // ============================================

    #[test]
    fn test_hit_grid_large_dimensions() {
        let grid = HitGrid::new(1000, 500);
        assert_eq!(grid.size(), (1000, 500));
        assert_eq!(grid.test(999, 499), None);
    }

    #[test]
    fn test_hit_grid_full_coverage() {
        let mut grid = HitGrid::new(10, 10);
        grid.register(0, 0, 10, 10, 42);

        // Every cell should have the ID
        for y in 0..10 {
            for x in 0..10 {
                assert_eq!(grid.test(x, y), Some(42), "Failed at ({x}, {y})");
            }
        }
    }

    // ============================================
    // Additional Tests per bd-1rxc Requirements
    // ============================================

    #[test]
    fn test_hit_grid_adjacent_regions() {
        // Test regions that touch but don't overlap
        let mut grid = HitGrid::new(100, 50);

        // Two adjacent regions horizontally
        grid.register(0, 0, 10, 10, 1);
        grid.register(10, 0, 10, 10, 2); // Starts where first ends

        // Both should be registered correctly
        assert_eq!(grid.test(9, 5), Some(1)); // Last cell of first region
        assert_eq!(grid.test(10, 5), Some(2)); // First cell of second region

        // Gap between them should not exist (they're adjacent)
        assert_eq!(grid.test(5, 5), Some(1));
        assert_eq!(grid.test(15, 5), Some(2));
    }

    #[test]
    fn test_hit_grid_adjacent_vertical() {
        let mut grid = HitGrid::new(100, 50);

        // Two adjacent regions vertically
        grid.register(0, 0, 10, 10, 1);
        grid.register(0, 10, 10, 10, 2); // Starts where first ends

        assert_eq!(grid.test(5, 9), Some(1)); // Last row of first
        assert_eq!(grid.test(5, 10), Some(2)); // First row of second
    }

    #[test]
    fn test_hit_grid_widget_ids_are_preserved() {
        // Test that arbitrary widget IDs are preserved correctly
        let mut grid = HitGrid::new(100, 50);

        // Use various ID values including edge cases
        grid.register(0, 0, 5, 5, 0); // ID 0
        grid.register(10, 0, 5, 5, 1); // ID 1
        grid.register(20, 0, 5, 5, u32::MAX); // Max ID
        grid.register(30, 0, 5, 5, 12345); // Arbitrary ID

        assert_eq!(grid.test(2, 2), Some(0));
        assert_eq!(grid.test(12, 2), Some(1));
        assert_eq!(grid.test(22, 2), Some(u32::MAX));
        assert_eq!(grid.test(32, 2), Some(12345));
    }

    #[test]
    fn test_hit_grid_max_coordinate_values() {
        // Test with maximum valid coordinates
        let grid = HitGrid::new(u32::MAX / 2, 2);
        assert_eq!(grid.size(), (u32::MAX / 2, 2));

        // Should not panic on large coordinate queries
        let result = grid.test(u32::MAX, u32::MAX);
        assert_eq!(result, None);
    }

    #[test]
    fn test_hit_grid_diagonal_layout() {
        // Test diagonal arrangement of widgets
        let mut grid = HitGrid::new(50, 50);

        for i in 0..5 {
            grid.register(i * 10, i * 10, 5, 5, i as u32);
        }

        // Test each diagonal region
        assert_eq!(grid.test(2, 2), Some(0));
        assert_eq!(grid.test(12, 12), Some(1));
        assert_eq!(grid.test(22, 22), Some(2));
        assert_eq!(grid.test(32, 32), Some(3));
        assert_eq!(grid.test(42, 42), Some(4));

        // Test gaps between diagonal regions
        assert_eq!(grid.test(7, 7), None);
        assert_eq!(grid.test(17, 17), None);
    }

    #[test]
    fn test_hit_grid_row_of_widgets() {
        // Test a horizontal row of widgets with gaps
        let mut grid = HitGrid::new(100, 20);

        for i in 0..5 {
            grid.register(i * 20, 5, 15, 10, i as u32);
        }

        // Test each widget
        for i in 0..5 {
            let x = i * 20 + 7;
            assert_eq!(grid.test(x as u32, 10), Some(i as u32));
        }

        // Test gaps
        for i in 0..5 {
            let gap_x = i * 20 + 17; // In the gap
            if gap_x < 100 {
                assert_eq!(
                    grid.test(gap_x as u32, 10),
                    None,
                    "Gap at x={gap_x} should be empty"
                );
            }
        }
    }

    #[test]
    fn test_hit_grid_column_of_widgets() {
        // Test a vertical column of widgets with gaps
        let mut grid = HitGrid::new(20, 100);

        for i in 0..5 {
            grid.register(5, i * 20, 10, 15, i as u32);
        }

        // Test each widget
        for i in 0..5 {
            let y = i * 20 + 7;
            assert_eq!(grid.test(10, y as u32), Some(i as u32));
        }

        // Test gaps
        for i in 0..5 {
            let gap_y = i * 20 + 17; // In the gap
            if gap_y < 100 {
                assert_eq!(
                    grid.test(10, gap_y as u32),
                    None,
                    "Gap at y={gap_y} should be empty"
                );
            }
        }
    }

    #[test]
    fn test_hit_grid_registration_order_matters() {
        // Verify that later registrations overwrite earlier ones
        let mut grid = HitGrid::new(100, 50);

        // Register same area multiple times with different IDs
        grid.register(10, 10, 20, 20, 100);
        grid.register(10, 10, 20, 20, 200);
        grid.register(10, 10, 20, 20, 300);

        // Final registration should win
        assert_eq!(grid.test(20, 20), Some(300));
    }

    #[test]
    fn test_hit_grid_partial_overlap_chain() {
        // Test chain of partially overlapping regions
        let mut grid = HitGrid::new(100, 50);

        // Each region overlaps with the previous one by half
        grid.register(0, 0, 20, 20, 1);
        grid.register(10, 0, 20, 20, 2);
        grid.register(20, 0, 20, 20, 3);

        // Unique areas
        assert_eq!(grid.test(5, 10), Some(1)); // Only in region 1
        assert_eq!(grid.test(35, 10), Some(3)); // Only in region 3

        // Overlap areas (later wins)
        assert_eq!(grid.test(15, 10), Some(2)); // Overlap 1&2, 2 wins
        assert_eq!(grid.test(25, 10), Some(3)); // Overlap 2&3, 3 wins
    }

    #[test]
    fn test_hit_grid_clone() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(10, 10, 20, 20, 42);

        let cloned = grid.clone();

        // Cloned grid should have same content
        assert_eq!(cloned.size(), grid.size());
        assert_eq!(cloned.test(15, 15), Some(42));

        // Modifications to original shouldn't affect clone
        grid.clear();
        assert_eq!(cloned.test(15, 15), Some(42));
        assert_eq!(grid.test(15, 15), None);
    }

    #[test]
    fn test_hit_grid_1x1_dimensions() {
        // Smallest possible grid
        let mut grid = HitGrid::new(1, 1);
        assert_eq!(grid.size(), (1, 1));

        grid.register(0, 0, 1, 1, 99);
        assert_eq!(grid.test(0, 0), Some(99));
        assert_eq!(grid.test(1, 0), None); // Out of bounds
        assert_eq!(grid.test(0, 1), None); // Out of bounds
    }

    #[test]
    fn test_hit_grid_only_width() {
        // Very wide but short grid (1 row)
        let mut grid = HitGrid::new(1000, 1);
        grid.register(500, 0, 100, 1, 1);

        assert_eq!(grid.test(550, 0), Some(1));
        assert_eq!(grid.test(400, 0), None);
        assert_eq!(grid.test(700, 0), None);
    }

    #[test]
    fn test_hit_grid_only_height() {
        // Very tall but narrow grid (1 column)
        let mut grid = HitGrid::new(1, 1000);
        grid.register(0, 500, 1, 100, 1);

        assert_eq!(grid.test(0, 550), Some(1));
        assert_eq!(grid.test(0, 400), None);
        assert_eq!(grid.test(0, 700), None);
    }
}
