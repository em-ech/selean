//! Image cache mapping asset references to atlas locations.
//!
//! Simple `HashMap<String, CachedImage>` with lookup via `&str`.

use std::collections::HashMap;

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

/// Maps asset references to atlas locations.
///
/// Entries persist across frames. An image is cached once when first placed
/// in the atlas and reused on subsequent frames.
pub struct ImageCache {
    entries: HashMap<String, CachedImage>,
}

impl ImageCache {
    /// Creates a new empty image cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Looks up a cached image by asset reference.
    #[must_use]
    pub fn get(&self, asset_ref: &str) -> Option<&CachedImage> {
        self.entries.get(asset_ref)
    }

    /// Inserts an image into the cache.
    pub fn insert(&mut self, asset_ref: String, image: CachedImage) {
        self.entries.insert(asset_ref, image);
    }

    /// Returns `true` if the cache contains the given asset reference.
    #[must_use]
    pub fn contains(&self, asset_ref: &str) -> bool {
        self.entries.contains_key(asset_ref)
    }

    /// Returns the number of cached images.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes a specific cached image by asset reference.
    pub fn clear_entry(&mut self, asset_ref: &str) {
        self.entries.remove(asset_ref);
    }

    /// Removes all cached images.
    pub fn clear(&mut self) {
        self.entries.clear();
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
}
