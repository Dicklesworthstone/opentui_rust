//! Reference-counted grapheme pool for multi-codepoint character clusters.
//!
//! This module implements a pool for storing grapheme clusters (emoji, ZWJ sequences,
//! combining characters) that are too complex to represent as a single `char`. The pool
//! uses reference counting and a free-list for efficient memory reuse.
//!
//! # Design
//!
//! Per the Zig spec (EXISTING_OPENTUI_STRUCTURE.md section 3):
//! - Slots store UTF-8 bytes of grapheme clusters
//! - 24-bit ID allows ~16M unique graphemes
//! - Reference counting for memory reuse
//! - Free-list for O(1) slot reuse
//!
//! # Usage
//!
//! ```
//! use opentui::grapheme_pool::GraphemePool;
//!
//! let mut pool = GraphemePool::new();
//!
//! // Allocate a grapheme
//! let id = pool.alloc("👨‍👩‍👧");
//!
//! // Retrieve it later
//! assert_eq!(pool.get(id), Some("👨‍👩‍👧"));
//!
//! // Reference counting
//! pool.incref(id);
//! assert!(pool.decref(id)); // Still has references
//! assert!(!pool.decref(id)); // Freed, returns false
//! ```
//!
//! # Invariants
//!
//! - Pool ID 0 is reserved/invalid (placeholder IDs use pool_id 0)
//! - Refcount starts at 1 on alloc
//! - decref returns `true` if references remain, `false` if freed
//! - get returns `None` for freed or invalid IDs

use crate::cell::GraphemeId;

/// Maximum pool ID (24-bit limit).
pub const MAX_POOL_ID: u32 = 0x00FF_FFFF;

/// Internal slot in the grapheme pool.
#[derive(Clone, Debug)]
struct Slot {
    /// The grapheme cluster string.
    bytes: String,
    /// Reference count (0 = free).
    refcount: u32,
    /// Cached display width.
    width: u8,
}

impl Slot {
    /// Create a new slot with initial refcount of 1.
    fn new(bytes: String, width: u8) -> Self {
        Self {
            bytes,
            refcount: 1,
            width,
        }
    }

    /// Check if this slot is free.
    fn is_free(&self) -> bool {
        self.refcount == 0
    }
}

/// Reference-counted pool for grapheme clusters.
///
/// Stores multi-codepoint graphemes (emoji, ZWJ sequences, combining characters)
/// and provides O(1) access via [`GraphemeId`].
///
/// # Thread Safety
///
/// `GraphemePool` is not thread-safe. For concurrent access, wrap in appropriate
/// synchronization primitives (e.g., `Mutex` or `RwLock`).
#[derive(Clone, Debug, Default)]
pub struct GraphemePool {
    /// Storage for grapheme slots. Index 0 is reserved (invalid).
    slots: Vec<Slot>,
    /// Stack of free slot indices for reuse.
    free_list: Vec<u32>,
}

impl GraphemePool {
    /// Create a new empty grapheme pool.
    ///
    /// The pool starts with slot 0 reserved as invalid/placeholder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            // Reserve slot 0 as invalid placeholder
            slots: vec![Slot {
                bytes: String::new(),
                refcount: 0,
                width: 0,
            }],
            free_list: Vec::new(),
        }
    }

    /// Create a pool with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Number of slots to pre-allocate (excludes reserved slot 0)
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut slots = Vec::with_capacity(capacity + 1);
        // Reserve slot 0
        slots.push(Slot {
            bytes: String::new(),
            refcount: 0,
            width: 0,
        });
        Self {
            slots,
            free_list: Vec::new(),
        }
    }

    /// Allocate a new grapheme in the pool.
    ///
    /// Returns a [`GraphemeId`] with the pool slot ID and cached display width.
    /// The initial reference count is 1.
    ///
    /// # Arguments
    ///
    /// * `grapheme` - The grapheme cluster string to store
    ///
    /// # Panics
    ///
    /// Panics if the pool exceeds 16M entries (24-bit ID limit).
    #[must_use]
    pub fn alloc(&mut self, grapheme: &str) -> GraphemeId {
        let width = crate::unicode::display_width(grapheme);
        // Saturate width to u8 range, then GraphemeId::new() will saturate to 127
        let width_u8 = width.min(u8::MAX as usize) as u8;
        let slot = Slot::new(grapheme.to_owned(), width_u8);

        let pool_id = if let Some(free_id) = self.free_list.pop() {
            // Reuse a freed slot
            self.slots[free_id as usize] = slot;
            free_id
        } else {
            // Allocate new slot
            let id = self.slots.len() as u32;
            assert!(id <= MAX_POOL_ID, "GraphemePool exceeded 16M entry limit");
            self.slots.push(slot);
            id
        };

        GraphemeId::new(pool_id, width_u8)
    }

    /// Intern a grapheme, returning an existing ID if already allocated.
    ///
    /// If the grapheme already exists in the pool (with refcount > 0), increments
    /// its refcount and returns the existing ID. Otherwise, allocates a new slot.
    ///
    /// This is useful for deduplicating repeated graphemes.
    #[must_use]
    pub fn intern(&mut self, grapheme: &str) -> GraphemeId {
        // Linear search for existing (could use HashMap for O(1) lookup if needed)
        // First pass: find existing slot
        let existing = self
            .slots
            .iter()
            .enumerate()
            .skip(1)
            .find(|(_, slot)| !slot.is_free() && slot.bytes == grapheme)
            .map(|(i, slot)| (i as u32, slot.width));

        if let Some((pool_id, width)) = existing {
            self.incref_by_pool_id(pool_id);
            GraphemeId::new(pool_id, width)
        } else {
            self.alloc(grapheme)
        }
    }

    /// Increment the reference count for a grapheme ID.
    ///
    /// # Safety
    ///
    /// If the ID is invalid or freed, this is a no-op.
    pub fn incref(&mut self, id: GraphemeId) {
        self.incref_by_pool_id(id.pool_id());
    }

    /// Increment refcount by pool ID directly.
    fn incref_by_pool_id(&mut self, pool_id: u32) {
        if let Some(slot) = self.slots.get_mut(pool_id as usize) {
            if slot.refcount > 0 {
                slot.refcount = slot.refcount.saturating_add(1);
            }
        }
    }

    /// Decrement the reference count for a grapheme ID.
    ///
    /// Returns `true` if references remain, `false` if the slot was freed.
    ///
    /// # Safety
    ///
    /// If the ID is invalid or already freed, returns `false` without modification.
    pub fn decref(&mut self, id: GraphemeId) -> bool {
        self.decref_by_pool_id(id.pool_id())
    }

    /// Decrement refcount by pool ID directly.
    fn decref_by_pool_id(&mut self, pool_id: u32) -> bool {
        if let Some(slot) = self.slots.get_mut(pool_id as usize) {
            if slot.refcount > 0 {
                slot.refcount -= 1;
                if slot.refcount == 0 {
                    // Free the slot
                    slot.bytes.clear();
                    self.free_list.push(pool_id);
                    return false;
                }
                return true;
            }
        }
        false
    }

    /// Get the grapheme string for an ID.
    ///
    /// Returns `None` if the ID is invalid or the slot is freed.
    #[must_use]
    pub fn get(&self, id: GraphemeId) -> Option<&str> {
        self.get_by_pool_id(id.pool_id())
    }

    /// Get grapheme by pool ID directly.
    #[must_use]
    pub fn get_by_pool_id(&self, pool_id: u32) -> Option<&str> {
        self.slots.get(pool_id as usize).and_then(|slot| {
            if slot.is_free() {
                None
            } else {
                Some(slot.bytes.as_str())
            }
        })
    }

    /// Get the refcount for a grapheme ID.
    ///
    /// Returns 0 for invalid or freed IDs.
    #[must_use]
    pub fn refcount(&self, id: GraphemeId) -> u32 {
        self.slots
            .get(id.pool_id() as usize)
            .map_or(0, |slot| slot.refcount)
    }

    /// Check if an ID is valid (allocated and not freed).
    #[must_use]
    pub fn is_valid(&self, id: GraphemeId) -> bool {
        self.slots
            .get(id.pool_id() as usize)
            .is_some_and(|slot| !slot.is_free())
    }

    /// Get the number of active (non-freed) graphemes in the pool.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.slots.iter().skip(1).filter(|s| !s.is_free()).count()
    }

    /// Get the total number of slots (including freed ones, excluding reserved slot 0).
    #[must_use]
    pub fn total_slots(&self) -> usize {
        self.slots.len().saturating_sub(1)
    }

    /// Get the number of free slots available for reuse.
    #[must_use]
    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }

    /// Clear all graphemes from the pool.
    ///
    /// This resets the pool to its initial state with only slot 0 reserved.
    pub fn clear(&mut self) {
        self.slots.truncate(1);
        self.free_list.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_new() {
        let pool = GraphemePool::new();
        assert_eq!(pool.total_slots(), 0);
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.free_count(), 0);
    }

    #[test]
    fn test_alloc_and_get() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("👨‍👩‍👧");

        assert_eq!(pool.get(id), Some("👨‍👩‍👧"));
        assert_eq!(pool.refcount(id), 1);
        assert!(pool.is_valid(id));
    }

    #[test]
    fn test_grapheme_id_width_encoding() {
        let mut pool = GraphemePool::new();

        // ZWJ family emoji has width 2
        let id = pool.alloc("👨‍👩‍👧");
        assert_eq!(id.width(), 2);

        // Simple emoji has width 2
        let id2 = pool.alloc("👍");
        assert_eq!(id2.width(), 2);
    }

    #[test]
    fn test_incref_decref() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        assert_eq!(pool.refcount(id), 1);

        pool.incref(id);
        assert_eq!(pool.refcount(id), 2);

        pool.incref(id);
        assert_eq!(pool.refcount(id), 3);

        assert!(pool.decref(id)); // 3 -> 2
        assert_eq!(pool.refcount(id), 2);

        assert!(pool.decref(id)); // 2 -> 1
        assert_eq!(pool.refcount(id), 1);

        assert!(!pool.decref(id)); // 1 -> 0, freed
        assert_eq!(pool.refcount(id), 0);
        assert!(!pool.is_valid(id));
        assert_eq!(pool.get(id), None);
    }

    #[test]
    fn test_slot_reuse() {
        let mut pool = GraphemePool::new();

        // Allocate and free
        let id1 = pool.alloc("first");
        let pool_id1 = id1.pool_id();
        pool.decref(id1);

        // Next alloc should reuse the freed slot
        let id2 = pool.alloc("second");
        assert_eq!(id2.pool_id(), pool_id1);
        assert_eq!(pool.get(id2), Some("second"));
    }

    #[test]
    fn test_multiple_allocations() {
        let mut pool = GraphemePool::new();

        let ids: Vec<_> = (0..10).map(|i| pool.alloc(&format!("item{i}"))).collect();

        assert_eq!(pool.active_count(), 10);
        assert_eq!(pool.total_slots(), 10);

        for (i, id) in ids.iter().enumerate() {
            assert_eq!(pool.get(*id), Some(format!("item{i}").as_str()));
        }
    }

    #[test]
    fn test_intern_deduplication() {
        let mut pool = GraphemePool::new();

        let id1 = pool.intern("duplicate");
        let id2 = pool.intern("duplicate");

        // Should return same ID
        assert_eq!(id1, id2);
        // Refcount should be 2
        assert_eq!(pool.refcount(id1), 2);
        // Only one slot used
        assert_eq!(pool.active_count(), 1);
    }

    #[test]
    fn test_intern_different_graphemes() {
        let mut pool = GraphemePool::new();

        let id1 = pool.intern("first");
        let id2 = pool.intern("second");

        assert_ne!(id1, id2);
        assert_eq!(pool.active_count(), 2);
    }

    #[test]
    fn test_invalid_id_handling() {
        let pool = GraphemePool::new();

        // ID 0 is reserved/invalid
        let invalid = GraphemeId::new(0, 1);
        assert_eq!(pool.get(invalid), None);
        assert!(!pool.is_valid(invalid));

        // ID beyond allocated range
        let beyond = GraphemeId::new(9999, 1);
        assert_eq!(pool.get(beyond), None);
        assert!(!pool.is_valid(beyond));
    }

    #[test]
    fn test_invalid_id_incref_decref() {
        let mut pool = GraphemePool::new();

        let invalid = GraphemeId::new(0, 1);

        // Should be no-ops / return false
        pool.incref(invalid);
        assert!(!pool.decref(invalid));
    }

    #[test]
    fn test_clear() {
        let mut pool = GraphemePool::new();

        let _ = pool.alloc("a");
        let _ = pool.alloc("b");
        let _ = pool.alloc("c");

        assert_eq!(pool.active_count(), 3);

        pool.clear();

        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.total_slots(), 0);
        assert_eq!(pool.free_count(), 0);
    }

    #[test]
    fn test_freed_slot_not_found_by_intern() {
        let mut pool = GraphemePool::new();

        let id = pool.alloc("ephemeral");
        pool.decref(id);

        // After freeing, intern should allocate new (not find freed)
        let id2 = pool.intern("ephemeral");

        // Should reuse the slot but as new allocation
        assert_eq!(id2.pool_id(), id.pool_id());
        assert_eq!(pool.refcount(id2), 1);
    }

    #[test]
    fn test_refcount_saturation() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Incref many times shouldn't overflow
        for _ in 0..100 {
            pool.incref(id);
        }

        assert_eq!(pool.refcount(id), 101);
    }

    #[test]
    fn test_with_capacity() {
        let pool = GraphemePool::with_capacity(100);
        assert_eq!(pool.total_slots(), 0);
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_grapheme_id_roundtrip() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("🎉");

        // Can get the original string back
        assert_eq!(pool.get(id), Some("🎉"));

        // Width is correct
        assert_eq!(id.width(), 2);

        // Pool ID is 1 (first allocation after reserved 0)
        assert_eq!(id.pool_id(), 1);
    }
}
