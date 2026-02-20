//! Vector rendering system.
//!
//! Provides SVG path parsing, rasterization via `tiny-skia`, caching with
//! dimension quantization, and per-frame preparation for GPU rendering.

pub mod cache;
pub mod parser;
pub mod rasterizer;

pub use cache::{CachedVector, VectorCache, VectorCacheKey, quantize_dimension};
pub use parser::parse_path_data;
pub use rasterizer::{RasterizedVector, rasterize_path};

use selean_common::error::EngineError;
use tracing::{debug, warn};

use crate::renderer::texture_atlas::TextureAtlas;
use crate::renderer::textured_quad::TexturedQuadBatch;
use crate::scene::SceneNode;

/// Coordinates vector rendering: parsing, rasterization, caching, and batching.
///
/// Receives the shared RGBA atlas as a `&mut TextureAtlas<4>` parameter
/// during prepare calls.
pub struct VectorSystem {
    /// Cache of rasterized vectors in the atlas.
    cache: VectorCache,
}

impl VectorSystem {
    /// Creates a new empty vector system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cache: VectorCache::new(),
        }
    }

    /// Prepares a vector node for rendering.
    ///
    /// Parses the path data, rasterizes at quantized dimensions, caches in
    /// the atlas, and adds a textured quad to the batch.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Vector` if parsing or rasterization fails.
    pub fn prepare_vector_node(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut TextureAtlas<4>,
        node: &SceneNode,
        path_data: &str,
        batch: &mut TexturedQuadBatch,
    ) -> Result<(), EngineError> {
        if path_data.is_empty() || !node.visible || node.bounds.is_empty() {
            return Ok(());
        }

        let quantized_w = quantize_dimension(node.bounds.width);
        let quantized_h = quantize_dimension(node.bounds.height);

        let cache_key = VectorCacheKey {
            path_data: path_data.to_owned(),
            width: quantized_w,
            height: quantized_h,
        };

        // Check cache first.
        if let Some(cached) = self.cache.get(&cache_key) {
            batch.push_pending(node, cached.atlas_region, node.fill);
            return Ok(());
        }

        // Parse, rasterize, and cache.
        let path = match parse_path_data(path_data) {
            Ok(p) => p,
            Err(e) => {
                warn!(?e, "Failed to parse vector path data");
                return Ok(()); // Skip unparseable paths silently.
            }
        };

        let fill = node.fill;
        let stroke = node.stroke;
        let stroke_width = node.stroke_width;

        let rasterized =
            rasterize_path(&path, quantized_w, quantized_h, fill, stroke, stroke_width)?;

        let region = atlas.allocate(rasterized.width, rasterized.height, device, queue)?;
        atlas.upload(queue, &region, &rasterized.data);

        debug!(
            path_len = path_data.len(),
            w = quantized_w,
            h = quantized_h,
            "Cached rasterized vector"
        );

        self.cache.insert(
            cache_key,
            CachedVector {
                atlas_region: region,
            },
        );

        batch.push_pending(node, region, node.fill);
        Ok(())
    }

    /// Resets the vector system, clearing the cache.
    pub fn reset(&mut self) {
        self.cache.clear();
    }

    /// Returns the number of cached vectors.
    #[must_use]
    pub fn cache_count(&self) -> usize {
        self.cache.len()
    }
}

impl Default for VectorSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for VectorSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VectorSystem")
            .field("cached", &self.cache.len())
            .finish()
    }
}
