//! Hyperlink pool for OSC 8 link storage.

/// Pool of hyperlinks with reference counting.
#[derive(Clone, Debug, Default)]
pub struct LinkPool {
    urls: Vec<Option<String>>,
    ref_counts: Vec<u32>,
    free_list: Vec<u32>,
}

impl LinkPool {
    /// Create a new empty link pool.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate a link ID for the given URL.
    ///
    /// Returns a non-zero link ID (0 means no link).
    pub fn alloc(&mut self, url: &str) -> u32 {
        if let Some(id) = self.free_list.pop() {
            let idx = (id - 1) as usize;
            self.urls[idx] = Some(url.to_string());
            self.ref_counts[idx] = 1;
            return id;
        }

        self.urls.push(Some(url.to_string()));
        self.ref_counts.push(1);
        self.urls.len() as u32
    }

    /// Get the URL for a link ID.
    #[must_use]
    pub fn get(&self, id: u32) -> Option<&str> {
        if id == 0 {
            return None;
        }
        let idx = id.saturating_sub(1) as usize;
        self.urls.get(idx).and_then(|u| u.as_deref())
    }

    /// Increment the reference count for a link ID.
    pub fn incref(&mut self, id: u32) {
        if id == 0 {
            return;
        }
        let idx = id.saturating_sub(1) as usize;
        if let Some(count) = self.ref_counts.get_mut(idx) {
            *count = count.saturating_add(1);
        }
    }

    /// Decrement the reference count and free if it reaches zero.
    pub fn decref(&mut self, id: u32) {
        if id == 0 {
            return;
        }
        let idx = id.saturating_sub(1) as usize;
        if let Some(count) = self.ref_counts.get_mut(idx) {
            if *count > 0 {
                *count -= 1;
                if *count == 0 {
                    self.urls[idx] = None;
                    self.free_list.push(id);
                }
            }
        }
    }

    /// Clear all links.
    pub fn clear(&mut self) {
        self.urls.clear();
        self.ref_counts.clear();
        self.free_list.clear();
    }

    /// Number of allocated slots (including freed slots).
    #[must_use]
    pub fn len(&self) -> usize {
        self.urls.len()
    }

    /// Check if pool is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // Basic Operations Tests
    // ============================================

    #[test]
    fn test_link_pool_new() {
        let pool = LinkPool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_link_pool_default() {
        let pool = LinkPool::default();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_link_pool_alloc_get() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");
        assert_ne!(id, 0);
        assert_eq!(pool.get(id), Some("https://example.com"));
    }

    #[test]
    fn test_link_pool_multiple_allocs_unique_ids() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://one.example");
        let id2 = pool.alloc("https://two.example");
        let id3 = pool.alloc("https://three.example");

        assert_ne!(id1, 0);
        assert_ne!(id2, 0);
        assert_ne!(id3, 0);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_link_pool_get_all_urls() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://one.example");
        let id2 = pool.alloc("https://two.example");
        let id3 = pool.alloc("https://three.example");

        assert_eq!(pool.get(id1), Some("https://one.example"));
        assert_eq!(pool.get(id2), Some("https://two.example"));
        assert_eq!(pool.get(id3), Some("https://three.example"));
    }

    // ============================================
    // Reference Counting Tests
    // ============================================

    #[test]
    fn test_link_pool_incref() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");

        // Increment reference count
        pool.incref(id);
        pool.incref(id);

        // Should still be accessible
        assert_eq!(pool.get(id), Some("https://example.com"));

        // Decrement once - should still exist (count was 3)
        pool.decref(id);
        assert_eq!(pool.get(id), Some("https://example.com"));
    }

    #[test]
    fn test_link_pool_decref_frees_slot() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");

        // Initial refcount is 1, decref should free it
        pool.decref(id);
        assert_eq!(pool.get(id), None);
    }

    #[test]
    fn test_link_pool_reuse() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://one.example");
        pool.decref(id1);
        let id2 = pool.alloc("https://two.example");
        assert_eq!(id1, id2);
        assert_eq!(pool.get(id2), Some("https://two.example"));
    }

    #[test]
    fn test_link_pool_double_decref_safe() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");

        // First decref frees the slot
        pool.decref(id);
        // Second decref should be safe (no panic, no underflow)
        pool.decref(id);

        // Should still be None
        assert_eq!(pool.get(id), None);
    }

    #[test]
    fn test_link_pool_refcount_saturating() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");

        // Increment many times
        for _ in 0..1000 {
            pool.incref(id);
        }

        // Should still be accessible
        assert_eq!(pool.get(id), Some("https://example.com"));
    }

    // ============================================
    // ID Space Management Tests
    // ============================================

    #[test]
    fn test_link_pool_free_list_lifo() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://one.example");
        let id2 = pool.alloc("https://two.example");
        let id3 = pool.alloc("https://three.example");

        // Free in order: 1, 2, 3
        pool.decref(id1);
        pool.decref(id2);
        pool.decref(id3);

        // LIFO: should get id3 first, then id2, then id1
        let new_id1 = pool.alloc("https://new1.example");
        let new_id2 = pool.alloc("https://new2.example");
        let new_id3 = pool.alloc("https://new3.example");

        assert_eq!(new_id1, id3); // LIFO: last freed is first reused
        assert_eq!(new_id2, id2);
        assert_eq!(new_id3, id1);
    }

    #[test]
    fn test_link_pool_ids_are_1_indexed() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");
        // IDs should be 1-indexed (0 means no link)
        assert_eq!(id, 1);
    }

    #[test]
    fn test_link_pool_sequential_ids() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://one.example");
        let id2 = pool.alloc("https://two.example");
        let id3 = pool.alloc("https://three.example");

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);
    }

    // ============================================
    // Zero ID Handling Tests
    // ============================================

    #[test]
    fn test_link_pool_get_zero_returns_none() {
        let pool = LinkPool::new();
        assert_eq!(pool.get(0), None);
    }

    #[test]
    fn test_link_pool_incref_zero_safe() {
        let mut pool = LinkPool::new();
        // Should not panic
        pool.incref(0);
    }

    #[test]
    fn test_link_pool_decref_zero_safe() {
        let mut pool = LinkPool::new();
        // Should not panic
        pool.decref(0);
    }

    // ============================================
    // Edge Cases Tests
    // ============================================

    #[test]
    fn test_link_pool_empty_url() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("");
        assert_ne!(id, 0);
        assert_eq!(pool.get(id), Some(""));
    }

    #[test]
    fn test_link_pool_long_url() {
        let mut pool = LinkPool::new();
        let long_url = "https://example.com/".to_string() + &"a".repeat(10000);
        let id = pool.alloc(&long_url);
        assert_ne!(id, 0);
        assert_eq!(pool.get(id), Some(long_url.as_str()));
    }

    #[test]
    fn test_link_pool_unicode_url() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com/路径/文件");
        assert_ne!(id, 0);
        assert_eq!(pool.get(id), Some("https://example.com/路径/文件"));
    }

    #[test]
    fn test_link_pool_special_chars_url() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com/path?query=value&other=<>\"'");
        assert_ne!(id, 0);
        assert_eq!(
            pool.get(id),
            Some("https://example.com/path?query=value&other=<>\"'")
        );
    }

    #[test]
    fn test_link_pool_get_invalid_id() {
        let mut pool = LinkPool::new();
        pool.alloc("https://example.com");

        // ID 999 was never allocated
        assert_eq!(pool.get(999), None);
    }

    #[test]
    fn test_link_pool_get_after_free() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");
        pool.decref(id);

        // After freeing, get should return None
        assert_eq!(pool.get(id), None);
    }

    // ============================================
    // Clear Tests
    // ============================================

    #[test]
    fn test_link_pool_clear() {
        let mut pool = LinkPool::new();
        pool.alloc("https://one.example");
        pool.alloc("https://two.example");
        pool.alloc("https://three.example");

        assert_eq!(pool.len(), 3);
        assert!(!pool.is_empty());

        pool.clear();

        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }

    #[test]
    fn test_link_pool_alloc_after_clear() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://example.com");
        pool.clear();

        // After clear, IDs should start from 1 again
        let id2 = pool.alloc("https://new.example");
        assert_eq!(id2, 1);
        assert_eq!(pool.get(id2), Some("https://new.example"));

        // Old ID should be invalid (pool was cleared)
        // Note: id1 == 1, which is now reused, so get(id1) returns new URL
        assert_eq!(id1, 1);
    }

    // ============================================
    // Len and is_empty Tests
    // ============================================

    #[test]
    fn test_link_pool_len_increases() {
        let mut pool = LinkPool::new();
        assert_eq!(pool.len(), 0);

        pool.alloc("https://one.example");
        assert_eq!(pool.len(), 1);

        pool.alloc("https://two.example");
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_link_pool_len_includes_freed_slots() {
        let mut pool = LinkPool::new();
        let id1 = pool.alloc("https://one.example");
        pool.alloc("https://two.example");

        pool.decref(id1);

        // len() returns total slots, including freed ones
        // The slot is freed but still counted in the vector length
        assert_eq!(pool.len(), 2);
    }

    #[test]
    fn test_link_pool_is_empty() {
        let mut pool = LinkPool::new();
        assert!(pool.is_empty());

        pool.alloc("https://example.com");
        assert!(!pool.is_empty());
    }

    // ============================================
    // Clone and Debug Tests
    // ============================================

    #[test]
    fn test_link_pool_clone() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");

        let cloned = pool.clone();
        assert_eq!(cloned.get(id), Some("https://example.com"));
        assert_eq!(cloned.len(), pool.len());
    }

    #[test]
    fn test_link_pool_debug() {
        let mut pool = LinkPool::new();
        pool.alloc("https://example.com");

        let debug_str = format!("{:?}", pool);
        assert!(debug_str.contains("LinkPool"));
    }
}
