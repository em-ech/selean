//! SDF text rendering system.
//!
//! Provides font loading, text shaping via `rustybuzz`, SDF glyph generation,
//! GPU texture atlas management, and a text rendering pipeline.

pub mod atlas;
pub mod cache;
pub mod font;
pub mod layout;
pub mod pipeline;
pub mod sdf;
pub mod shaper;

pub use atlas::{AtlasRegion, GlyphAtlas};
pub use cache::{CachedGlyph, GlyphCache, GlyphCacheKey};
pub use font::FontData;
pub use layout::{PositionedGlyph, TextLayout, layout_text};
pub use pipeline::{GlyphInstance, TextBatch, TextPipeline};
pub use sdf::{SdfBitmap, SdfParams, generate_glyph_sdf};
pub use shaper::{ShapedGlyph, ShapedRun, shape_text};

use selean_common::error::EngineError;
use tracing::debug;

use crate::scene::SceneNode;

/// Coordinates all text subsystem state: font, cache, atlas, and SDF parameters.
///
/// Created once at renderer initialization and used each frame to process
/// text nodes during the prepare phase.
pub struct TextSystem {
    /// The loaded font data.
    font: FontData,
    /// Glyph cache mapping (`glyph_id`, `sdf_size`) to atlas locations.
    cache: GlyphCache,
    /// GPU texture atlas for SDF glyph bitmaps.
    atlas: GlyphAtlas,
    /// SDF generation parameters.
    sdf_params: SdfParams,
}

impl TextSystem {
    /// Creates a new text system with the default embedded font.
    ///
    /// # Arguments
    /// * `device` — The GPU device for atlas texture creation.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the embedded font cannot be parsed.
    pub fn new(device: &wgpu::Device) -> Result<Self, EngineError> {
        let font = FontData::default_font()?;
        let atlas = GlyphAtlas::new(device);

        Ok(Self {
            font,
            cache: GlyphCache::new(),
            atlas,
            sdf_params: SdfParams::default(),
        })
    }

    /// Returns a reference to the glyph atlas (for bind group access).
    #[must_use]
    pub fn atlas(&self) -> &GlyphAtlas {
        &self.atlas
    }

    /// Returns a reference to the glyph cache.
    #[must_use]
    pub fn cache(&self) -> &GlyphCache {
        &self.cache
    }

    /// Returns the SDF render size used for cache keys.
    #[must_use]
    pub fn sdf_size(&self) -> u16 {
        #[allow(clippy::cast_possible_truncation)]
        let size = self.sdf_params.render_size as u16;
        size
    }

    /// Processes a text node: shapes, generates SDFs for new glyphs,
    /// lays out, and pushes glyph instances to the batch.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if atlas allocation fails.
    pub fn prepare_text_node(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        node: &SceneNode,
        content: &str,
        font_size: f32,
        batch: &mut TextBatch,
    ) -> Result<(), EngineError> {
        if content.is_empty() || !node.visible || node.bounds.is_empty() {
            return Ok(());
        }

        // Step 1: Shape the text.
        let Some(shaped) = shape_text(&self.font, content) else {
            return Ok(());
        };

        // Step 2: Ensure all glyphs have SDF bitmaps in the atlas.
        let face = self.font.face()?;
        let sdf_size = self.sdf_size();

        for sg in &shaped.glyphs {
            let key = GlyphCacheKey {
                glyph_id: sg.glyph_id,
                sdf_size,
            };

            if self.cache.contains(&key) {
                continue;
            }

            // Generate SDF for this glyph.
            let glyph_id = ttf_parser::GlyphId(sg.glyph_id);
            if let Some(sdf_bmp) = generate_glyph_sdf(&face, glyph_id, &self.sdf_params) {
                // Allocate atlas region and upload.
                let region = self
                    .atlas
                    .allocate(sdf_bmp.width, sdf_bmp.height, device, queue)?;
                self.atlas.upload(queue, &region, &sdf_bmp.data);

                self.cache.insert(
                    key,
                    CachedGlyph {
                        atlas_region: region,
                        bearing_x: sdf_bmp.bearing_x,
                        bearing_y: sdf_bmp.bearing_y,
                        glyph_width_funits: sdf_bmp.glyph_width_funits,
                        glyph_height_funits: sdf_bmp.glyph_height_funits,
                    },
                );

                debug!(glyph_id = sg.glyph_id, "Cached new glyph SDF");
            }
            // If the glyph has no outline (e.g., space), skip caching — it won't be rendered.
        }

        // Step 3: Layout the text.
        let text_layout = layout_text(
            &shaped,
            &self.cache,
            self.font.ascender(),
            node.bounds.x,
            node.bounds.y,
            font_size,
            sdf_size,
        );

        // Step 4: Push glyph instances to the batch.
        let (atlas_w, atlas_h) = self.atlas.dimensions();
        batch.push_text_node(
            node,
            &text_layout.glyphs,
            &self.cache,
            atlas_w,
            atlas_h,
            sdf_size,
        );

        Ok(())
    }
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem")
            .field("font", &self.font)
            .field("cache", &self.cache)
            .field("atlas", &self.atlas)
            .field("sdf_params", &self.sdf_params)
            .finish()
    }
}
