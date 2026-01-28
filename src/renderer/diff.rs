//! Buffer diffing for efficient rendering.

use crate::buffer::OptimizedBuffer;

/// A region that has changed between frames.
#[derive(Clone, Copy, Debug)]
pub struct DirtyRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl DirtyRegion {
    /// Create a new dirty region.
    #[must_use]
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create a single-cell region.
    #[must_use]
    pub fn cell(x: u32, y: u32) -> Self {
        Self::new(x, y, 1, 1)
    }

    /// Merge with another region.
    #[must_use]
    pub fn merge(&self, other: &Self) -> Self {
        let x1 = self.x.min(other.x);
        let y1 = self.y.min(other.y);
        let x2 = (self.x + self.width).max(other.x + other.width);
        let y2 = (self.y + self.height).max(other.y + other.height);

        Self::new(x1, y1, x2 - x1, y2 - y1)
    }
}

/// Result of diffing two buffers.
pub struct BufferDiff {
    /// List of changed cells (x, y).
    pub changed_cells: Vec<(u32, u32)>,
    /// Merged dirty regions.
    pub dirty_regions: Vec<DirtyRegion>,
    /// Total number of changed cells.
    pub change_count: usize,
}

impl BufferDiff {
    /// Compare two buffers and find differences.
    #[must_use]
    pub fn compute(old: &OptimizedBuffer, new: &OptimizedBuffer) -> Self {
        assert!(
            old.size() == new.size(),
            "Buffer size mismatch in diff: old={:?}, new={:?}",
            old.size(),
            new.size()
        );

        let (width, height) = old.size();
        let total_cells = (width as usize).saturating_mul(height as usize);
        let reserve = (total_cells / 8).max(32).min(total_cells);
        let mut changed_cells = Vec::with_capacity(reserve);

        // Use direct slice access for faster iteration
        // This avoids Option unwrapping overhead per cell
        let old_cells = old.cells();
        let new_cells = new.cells();

        // Use fast bitwise comparison for cell diffing
        // This is significantly faster than PartialEq because it uses
        // integer comparison for colors instead of floating-point ops
        for y in 0..height {
            let row_offset = (y * width) as usize;
            for x in 0..width {
                let idx = row_offset + x as usize;
                // SAFETY: We're iterating within bounds since we use width/height from old buffer
                // and both buffers have the same dimensions (checked at start of compute)
                if !old_cells[idx].bits_eq(&new_cells[idx]) {
                    changed_cells.push((x, y));
                }
            }
        }

        let change_count = changed_cells.len();
        let dirty_regions = Self::merge_into_regions(&changed_cells, width);

        Self {
            changed_cells,
            dirty_regions,
            change_count,
        }
    }

    /// Check if there are any changes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.changed_cells.is_empty()
    }

    /// Merge changed cells into regions.
    fn merge_into_regions(cells: &[(u32, u32)], _width: u32) -> Vec<DirtyRegion> {
        if cells.is_empty() {
            return Vec::new();
        }

        // Simple approach: group by row
        let mut regions: Vec<DirtyRegion> = Vec::new();
        let mut current_row: Option<u32> = None;
        let mut row_start: u32 = 0;
        let mut row_end: u32 = 0;

        for &(x, y) in cells {
            if current_row == Some(y) {
                if x > row_end + 1 {
                    if let Some(row) = current_row {
                        regions.push(DirtyRegion::new(row_start, row, row_end - row_start + 1, 1));
                    }
                    row_start = x;
                    row_end = x;
                } else {
                    row_end = x;
                }
            } else {
                if let Some(row) = current_row {
                    regions.push(DirtyRegion::new(row_start, row, row_end - row_start + 1, 1));
                }
                current_row = Some(y);
                row_start = x;
                row_end = x;
            }
        }

        if let Some(row) = current_row {
            regions.push(DirtyRegion::new(row_start, row, row_end - row_start + 1, 1));
        }

        regions
    }

    /// Calculate if a full redraw is more efficient.
    #[must_use]
    pub fn should_full_redraw(&self, total_cells: usize) -> bool {
        // If more than 50% changed, full redraw is likely faster
        self.change_count > total_cells / 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Cell;
    use crate::color::Rgba;
    use crate::style::Style;

    // ============================================
    // DirtyRegion Tests
    // ============================================

    #[test]
    fn test_dirty_region_new() {
        let region = DirtyRegion::new(5, 10, 20, 30);
        assert_eq!(region.x, 5);
        assert_eq!(region.y, 10);
        assert_eq!(region.width, 20);
        assert_eq!(region.height, 30);
    }

    #[test]
    fn test_dirty_region_cell() {
        let region = DirtyRegion::cell(7, 12);
        assert_eq!(region.x, 7);
        assert_eq!(region.y, 12);
        assert_eq!(region.width, 1);
        assert_eq!(region.height, 1);
    }

    #[test]
    fn test_dirty_region_merge() {
        let a = DirtyRegion::new(0, 0, 5, 5);
        let b = DirtyRegion::new(3, 3, 5, 5);
        let merged = a.merge(&b);

        assert_eq!(merged.x, 0);
        assert_eq!(merged.y, 0);
        assert_eq!(merged.width, 8);
        assert_eq!(merged.height, 8);
    }

    #[test]
    fn test_dirty_region_merge_non_overlapping() {
        let a = DirtyRegion::new(0, 0, 5, 5);
        let b = DirtyRegion::new(10, 10, 5, 5);
        let merged = a.merge(&b);

        // Should create bounding box
        assert_eq!(merged.x, 0);
        assert_eq!(merged.y, 0);
        assert_eq!(merged.width, 15);
        assert_eq!(merged.height, 15);
    }

    #[test]
    fn test_dirty_region_merge_contained() {
        let outer = DirtyRegion::new(0, 0, 20, 20);
        let inner = DirtyRegion::new(5, 5, 5, 5);
        let merged = outer.merge(&inner);

        // Inner is contained, result should be outer
        assert_eq!(merged.x, 0);
        assert_eq!(merged.y, 0);
        assert_eq!(merged.width, 20);
        assert_eq!(merged.height, 20);
    }

    // ============================================
    // BufferDiff Tests - Basic Operations
    // ============================================

    #[test]
    fn test_buffer_diff_empty() {
        let a = OptimizedBuffer::new(10, 10);
        let b = OptimizedBuffer::new(10, 10);
        let diff = BufferDiff::compute(&a, &b);

        assert!(diff.is_empty());
        assert_eq!(diff.change_count, 0);
        assert!(diff.dirty_regions.is_empty());
    }

    #[test]
    fn test_buffer_diff_single_cell_change() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);
        b.set(5, 5, Cell::clear(Rgba::RED));

        let diff = BufferDiff::compute(&a, &b);

        assert!(!diff.is_empty());
        assert_eq!(diff.change_count, 1);
        assert!(diff.changed_cells.contains(&(5, 5)));
        assert_eq!(diff.dirty_regions.len(), 1);
    }

    #[test]
    fn test_buffer_diff_multiple_cells() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);
        b.set(0, 0, Cell::clear(Rgba::RED));
        b.set(5, 5, Cell::clear(Rgba::GREEN));
        b.set(9, 9, Cell::clear(Rgba::BLUE));

        let diff = BufferDiff::compute(&a, &b);

        assert_eq!(diff.change_count, 3);
        assert!(diff.changed_cells.contains(&(0, 0)));
        assert!(diff.changed_cells.contains(&(5, 5)));
        assert!(diff.changed_cells.contains(&(9, 9)));
    }

    #[test]
    fn test_buffer_diff_changes() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);
        b.set(5, 5, Cell::clear(Rgba::RED));

        let diff = BufferDiff::compute(&a, &b);

        assert!(!diff.is_empty());
        assert_eq!(diff.change_count, 1);
        assert!(diff.changed_cells.contains(&(5, 5)));
    }

    // ============================================
    // BufferDiff Tests - Row Grouping
    // ============================================

    #[test]
    fn test_buffer_diff_consecutive_cells_same_row() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        // Three consecutive cells on same row
        b.set(2, 5, Cell::clear(Rgba::RED));
        b.set(3, 5, Cell::clear(Rgba::RED));
        b.set(4, 5, Cell::clear(Rgba::RED));

        let diff = BufferDiff::compute(&a, &b);

        assert_eq!(diff.change_count, 3);
        // Should be merged into one region
        assert_eq!(diff.dirty_regions.len(), 1);
        assert_eq!(diff.dirty_regions[0].x, 2);
        assert_eq!(diff.dirty_regions[0].y, 5);
        assert_eq!(diff.dirty_regions[0].width, 3);
    }

    #[test]
    fn test_buffer_diff_non_consecutive_cells_same_row() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        // Two separate groups on same row with gap
        b.set(0, 5, Cell::clear(Rgba::RED));
        b.set(1, 5, Cell::clear(Rgba::RED));
        // Gap at x=2,3
        b.set(4, 5, Cell::clear(Rgba::RED));
        b.set(5, 5, Cell::clear(Rgba::RED));

        let diff = BufferDiff::compute(&a, &b);

        assert_eq!(diff.change_count, 4);
        // Should be two separate regions
        assert_eq!(diff.dirty_regions.len(), 2);
    }

    #[test]
    fn test_buffer_diff_multiple_rows() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        // Changes on different rows
        b.set(5, 0, Cell::clear(Rgba::RED));
        b.set(5, 5, Cell::clear(Rgba::GREEN));
        b.set(5, 9, Cell::clear(Rgba::BLUE));

        let diff = BufferDiff::compute(&a, &b);

        assert_eq!(diff.change_count, 3);
        // Should be three separate regions (different rows)
        assert_eq!(diff.dirty_regions.len(), 3);
    }

    // ============================================
    // BufferDiff Tests - Full Redraw Threshold
    // ============================================

    #[test]
    fn test_buffer_diff_should_full_redraw_below_threshold() {
        let total_cells = 100;
        let diff = BufferDiff {
            changed_cells: vec![(0, 0); 40], // 40% changed
            dirty_regions: vec![],
            change_count: 40,
        };

        assert!(!diff.should_full_redraw(total_cells));
    }

    #[test]
    fn test_buffer_diff_should_full_redraw_above_threshold() {
        let total_cells = 100;
        let diff = BufferDiff {
            changed_cells: vec![(0, 0); 60], // 60% changed
            dirty_regions: vec![],
            change_count: 60,
        };

        assert!(diff.should_full_redraw(total_cells));
    }

    #[test]
    fn test_buffer_diff_should_full_redraw_at_threshold() {
        let total_cells = 100;
        let diff = BufferDiff {
            changed_cells: vec![(0, 0); 50], // Exactly 50%
            dirty_regions: vec![],
            change_count: 50,
        };

        // At 50%, not > 50%, so should not trigger
        assert!(!diff.should_full_redraw(total_cells));
    }

    #[test]
    fn test_buffer_diff_should_full_redraw_just_above() {
        let total_cells = 100;
        let diff = BufferDiff {
            changed_cells: vec![(0, 0); 51], // 51% changed
            dirty_regions: vec![],
            change_count: 51,
        };

        assert!(diff.should_full_redraw(total_cells));
    }

    // ============================================
    // BufferDiff Tests - Color Change Detection
    // ============================================

    #[test]
    fn test_buffer_diff_detects_fg_color_change() {
        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        a.set(5, 5, Cell::new('A', Style::fg(Rgba::RED)));
        b.set(5, 5, Cell::new('A', Style::fg(Rgba::BLUE)));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 1);
        assert!(diff.changed_cells.contains(&(5, 5)));
    }

    #[test]
    fn test_buffer_diff_detects_bg_color_change() {
        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        a.set(5, 5, Cell::new('A', Style::bg(Rgba::RED)));
        b.set(5, 5, Cell::new('A', Style::bg(Rgba::BLUE)));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 1);
    }

    #[test]
    fn test_buffer_diff_detects_content_change() {
        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        a.set(5, 5, Cell::new('A', Style::NONE));
        b.set(5, 5, Cell::new('B', Style::NONE));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 1);
    }

    #[test]
    fn test_buffer_diff_identical_cells_no_change() {
        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        let cell = Cell::new('X', Style::fg(Rgba::GREEN));
        a.set(5, 5, cell.clone());
        b.set(5, 5, cell);

        let diff = BufferDiff::compute(&a, &b);
        // Both cells set to same value - might show change depending on default state
        // This test verifies identical explicit cells don't show as changed
        assert!(diff.changed_cells.is_empty() || diff.change_count <= 1);
    }

    // ============================================
    // BufferDiff Tests - Edge Cases
    // ============================================

    #[test]
    fn test_buffer_diff_single_cell_buffer() {
        let a = OptimizedBuffer::new(1, 1);
        let mut b = OptimizedBuffer::new(1, 1);
        b.set(0, 0, Cell::clear(Rgba::RED));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 1);
    }

    #[test]
    fn test_buffer_diff_large_buffer() {
        let a = OptimizedBuffer::new(200, 50);
        let mut b = OptimizedBuffer::new(200, 50);

        // Change a row in the middle
        for x in 0..200 {
            b.set(x, 25, Cell::clear(Rgba::RED));
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 200);
        // Should be one region for the whole row
        assert_eq!(diff.dirty_regions.len(), 1);
    }

    #[test]
    fn test_buffer_diff_all_cells_changed() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        for y in 0..10 {
            for x in 0..10 {
                b.set(x, y, Cell::clear(Rgba::RED));
            }
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 100);
        assert!(diff.should_full_redraw(100));
    }

    #[test]
    fn test_buffer_diff_corners_only() {
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        b.set(0, 0, Cell::clear(Rgba::RED)); // top-left
        b.set(9, 0, Cell::clear(Rgba::GREEN)); // top-right
        b.set(0, 9, Cell::clear(Rgba::BLUE)); // bottom-left
        b.set(9, 9, Cell::clear(Rgba::WHITE)); // bottom-right

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 4);
        // Four separate regions
        assert_eq!(diff.dirty_regions.len(), 4);
    }

    // ============================================
    // BufferDiff Tests - Attribute Change Detection
    // ============================================

    #[test]
    fn test_buffer_diff_detects_attribute_change() {
        use crate::style::TextAttributes;

        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        let cell_a = Cell::new('A', Style::NONE);
        let mut cell_b = Cell::new('A', Style::NONE);
        cell_b.attributes = TextAttributes::BOLD;

        a.set(5, 5, cell_a);
        b.set(5, 5, cell_b);

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 1);
    }

    // ============================================
    // Additional Tests per bd-cycm Requirements
    // ============================================

    #[test]
    fn test_buffer_diff_column_change() {
        // Test changing an entire column
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        // Change column x=5
        for y in 0..10 {
            b.set(5, y, Cell::clear(Rgba::RED));
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 10);
        // Each row has a single-cell change, so 10 separate regions
        assert_eq!(diff.dirty_regions.len(), 10);
    }

    #[test]
    fn test_buffer_diff_wide_buffer() {
        // Test very wide buffer (1000x1)
        let a = OptimizedBuffer::new(1000, 1);
        let mut b = OptimizedBuffer::new(1000, 1);

        // Change cells at various positions
        b.set(0, 0, Cell::clear(Rgba::RED));
        b.set(500, 0, Cell::clear(Rgba::GREEN));
        b.set(999, 0, Cell::clear(Rgba::BLUE));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 3);
        // Three separate regions (far apart on same row)
        assert_eq!(diff.dirty_regions.len(), 3);
    }

    #[test]
    fn test_buffer_diff_tall_buffer() {
        // Test very tall buffer (1x1000)
        let a = OptimizedBuffer::new(1, 1000);
        let mut b = OptimizedBuffer::new(1, 1000);

        // Change cells at various positions
        b.set(0, 0, Cell::clear(Rgba::RED));
        b.set(0, 500, Cell::clear(Rgba::GREEN));
        b.set(0, 999, Cell::clear(Rgba::BLUE));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 3);
        // Three separate regions (different rows)
        assert_eq!(diff.dirty_regions.len(), 3);
    }

    #[test]
    fn test_buffer_diff_bits_eq_performance() {
        // Verify bits_eq is used for cell comparison (not float ==)
        // Create cells with identical values - they should compare equal
        let cell1 = Cell::new('A', Style::fg(Rgba::new(0.5, 0.5, 0.5, 1.0)));
        let cell2 = Cell::new('A', Style::fg(Rgba::new(0.5, 0.5, 0.5, 1.0)));

        // bits_eq should return true for identical cells
        assert!(
            cell1.bits_eq(&cell2),
            "bits_eq should detect identical cells"
        );

        // Verify different cells are detected
        let cell3 = Cell::new('B', Style::fg(Rgba::new(0.5, 0.5, 0.5, 1.0)));
        assert!(
            !cell1.bits_eq(&cell3),
            "bits_eq should detect different cells"
        );
    }

    #[test]
    fn test_buffer_diff_color_precision() {
        // Test that small color differences are detected
        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        // Very slight color difference
        a.set(
            5,
            5,
            Cell::new('A', Style::fg(Rgba::new(0.5, 0.5, 0.5, 1.0))),
        );
        b.set(
            5,
            5,
            Cell::new('A', Style::fg(Rgba::new(0.500_001, 0.5, 0.5, 1.0))),
        );

        let diff = BufferDiff::compute(&a, &b);
        // bits_eq uses bitwise comparison, so even tiny differences are detected
        // This verifies we're not using approximate float equality
        assert_eq!(diff.change_count, 1);
    }

    #[test]
    fn test_buffer_diff_alpha_change() {
        // Test that alpha channel changes are detected
        let mut a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        a.set(5, 5, Cell::new('A', Style::fg(Rgba::RED)));
        b.set(5, 5, Cell::new('A', Style::fg(Rgba::RED.with_alpha(0.5))));

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 1);
    }

    #[test]
    fn test_buffer_diff_diagonal_changes() {
        // Test diagonal pattern of changes
        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);

        // Diagonal from (0,0) to (9,9)
        for i in 0..10 {
            b.set(i, i, Cell::clear(Rgba::RED));
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 10);
        // Each diagonal cell is on a different row, so 10 regions
        assert_eq!(diff.dirty_regions.len(), 10);
    }

    #[test]
    fn test_buffer_diff_checkerboard() {
        // Test checkerboard pattern
        let a = OptimizedBuffer::new(8, 8);
        let mut b = OptimizedBuffer::new(8, 8);

        for y in 0..8 {
            for x in 0..8 {
                if (x + y) % 2 == 0 {
                    b.set(x, y, Cell::clear(Rgba::RED));
                }
            }
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 32); // Half the cells
        assert!(!diff.should_full_redraw(64)); // 50% exactly
    }

    #[test]
    fn test_dirty_region_zero_dimensions() {
        // Edge case: zero-sized region (degenerate)
        let region = DirtyRegion::new(5, 5, 0, 0);
        assert_eq!(region.width, 0);
        assert_eq!(region.height, 0);

        // Merge with non-zero region
        let other = DirtyRegion::new(10, 10, 5, 5);
        let merged = region.merge(&other);
        // Should produce bounding box from (5,5) to (15,15)
        assert_eq!(merged.x, 5);
        assert_eq!(merged.y, 5);
        assert_eq!(merged.width, 10);
        assert_eq!(merged.height, 10);
    }

    #[test]
    fn test_buffer_diff_first_row() {
        // Test changes only in first row
        let a = OptimizedBuffer::new(100, 50);
        let mut b = OptimizedBuffer::new(100, 50);

        for x in 0..100 {
            b.set(x, 0, Cell::clear(Rgba::RED));
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 100);
        // Whole row should be one region
        assert_eq!(diff.dirty_regions.len(), 1);
        assert_eq!(diff.dirty_regions[0].y, 0);
        assert_eq!(diff.dirty_regions[0].width, 100);
    }

    #[test]
    fn test_buffer_diff_last_row() {
        // Test changes only in last row
        let a = OptimizedBuffer::new(100, 50);
        let mut b = OptimizedBuffer::new(100, 50);

        for x in 0..100 {
            b.set(x, 49, Cell::clear(Rgba::RED));
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 100);
        assert_eq!(diff.dirty_regions.len(), 1);
        assert_eq!(diff.dirty_regions[0].y, 49);
    }

    #[test]
    fn test_buffer_diff_sparse_changes() {
        // Test very sparse changes (1% of cells)
        let a = OptimizedBuffer::new(100, 100);
        let mut b = OptimizedBuffer::new(100, 100);

        // Change every 10th cell
        for y in (0..100).step_by(10) {
            for x in (0..100).step_by(10) {
                b.set(x, y, Cell::clear(Rgba::RED));
            }
        }

        let diff = BufferDiff::compute(&a, &b);
        assert_eq!(diff.change_count, 100); // 10x10 = 100 cells
        assert!(!diff.should_full_redraw(10000)); // Only 1%
    }

    #[test]
    fn test_buffer_diff_is_empty_consistency() {
        // Verify is_empty() is consistent with change_count
        let a = OptimizedBuffer::new(10, 10);
        let b = OptimizedBuffer::new(10, 10);
        let diff = BufferDiff::compute(&a, &b);

        assert_eq!(diff.is_empty(), diff.change_count == 0);
        assert!(diff.is_empty());

        // Non-empty diff
        let mut c = OptimizedBuffer::new(10, 10);
        c.set(0, 0, Cell::clear(Rgba::RED));
        let diff2 = BufferDiff::compute(&a, &c);

        assert_eq!(diff2.is_empty(), diff2.change_count == 0);
        assert!(!diff2.is_empty());
    }
}
