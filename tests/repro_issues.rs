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
    fn test_overwrite_leak() {
        let mut buffer = OptimizedBuffer::new(10, 10);
        let mut pool = GraphemePool::new();

        // 1. Alloc grapheme
        let id = pool.alloc("👨‍👩‍👧");
        let initial_refcount = pool.refcount(id);
        assert_eq!(initial_refcount, 1);

        // 2. Draw it to buffer with pool (increments refcount?)
        // buffer.set_with_pool doesn't increment if we pass the cell in,
        // it assumes we are transferring ownership of the ID reference if it was just allocated?
        // Wait, GraphemeId is Copy.
        // Let's look at set_with_pool implementation again.

        /*
        if let Some(dest) = self.get_mut(x, y) {
            let old_content = dest.content;
            let new_content = cell.content;

            if old_content != new_content {
                if let CellContent::Grapheme(id) = old_content {
                    if id.pool_id() != 0 {
                        pool.decref(id);
                    }
                }
            } else if let CellContent::Grapheme(id) = new_content {
                 // ...
            }
            *dest = cell;
        }
        */

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
        let clear_cell = opentui::cell::Cell::clear(opentui::color::Rgba::BLACK);
        buffer.set(0, 0, clear_cell);

        // Buffer now has clear_cell.
        // But pool.decref was NEVER called.
        // Refcount should still be 1 -> LEAK.

        assert_eq!(pool.refcount(id), 1, "Refcount should be leaked (remain 1)");

        // 4. Clear buffer with pool
        buffer.clear_with_pool(&mut pool, opentui::color::Rgba::BLACK);

        // Since buffer cell is already cleared (by step 3), clear_with_pool sees Empty/Char.
        // It won't decref the emoji because it's already gone from the buffer array.

        assert_eq!(
            pool.refcount(id),
            1,
            "Refcount should still be leaked after clear"
        );
    }
}
