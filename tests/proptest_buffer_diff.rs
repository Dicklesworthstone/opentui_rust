//! Property-based tests for buffer diff algorithm (bd-1m90).
//!
//! Uses proptest to verify invariants of `BufferDiff::compute`, `Cell::bits_eq`,
//! and dirty region merging.

use opentui::buffer::OptimizedBuffer;
use opentui::cell::Cell;
use opentui::color::Rgba;
use opentui::renderer::BufferDiff;
use opentui::style::Style;
use opentui_rust as opentui;
use proptest::prelude::*;

// ============================================================================
// Strategies
// ============================================================================

/// Generate an RGBA color with components in [0, 1].
fn rgba_strategy() -> impl Strategy<Value = Rgba> {
    (0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0, 0.0f32..=1.0)
        .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a))
}

/// Generate a Cell with random char and colors.
fn cell_strategy() -> impl Strategy<Value = Cell> {
    (
        prop::char::range('A', 'z'),
        rgba_strategy(),
        rgba_strategy(),
    )
        .prop_map(|(ch, fg, bg)| {
            let style = Style {
                fg: Some(fg),
                bg: Some(bg),
                ..Style::NONE
            };
            Cell::new(ch, style)
        })
}

/// Generate buffer dimensions (small for performance).
fn dim_strategy() -> impl Strategy<Value = (u32, u32)> {
    (1u32..=20, 1u32..=20)
}

/// Generate a pair of buffers with same dimensions, second one with random changes.
fn buffer_pair_strategy() -> impl Strategy<Value = (OptimizedBuffer, OptimizedBuffer, u32, u32)> {
    dim_strategy().prop_flat_map(|(w, h)| {
        let total = (w * h) as usize;
        let changes = prop::collection::vec((0u32..w, 0u32..h, cell_strategy()), 0..=total);
        changes.prop_map(move |mods| {
            let a = OptimizedBuffer::new(w, h);
            let mut b = OptimizedBuffer::new(w, h);
            for (x, y, cell) in mods {
                b.set(x, y, cell);
            }
            (a, b, w, h)
        })
    })
}

// ============================================================================
// BufferDiff Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Identical buffers produce an empty diff.
    #[test]
    fn identical_buffers_empty_diff((w, h) in dim_strategy()) {
        let a = OptimizedBuffer::new(w, h);
        let b = OptimizedBuffer::new(w, h);
        let diff = BufferDiff::compute(&a, &b);
        prop_assert_eq!(diff.change_count, 0);
        prop_assert!(diff.is_empty());
        prop_assert!(diff.dirty_regions.is_empty());
    }

    /// change_count equals changed_cells.len().
    #[test]
    fn change_count_consistent((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        prop_assert_eq!(diff.change_count, diff.changed_cells.len());
    }

    /// change_count is at most width * height.
    #[test]
    fn change_count_bounded((a, b, w, h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        let total = (w as usize) * (h as usize);
        prop_assert!(diff.change_count <= total,
            "change_count {} exceeds total cells {}", diff.change_count, total);
    }

    /// All changed_cells are within buffer bounds.
    #[test]
    fn changed_cells_in_bounds((a, b, w, h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        for &(x, y) in &diff.changed_cells {
            prop_assert!(x < w, "changed cell x={} out of bounds (w={})", x, w);
            prop_assert!(y < h, "changed cell y={} out of bounds (h={})", y, h);
        }
    }

    /// No duplicate entries in changed_cells.
    #[test]
    fn no_duplicate_changed_cells((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        let mut sorted = diff.changed_cells.clone();
        sorted.sort_unstable();
        sorted.dedup();
        prop_assert_eq!(sorted.len(), diff.changed_cells.len(),
            "changed_cells should have no duplicates");
    }

    /// changed_cells are sorted by (y, x) order.
    #[test]
    fn changed_cells_sorted((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        for i in 1..diff.changed_cells.len() {
            let (x1, y1) = diff.changed_cells[i - 1];
            let (x2, y2) = diff.changed_cells[i];
            prop_assert!(
                (y1, x1) < (y2, x2),
                "changed_cells not sorted: ({},{}) before ({},{})", x1, y1, x2, y2
            );
        }
    }

    /// is_empty is consistent with change_count.
    #[test]
    fn is_empty_consistent((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        prop_assert_eq!(diff.is_empty(), diff.change_count == 0);
    }

    /// Dirty regions are non-empty when there are changes.
    #[test]
    fn dirty_regions_nonempty_when_changes((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        if diff.change_count > 0 {
            prop_assert!(!diff.dirty_regions.is_empty(),
                "dirty_regions should be non-empty when there are changes");
        }
    }

    /// Dirty regions have positive dimensions.
    #[test]
    fn dirty_regions_positive_dims((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        for region in &diff.dirty_regions {
            prop_assert!(region.width > 0, "dirty region width should be > 0");
            prop_assert!(region.height > 0, "dirty region height should be > 0");
        }
    }

    /// Dirty regions are within buffer bounds.
    #[test]
    fn dirty_regions_in_bounds((a, b, w, h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        for region in &diff.dirty_regions {
            prop_assert!(region.x + region.width <= w,
                "dirty region x+w={}+{}={} exceeds width {}",
                region.x, region.width, region.x + region.width, w);
            prop_assert!(region.y + region.height <= h,
                "dirty region y+h={}+{}={} exceeds height {}",
                region.y, region.height, region.y + region.height, h);
        }
    }

    /// Total area of dirty regions >= change_count (regions may include unchanged cells).
    #[test]
    fn dirty_region_area_covers_changes((a, b, _w, _h) in buffer_pair_strategy()) {
        let diff = BufferDiff::compute(&a, &b);
        let total_area: u64 = diff.dirty_regions.iter()
            .map(|r| u64::from(r.width) * u64::from(r.height))
            .sum();
        prop_assert!(total_area >= diff.change_count as u64,
            "region area {} < change_count {}", total_area, diff.change_count);
    }
}

// ============================================================================
// Cell bits_eq Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// bits_eq is reflexive: a cell is always equal to itself.
    #[test]
    fn bits_eq_reflexive(cell in cell_strategy()) {
        prop_assert!(cell.bits_eq(&cell), "bits_eq should be reflexive");
    }

    /// bits_eq is symmetric: a.bits_eq(b) == b.bits_eq(a).
    #[test]
    fn bits_eq_symmetric(a in cell_strategy(), b in cell_strategy()) {
        prop_assert_eq!(a.bits_eq(&b), b.bits_eq(&a),
            "bits_eq should be symmetric");
    }

    /// Cells with different content are not bits_eq.
    #[test]
    fn bits_eq_different_content(
        c1 in prop::char::range('A', 'M'),
        c2 in prop::char::range('N', 'Z'),
        fg in rgba_strategy(),
        bg in rgba_strategy(),
    ) {
        let style = Style { fg: Some(fg), bg: Some(bg), ..Style::NONE };
        let cell1 = Cell::new(c1, style);
        let cell2 = Cell::new(c2, style);
        prop_assert!(!cell1.bits_eq(&cell2),
            "cells with different content should not be bits_eq");
    }
}

// ============================================================================
// Rgba bits_eq Properties
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Rgba bits_eq is reflexive.
    #[test]
    fn rgba_bits_eq_reflexive(color in rgba_strategy()) {
        prop_assert!(color.bits_eq(color));
    }

    /// Rgba bits_eq is symmetric.
    #[test]
    fn rgba_bits_eq_symmetric(a in rgba_strategy(), b in rgba_strategy()) {
        prop_assert_eq!(a.bits_eq(b), b.bits_eq(a));
    }

    /// Rgba to_bits is deterministic.
    #[test]
    fn rgba_to_bits_deterministic(color in rgba_strategy()) {
        let b1 = color.to_bits();
        let b2 = color.to_bits();
        prop_assert_eq!(b1, b2);
    }

    /// Two Rgba with same bits are bits_eq.
    #[test]
    fn rgba_same_bits_are_eq(r in 0.0f32..=1.0, g in 0.0f32..=1.0, b in 0.0f32..=1.0, a in 0.0f32..=1.0) {
        let c1 = Rgba::new(r, g, b, a);
        let c2 = Rgba::new(r, g, b, a);
        prop_assert!(c1.bits_eq(c2),
            "identical Rgba values should be bits_eq");
    }
}

// Note: DirtyRegion merge properties are tested in src/renderer/diff.rs
// (DirtyRegion is not re-exported from the public API)
