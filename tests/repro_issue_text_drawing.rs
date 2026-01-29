#[cfg(test)]
mod tests {
    use opentui::buffer::OptimizedBuffer;
    use opentui::cell::CellContent;
    use opentui::grapheme_pool::GraphemePool;
    use opentui::style::Style;

    #[test]
    fn test_draw_text_consistency() {
        // Case 1: draw_text (Fast path for ASCII)
        let mut buf1 = OptimizedBuffer::new(10, 1);
        buf1.draw_text(0, 0, "A\tB", Style::NONE);

        // Case 2: draw_text_with_pool (Standard path, even for ASCII if pool is used)
        // Note: draw_text_with_pool uses graphemes(true) which handles ASCII too.
        let mut buf2 = OptimizedBuffer::new(10, 1);
        let mut pool = GraphemePool::new();
        buf2.draw_text_with_pool(&mut pool, 0, 0, "A\tB", Style::NONE);

        // Analyze buf1 (Fixed behavior: consistent with buf2)
        // 'A' at 0
        assert_eq!(buf1.get(0, 0).unwrap().content, CellContent::Char('A'));
        // 'B' at 1 (tab was width 0, so overwritten)
        assert_eq!(buf1.get(1, 0).unwrap().content, CellContent::Char('B'));

        // Analyze buf2 (current behavior expectation: A, B overwrites \t)
        // 'A' at 0
        assert_eq!(buf2.get(0, 0).unwrap().content, CellContent::Char('A'));
        // 'B' at 1 (draw_text_with_pool treats \t as width 0, so next char overwrites)
        assert_eq!(buf2.get(1, 0).unwrap().content, CellContent::Char('B'));

        // Assert consistency
        assert_eq!(
            buf1.get(1, 0).unwrap().content,
            buf2.get(1, 0).unwrap().content
        );
    }
}
