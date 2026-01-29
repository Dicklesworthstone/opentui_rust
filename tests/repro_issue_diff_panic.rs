#[cfg(test)]
mod tests {
    use opentui::buffer::OptimizedBuffer;
    use opentui::renderer::BufferDiff;

    #[test]
    #[should_panic(expected = "buffer size mismatch")]
    fn test_diff_mismatch_panics() {
        let b1 = OptimizedBuffer::new(10, 10);
        let b2 = OptimizedBuffer::new(20, 20);
        let _ = BufferDiff::compute(&b1, &b2);
    }
}
