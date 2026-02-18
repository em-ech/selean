//! Text layout: converts shaped glyph runs into positioned quads in world space.
//!
//! Takes a `ShapedRun` (glyphs in font units), font metrics, and a target
//! position/size, and produces a list of `PositionedGlyph`s ready for rendering.

use super::cache::{CachedGlyph, GlyphCache, GlyphCacheKey};
use super::shaper::ShapedRun;

/// A positioned glyph ready for rendering as an instanced quad.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    /// Glyph ID for atlas lookup.
    pub glyph_id: u16,
    /// Top-left X position of the glyph quad in world space.
    pub x: f32,
    /// Top-left Y position of the glyph quad in world space.
    pub y: f32,
    /// Width of the glyph quad in world space.
    pub width: f32,
    /// Height of the glyph quad in world space.
    pub height: f32,
}

/// Result of laying out a text run.
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Positioned glyphs ready for rendering.
    pub glyphs: Vec<PositionedGlyph>,
}

impl TextLayout {
    /// Returns the number of positioned glyphs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Returns `true` if no glyphs were positioned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }
}

/// Lays out shaped text at a given position and font size.
///
/// Converts the `ShapedRun` (in font units) to world-space glyph positions.
/// The text baseline is placed at `node_y + ascender * scale`.
///
/// # Arguments
/// * `shaped` — The shaped glyph run from `rustybuzz`.
/// * `cache` — The glyph cache (for looking up SDF metrics).
/// * `ascender` — Font ascender in font units (positive).
/// * `node_x` — Left edge of the text node in world space.
/// * `node_y` — Top edge of the text node in world space.
/// * `font_size` — Desired font size in world-space pixels.
/// * `sdf_size` — The SDF render size used for cache lookup.
pub fn layout_text(
    shaped: &ShapedRun,
    cache: &GlyphCache,
    ascender: i16,
    node_x: f32,
    node_y: f32,
    font_size: f32,
    sdf_size: u16,
) -> TextLayout {
    if shaped.is_empty() || shaped.units_per_em == 0 {
        return TextLayout { glyphs: Vec::new() };
    }

    let scale = font_size / f32::from(shaped.units_per_em);
    let baseline_y = node_y + f32::from(ascender) * scale;

    let mut pen_x = node_x;
    let mut glyphs = Vec::with_capacity(shaped.glyphs.len());

    for sg in &shaped.glyphs {
        let cache_key = GlyphCacheKey {
            glyph_id: sg.glyph_id,
            sdf_size,
        };

        if let Some(cached) = cache.get(&cache_key) {
            let positioned = position_glyph(sg, cached, pen_x, baseline_y, scale);
            glyphs.push(positioned);
        }
        // If not in cache (e.g., space character), skip the quad but still advance.

        #[allow(clippy::cast_precision_loss)]
        {
            pen_x += sg.x_advance as f32 * scale;
        }
    }

    TextLayout { glyphs }
}

/// Positions a single glyph quad in world space using cached metrics.
#[allow(clippy::similar_names)]
fn position_glyph(
    shaped: &super::shaper::ShapedGlyph,
    cached: &CachedGlyph,
    pen_x: f32,
    baseline_y: f32,
    scale: f32,
) -> PositionedGlyph {
    // The glyph's world-space dimensions.
    let glyph_w = f32::from(cached.glyph_width_funits) * scale;
    let glyph_h = f32::from(cached.glyph_height_funits) * scale;

    // Expand the quad to include the SDF spread.
    // The SDF bitmap is larger than the glyph by `spread` on each side.
    // The ratio of SDF bitmap to glyph is: sdf_bitmap_size / glyph_size_in_sdf_pixels.
    #[allow(clippy::cast_precision_loss)]
    let sdf_w = cached.atlas_region.width as f32;
    #[allow(clippy::cast_precision_loss)]
    let sdf_h = cached.atlas_region.height as f32;
    let glyph_w_sdf = f32::from(cached.glyph_width_funits)
        * (sdf_w / f32::from(cached.glyph_width_funits).max(1.0));
    let glyph_h_sdf = f32::from(cached.glyph_height_funits)
        * (sdf_h / f32::from(cached.glyph_height_funits).max(1.0));

    // Scale the SDF bitmap dimensions to world space.
    let _ = (glyph_w_sdf, glyph_h_sdf); // used below
    let quad_w = sdf_w * (glyph_w / f32::from(cached.glyph_width_funits).max(1.0));
    let quad_h = sdf_h * (glyph_h / f32::from(cached.glyph_height_funits).max(1.0));

    // Position: pen position + shaping offsets + bearing adjustments.
    #[allow(clippy::cast_precision_loss)]
    let x_offset = shaped.x_offset as f32 * scale;
    #[allow(clippy::cast_precision_loss)]
    let y_offset = shaped.y_offset as f32 * scale;

    // bearing_x is in SDF pixel space; scale to world space.
    let bearing_scale = glyph_w / f32::from(cached.glyph_width_funits).max(1.0);
    let quad_x = pen_x + x_offset + cached.bearing_x * bearing_scale;
    // bearing_y is from baseline to top of SDF bitmap.
    let quad_y = baseline_y - y_offset - cached.bearing_y * bearing_scale;

    PositionedGlyph {
        glyph_id: shaped.glyph_id,
        x: quad_x,
        y: quad_y,
        width: quad_w,
        height: quad_h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::atlas::AtlasRegion;
    use crate::text::cache::CachedGlyph;
    use crate::text::shaper::ShapedGlyph;

    fn make_shaped_run(glyphs: Vec<ShapedGlyph>) -> ShapedRun {
        ShapedRun {
            glyphs,
            units_per_em: 1000,
        }
    }

    fn make_cached_glyph(glyph_id: u16) -> (GlyphCacheKey, CachedGlyph) {
        let key = GlyphCacheKey {
            glyph_id,
            sdf_size: 48,
        };
        let glyph = CachedGlyph {
            atlas_region: AtlasRegion {
                x: 0,
                y: 0,
                width: 48,
                height: 48,
            },
            bearing_x: -3.0,
            bearing_y: 36.0,
            glyph_width_funits: 600,
            glyph_height_funits: 800,
        };
        (key, glyph)
    }

    #[test]
    fn empty_shaped_run_produces_empty_layout() {
        let run = make_shaped_run(vec![]);
        let cache = GlyphCache::new();

        let layout = layout_text(&run, &cache, 800, 0.0, 0.0, 16.0, 48);
        assert!(layout.is_empty());
        assert_eq!(layout.len(), 0);
    }

    #[test]
    fn single_glyph_layout() {
        let run = make_shaped_run(vec![ShapedGlyph {
            glyph_id: 42,
            x_offset: 0,
            y_offset: 0,
            x_advance: 600,
            cluster: 0,
        }]);

        let mut cache = GlyphCache::new();
        let (key, cached) = make_cached_glyph(42);
        cache.insert(key, cached);

        let layout = layout_text(&run, &cache, 800, 100.0, 200.0, 16.0, 48);
        assert_eq!(layout.len(), 1);

        let g = &layout.glyphs[0];
        assert_eq!(g.glyph_id, 42);
        assert!(g.width > 0.0, "glyph should have positive width");
        assert!(g.height > 0.0, "glyph should have positive height");
    }

    #[test]
    fn uncached_glyphs_are_skipped() {
        let run = make_shaped_run(vec![ShapedGlyph {
            glyph_id: 99,
            x_offset: 0,
            y_offset: 0,
            x_advance: 500,
            cluster: 0,
        }]);

        let cache = GlyphCache::new(); // empty — nothing cached

        let layout = layout_text(&run, &cache, 800, 0.0, 0.0, 16.0, 48);
        assert!(layout.is_empty(), "uncached glyphs should be skipped");
    }

    #[test]
    fn multiple_glyphs_advance_pen() {
        let run = make_shaped_run(vec![
            ShapedGlyph {
                glyph_id: 1,
                x_offset: 0,
                y_offset: 0,
                x_advance: 600,
                cluster: 0,
            },
            ShapedGlyph {
                glyph_id: 2,
                x_offset: 0,
                y_offset: 0,
                x_advance: 600,
                cluster: 1,
            },
        ]);

        let mut cache = GlyphCache::new();
        let (k1, c1) = make_cached_glyph(1);
        let (k2, c2) = make_cached_glyph(2);
        cache.insert(k1, c1);
        cache.insert(k2, c2);

        let layout = layout_text(&run, &cache, 800, 0.0, 0.0, 16.0, 48);
        assert_eq!(layout.len(), 2);

        // Second glyph should be to the right of the first.
        assert!(
            layout.glyphs[1].x > layout.glyphs[0].x,
            "second glyph should be to the right"
        );
    }
}
