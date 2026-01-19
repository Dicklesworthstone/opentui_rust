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

    #[test]
    fn test_link_pool_alloc_get() {
        let mut pool = LinkPool::new();
        let id = pool.alloc("https://example.com");
        assert_ne!(id, 0);
        assert_eq!(pool.get(id), Some("https://example.com"));
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
}
