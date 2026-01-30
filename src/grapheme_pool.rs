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

/// Minimum fragmentation ratio to consider compaction (50%).
pub const COMPACTION_FRAGMENTATION_THRESHOLD: f32 = 0.5;

/// Minimum pool size to consider compaction worthwhile.
pub const COMPACTION_MIN_SLOTS: usize = 1000;

/// Result of a pool compaction operation.
///
/// Contains the remapping from old IDs to new IDs, which callers must use
/// to update their stored [`GraphemeId`] references. Any ID not present in
/// `old_to_new` was either invalid or freed before compaction.
#[derive(Clone, Debug, Default)]
pub struct CompactionResult {
    /// Mapping from old pool IDs to new pool IDs.
    ///
    /// Callers must iterate through their stored IDs and remap them:
    /// ```ignore
    /// for id in buffer.grapheme_ids_mut() {
    ///     if let Some(&new_id) = result.old_to_new.get(&id.pool_id()) {
    ///         *id = GraphemeId::new(new_id, id.width());
    ///     }
    /// }
    /// ```
    pub old_to_new: HashMap<u32, u32>,
    /// Number of free slots removed during compaction.
    pub slots_freed: usize,
    /// Estimated bytes saved by compaction.
    pub bytes_saved: usize,
}

impl CompactionResult {
    /// Check if any IDs were remapped.
    #[must_use]
    pub fn has_remappings(&self) -> bool {
        !self.old_to_new.is_empty()
    }

    /// Remap a pool ID to its new value, if it changed.
    ///
    /// Returns `Some(new_id)` if the ID was remapped, `None` if it wasn't
    /// (either because it didn't exist or because compaction didn't occur).
    #[must_use]
    pub fn remap(&self, old_id: u32) -> Option<u32> {
        self.old_to_new.get(&old_id).copied()
    }
}

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

    /// Iterate over all active (non-freed) entries in the pool.
    ///
    /// Yields `(pool_id, grapheme)` pairs for each allocated slot that has not been freed.
    /// Skips slot 0 (reserved/invalid) and any freed slots.
    ///
    /// This is useful for debugging, serialization, and pool inspection.
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    /// let _id1 = pool.alloc("alpha");
    /// let id2 = pool.alloc("beta");
    /// let _id3 = pool.alloc("gamma");
    /// pool.decref(id2); // Free "beta"
    ///
    /// let active: Vec<_> = pool.iter_active().collect();
    /// assert_eq!(active.len(), 2);
    /// assert!(active.iter().any(|(_, s)| *s == "alpha"));
    /// assert!(active.iter().any(|(_, s)| *s == "gamma"));
    /// ```
    pub fn iter_active(&self) -> impl Iterator<Item = (u32, &str)> {
        self.slots
            .iter()
            .enumerate()
            .skip(1) // Skip reserved slot 0
            .filter_map(|(idx, slot)| {
                if slot.is_free() {
                    None
                } else {
                    Some((idx as u32, slot.bytes.as_str()))
                }
            })
    }

    /// Check if the pool should be compacted based on fragmentation and size.
    ///
    /// Returns `true` if compaction would be beneficial. The heuristic considers:
    /// - Fragmentation ratio > 50% (more than half of allocated slots are freed)
    /// - Pool size > 1000 slots (compaction overhead is worth it for larger pools)
    ///
    /// Small pools or lightly fragmented pools return `false` since the overhead
    /// of compaction would outweigh the benefits.
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    ///
    /// // Small pool - not worth compacting even if fragmented
    /// let small_ids: Vec<_> = (0..10).map(|i| pool.alloc(&format!("g{i}"))).collect();
    /// for id in &small_ids {
    ///     pool.decref(*id);
    /// }
    /// assert!(!pool.should_compact()); // Too small
    ///
    /// // Large fragmented pool - should compact
    /// // First allocate all entries, THEN free some (to avoid slot reuse)
    /// let mut pool = GraphemePool::new();
    /// let ids: Vec<_> = (0..2000).map(|i| pool.alloc(&format!("g{i}"))).collect();
    /// for (i, id) in ids.iter().enumerate() {
    ///     if i % 3 != 0 {
    ///         pool.decref(*id); // Free ~66%
    ///     }
    /// }
    /// assert!(pool.should_compact()); // Large and >50% fragmented
    /// ```
    #[must_use]
    pub fn should_compact(&self) -> bool {
        let ratio = self.get_fragmentation_ratio();
        let size = self.total_slots();
        ratio > COMPACTION_FRAGMENTATION_THRESHOLD && size > COMPACTION_MIN_SLOTS
    }

    /// Increment reference counts for multiple pool IDs.
    ///
    /// This is more efficient than calling [`incref()`](Self::incref) individually
    /// when copying regions between buffers. Invalid IDs (including ID 0 and
    /// IDs for freed slots) are silently skipped.
    ///
    /// # Arguments
    ///
    /// * `ids` - Slice of pool IDs to increment references for
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    /// let id1 = pool.alloc("alpha");
    /// let id2 = pool.alloc("beta");
    ///
    /// // Clone both references at once
    /// pool.clone_batch(&[id1.pool_id(), id2.pool_id()]);
    ///
    /// assert_eq!(pool.refcount(id1), 2);
    /// assert_eq!(pool.refcount(id2), 2);
    /// ```
    pub fn clone_batch(&mut self, ids: &[u32]) {
        for &pool_id in ids {
            self.incref_by_pool_id(pool_id);
        }
    }

    /// Decrement reference counts for multiple pool IDs.
    ///
    /// This is more efficient than calling [`decref()`](Self::decref) individually
    /// when clearing regions or buffers. Invalid IDs (including ID 0 and
    /// IDs for already-freed slots) are silently skipped.
    ///
    /// # Arguments
    ///
    /// * `ids` - Slice of pool IDs to decrement references for
    ///
    /// # Returns
    ///
    /// The number of entries that were actually freed (refcount went to 0).
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    /// let id1 = pool.alloc("alpha");
    /// let id2 = pool.alloc("beta");
    ///
    /// // Free both at once
    /// let freed = pool.free_batch(&[id1.pool_id(), id2.pool_id()]);
    ///
    /// assert_eq!(freed, 2);
    /// assert!(!pool.is_valid(id1));
    /// assert!(!pool.is_valid(id2));
    /// ```
    pub fn free_batch(&mut self, ids: &[u32]) -> usize {
        let mut freed_count = 0;
        for &pool_id in ids {
            // Check if this ID was valid before decrementing
            let was_valid = self
                .slots
                .get(pool_id as usize)
                .is_some_and(|slot| slot.refcount > 0);

            if was_valid && !self.decref_by_pool_id(pool_id) {
                // Was valid and now freed (refcount went to 0)
                freed_count += 1;
            }
        }
        freed_count
    }

    /// Allocate multiple graphemes at once.
    ///
    /// This is more efficient than calling [`alloc()`](Self::alloc) individually
    /// as it can pre-size internal structures. Like `alloc()`, this does NOT
    /// deduplicate - each grapheme gets its own slot even if duplicates exist.
    /// Use [`intern()`](Self::intern) if you want deduplication.
    ///
    /// # Arguments
    ///
    /// * `graphemes` - Slice of grapheme strings to allocate
    ///
    /// # Returns
    ///
    /// A vector of [`GraphemeId`]s in the same order as the input.
    ///
    /// # Panics
    ///
    /// Panics if the pool would exceed 16M entries.
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    ///
    /// let ids = pool.alloc_batch(&["alpha", "beta", "gamma"]);
    ///
    /// assert_eq!(ids.len(), 3);
    /// assert_eq!(pool.get(ids[0]), Some("alpha"));
    /// assert_eq!(pool.get(ids[1]), Some("beta"));
    /// assert_eq!(pool.get(ids[2]), Some("gamma"));
    /// ```
    #[must_use]
    pub fn alloc_batch(&mut self, graphemes: &[&str]) -> Vec<GraphemeId> {
        // Pre-allocate result vector
        let mut result = Vec::with_capacity(graphemes.len());

        for &grapheme in graphemes {
            result.push(self.alloc(grapheme));
        }

        result
    }

    /// Get an estimate of total memory used by the pool in bytes.
    ///
    /// This includes:
    /// - The slots vector (stack + heap)
    /// - String heap allocations for each grapheme
    /// - The free list vector
    /// - The deduplication index HashMap (estimated)
    ///
    /// Note: This is an estimate. Actual memory usage may differ due to
    /// allocator overhead, alignment, and HashMap implementation details.
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    /// let empty_usage = pool.get_memory_usage();
    ///
    /// pool.alloc("hello");
    /// pool.alloc("world");
    ///
    /// let usage_with_data = pool.get_memory_usage();
    /// assert!(usage_with_data > empty_usage);
    /// ```
    #[must_use]
    pub fn get_memory_usage(&self) -> usize {
        // Size of Slot struct on the stack (String + u32 + u8 + padding)
        let slot_size = std::mem::size_of::<Slot>();

        // slots Vec: capacity * slot_size (heap portion)
        let slots_heap = self.slots.capacity() * slot_size;

        // Each Slot's String heap allocation (capacity bytes)
        let string_heap: usize = self.slots.iter().map(|slot| slot.bytes.capacity()).sum();

        // free_list Vec: capacity * sizeof(u32)
        let free_list_heap = self.free_list.capacity() * std::mem::size_of::<u32>();

        // index HashMap: rough estimate
        // Each entry is approximately: String (24 bytes) + key heap + u32 value + overhead
        // Conservatively estimate ~64 bytes per entry for HashMap overhead
        let index_overhead = self.index.len() * 64;
        let index_key_heap: usize = self.index.keys().map(String::len).sum();

        // Stack sizes of the struct fields themselves
        let stack_size = std::mem::size_of::<GraphemePool>();

        stack_size + slots_heap + string_heap + free_list_heap + index_overhead + index_key_heap
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

    /// Compact the pool by removing gaps from freed slots.
    ///
    /// This defragments the pool by creating new contiguous storage containing
    /// only active entries. All existing pool IDs become invalid and must be
    /// remapped using the returned [`CompactionResult::old_to_new`] mapping.
    ///
    /// # Important
    ///
    /// **All code holding [`GraphemeId`] references must update them after compaction!**
    /// Failure to do so will result in incorrect lookups or panics.
    ///
    /// # When to Compact
    ///
    /// Use [`should_compact()`](Self::should_compact) to check if compaction would be beneficial.
    /// Compaction is most useful when:
    /// - Fragmentation is high (>50% of slots are freed)
    /// - The pool is large (>1000 slots)
    /// - Memory pressure is a concern
    ///
    /// # Returns
    ///
    /// A [`CompactionResult`] containing:
    /// - `old_to_new`: Mapping from old pool IDs to new pool IDs
    /// - `slots_freed`: Number of free slots removed
    /// - `bytes_saved`: Estimated memory reclaimed
    ///
    /// # Example
    ///
    /// ```
    /// use opentui::grapheme_pool::GraphemePool;
    ///
    /// let mut pool = GraphemePool::new();
    ///
    /// // Allocate some entries
    /// let id1 = pool.alloc("alpha");
    /// let id2 = pool.alloc("beta");
    /// let id3 = pool.alloc("gamma");
    ///
    /// // Free the middle one - creates a gap
    /// pool.decref(id2);
    ///
    /// // Now id1 is at slot 1, id3 is at slot 3 (gap at 2)
    /// assert_eq!(id1.pool_id(), 1);
    /// assert_eq!(id3.pool_id(), 3);
    ///
    /// // Compact to remove the gap
    /// let result = pool.compact();
    ///
    /// // Slots are now contiguous: 0 (reserved), 1 (alpha), 2 (gamma)
    /// assert_eq!(result.slots_freed, 1);
    ///
    /// // Remap the IDs
    /// let new_id1 = result.remap(id1.pool_id()).unwrap_or(id1.pool_id());
    /// let new_id3 = result.remap(id3.pool_id()).unwrap_or(id3.pool_id());
    ///
    /// // Verify the remapped IDs work
    /// use opentui::cell::GraphemeId;
    /// let remapped1 = GraphemeId::new(new_id1, id1.width());
    /// let remapped3 = GraphemeId::new(new_id3, id3.width());
    ///
    /// assert_eq!(pool.get(remapped1), Some("alpha"));
    /// assert_eq!(pool.get(remapped3), Some("gamma"));
    /// ```
    #[must_use]
    pub fn compact(&mut self) -> CompactionResult {
        // Early exit if nothing to compact
        if self.free_list.is_empty() {
            return CompactionResult::default();
        }

        let slots_freed = self.free_list.len();

        // Estimate bytes saved: average slot size * freed count
        // This is approximate since we're clearing String heap allocations
        let avg_slot_heap: usize = self
            .slots
            .iter()
            .skip(1)
            .filter(|s| !s.is_free())
            .map(|s| s.bytes.capacity())
            .sum::<usize>()
            .checked_div(self.active_count())
            .unwrap_or(0);
        let bytes_saved = slots_freed * (std::mem::size_of::<Slot>() + avg_slot_heap);

        // Build new compact storage
        let active_count = self.active_count();
        let mut new_slots = Vec::with_capacity(active_count + 1);
        let mut old_to_new = HashMap::with_capacity(active_count);
        let mut new_index = HashMap::with_capacity(active_count);

        // Keep reserved slot 0
        new_slots.push(Slot {
            bytes: String::new(),
            refcount: 0,
            width: 0,
        });

        // Copy active slots to new contiguous positions
        for (old_id, slot) in self.slots.iter().enumerate().skip(1) {
            if slot.is_free() {
                continue;
            }

            let new_id = new_slots.len() as u32;
            old_to_new.insert(old_id as u32, new_id);
            new_index.insert(slot.bytes.clone(), new_id);
            new_slots.push(slot.clone());
        }

        // Swap in new data structures
        self.slots = new_slots;
        self.free_list.clear();
        self.index = new_index;

        CompactionResult {
            old_to_new,
            slots_freed,
            bytes_saved,
        }
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

    #[test]
    fn test_iter_active_empty_pool() {
        let pool = GraphemePool::new();
        let active: Vec<_> = pool.iter_active().collect();
        assert!(active.is_empty());
    }

    #[test]
    fn test_iter_active_all_entries() {
        let mut pool = GraphemePool::new();
        let _ = pool.alloc("alpha");
        let _ = pool.alloc("beta");
        let _ = pool.alloc("gamma");

        let active: Vec<_> = pool.iter_active().collect();
        assert_eq!(active.len(), 3);

        // Verify IDs start at 1 (slot 0 reserved)
        let ids: Vec<_> = active.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));

        // Verify graphemes
        let graphemes: Vec<_> = active.iter().map(|(_, s)| *s).collect();
        assert!(graphemes.contains(&"alpha"));
        assert!(graphemes.contains(&"beta"));
        assert!(graphemes.contains(&"gamma"));
    }

    #[test]
    fn test_iter_active_skips_freed() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");
        let id3 = pool.alloc("gamma");

        // Free the middle one
        pool.decref(id2);

        let active: Vec<_> = pool.iter_active().collect();
        assert_eq!(active.len(), 2);

        // Should have alpha and gamma, not beta
        let graphemes: Vec<_> = active.iter().map(|(_, s)| *s).collect();
        assert!(graphemes.contains(&"alpha"));
        assert!(!graphemes.contains(&"beta"));
        assert!(graphemes.contains(&"gamma"));

        // IDs should match
        assert!(active.iter().any(|(id, _)| *id == id1.pool_id()));
        assert!(active.iter().any(|(id, _)| *id == id3.pool_id()));
    }

    #[test]
    fn test_iter_active_ids_match_get() {
        let mut pool = GraphemePool::new();
        let _ = pool.alloc("one");
        let _ = pool.alloc("two");
        let id3 = pool.alloc("three");
        pool.decref(id3);

        // Verify that for each (id, grapheme) pair, pool.get_by_pool_id(id) == Some(grapheme)
        for (id, grapheme) in pool.iter_active() {
            assert_eq!(pool.get_by_pool_id(id), Some(grapheme));
        }
    }

    #[test]
    fn test_iter_active_after_reuse() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("old");
        pool.decref(id1);

        // This should reuse slot 1
        let _ = pool.alloc("new");

        let active: Vec<_> = pool.iter_active().collect();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0], (1, "new"));
    }

    #[test]
    fn test_should_compact_empty_pool() {
        let pool = GraphemePool::new();
        assert!(!pool.should_compact());
    }

    #[test]
    fn test_should_compact_small_fragmented_pool() {
        let mut pool = GraphemePool::new();

        // Allocate 10 entries and free all of them - 100% fragmented but small
        for i in 0..10 {
            let id = pool.alloc(&format!("g{i}"));
            pool.decref(id);
        }

        // Should return false because pool is too small
        assert!(!pool.should_compact());
        assert!((pool.get_fragmentation_ratio() - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_should_compact_large_unfragmented_pool() {
        let mut pool = GraphemePool::new();

        // Allocate 2000 entries but don't free any
        for i in 0..2000 {
            let _ = pool.alloc(&format!("g{i}"));
        }

        // Should return false because pool is not fragmented
        assert!(!pool.should_compact());
        assert_eq!(pool.get_fragmentation_ratio(), 0.0);
    }

    #[test]
    fn test_should_compact_large_fragmented_pool() {
        let mut pool = GraphemePool::new();

        // First allocate all 2000 entries
        let ids: Vec<_> = (0..2000).map(|i| pool.alloc(&format!("g{i}"))).collect();

        // Then free more than half (free every entry where index % 3 != 0)
        for (i, id) in ids.iter().enumerate() {
            if i % 3 != 0 {
                pool.decref(*id);
            }
        }

        // Should return true: large (2000 slots) and >50% fragmented (~66% freed)
        assert!(pool.should_compact());
        assert!(pool.get_fragmentation_ratio() > 0.5);
    }

    #[test]
    fn test_should_compact_threshold_boundary() {
        let mut pool = GraphemePool::new();

        // Allocate exactly 1001 entries and free exactly 501 (just over 50%)
        let mut ids = Vec::new();
        for i in 0..1001 {
            ids.push(pool.alloc(&format!("g{i}")));
        }

        // Free 501 entries (>50%)
        for id in ids.iter().take(501) {
            pool.decref(*id);
        }

        // Should return true: size > 1000 and fragmentation > 50%
        assert!(pool.should_compact());
        assert!(pool.total_slots() > 1000);
        assert!(pool.get_fragmentation_ratio() > 0.5);
    }

    #[test]
    fn test_clone_batch_empty() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Clone empty batch - should be no-op
        pool.clone_batch(&[]);

        assert_eq!(pool.refcount(id), 1);
    }

    #[test]
    fn test_clone_batch_valid_ids() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");
        let id3 = pool.alloc("gamma");

        // Clone all three
        pool.clone_batch(&[id1.pool_id(), id2.pool_id(), id3.pool_id()]);

        assert_eq!(pool.refcount(id1), 2);
        assert_eq!(pool.refcount(id2), 2);
        assert_eq!(pool.refcount(id3), 2);
    }

    #[test]
    fn test_clone_batch_skips_id_zero() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Clone with ID 0 (invalid) - should skip it
        pool.clone_batch(&[0, id.pool_id()]);

        // Only the valid ID should be incremented
        assert_eq!(pool.refcount(id), 2);
    }

    #[test]
    fn test_clone_batch_skips_invalid_ids() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Clone with out-of-range ID - should skip it
        pool.clone_batch(&[9999, id.pool_id(), 12345]);

        // Only the valid ID should be incremented
        assert_eq!(pool.refcount(id), 2);
    }

    #[test]
    fn test_clone_batch_skips_freed_ids() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");

        // Free id1
        pool.decref(id1);

        // Clone both - id1 should be skipped
        pool.clone_batch(&[id1.pool_id(), id2.pool_id()]);

        // id1 is freed, should still be 0
        assert_eq!(pool.refcount(id1), 0);
        // id2 should be incremented
        assert_eq!(pool.refcount(id2), 2);
    }

    #[test]
    fn test_clone_batch_duplicate_ids() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Clone same ID multiple times
        pool.clone_batch(&[id.pool_id(), id.pool_id(), id.pool_id()]);

        // Should increment by 3
        assert_eq!(pool.refcount(id), 4);
    }

    #[test]
    fn test_free_batch_empty() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Free empty batch - should be no-op
        let freed = pool.free_batch(&[]);

        assert_eq!(freed, 0);
        assert_eq!(pool.refcount(id), 1);
    }

    #[test]
    fn test_free_batch_valid_ids() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");
        let id3 = pool.alloc("gamma");

        // Free all three
        let freed = pool.free_batch(&[id1.pool_id(), id2.pool_id(), id3.pool_id()]);

        assert_eq!(freed, 3);
        assert!(!pool.is_valid(id1));
        assert!(!pool.is_valid(id2));
        assert!(!pool.is_valid(id3));
    }

    #[test]
    fn test_free_batch_skips_id_zero() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Free with ID 0 (invalid) - should skip it
        let freed = pool.free_batch(&[0, id.pool_id()]);

        assert_eq!(freed, 1);
        assert!(!pool.is_valid(id));
    }

    #[test]
    fn test_free_batch_skips_invalid_ids() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");

        // Free with out-of-range IDs - should skip them
        let freed = pool.free_batch(&[9999, id.pool_id(), 12345]);

        assert_eq!(freed, 1);
        assert!(!pool.is_valid(id));
    }

    #[test]
    fn test_free_batch_skips_freed_ids() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");

        // Free id1 first
        pool.decref(id1);

        // Free both - id1 should be skipped (already freed)
        let freed = pool.free_batch(&[id1.pool_id(), id2.pool_id()]);

        // Only id2 was actually freed
        assert_eq!(freed, 1);
    }

    #[test]
    fn test_free_batch_with_multiple_refs() {
        let mut pool = GraphemePool::new();
        let id = pool.alloc("test");
        pool.incref(id);
        pool.incref(id);

        // Refcount is 3, free once should decrement but not free
        let freed = pool.free_batch(&[id.pool_id()]);

        assert_eq!(freed, 0); // Not freed yet
        assert_eq!(pool.refcount(id), 2);

        // Free twice more
        let freed = pool.free_batch(&[id.pool_id(), id.pool_id()]);

        assert_eq!(freed, 1); // Now freed
        assert!(!pool.is_valid(id));
    }

    #[test]
    fn test_alloc_batch_empty() {
        let mut pool = GraphemePool::new();

        let ids = pool.alloc_batch(&[]);

        assert!(ids.is_empty());
        assert_eq!(pool.active_count(), 0);
    }

    #[test]
    fn test_alloc_batch_single() {
        let mut pool = GraphemePool::new();

        let ids = pool.alloc_batch(&["single"]);

        assert_eq!(ids.len(), 1);
        assert_eq!(pool.get(ids[0]), Some("single"));
    }

    #[test]
    fn test_alloc_batch_multiple() {
        let mut pool = GraphemePool::new();

        let ids = pool.alloc_batch(&["alpha", "beta", "gamma", "delta"]);

        assert_eq!(ids.len(), 4);
        assert_eq!(pool.active_count(), 4);

        // Order should be preserved
        assert_eq!(pool.get(ids[0]), Some("alpha"));
        assert_eq!(pool.get(ids[1]), Some("beta"));
        assert_eq!(pool.get(ids[2]), Some("gamma"));
        assert_eq!(pool.get(ids[3]), Some("delta"));
    }

    #[test]
    fn test_alloc_batch_with_duplicates() {
        let mut pool = GraphemePool::new();

        // Unlike intern, alloc_batch should NOT deduplicate
        let ids = pool.alloc_batch(&["dup", "dup", "dup"]);

        assert_eq!(ids.len(), 3);
        assert_eq!(pool.active_count(), 3);

        // Each should be a different slot
        assert_ne!(ids[0].pool_id(), ids[1].pool_id());
        assert_ne!(ids[1].pool_id(), ids[2].pool_id());

        // But all should retrieve the same string
        assert_eq!(pool.get(ids[0]), Some("dup"));
        assert_eq!(pool.get(ids[1]), Some("dup"));
        assert_eq!(pool.get(ids[2]), Some("dup"));
    }

    #[test]
    fn test_alloc_batch_preserves_width() {
        let mut pool = GraphemePool::new();

        let ids = pool.alloc_batch(&["A", "👍", "世"]);

        // ASCII has width 1
        assert_eq!(ids[0].width(), 1);
        // Emoji has width 2
        assert_eq!(ids[1].width(), 2);
        // CJK has width 2
        assert_eq!(ids[2].width(), 2);
    }

    #[test]
    fn test_alloc_batch_reuses_freed_slots() {
        let mut pool = GraphemePool::new();

        // Allocate and free some slots
        let old_ids = pool.alloc_batch(&["old1", "old2", "old3"]);
        for id in &old_ids {
            pool.decref(*id);
        }

        // Now allocate new batch - should reuse freed slots
        let new_ids = pool.alloc_batch(&["new1", "new2"]);

        // Pool should not have grown beyond original size
        assert_eq!(pool.total_slots(), 3);
        assert_eq!(pool.active_count(), 2);
        assert_eq!(pool.free_count(), 1);

        // New graphemes should be retrievable
        assert_eq!(pool.get(new_ids[0]), Some("new1"));
        assert_eq!(pool.get(new_ids[1]), Some("new2"));
    }

    #[test]
    fn test_get_memory_usage_empty_pool() {
        let pool = GraphemePool::new();
        let usage = pool.get_memory_usage();

        // Even empty pool has some baseline memory (struct, reserved slot 0, etc.)
        assert!(usage > 0);
    }

    #[test]
    fn test_get_memory_usage_increases_with_data() {
        let mut pool = GraphemePool::new();
        let empty_usage = pool.get_memory_usage();

        // Add some graphemes
        let _ = pool.alloc("hello");
        let _ = pool.alloc("world");
        let _ = pool.alloc("this is a longer string");

        let usage_with_data = pool.get_memory_usage();

        // Memory should increase
        assert!(usage_with_data > empty_usage);
    }

    #[test]
    fn test_get_memory_usage_accounts_for_string_length() {
        let mut pool1 = GraphemePool::new();
        let mut pool2 = GraphemePool::new();

        // Pool 1: short strings
        for i in 0..10 {
            let _ = pool1.alloc(&format!("{i}"));
        }

        // Pool 2: long strings
        for i in 0..10 {
            let _ = pool2.alloc(&format!("this_is_a_much_longer_string_{i}"));
        }

        // Pool 2 should use more memory due to longer strings
        assert!(pool2.get_memory_usage() > pool1.get_memory_usage());
    }

    #[test]
    fn test_get_memory_usage_includes_free_list() {
        let mut pool = GraphemePool::new();

        // Allocate then free to populate free list
        let ids: Vec<_> = (0..100).map(|i| pool.alloc(&format!("g{i}"))).collect();
        let usage_before_free = pool.get_memory_usage();

        for id in ids {
            pool.decref(id);
        }

        let usage_after_free = pool.get_memory_usage();

        // Usage shouldn't drop dramatically since freed slots are kept
        // (free list grows, but strings are cleared)
        // The difference should be positive (strings were cleared)
        // but memory is still allocated for the slots
        assert!(usage_before_free > 0);
        assert!(usage_after_free > 0);
    }

    // ========== compact() tests ==========

    #[test]
    fn test_compact_empty_pool() {
        let mut pool = GraphemePool::new();
        let result = pool.compact();

        assert!(result.old_to_new.is_empty());
        assert_eq!(result.slots_freed, 0);
        assert_eq!(result.bytes_saved, 0);
        assert!(!result.has_remappings());
    }

    #[test]
    fn test_compact_no_free_slots() {
        let mut pool = GraphemePool::new();
        let _ = pool.alloc("a");
        let _ = pool.alloc("b");
        let _ = pool.alloc("c");

        let result = pool.compact();

        // No free slots means nothing to compact
        assert!(result.old_to_new.is_empty());
        assert_eq!(result.slots_freed, 0);
        assert!(!result.has_remappings());

        // Pool should be unchanged
        assert_eq!(pool.total_slots(), 3);
        assert_eq!(pool.active_count(), 3);
    }

    #[test]
    fn test_compact_single_gap() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");
        let id3 = pool.alloc("gamma");

        // Free the middle one to create a gap
        pool.decref(id2);

        // Before compact: slots at 1, 3 with gap at 2
        assert_eq!(pool.total_slots(), 3);
        assert_eq!(pool.active_count(), 2);
        assert_eq!(pool.free_count(), 1);

        let result = pool.compact();

        // After compact: slots at 1, 2 with no gaps
        assert_eq!(result.slots_freed, 1);
        assert!(result.has_remappings());
        assert_eq!(pool.total_slots(), 2);
        assert_eq!(pool.active_count(), 2);
        assert_eq!(pool.free_count(), 0);

        // Verify remapping
        let new_id1 = result.remap(id1.pool_id()).unwrap_or(id1.pool_id());
        let new_id3 = result.remap(id3.pool_id()).unwrap_or(id3.pool_id());

        // id1 was at slot 1, should stay at slot 1
        assert_eq!(new_id1, 1);
        // id3 was at slot 3, should now be at slot 2
        assert_eq!(new_id3, 2);

        // Verify data is accessible via remapped IDs
        let remapped1 = GraphemeId::new(new_id1, id1.width() as u8);
        let remapped3 = GraphemeId::new(new_id3, id3.width() as u8);

        assert_eq!(pool.get(remapped1), Some("alpha"));
        assert_eq!(pool.get(remapped3), Some("gamma"));
    }

    #[test]
    fn test_compact_multiple_gaps() {
        let mut pool = GraphemePool::new();
        let ids: Vec<_> = (0..10).map(|i| pool.alloc(&format!("g{i}"))).collect();

        // Free every other slot (creating 5 gaps)
        for (i, id) in ids.iter().enumerate() {
            if i % 2 == 1 {
                pool.decref(*id);
            }
        }

        // Before compact: 10 slots, 5 active, 5 free
        assert_eq!(pool.total_slots(), 10);
        assert_eq!(pool.active_count(), 5);
        assert_eq!(pool.free_count(), 5);

        let result = pool.compact();

        // After compact: 5 slots, 5 active, 0 free
        assert_eq!(result.slots_freed, 5);
        assert_eq!(pool.total_slots(), 5);
        assert_eq!(pool.active_count(), 5);
        assert_eq!(pool.free_count(), 0);

        // Verify all surviving entries are accessible
        for (i, id) in ids.iter().enumerate() {
            if i % 2 == 0 {
                let new_pool_id = result.remap(id.pool_id()).unwrap_or(id.pool_id());
                let remapped = GraphemeId::new(new_pool_id, id.width() as u8);
                assert_eq!(pool.get(remapped), Some(format!("g{i}").as_str()));
            }
        }
    }

    #[test]
    fn test_compact_all_freed() {
        let mut pool = GraphemePool::new();
        let ids: Vec<_> = (0..5).map(|i| pool.alloc(&format!("g{i}"))).collect();

        // Free all slots
        for id in &ids {
            pool.decref(*id);
        }

        // Before compact: 5 slots, 0 active, 5 free
        assert_eq!(pool.total_slots(), 5);
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.free_count(), 5);

        let result = pool.compact();

        // After compact: 0 slots (just reserved slot 0)
        assert_eq!(result.slots_freed, 5);
        assert_eq!(pool.total_slots(), 0);
        assert_eq!(pool.active_count(), 0);
        assert_eq!(pool.free_count(), 0);

        // old_to_new should be empty (nothing to remap)
        assert!(result.old_to_new.is_empty());
    }

    #[test]
    fn test_compact_preserves_refcounts() {
        let mut pool = GraphemePool::new();
        let id1 = pool.alloc("alpha");
        pool.incref(id1);
        pool.incref(id1);

        let id2 = pool.alloc("beta");
        pool.decref(id2);

        let id3 = pool.alloc("gamma");

        // id1 has refcount 3, id2 freed (gap), id3 has refcount 1
        assert_eq!(pool.refcount(id1), 3);
        assert_eq!(pool.refcount(id3), 1);

        let result = pool.compact();

        // Remap and verify refcounts are preserved
        let new_id1 = result.remap(id1.pool_id()).unwrap_or(id1.pool_id());
        let new_id3 = result.remap(id3.pool_id()).unwrap_or(id3.pool_id());

        let remapped1 = GraphemeId::new(new_id1, id1.width());
        let remapped3 = GraphemeId::new(new_id3, id3.width());

        assert_eq!(pool.refcount(remapped1), 3);
        assert_eq!(pool.refcount(remapped3), 1);
    }

    #[test]
    fn test_compact_preserves_widths() {
        let mut pool = GraphemePool::new();

        // Allocate entries with different widths
        let id1 = pool.alloc("A"); // width 1
        let id2 = pool.alloc("👍"); // width 2
        let id3 = pool.alloc("世"); // width 2

        // Free the middle one
        pool.decref(id2);

        let result = pool.compact();

        let new_id1 = result.remap(id1.pool_id()).unwrap_or(id1.pool_id());
        let new_id3 = result.remap(id3.pool_id()).unwrap_or(id3.pool_id());

        // Original IDs had correct widths - verify they're preserved in slots
        // The width is stored in the Slot, not just the GraphemeId
        assert_eq!(pool.get_by_pool_id(new_id1), Some("A"));
        assert_eq!(pool.get_by_pool_id(new_id3), Some("世"));
    }

    #[test]
    fn test_compact_updates_index() {
        let mut pool = GraphemePool::new();

        let id1 = pool.alloc("alpha");
        let id2 = pool.alloc("beta");
        let id3 = pool.alloc("gamma");

        // Free middle one
        pool.decref(id2);

        pool.compact();

        // Index should be updated - intern should find existing entries
        let interned1 = pool.intern("alpha");
        let interned3 = pool.intern("gamma");

        // Should find existing entries (refcount increases)
        assert_eq!(pool.refcount(interned1), 2);
        assert_eq!(pool.refcount(interned3), 2);

        // And intern of "beta" should create new entry
        let interned2 = pool.intern("beta");
        assert_eq!(pool.refcount(interned2), 1);
    }

    #[test]
    fn test_compact_result_remap_helper() {
        let mut pool = GraphemePool::new();

        let id1 = pool.alloc("a");
        let id2 = pool.alloc("b");
        pool.decref(id1);

        let result = pool.compact();

        // id2 was at slot 2, now at slot 1
        assert_eq!(result.remap(id2.pool_id()), Some(1));

        // id1 was freed - not in remapping
        assert_eq!(result.remap(id1.pool_id()), None);

        // Non-existent ID
        assert_eq!(result.remap(9999), None);
    }

    #[test]
    fn test_compact_result_has_remappings() {
        let mut pool = GraphemePool::new();

        // No compaction needed
        let result1 = pool.compact();
        assert!(!result1.has_remappings());

        // Compaction with only freed slots
        let id = pool.alloc("test");
        pool.decref(id);
        let result2 = pool.compact();
        // Even though slot was freed, there's nothing to remap
        assert!(!result2.has_remappings());

        // Actual remapping needed
        let _ = pool.alloc("a");
        let id_to_free = pool.alloc("b");
        let _ = pool.alloc("c");
        pool.decref(id_to_free);
        let result3 = pool.compact();
        assert!(result3.has_remappings());
    }

    #[test]
    fn test_compact_bytes_saved_estimate() {
        let mut pool = GraphemePool::new();

        // Allocate entries with some string content
        for i in 0..100 {
            let _ = pool.alloc(&format!("grapheme_string_{i}"));
        }

        // Free half
        // We need to collect IDs first since intern would affect the test
        let ids: Vec<_> = pool.iter_active().map(|(id, _)| id).collect();
        for (i, id) in ids.iter().enumerate() {
            if i % 2 == 0 {
                pool.decref_by_pool_id(*id);
            }
        }

        let result = pool.compact();

        // bytes_saved should be positive
        assert!(result.bytes_saved > 0);
        assert_eq!(result.slots_freed, 50);
    }

    #[test]
    fn test_compact_large_pool() {
        let mut pool = GraphemePool::new();

        // Create a large fragmented pool
        let ids: Vec<_> = (0..2000).map(|i| pool.alloc(&format!("g{i}"))).collect();

        // Free ~66% of entries
        for (i, id) in ids.iter().enumerate() {
            if i % 3 != 0 {
                pool.decref(*id);
            }
        }

        assert!(pool.should_compact()); // Verify precondition

        let result = pool.compact();

        // Should have freed ~1333 slots
        assert!(result.slots_freed > 1300);
        assert!(result.slots_freed < 1400);

        // Pool should now be compact
        assert!(!pool.should_compact());
        assert_eq!(pool.free_count(), 0);
        assert_eq!(pool.active_count(), pool.total_slots());

        // Verify surviving entries
        for (i, id) in ids.iter().enumerate() {
            if i % 3 == 0 {
                let new_pool_id = result.remap(id.pool_id()).unwrap_or(id.pool_id());
                let remapped = GraphemeId::new(new_pool_id, id.width());
                assert_eq!(pool.get(remapped), Some(format!("g{i}").as_str()));
            }
        }
    }

    #[test]
    fn test_compact_then_alloc_reuses_correctly() {
        let mut pool = GraphemePool::new();

        let _ = pool.alloc("a");
        let id2 = pool.alloc("b");
        let _ = pool.alloc("c");

        pool.decref(id2);
        pool.compact();

        // After compaction, pool has 2 slots, no free list
        assert_eq!(pool.total_slots(), 2);
        assert_eq!(pool.free_count(), 0);

        // New alloc should extend the pool (no free slots to reuse)
        let id_new = pool.alloc("new");
        assert_eq!(id_new.pool_id(), 3);
        assert_eq!(pool.total_slots(), 3);
    }

    #[test]
    fn test_compact_idempotent() {
        let mut pool = GraphemePool::new();

        let _ = pool.alloc("a");
        let id2 = pool.alloc("b");
        let _ = pool.alloc("c");
        pool.decref(id2);

        // First compact
        let result1 = pool.compact();
        assert_eq!(result1.slots_freed, 1);

        // Second compact should be no-op
        let result2 = pool.compact();
        assert_eq!(result2.slots_freed, 0);
        assert!(!result2.has_remappings());
    }
}
