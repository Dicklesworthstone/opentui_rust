#![allow(clippy::uninlined_format_args)] // Clarity over style in test code

#[cfg(test)]
mod tests {
    use opentui::buffer::{BoxOptions, BoxStyle, OptimizedBuffer};
    use opentui::cell::CellContent;
    use opentui::grapheme_pool::GraphemePool;

    #[test]
    fn test_box_title_emoji_placeholder_problem() {
        let mut buffer = OptimizedBuffer::new(20, 5);
        let options = BoxOptions {
            title: Some("Title 👨‍👩‍👧".to_string()),
            ..BoxOptions::new(BoxStyle::default())
        };

        // Use standard draw_box_with_options (no pool)
        buffer.draw_box_with_options(0, 0, 15, 5, options);

        // Find the emoji cell. Title starts at x=2. "Title " is 6 chars.
        // T(2), i(3), t(4), l(5), e(6),  (7), emoji(8)
        let cell = buffer.get(8, 0).unwrap();

        if let CellContent::Grapheme(id) = cell.content {
            // Expect placeholder ID (0) because we didn't use a pool
            assert_eq!(
                id.pool_id(),
                0,
                "Expected placeholder ID 0 from non-pool drawing"
            );
            assert_eq!(id.width(), 2, "Expected width 2");
        } else {
            panic!("Expected grapheme content, got {:?}", cell.content);
        }
    }

    #[test]
    fn test_overwrite_no_longer_leaks() {
        let mut buffer = OptimizedBuffer::new(10, 10);
        let mut pool = GraphemePool::new();

        // 1. Alloc grapheme
        let id = pool.alloc("👨‍👩‍👧");
        let initial_refcount = pool.refcount(id);
        assert_eq!(initial_refcount, 1);

        // 2. Draw it to buffer with pool
        // set_with_pool DECREMENTS the OLD content. It does NOT increment the NEW content.
        // The caller is responsible for ensuring the new content has a valid refcount.
        // When we called pool.alloc(), we got refcount 1. So we are "giving" that refcount to the buffer.

        let cell = opentui::cell::Cell {
            content: CellContent::Grapheme(id),
            fg: opentui::color::Rgba::WHITE,
            bg: opentui::color::Rgba::TRANSPARENT,
            attributes: opentui::style::TextAttributes::empty(),
        };

        buffer.set_with_pool(&mut pool, 0, 0, cell);

        // Refcount should still be 1 (held by buffer now)
        assert_eq!(pool.refcount(id), 1);

        // 3. Overwrite with set() (no pool)
        // The buffer now tracks the orphaned grapheme ID
        let clear_cell = opentui::cell::Cell::clear(opentui::color::Rgba::BLACK);
        buffer.set(0, 0, clear_cell);

        // Buffer now has clear_cell, but orphaned grapheme is tracked internally.
        // Refcount is still 1 because we haven't called a pool-aware method yet.
        assert_eq!(
            pool.refcount(id),
            1,
            "Refcount should still be 1 before drain"
        );

        // 4. Clear buffer with pool - this drains orphaned graphemes
        buffer.clear_with_pool(&mut pool, opentui::color::Rgba::BLACK);

        // The orphaned grapheme has been released!
        assert_eq!(
            pool.refcount(id),
            0,
            "Refcount should be 0 after clear_with_pool drains orphans"
        );
    }

    /// Regression test for cursor drift when continuation cells are skipped.
    ///
    /// BUG: In `present_diff()`, when iterating over dirty regions and skipping
    /// continuation cells, the cursor position would not advance properly,
    /// causing subsequent characters to be written at wrong positions.
    ///
    /// SYMPTOM: Text became garbled as log lines scrolled ("`HashMap`" -> "skseshap").
    #[test]
    fn test_continuation_cell_cursor_positioning() {
        use opentui::buffer::OptimizedBuffer;
        use opentui::cell::{Cell, GraphemeId};
        use opentui::color::Rgba;
        use opentui::renderer::BufferDiff;
        use opentui::style::{Style, TextAttributes};

        // Create old and new buffers
        let mut old_buf = OptimizedBuffer::new(10, 1);
        let mut new_buf = OptimizedBuffer::new(10, 1);

        // Manually set up old buffer with a wide character (simulating emoji)
        // GraphemeId with width 2 at position 0, continuation at position 1
        let wide_id = GraphemeId::new(1, 2); // pool_id=1, width=2
        old_buf.set(
            0,
            0,
            Cell {
                content: CellContent::Grapheme(wide_id),
                fg: Rgba::WHITE,
                bg: Rgba::BLACK,
                attributes: TextAttributes::empty(),
            },
        );
        old_buf.set(1, 0, Cell::continuation(Rgba::BLACK));
        old_buf.set(2, 0, Cell::new('B', Style::NONE));
        old_buf.set(3, 0, Cell::new('C', Style::NONE));
        old_buf.set(4, 0, Cell::new('D', Style::NONE));

        // Verify old buffer has: wide(0), continuation(1), B(2), C(3), D(4)
        assert!(
            old_buf.get(1, 0).unwrap().is_continuation(),
            "Position 1 should be continuation"
        );
        assert!(
            matches!(old_buf.get(2, 0).unwrap().content, CellContent::Char('B')),
            "Position 2 should be 'B'"
        );

        // Set up new buffer - same wide char, but different letters after
        new_buf.set(
            0,
            0,
            Cell {
                content: CellContent::Grapheme(wide_id),
                fg: Rgba::WHITE,
                bg: Rgba::BLACK,
                attributes: TextAttributes::empty(),
            },
        );
        new_buf.set(1, 0, Cell::continuation(Rgba::BLACK));
        new_buf.set(2, 0, Cell::new('X', Style::NONE));
        new_buf.set(3, 0, Cell::new('Y', Style::NONE));
        new_buf.set(4, 0, Cell::new('Z', Style::NONE));

        // Compute diff - should detect changes at positions 2, 3, 4
        let diff = BufferDiff::compute(&old_buf, &new_buf);

        // The diff should contain the changed cells
        assert!(
            !diff.is_empty(),
            "Diff should detect changes at positions 2, 3, 4"
        );

        // Verify that the changed cells are at positions 2, 3, 4
        // (The grapheme and continuation at 0,1 are the same, so shouldn't be in diff)
        assert_eq!(
            diff.changed_cells.len(),
            3,
            "Should have exactly 3 changed cells (positions 2, 3, 4)"
        );

        for &(x, _y) in &diff.changed_cells {
            assert!(
                (2..=4).contains(&x),
                "Changed cell at x={} should be in range [2, 4]",
                x
            );
        }

        // The key insight: when rendering this diff, if we have a dirty region
        // that includes positions 2, 3, 4 and the renderer incorrectly assumes
        // continuous cursor advancement, the output would be wrong.
        //
        // The fix ensures we move_cursor to exact position before each cell write:
        //   writer.move_cursor(y, x);  // Exact position before each write
        // Instead of:
        //   writer.move_cursor(region.y, region.x);  // Only at region start
        //
        // This test verifies the diff computation correctly identifies
        // which cells changed without including the unchanged wide char.
    }
}
