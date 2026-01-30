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
//! - HashMap index for O(1) intern() lookup (avoiding O(n) linear scan)
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
use std::collections::HashMap;

/// Maximum pool ID (24-bit limit).
pub const MAX_POOL_ID: u32 = 0x00FF_FFFF;

/// Default soft limit for pool size (1 million entries).
pub const DEFAULT_SOFT_LIMIT: usize = 1_000_000;

/// Utilization threshold considered "high" (80%).
pub const HIGH_UTILIZATION_THRESHOLD: usize = 80;

/// Statistics about pool utilization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolStats {
    /// Total number of allocated slots (including freed).
    pub total_slots: usize,
    /// Number of actively used slots.
    pub active_slots: usize,
    /// Number of free slots available for reuse.
    pub free_slots: usize,
    /// Configured soft limit for the pool.
    pub soft_limit: usize,
    /// Current utilization percentage (0-100).
    pub utilization_percent: usize,
}

impl PoolStats {
    /// Check if utilization is at or above a given threshold percentage.
    #[must_use]
    pub fn is_above_threshold(&self, threshold_percent: usize) -> bool {
        self.utilization_percent >= threshold_percent
    }
}

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
#[derive(Clone, Debug)]
pub struct GraphemePool {
    /// Storage for grapheme slots. Index 0 is reserved (invalid).
    slots: Vec<Slot>,
    /// Stack of free slot indices for reuse.
    free_list: Vec<u32>,
    /// O(1) lookup index: grapheme string → slot ID.
    /// Kept in sync with slots: entries are added on alloc/intern, removed on decref to 0.
    index: HashMap<String, u32>,
    /// Configurable soft limit for pool size (advisory, not enforced by alloc).
    soft_limit: usize,
}

impl Default for GraphemePool {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphemePool {
    /// Create a new empty grapheme pool with default soft limit.
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
            index: HashMap::new(),
            soft_limit: DEFAULT_SOFT_LIMIT,
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
            index: HashMap::with_capacity(capacity),
            soft_limit: DEFAULT_SOFT_LIMIT,
        }
    }

    /// Create a pool with a custom soft limit.
    ///
    /// The soft limit is advisory and used for utilization metrics.
    /// It does not prevent allocations - use [`try_alloc()`](Self::try_alloc)
    /// if you want to check before allocating.
    ///
    /// # Arguments
    ///
    /// * `soft_limit` - Maximum number of active entries for "normal" operation
    #[must_use]
    pub fn with_soft_limit(soft_limit: usize) -> Self {
        Self {
            slots: vec![Slot {
                bytes: String::new(),
                refcount: 0,
                width: 0,
            }],
            free_list: Vec::new(),
            index: HashMap::new(),
            soft_limit,
        }
    }

    /// Set the soft limit for this pool.
    ///
    /// Returns `&mut self` for builder-style chaining.
    pub fn set_soft_limit(&mut self, limit: usize) -> &mut Self {
        self.soft_limit = limit;
        self
    }

    /// Get the configured soft limit.
    #[must_use]
    pub fn soft_limit(&self) -> usize {
        self.soft_limit
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
    ///
    /// # Note
    ///
    /// This method does NOT deduplicate. If you want to reuse existing graphemes,
    /// use [`intern()`](Self::intern) instead.
    #[must_use]
    pub fn alloc(&mut self, grapheme: &str) -> GraphemeId {
        let width = crate::unicode::display_width(grapheme);
        // Saturate width to u8 range, then GraphemeId::new() will saturate to 127
        let width_u8 = width.min(u8::MAX as usize) as u8;
        let grapheme_owned = grapheme.to_owned();
        let slot = Slot::new(grapheme_owned.clone(), width_u8);

        let pool_id = if let Some(free_id) = self.free_list.pop() {
            // Reuse a freed slot
            self.slots[free_id as usize] = slot;
            free_id
        } else {
            // Allocate new slot
            let id = self.slots.len() as u32;
            // Exceeding 24-bit pool ID limit would cause ID collisions and use-after-free bugs.
            assert!(
                id <= MAX_POOL_ID,
                "GraphemePool exceeded 16M entry limit (id={id})"
            );
            self.slots.push(slot);
            id
        };

        // Add to index for O(1) intern() lookup
        self.index.insert(grapheme_owned, pool_id);

        GraphemeId::new(pool_id, width_u8)
    }

    /// Intern a grapheme, returning an existing ID if already allocated.
    ///
    /// If the grapheme already exists in the pool (with refcount > 0), increments
    /// its refcount and returns the existing ID. Otherwise, allocates a new slot.
    ///
    /// This is useful for deduplicating repeated graphemes.
    ///
    /// # Performance
    ///
    /// Uses O(1) HashMap lookup instead of linear scan.
    #[must_use]
    pub fn intern(&mut self, grapheme: &str) -> GraphemeId {
        // O(1) lookup via HashMap index
        if let Some(&pool_id) = self.index.get(grapheme) {
            // Verify slot is still active (not freed)
            if let Some(slot) = self.slots.get(pool_id as usize) {
                if !slot.is_free() {
                    let width = slot.width; // Save before mutable borrow
                    self.incref_by_pool_id(pool_id);
                    return GraphemeId::new(pool_id, width);
                }
            }
            // Index entry is stale (slot was freed) - remove it and allocate fresh
            self.index.remove(grapheme);
        }

        // Not found in index - allocate new (which also adds to index)
        self.alloc(grapheme)
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
                    // Remove from index before clearing bytes
                    self.index.remove(&slot.bytes);
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

    /// Check if the pool is at capacity (16M entries).
    ///
    /// When full, new allocations will panic. Use `free_count()` to check
    /// if slots can be reused instead of allocating new ones.
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.free_list.is_empty() && self.slots.len() > MAX_POOL_ID as usize
    }

    /// Get the remaining capacity for new slot allocations.
    ///
    /// This counts both reusable free slots and slots that can still be allocated.
    #[must_use]
    pub fn capacity_remaining(&self) -> usize {
        let free_slots = self.free_list.len();
        let allocatable = (MAX_POOL_ID as usize + 1).saturating_sub(self.slots.len());
        free_slots + allocatable
    }

    /// Clear all graphemes from the pool.
    ///
    /// This resets the pool to its initial state with only slot 0 reserved.
    pub fn clear(&mut self) {
        self.slots.truncate(1);
        self.free_list.clear();
        self.index.clear();
    }

    /// Get current pool utilization statistics.
    ///
    /// Returns a [`PoolStats`] struct with counts and utilization percentage.
    #[must_use]
    pub fn stats(&self) -> PoolStats {
        let total_slots = self.total_slots();
        let free_slots = self.free_count();
        let active_slots = total_slots.saturating_sub(free_slots);

        // Calculate utilization as percentage of soft_limit
        let utilization_percent = (active_slots * 100)
            .checked_div(self.soft_limit)
            .unwrap_or(0);

        PoolStats {
            total_slots,
            active_slots,
            free_slots,
            soft_limit: self.soft_limit,
            utilization_percent,
        }
    }

    /// Get current utilization as a percentage of the soft limit.
    ///
    /// Returns a value from 0 to 100+ (can exceed 100 if over soft limit).
    #[must_use]
    pub fn utilization_percent(&self) -> usize {
        self.stats().utilization_percent
    }

    /// Check if pool utilization is above the high threshold (80% by default).
    ///
    /// Use this to trigger warnings or preemptive cleanup.
    #[must_use]
    pub fn is_high_utilization(&self) -> bool {
        self.utilization_percent() >= HIGH_UTILIZATION_THRESHOLD
    }

    /// Check if pool is at or above a specific utilization threshold.
    ///
    /// # Arguments
    ///
    /// * `threshold_percent` - Threshold percentage (0-100)
    #[must_use]
    pub fn is_above_utilization(&self, threshold_percent: usize) -> bool {
        self.utilization_percent() >= threshold_percent
    }

    /// Get the fragmentation ratio of the pool.
    ///
    /// Returns the ratio of freed slots to total slots as a value in `[0.0, 1.0]`.
    /// A high ratio suggests that compaction may be beneficial.
    ///
    /// # Returns
    ///
    /// - `0.0` if the pool is empty or has no freed slots
    /// - Values approaching `1.0` indicate high fragmentation
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    /// assert_eq!(pool.get_fragmentation_ratio(), 0.0); // Empty pool
    ///
    /// let id1 = pool.alloc("a");
    /// let id2 = pool.alloc("b");
    /// pool.decref(id1); // Free one slot
    ///
    /// assert_eq!(pool.get_fragmentation_ratio(), 0.5); // 1 freed / 2 total
    /// ```
    #[must_use]
    pub fn get_fragmentation_ratio(&self) -> f32 {
        let total = self.total_slots();
        if total == 0 {
            return 0.0;
        }
        self.free_count() as f32 / total as f32
    }

    /// Try to allocate a grapheme, returning `None` if the pool is at soft limit.
    ///
    /// Unlike [`alloc()`](Self::alloc), this respects the soft limit and returns
    /// `None` instead of allocating when the pool is full. It still allows
    /// reuse of freed slots.
    ///
    /// # Arguments
    ///
    /// * `grapheme` - The grapheme cluster string to store
    ///
    /// # Returns
    ///
    /// `Some(GraphemeId)` if allocation succeeded, `None` if at soft limit
    /// and no free slots are available for reuse.
    #[must_use]
    pub fn try_alloc(&mut self, grapheme: &str) -> Option<GraphemeId> {
        // Allow allocation if:
        // 1. There are free slots to reuse, OR
        // 2. We're below the soft limit
        let active = self.active_count();
        if self.free_list.is_empty() && active >= self.soft_limit {
            return None;
        }

        Some(self.alloc(grapheme))
    }

    /// Try to intern a grapheme, returning `None` if new allocation would exceed soft limit.
    ///
    /// If the grapheme already exists, always succeeds (just increments refcount).
    /// Only returns `None` when a new allocation would be needed and soft limit is reached.
    #[must_use]
    pub fn try_intern(&mut self, grapheme: &str) -> Option<GraphemeId> {
        // O(1) lookup via HashMap index
        if let Some(&pool_id) = self.index.get(grapheme) {
            // Verify slot is still active (not freed)
            if let Some(slot) = self.slots.get(pool_id as usize) {
                if !slot.is_free() {
                    let width = slot.width;
                    self.incref_by_pool_id(pool_id);
                    return Some(GraphemeId::new(pool_id, width));
                }
            }
            // Index entry is stale - remove it
            self.index.remove(grapheme);
        }

        // Need to allocate - use try_alloc which respects soft limit
        self.try_alloc(grapheme)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::let_underscore_must_use)] // Intentionally discarding alloc() returns in tests
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

    #[test]
    fn test_capacity_remaining() {
        let mut pool = GraphemePool::new();

        // Initially all capacity is available
        let initial_capacity = pool.capacity_remaining();
        assert_eq!(initial_capacity, MAX_POOL_ID as usize);

        // After allocation, capacity decreases
        let _id = pool.alloc("test");
        assert_eq!(pool.capacity_remaining(), initial_capacity - 1);

        // Free slot adds to capacity
        pool.decref(_id);
        assert_eq!(pool.capacity_remaining(), initial_capacity);
    }

    #[test]
    fn test_is_full_empty_pool() {
        let pool = GraphemePool::new();
        assert!(!pool.is_full(), "empty pool should not be full");
    }

    #[test]
    fn test_index_consistency_many_graphemes() {
        let mut pool = GraphemePool::new();

        // Allocate many unique graphemes
        let graphemes: Vec<String> = (0..1000).map(|i| format!("g{i}")).collect();
        let ids: Vec<_> = graphemes.iter().map(|g| pool.alloc(g)).collect();

        // All should be retrievable
        for (i, id) in ids.iter().enumerate() {
            assert_eq!(pool.get(*id), Some(graphemes[i].as_str()));
        }

        // Intern should return same IDs (via O(1) HashMap lookup)
        for (i, g) in graphemes.iter().enumerate() {
            let interned = pool.intern(g);
            assert_eq!(interned.pool_id(), ids[i].pool_id());
            assert_eq!(pool.refcount(interned), 2); // Original + interned
        }

        // Decref all twice to free
        for id in &ids {
            pool.decref(*id);
            pool.decref(*id);
        }

        // All should be freed
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.free_count(), 1000);

        // Interning after free should allocate fresh (reusing slots)
        for g in &graphemes {
            let fresh = pool.intern(g);
            assert_eq!(pool.refcount(fresh), 1);
        }

        // Should have reused slots, not grown
        assert_eq!(pool.active_count(), 1000);
        assert_eq!(pool.free_count(), 0);
        assert_eq!(pool.total_slots(), 1000);
    }

    #[test]
    fn test_index_cleared_on_clear() {
        let mut pool = GraphemePool::new();

        let _ = pool.alloc("a");
        let _ = pool.alloc("b");
        let _ = pool.alloc("c");

        pool.clear();

        // After clear, intern should allocate fresh
        let id = pool.intern("a");
        assert_eq!(id.pool_id(), 1); // First slot after reserved 0
        assert_eq!(pool.refcount(id), 1);
    }

    #[test]
    fn test_with_soft_limit() {
        let pool = GraphemePool::with_soft_limit(100);
        assert_eq!(pool.soft_limit(), 100);
        assert_eq!(pool.total_slots(), 0);
    }

    #[test]
    fn test_set_soft_limit() {
        let mut pool = GraphemePool::new();
        assert_eq!(pool.soft_limit(), DEFAULT_SOFT_LIMIT);

        pool.set_soft_limit(500);
        assert_eq!(pool.soft_limit(), 500);
    }

    #[test]
    fn test_pool_stats() {
        let mut pool = GraphemePool::with_soft_limit(100);

        // Empty pool
        let stats = pool.stats();
        assert_eq!(stats.total_slots, 0);
        assert_eq!(stats.active_slots, 0);
        assert_eq!(stats.free_slots, 0);
        assert_eq!(stats.soft_limit, 100);
        assert_eq!(stats.utilization_percent, 0);

        // Add some graphemes
        for i in 0..50 {
            pool.alloc(&format!("g{i}"));
        }

        let stats = pool.stats();
        assert_eq!(stats.total_slots, 50);
        assert_eq!(stats.active_slots, 50);
        assert_eq!(stats.free_slots, 0);
        assert_eq!(stats.utilization_percent, 50);
    }

    #[test]
    fn test_utilization_percent() {
        let mut pool = GraphemePool::with_soft_limit(100);

        // 0% utilization
        assert_eq!(pool.utilization_percent(), 0);

        // 10% utilization
        for i in 0..10 {
            pool.alloc(&format!("g{i}"));
        }
        assert_eq!(pool.utilization_percent(), 10);

        // 80% utilization
        for i in 10..80 {
            pool.alloc(&format!("g{i}"));
        }
        assert_eq!(pool.utilization_percent(), 80);
    }

    #[test]
    fn test_is_high_utilization() {
        let mut pool = GraphemePool::with_soft_limit(100);

        // Under 80% - not high
        for i in 0..79 {
            pool.alloc(&format!("g{i}"));
        }
        assert!(!pool.is_high_utilization());

        // At 80% - high
        pool.alloc("g79");
        assert!(pool.is_high_utilization());

        // Over 80% - still high
        pool.alloc("g80");
        assert!(pool.is_high_utilization());
    }

    #[test]
    fn test_is_above_utilization() {
        let mut pool = GraphemePool::with_soft_limit(100);

        for i in 0..90 {
            pool.alloc(&format!("g{i}"));
        }

        assert!(pool.is_above_utilization(80));
        assert!(pool.is_above_utilization(90));
        assert!(!pool.is_above_utilization(91));
        assert!(!pool.is_above_utilization(95));
    }

    #[test]
    fn test_try_alloc_respects_soft_limit() {
        let mut pool = GraphemePool::with_soft_limit(10);

        // Can allocate up to soft limit
        for i in 0..10 {
            let result = pool.try_alloc(&format!("g{i}"));
            assert!(result.is_some(), "should be able to allocate g{i}");
        }

        // At soft limit, try_alloc returns None
        let result = pool.try_alloc("overflow");
        assert!(result.is_none(), "should fail when at soft limit");

        // But if we free a slot, we can allocate again (reuses free slot)
        let id = pool.intern("g0");
        pool.decref(id); // refcount 2 -> 1
        pool.decref(id); // refcount 1 -> 0, freed

        let result = pool.try_alloc("reuse");
        assert!(result.is_some(), "should reuse freed slot");
    }

    #[test]
    fn test_try_intern_existing_always_succeeds() {
        let mut pool = GraphemePool::with_soft_limit(5);

        // Fill to capacity
        for i in 0..5 {
            let _ = pool.alloc(&format!("g{i}"));
        }

        // try_alloc would fail
        assert!(pool.try_alloc("new").is_none());

        // But try_intern of existing grapheme should succeed
        let existing = pool.try_intern("g0");
        assert!(existing.is_some());
        assert_eq!(pool.refcount(existing.unwrap()), 2);
    }

    #[test]
    fn test_try_intern_new_respects_limit() {
        let mut pool = GraphemePool::with_soft_limit(5);

        // Fill to capacity
        for i in 0..5 {
            let _ = pool.alloc(&format!("g{i}"));
        }

        // try_intern of new grapheme should fail
        let new = pool.try_intern("totally_new");
        assert!(new.is_none());
    }

    #[test]
    fn test_pool_stats_is_above_threshold() {
        let stats = PoolStats {
            total_slots: 100,
            active_slots: 85,
            free_slots: 15,
            soft_limit: 100,
            utilization_percent: 85,
        };

        assert!(stats.is_above_threshold(80));
        assert!(stats.is_above_threshold(85));
        assert!(!stats.is_above_threshold(86));
        assert!(!stats.is_above_threshold(90));
    }

    #[test]
    fn test_utilization_can_exceed_100_percent() {
        let mut pool = GraphemePool::with_soft_limit(10);

        // Allocate 15 entries (exceeds soft limit via regular alloc)
        for i in 0..15 {
            let _ = pool.alloc(&format!("g{i}"));
        }

        // Utilization should be 150%
        assert_eq!(pool.utilization_percent(), 150);
        assert!(pool.is_high_utilization());
    }

    #[test]
    fn test_get_fragmentation_ratio_empty_pool() {
        let pool = GraphemePool::new();
        assert_eq!(pool.get_fragmentation_ratio(), 0.0);
    }

    #[test]
    fn test_get_fragmentation_ratio_no_freed_slots() {
        let mut pool = GraphemePool::new();
        let _ = pool.alloc("a");
        let _ = pool.alloc("b");
        let _ = pool.alloc("c");

        // No freed slots, ratio should be 0.0
        assert_eq!(pool.get_fragmentation_ratio(), 0.0);
    }

    #[test]
    fn test_get_fragmentation_ratio_half_freed() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("a");
        let _ = pool.alloc("b");

        // Free one of two slots
        pool.decref(id1);

        // 1 freed / 2 total = 0.5
        assert!((pool.get_fragmentation_ratio() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_fragmentation_ratio_all_freed() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("a");
        let id2 = pool.alloc("b");
        let id3 = pool.alloc("c");

        // Free all slots
        pool.decref(id1);
        pool.decref(id2);
        pool.decref(id3);

        // 3 freed / 3 total = 1.0
        assert!((pool.get_fragmentation_ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_get_fragmentation_ratio_after_reuse() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("a");
        let _ = pool.alloc("b");

        // Free one slot
        pool.decref(id1);
        assert!((pool.get_fragmentation_ratio() - 0.5).abs() < f32::EPSILON);

        // Allocate again - should reuse the freed slot
        let _ = pool.alloc("c");

        // Now no freed slots
        assert_eq!(pool.get_fragmentation_ratio(), 0.0);
    }
}
