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
        let (width, height) = old.size();
        let mut changed_cells = Vec::new();

        for y in 0..height {
            for x in 0..width {
                if old.get(x, y) != new.get(x, y) {
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
                row_end = x;
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
    fn test_buffer_diff_empty() {
        let a = OptimizedBuffer::new(10, 10);
        let b = OptimizedBuffer::new(10, 10);
        let diff = BufferDiff::compute(&a, &b);

        assert!(diff.is_empty());
        assert_eq!(diff.change_count, 0);
    }

    #[test]
    fn test_buffer_diff_changes() {
        use crate::cell::Cell;
        use crate::color::Rgba;

        let a = OptimizedBuffer::new(10, 10);
        let mut b = OptimizedBuffer::new(10, 10);
        b.set(5, 5, Cell::clear(Rgba::RED));

        let diff = BufferDiff::compute(&a, &b);

        assert!(!diff.is_empty());
        assert_eq!(diff.change_count, 1);
        assert!(diff.changed_cells.contains(&(5, 5)));
    }
}
