//! SDF text rendering system.
//!
//! Provides font loading, text shaping via `rustybuzz`, SDF glyph generation,
//! GPU texture atlas management, and a text rendering pipeline.

pub mod atlas;
pub mod cache;
pub mod font;
pub mod layout;
pub mod packer;
pub mod pipeline;
pub mod sdf;
pub mod shaper;

pub use atlas::{AtlasRegion, GlyphAtlas};
pub use cache::{CachedGlyph, GlyphCache, GlyphCacheKey};
pub use font::{FontData, FontId, FontRegistry};
pub use layout::{PositionedGlyph, TextLayout, layout_text};
pub use packer::{PackResult, ShelfPacker};
pub use pipeline::{GlyphInstance, TextBatch, TextPipeline};
pub use sdf::{SdfBitmap, SdfParams, generate_glyph_sdf};
pub use shaper::{ShapedGlyph, ShapedRun, shape_text};

use std::collections::HashMap;

use selean_common::error::EngineError;
use selean_common::types::NodeId;
use tracing::debug;

use crate::scene::{DirtyFlags, SceneNode, SceneNodeKind, TextAlign};

/// Extracts `text_align`, `line_height`, `font_weight`, and `font_family`
/// from a `SceneNodeKind::Text`. Returns defaults for non-text nodes.
fn extract_text_props(kind: &SceneNodeKind) -> (TextAlign, f32, u16, &str) {
    match kind {
        SceneNodeKind::Text {
            text_align,
            line_height,
            font_weight,
            font_family,
            ..
        } => (
            *text_align,
            *line_height,
            *font_weight,
            font_family.as_str(),
        ),
        _ => (TextAlign::Left, 1.2, 400, "Inter"),
    }
}

/// Per-node cached text processing state.
///
/// Stores the inputs that produced the cached outputs so we can detect
/// staleness via input comparison (robust even when dirty flags are cleared
/// while a node is off-screen).
struct CachedTextState {
    /// Content string used for shaping (for staleness detection).
    content: String,
    /// Font size used for layout.
    font_size: f32,
    /// Font weight used for shaping and cache keys.
    font_weight: u16,
    /// Font family name (for staleness detection).
    font_family: String,
    /// Node X position used for layout.
    node_x: f32,
    /// Node Y position used for layout.
    node_y: f32,
    /// Cached shaping result.
    shaped: ShapedRun,
    /// Cached layout result.
    layout: TextLayout,
}

/// What work the text cache needs to do for a given node.
enum TextCacheAction {
    /// Cached layout is still valid — just push instances to the batch.
    FullHit,
    /// Content unchanged but position, size, or font size changed — re-layout only.
    RelayoutOnly,
    /// Content changed or no cache entry — full re-shape + re-layout.
    FullReshape,
}

/// Coordinates all text subsystem state: font, cache, atlas, and SDF parameters.
///
/// Created once at renderer initialization and used each frame to process
/// text nodes during the prepare phase. Maintains a per-node cache of shaping
/// and layout results to avoid redundant work across frames.
pub struct TextSystem {
    /// Registry of loaded fonts (Inter is always ID 0).
    fonts: FontRegistry,
    /// Glyph cache mapping (`glyph_id`, `sdf_size`, `font_id`) to atlas locations.
    cache: GlyphCache,
    /// GPU texture atlas for SDF glyph bitmaps.
    atlas: GlyphAtlas,
    /// SDF generation parameters.
    sdf_params: SdfParams,
    /// Per-node cache of shaping and layout results.
    node_text_cache: HashMap<NodeId, CachedTextState>,
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
        let fonts = FontRegistry::new()?;
        let atlas = GlyphAtlas::new(device);

        Ok(Self {
            fonts,
            cache: GlyphCache::new(),
            atlas,
            sdf_params: SdfParams::default(),
            node_text_cache: HashMap::new(),
        })
    }

    /// Registers a new font family in the text system.
    ///
    /// Returns the assigned `FontId`. If the family is already registered,
    /// returns the existing ID.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the font bytes cannot be parsed.
    pub fn register_font(&mut self, family: &str, data: Vec<u8>) -> Result<FontId, EngineError> {
        self.fonts.register(family, data)
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

    /// Resets the text system, clearing all cached glyphs, atlas, and
    /// per-node text state.
    ///
    /// Use when switching fonts or documents.
    pub fn reset(&mut self, device: &wgpu::Device) {
        self.cache.clear();
        self.atlas.reset(device);
        self.node_text_cache.clear();
    }

    /// Removes cached text state for a specific node.
    ///
    /// Call when a node is removed from the scene graph to free memory.
    pub fn evict_node(&mut self, id: NodeId) {
        self.node_text_cache.remove(&id);
    }

    /// Returns the number of nodes with cached text state.
    #[must_use]
    pub fn cached_node_count(&self) -> usize {
        self.node_text_cache.len()
    }

    /// Processes a text node: shapes, generates SDFs for new glyphs,
    /// lays out, and pushes glyph instances to the batch.
    ///
    /// Uses a per-node cache to skip shaping and/or layout when inputs
    /// haven't changed. Dirty flags provide a fast invalidation signal;
    /// input comparison provides correctness even when flags are cleared
    /// while a node is off-screen.
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

        let sdf_size = self.sdf_size();
        let (text_align, line_height, font_weight, font_family) = extract_text_props(&node.kind);
        let font_id = self.fonts.resolve(font_family);
        let action = Self::text_cache_action(
            &self.node_text_cache,
            node,
            content,
            font_size,
            font_weight,
            font_family,
        );

        // Extract ascender (Copy) upfront to avoid holding an immutable
        // borrow on `self.fonts` across mutable method calls.
        let ascender = self.font_data(font_id)?.ascender();

        match action {
            TextCacheAction::FullHit => {
                // Fast path: reuse cached layout, just push instances to batch.
                if let Some(cached) = self.node_text_cache.get(&node.id) {
                    batch.push_text_node(
                        node,
                        &cached.layout.glyphs,
                        &self.cache,
                        sdf_size,
                        font_id,
                    );
                }
            }
            TextCacheAction::RelayoutOnly => {
                // Content unchanged: re-layout from cached shaped run.
                let layout = if let Some(cached) = self.node_text_cache.get(&node.id) {
                    layout_text(
                        &cached.shaped,
                        &self.cache,
                        ascender,
                        node.bounds.x,
                        node.bounds.y,
                        font_size,
                        sdf_size,
                        node.bounds.width,
                        text_align,
                        line_height,
                        font_weight,
                        font_id,
                    )
                } else {
                    return Ok(());
                };

                batch.push_text_node(node, &layout.glyphs, &self.cache, sdf_size, font_id);

                if let Some(entry) = self.node_text_cache.get_mut(&node.id) {
                    entry.font_size = font_size;
                    entry.node_x = node.bounds.x;
                    entry.node_y = node.bounds.y;
                    entry.layout = layout;
                }
            }
            TextCacheAction::FullReshape => {
                // Full re-shape + re-layout.
                let font_data = self.font_data(font_id)?;
                let Some(shaped) = shape_text(font_data, content, font_weight) else {
                    return Ok(());
                };

                self.ensure_glyphs_cached(&shaped, font_weight, font_id, device, queue)?;

                let layout = layout_text(
                    &shaped,
                    &self.cache,
                    ascender,
                    node.bounds.x,
                    node.bounds.y,
                    font_size,
                    sdf_size,
                    node.bounds.width,
                    text_align,
                    line_height,
                    font_weight,
                    font_id,
                );

                batch.push_text_node(node, &layout.glyphs, &self.cache, sdf_size, font_id);

                self.node_text_cache.insert(
                    node.id,
                    CachedTextState {
                        content: content.to_owned(),
                        font_size,
                        font_weight,
                        font_family: font_family.to_owned(),
                        node_x: node.bounds.x,
                        node_y: node.bounds.y,
                        shaped,
                        layout,
                    },
                );
            }
        }

        Ok(())
    }

    /// Returns the `FontData` for the given ID, falling back to ID 0 (Inter).
    fn font_data(&self, font_id: FontId) -> Result<&FontData, EngineError> {
        self.fonts
            .get(font_id)
            .or_else(|| self.fonts.get(0))
            .ok_or_else(|| EngineError::Font {
                reason: "no fonts registered".to_string(),
            })
    }

    /// Determines what cache action is needed for a text node.
    ///
    /// Uses dirty flags as a fast invalidation signal, with input comparison
    /// as a fallback for correctness when flags may have been cleared while
    /// the node was off-screen.
    fn text_cache_action(
        node_text_cache: &HashMap<NodeId, CachedTextState>,
        node: &SceneNode,
        content: &str,
        font_size: f32,
        font_weight: u16,
        font_family: &str,
    ) -> TextCacheAction {
        let Some(cached) = node_text_cache.get(&node.id) else {
            return TextCacheAction::FullReshape;
        };

        // Check if content, weight, or font family changed.
        if node.dirty.contains(DirtyFlags::TEXT)
            || cached.content != content
            || cached.font_weight != font_weight
            || cached.font_family != font_family
        {
            return TextCacheAction::FullReshape;
        }

        // Content same — check if layout inputs changed.
        if node.dirty.contains(DirtyFlags::GEOMETRY)
            || (cached.font_size - font_size).abs() > f32::EPSILON
            || (cached.node_x - node.bounds.x).abs() > f32::EPSILON
            || (cached.node_y - node.bounds.y).abs() > f32::EPSILON
        {
            return TextCacheAction::RelayoutOnly;
        }

        TextCacheAction::FullHit
    }

    /// Ensures all glyphs in a shaped run have SDF bitmaps in the atlas.
    ///
    /// Generates and uploads SDF bitmaps for any glyphs not yet in the cache.
    fn ensure_glyphs_cached(
        &mut self,
        shaped: &ShapedRun,
        font_weight: u16,
        font_id: FontId,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<(), EngineError> {
        // Access the font directly by field to limit the borrow scope.
        // `face` borrows `fonts` (immutable), while `cache` and `atlas`
        // are separate fields and can be mutated independently.
        let font_ref = self
            .fonts
            .get(font_id)
            .or_else(|| self.fonts.get(0))
            .ok_or_else(|| EngineError::Font {
                reason: "no fonts registered".to_string(),
            })?;
        let face = font_ref.face_with_weight(font_weight)?;
        let sdf_size = self.sdf_size();

        for sg in &shaped.glyphs {
            let key = GlyphCacheKey {
                glyph_id: sg.glyph_id,
                sdf_size,
                font_weight,
                font_id,
            };

            if self.cache.contains(&key) {
                continue;
            }

            let glyph_id = ttf_parser::GlyphId(sg.glyph_id);
            if let Some(sdf_bmp) = generate_glyph_sdf(&face, glyph_id, &self.sdf_params) {
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
            // Glyphs with no outline (e.g., space) are skipped.
        }

        Ok(())
    }
}

impl std::fmt::Debug for TextSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextSystem")
            .field("fonts", &self.fonts)
            .field("cache", &self.cache)
            .field("atlas", &self.atlas)
            .field("sdf_params", &self.sdf_params)
            .field("cached_nodes", &self.node_text_cache.len())
            .finish()
    }
}
