//! Image cache mapping asset references to atlas locations.
//!
//! Simple bounded cache keyed by asset reference string.

use crate::cache::BoundedCache;
use crate::renderer::texture_atlas::AtlasRegion;

/// Cached information about an image stored in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct CachedImage {
    /// Region in the RGBA atlas texture.
    pub atlas_region: AtlasRegion,
    /// Original decoded image width (before any atlas placement).
    pub source_width: u32,
    /// Original decoded image height.
    pub source_height: u32,
}

/// Default maximum number of entries in an `ImageCache`.
const DEFAULT_MAX_IMAGE_ENTRIES: usize = 256;

/// Maps asset references to atlas locations.
///
/// Entries persist across frames. An image is cached once when first placed
/// in the atlas and reused on subsequent frames. When the cache reaches its
/// maximum capacity, all entries are cleared before inserting the new entry.
/// This simple eviction strategy works well with atlas textures (the atlas
/// is rebuilt from scratch after a clear).
pub struct ImageCache {
    inner: BoundedCache<String, CachedImage>,
}

impl ImageCache {
    /// Creates a new empty image cache with the default maximum capacity (256).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: BoundedCache::new(DEFAULT_MAX_IMAGE_ENTRIES),
        }
    }

    /// Creates a new empty image cache with the given maximum capacity.
    #[must_use]
    pub fn with_max_entries(max: usize) -> Self {
        Self {
            inner: BoundedCache::new(max),
        }
    }

    /// Looks up a cached image by asset reference.
    #[must_use]
    pub fn get(&self, asset_ref: &str) -> Option<&CachedImage> {
        self.inner.get(asset_ref)
    }

    /// Inserts an image into the cache.
    ///
    /// If the cache is at capacity, all existing entries are cleared before
    /// inserting. This ensures the cache never exceeds its maximum size.
    pub fn insert(&mut self, asset_ref: String, image: CachedImage) {
        self.inner.insert(asset_ref, image);
    }

    /// Returns `true` if the cache contains the given asset reference.
    #[must_use]
    pub fn contains(&self, asset_ref: &str) -> bool {
        self.inner.contains(asset_ref)
    }

    /// Returns the number of cached images.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the maximum number of entries the cache will hold before evicting.
    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.inner.max_entries()
    }

    /// Removes a specific cached image by asset reference.
    pub fn clear_entry(&mut self, asset_ref: &str) {
        self.inner.remove(asset_ref);
    }

    /// Removes all cached images.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cached_image() -> CachedImage {
        CachedImage {
            atlas_region: AtlasRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            source_width: 256,
            source_height: 256,
        }
    }

    #[test]
    fn new_cache_is_empty() {
        let cache = ImageCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut cache = ImageCache::new();
        cache.insert("img_001.png".to_string(), test_cached_image());

        assert_eq!(cache.len(), 1);
        assert!(cache.contains("img_001.png"));
        assert!(cache.get("img_001.png").is_some());
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let cache = ImageCache::new();
        assert!(cache.get("missing.png").is_none());
    }

    #[test]
    fn clear_removes_all() {
        let mut cache = ImageCache::new();
        cache.insert("a.png".to_string(), test_cached_image());
        cache.insert("b.png".to_string(), test_cached_image());
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn eviction_clears_when_full() {
        let mut cache = ImageCache::with_max_entries(3);

        cache.insert("a.png".to_string(), test_cached_image());
        cache.insert("b.png".to_string(), test_cached_image());
        cache.insert("c.png".to_string(), test_cached_image());
        assert_eq!(cache.len(), 3);

        // Inserting a 4th entry should clear the cache first, then insert.
        cache.insert("d.png".to_string(), test_cached_image());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains("d.png"));
        assert!(!cache.contains("a.png"));
        assert!(!cache.contains("b.png"));
        assert!(!cache.contains("c.png"));
    }

    #[test]
    fn with_max_entries_constructor() {
        let cache = ImageCache::with_max_entries(50);
        assert_eq!(cache.max_entries(), 50);
        assert!(cache.is_empty());
    }

    #[test]
    fn insert_at_capacity_minus_one_does_not_evict() {
        let mut cache = ImageCache::with_max_entries(3);

        cache.insert("a.png".to_string(), test_cached_image());
        cache.insert("b.png".to_string(), test_cached_image());
        assert_eq!(cache.len(), 2);

        // Inserting a 3rd entry (capacity minus one -> capacity) should NOT evict.
        cache.insert("c.png".to_string(), test_cached_image());
        assert_eq!(cache.len(), 3);
        assert!(cache.contains("a.png"));
        assert!(cache.contains("b.png"));
        assert!(cache.contains("c.png"));
    }

    #[test]
    fn default_max_entries() {
        let cache = ImageCache::new();
        assert_eq!(cache.max_entries(), 256);
    }

    #[test]
    fn clear_entry_removes_specific_image() {
        let mut cache = ImageCache::new();
        cache.insert("a.png".to_string(), test_cached_image());
        cache.insert("b.png".to_string(), test_cached_image());

        cache.clear_entry("a.png");
        assert!(!cache.contains("a.png"));
        assert!(cache.contains("b.png"));
        assert_eq!(cache.len(), 1);
    }
}
