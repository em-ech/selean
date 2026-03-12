//! Generic bounded cache with full-clear eviction.
//!
//! Provides a HashMap-backed cache that clears all entries when capacity is
//! reached. This eviction strategy pairs well with atlas textures: when the
//! atlas is rebuilt from scratch, all cached atlas regions become invalid
//! anyway, so clearing everything is both correct and simple.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;

/// A bounded `HashMap` cache that clears all entries on overflow.
///
/// When [`insert`](BoundedCache::insert) is called and the cache is at
/// capacity, all existing entries are removed before the new entry is added.
pub struct BoundedCache<K, V> {
    entries: HashMap<K, V>,
    max_entries: usize,
}

impl<K: Eq + Hash, V> BoundedCache<K, V> {
    /// Creates a new empty cache with the given maximum capacity.
    #[must_use]
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: HashMap::new(),
            max_entries,
        }
    }

    /// Looks up a cached value by key.
    ///
    /// Accepts any borrowed form of the key type (e.g. `&str` for `String` keys)
    /// to avoid unnecessary allocations on lookup.
    #[must_use]
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.get(key)
    }

    /// Inserts a key-value pair into the cache.
    ///
    /// If the cache is at capacity, all existing entries are cleared first.
    pub fn insert(&mut self, key: K, value: V) {
        if self.entries.len() >= self.max_entries {
            self.entries.clear();
        }
        self.entries.insert(key, value);
    }

    /// Returns `true` if the cache contains the given key.
    ///
    /// Accepts any borrowed form of the key type (e.g. `&str` for `String` keys)
    /// to avoid unnecessary allocations on lookup.
    #[must_use]
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.contains_key(key)
    }

    /// Returns the number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the maximum number of entries before eviction.
    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.max_entries
    }

    /// Removes all cached entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Removes a specific entry by key. Returns `true` if it was present.
    ///
    /// Accepts any borrowed form of the key type (e.g. `&str` for `String` keys)
    /// to avoid unnecessary allocations on removal.
    pub fn remove<Q>(&mut self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.remove(key).is_some()
    }
}

impl<K: Eq + Hash, V> std::fmt::Debug for BoundedCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedCache")
            .field("count", &self.entries.len())
            .field("max_entries", &self.max_entries)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_is_empty() {
        let cache: BoundedCache<u32, String> = BoundedCache::new(100);
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert_eq!(cache.max_entries(), 100);
    }

    #[test]
    fn insert_and_get() {
        let mut cache = BoundedCache::new(100);
        cache.insert(42u32, "hello".to_string());
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get(&42), Some(&"hello".to_string()));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let cache: BoundedCache<u32, u32> = BoundedCache::new(10);
        assert!(cache.get(&99).is_none());
    }

    #[test]
    fn contains_check() {
        let mut cache = BoundedCache::new(10);
        assert!(!cache.contains(&1u32));
        cache.insert(1, "a");
        assert!(cache.contains(&1));
    }

    #[test]
    fn eviction_clears_when_full() {
        let mut cache = BoundedCache::new(3);
        cache.insert(1u32, "a");
        cache.insert(2, "b");
        cache.insert(3, "c");
        assert_eq!(cache.len(), 3);

        cache.insert(4, "d");
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&4));
        assert!(!cache.contains(&1));
    }

    #[test]
    fn insert_at_capacity_minus_one_does_not_evict() {
        let mut cache = BoundedCache::new(3);
        cache.insert(1u32, "a");
        cache.insert(2, "b");
        cache.insert(3, "c");
        assert_eq!(cache.len(), 3);
        assert!(cache.contains(&1));
        assert!(cache.contains(&2));
        assert!(cache.contains(&3));
    }

    #[test]
    fn clear_removes_all() {
        let mut cache = BoundedCache::new(10);
        cache.insert(1u32, "a");
        cache.insert(2, "b");
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn remove_specific_entry() {
        let mut cache = BoundedCache::new(10);
        cache.insert(1u32, "a");
        cache.insert(2, "b");
        assert!(cache.remove(&1));
        assert!(!cache.contains(&1));
        assert!(cache.contains(&2));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut cache: BoundedCache<u32, &str> = BoundedCache::new(10);
        assert!(!cache.remove(&99));
    }

    #[test]
    fn debug_format() {
        let cache: BoundedCache<u32, u32> = BoundedCache::new(50);
        let debug_str = format!("{cache:?}");
        assert!(debug_str.contains("BoundedCache"));
        assert!(debug_str.contains("count"));
        assert!(debug_str.contains("max_entries"));
    }
}
