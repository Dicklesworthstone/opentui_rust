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
    #[must_use]
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells: vec![None; (width * height) as usize],
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
                let idx = (row * self.width + col) as usize;
                if idx < self.cells.len() {
                    self.cells[idx] = Some(id);
                }
            }
        }
    }

    /// Test which ID is at a position.
    #[must_use]
    pub fn test(&self, x: u32, y: u32) -> Option<u32> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let idx = (y * self.width + x) as usize;
        self.cells.get(idx).copied().flatten()
    }

    /// Resize the grid, clearing all hit areas.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.cells = vec![None; (width * height) as usize];
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
    fn test_hit_grid_overlap() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 20, 20, 1);
        grid.register(10, 10, 20, 20, 2);

        // Later registration wins in overlap area
        assert_eq!(grid.test(5, 5), Some(1));
        assert_eq!(grid.test(15, 15), Some(2));
    }

    #[test]
    fn test_hit_grid_clear() {
        let mut grid = HitGrid::new(100, 50);
        grid.register(0, 0, 50, 50, 1);
        assert_eq!(grid.test(25, 25), Some(1));

        grid.clear();
        assert_eq!(grid.test(25, 25), None);
    }

    #[test]
    fn test_hit_grid_bounds() {
        let grid = HitGrid::new(100, 50);
        assert_eq!(grid.test(100, 50), None);
        assert_eq!(grid.test(1000, 1000), None);
    }
}
