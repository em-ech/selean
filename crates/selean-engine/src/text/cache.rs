//! Glyph cache mapping (`glyph_id`, `sdf_size`) to atlas locations.
//!
//! Avoids regenerating SDF bitmaps for glyphs that have already been rasterized
//! and stored in the atlas. Since SDF is scale-independent, a single SDF render
//! per unique glyph shape is sufficient for all font sizes.

use crate::cache::BoundedCache;

use super::atlas::AtlasRegion;
use super::font::FontId;

/// Key for looking up a cached glyph.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub struct GlyphCacheKey {
    /// Glyph ID within the font.
    pub glyph_id: u16,
    /// SDF render size used when generating the bitmap.
    /// Typically a fixed value (e.g., 48) for all glyphs.
    pub sdf_size: u16,
    /// Font weight (100-900). Variable fonts produce different outlines per weight.
    pub font_weight: u16,
    /// Font ID within the `FontRegistry`. Different fonts produce different glyph outlines.
    pub font_id: FontId,
}

/// Cached information about a glyph stored in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct CachedGlyph {
    /// Region in the atlas texture.
    pub atlas_region: AtlasRegion,
    /// Horizontal bearing in SDF pixels.
    pub bearing_x: f32,
    /// Vertical bearing in SDF pixels (baseline to top of bitmap).
    pub bearing_y: f32,
    /// Glyph bounding box width in font units.
    pub glyph_width_funits: u16,
    /// Glyph bounding box height in font units.
    pub glyph_height_funits: u16,
}

/// Default maximum number of entries in a `GlyphCache`.
const DEFAULT_MAX_GLYPH_ENTRIES: usize = 4096;

/// Maps glyph cache keys to atlas locations and metrics.
///
/// Entries persist across frames. A glyph is cached once when first encountered
/// and looked up on subsequent frames. When the cache reaches its maximum
/// capacity, all entries are cleared before inserting the new entry. This
/// simple eviction strategy works well with atlas textures (the atlas is
/// rebuilt from scratch after a clear).
pub struct GlyphCache {
    inner: BoundedCache<GlyphCacheKey, CachedGlyph>,
}

impl GlyphCache {
    /// Creates a new empty glyph cache with the default maximum capacity (4096).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: BoundedCache::new(DEFAULT_MAX_GLYPH_ENTRIES),
        }
    }

    /// Creates a new empty glyph cache with the given maximum capacity.
    #[must_use]
    pub fn with_max_entries(max: usize) -> Self {
        Self {
            inner: BoundedCache::new(max),
        }
    }

    /// Looks up a cached glyph.
    #[must_use]
    pub fn get(&self, key: &GlyphCacheKey) -> Option<&CachedGlyph> {
        self.inner.get(key)
    }

    /// Inserts a glyph into the cache.
    ///
    /// If the cache is at capacity, all existing entries are cleared before
    /// inserting. This ensures the cache never exceeds its maximum size.
    pub fn insert(&mut self, key: GlyphCacheKey, glyph: CachedGlyph) {
        self.inner.insert(key, glyph);
    }

    /// Returns the number of cached glyphs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns `true` if the cache contains the given key.
    #[must_use]
    pub fn contains(&self, key: &GlyphCacheKey) -> bool {
        self.inner.contains(key)
    }

    /// Returns the maximum number of entries the cache will hold before evicting.
    #[must_use]
    pub fn max_entries(&self) -> usize {
        self.inner.max_entries()
    }

    /// Removes all cached glyphs.
    ///
    /// Use when switching fonts or documents. The atlas should also be reset
    /// since cached atlas regions become invalid.
    pub fn clear(&mut self) {
        self.inner.clear();
    }
}

impl Default for GlyphCache {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for GlyphCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphCache")
            .field("count", &self.inner.len())
            .field("max_entries", &self.inner.max_entries())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key(glyph_id: u16) -> GlyphCacheKey {
        GlyphCacheKey {
            glyph_id,
            sdf_size: 48,
            font_weight: 400,
            font_id: 0,
        }
    }

    fn test_cached_glyph() -> CachedGlyph {
        CachedGlyph {
            atlas_region: AtlasRegion {
                x: 0,
                y: 0,
                width: 48,
                height: 48,
            },
            bearing_x: 2.0,
            bearing_y: 40.0,
            glyph_width_funits: 600,
            glyph_height_funits: 800,
        }
    }

    #[test]
    fn new_cache_is_empty() {
        let cache = GlyphCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn insert_and_get() {
        let mut cache = GlyphCache::new();
        let key = test_key(42);
        let glyph = test_cached_glyph();

        cache.insert(key, glyph);

        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());

        let retrieved = cache.get(&key);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.map(|g| g.atlas_region.width), Some(48));
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let cache = GlyphCache::new();
        assert!(cache.get(&test_key(99)).is_none());
    }

    #[test]
    fn contains_check() {
        let mut cache = GlyphCache::new();
        let key = test_key(10);
        assert!(!cache.contains(&key));

        cache.insert(key, test_cached_glyph());
        assert!(cache.contains(&key));
    }

    #[test]
    fn different_sdf_sizes_are_different_keys() {
        let mut cache = GlyphCache::new();

        let key1 = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 48,
            font_weight: 400,
            font_id: 0,
        };
        let key2 = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 64,
            font_weight: 400,
            font_id: 0,
        };

        cache.insert(key1, test_cached_glyph());
        assert!(cache.contains(&key1));
        assert!(!cache.contains(&key2));
    }

    #[test]
    fn different_weights_are_different_keys() {
        let mut cache = GlyphCache::new();

        let key_regular = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 48,
            font_weight: 400,
            font_id: 0,
        };
        let key_bold = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 48,
            font_weight: 700,
            font_id: 0,
        };

        cache.insert(key_regular, test_cached_glyph());
        assert!(cache.contains(&key_regular));
        assert!(!cache.contains(&key_bold));
    }

    #[test]
    fn different_font_ids_are_different_keys() {
        let mut cache = GlyphCache::new();

        let key_font0 = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 48,
            font_weight: 400,
            font_id: 0,
        };
        let key_font1 = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 48,
            font_weight: 400,
            font_id: 1,
        };

        cache.insert(key_font0, test_cached_glyph());
        assert!(cache.contains(&key_font0));
        assert!(!cache.contains(&key_font1));
    }

    #[test]
    fn debug_format() {
        let cache = GlyphCache::new();
        let debug_str = format!("{cache:?}");
        assert!(debug_str.contains("GlyphCache"));
        assert!(debug_str.contains("count"));
    }

    #[test]
    fn clear_removes_all_entries() {
        let mut cache = GlyphCache::new();
        cache.insert(test_key(1), test_cached_glyph());
        cache.insert(test_key(2), test_cached_glyph());
        assert_eq!(cache.len(), 2);

        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains(&test_key(1)));
        assert!(!cache.contains(&test_key(2)));
    }

    #[test]
    fn eviction_clears_when_full() {
        let mut cache = GlyphCache::with_max_entries(3);

        cache.insert(test_key(1), test_cached_glyph());
        cache.insert(test_key(2), test_cached_glyph());
        cache.insert(test_key(3), test_cached_glyph());
        assert_eq!(cache.len(), 3);

        // Inserting a 4th entry should clear the cache first, then insert.
        cache.insert(test_key(4), test_cached_glyph());
        assert_eq!(cache.len(), 1);
        assert!(cache.contains(&test_key(4)));
        assert!(!cache.contains(&test_key(1)));
        assert!(!cache.contains(&test_key(2)));
        assert!(!cache.contains(&test_key(3)));
    }

    #[test]
    fn with_max_entries_constructor() {
        let cache = GlyphCache::with_max_entries(100);
        assert_eq!(cache.max_entries(), 100);
        assert!(cache.is_empty());
    }

    #[test]
    fn insert_at_capacity_minus_one_does_not_evict() {
        let mut cache = GlyphCache::with_max_entries(3);

        cache.insert(test_key(1), test_cached_glyph());
        cache.insert(test_key(2), test_cached_glyph());
        assert_eq!(cache.len(), 2);

        // Inserting a 3rd entry (capacity minus one -> capacity) should NOT evict.
        cache.insert(test_key(3), test_cached_glyph());
        assert_eq!(cache.len(), 3);
        assert!(cache.contains(&test_key(1)));
        assert!(cache.contains(&test_key(2)));
        assert!(cache.contains(&test_key(3)));
    }

    #[test]
    fn default_max_entries() {
        let cache = GlyphCache::new();
        assert_eq!(cache.max_entries(), 4096);
    }
}
