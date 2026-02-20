//! Vector cache mapping `(path_data, width, height)` to atlas locations.
//!
//! Full `path_data` String used as key (no hashing) — cache reuse comes
//! from dimension quantization (power of 2).

use std::collections::HashMap;

use crate::renderer::texture_atlas::AtlasRegion;

/// Cache key for a rasterized vector.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct VectorCacheKey {
    /// The SVG path data string.
    pub path_data: String,
    /// Quantized rasterization width.
    pub width: u32,
    /// Quantized rasterization height.
    pub height: u32,
}

/// Cached information about a rasterized vector in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct CachedVector {
    /// Region in the RGBA atlas texture.
    pub atlas_region: AtlasRegion,
}

/// Maps vector cache keys to atlas locations.
pub struct VectorCache {
    entries: HashMap<VectorCacheKey, CachedVector>,
}

impl VectorCache {
    /// Creates a new empty vector cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Looks up a cached vector.
    #[must_use]
    pub fn get(&self, key: &VectorCacheKey) -> Option<&CachedVector> {
        self.entries.get(key)
    }

    /// Inserts a vector into the cache.
    pub fn insert(&mut self, key: VectorCacheKey, vector: CachedVector) {
        self.entries.insert(key, vector);
    }

    /// Returns `true` if the cache contains the given key.
    #[must_use]
    pub fn contains(&self, key: &VectorCacheKey) -> bool {
        self.entries.contains_key(key)
    }

    /// Returns the number of cached vectors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Removes all cached vectors.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for VectorCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Rounds a dimension to the nearest power of two for cache reuse.
///
/// Clamps the result to a minimum of 1 and a maximum of 512.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn quantize_dimension(dim: f32) -> u32 {
    let d = (dim as u32).max(1);
    d.next_power_of_two().min(512)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_cache_is_empty() {
        let cache = VectorCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut cache = VectorCache::new();
        let key = VectorCacheKey {
            path_data: "M 0 0 L 10 10".to_string(),
            width: 64,
            height: 64,
        };
        let cached = CachedVector {
            atlas_region: AtlasRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
        };

        cache.insert(key.clone(), cached);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&key));
        assert!(cache.get(&key).is_some());
    }

    #[test]
    fn different_dimensions_are_different_keys() {
        let mut cache = VectorCache::new();
        let key1 = VectorCacheKey {
            path_data: "M 0 0 L 10 10".to_string(),
            width: 64,
            height: 64,
        };
        let key2 = VectorCacheKey {
            path_data: "M 0 0 L 10 10".to_string(),
            width: 128,
            height: 128,
        };
        let cached = CachedVector {
            atlas_region: AtlasRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
        };

        cache.insert(key1.clone(), cached);
        assert!(cache.contains(&key1));
        assert!(!cache.contains(&key2));
    }

    #[test]
    fn clear_removes_all() {
        let mut cache = VectorCache::new();
        let key = VectorCacheKey {
            path_data: "M 0 0 L 10 10".to_string(),
            width: 64,
            height: 64,
        };
        cache.insert(
            key,
            CachedVector {
                atlas_region: AtlasRegion {
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 64,
                },
            },
        );
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn quantize_small_values() {
        assert_eq!(quantize_dimension(1.0), 1);
        assert_eq!(quantize_dimension(0.5), 1);
        assert_eq!(quantize_dimension(3.0), 4);
    }

    #[test]
    fn quantize_powers_of_two() {
        assert_eq!(quantize_dimension(32.0), 32);
        assert_eq!(quantize_dimension(64.0), 64);
        assert_eq!(quantize_dimension(128.0), 128);
    }

    #[test]
    fn quantize_rounds_up() {
        assert_eq!(quantize_dimension(33.0), 64);
        assert_eq!(quantize_dimension(65.0), 128);
        assert_eq!(quantize_dimension(129.0), 256);
    }

    #[test]
    fn quantize_clamps_to_512() {
        assert_eq!(quantize_dimension(1000.0), 512);
        assert_eq!(quantize_dimension(513.0), 512);
    }
}
