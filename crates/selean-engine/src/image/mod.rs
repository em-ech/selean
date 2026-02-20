//! Image rendering system.
//!
//! Provides image asset registration (eager decode), atlas placement, and
//! per-frame preparation for GPU rendering.

pub mod cache;
pub mod loader;

pub use cache::{CachedImage, ImageCache};
pub use loader::{DecodedImage, decode_image, decode_image_resized};

use std::collections::HashMap;

use selean_common::error::EngineError;
use tracing::debug;

use crate::renderer::texture_atlas::TextureAtlas;
use crate::renderer::textured_quad::TexturedQuadBatch;
use crate::scene::SceneNode;

/// Maximum dimension for decoded images before atlas upload.
const MAX_IMAGE_DIM: u32 = 2048;

/// Coordinates image asset management and per-frame rendering.
///
/// Owns the decoded asset store and image cache. Receives the shared RGBA
/// atlas as a `&mut TextureAtlas<4>` parameter during prepare calls.
pub struct ImageSystem {
    /// Decoded image assets, keyed by asset reference.
    decoded_assets: HashMap<String, DecodedImage>,
    /// Cache of images already placed in the atlas.
    cache: ImageCache,
}

impl ImageSystem {
    /// Creates a new empty image system.
    #[must_use]
    pub fn new() -> Self {
        Self {
            decoded_assets: HashMap::new(),
            cache: ImageCache::new(),
        }
    }

    /// Registers an image asset by decoding the raw bytes eagerly.
    ///
    /// The image is decoded and optionally resized to fit within `MAX_IMAGE_DIM`.
    /// The decoded data is stored for later atlas upload during the prepare phase.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Image` if the bytes cannot be decoded.
    pub fn register_asset(&mut self, asset_ref: &str, bytes: &[u8]) -> Result<(), EngineError> {
        let decoded = decode_image_resized(bytes, MAX_IMAGE_DIM)?;
        debug!(
            asset_ref,
            width = decoded.width,
            height = decoded.height,
            "Image asset registered"
        );
        self.decoded_assets.insert(asset_ref.to_owned(), decoded);
        // Invalidate cache entry if re-registering the same asset.
        self.cache.clear_entry(asset_ref);
        Ok(())
    }

    /// Returns `true` if an asset has been registered with the given reference.
    #[must_use]
    pub fn has_asset(&self, asset_ref: &str) -> bool {
        self.decoded_assets.contains_key(asset_ref)
    }

    /// Prepares an image node for rendering.
    ///
    /// If the image is not yet in the atlas, allocates space and uploads it.
    /// Adds the resulting textured quad to the batch.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Image` if atlas allocation fails.
    pub fn prepare_image_node(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        atlas: &mut TextureAtlas<4>,
        node: &SceneNode,
        asset_ref: &str,
        batch: &mut TexturedQuadBatch,
    ) -> Result<(), EngineError> {
        if asset_ref.is_empty() || !node.visible || node.bounds.is_empty() {
            return Ok(());
        }

        // Check cache first.
        if let Some(cached) = self.cache.get(asset_ref) {
            batch.push_pending(node, cached.atlas_region, node.fill);
            return Ok(());
        }

        // Asset not in atlas yet — look up decoded data and upload.
        let Some(decoded) = self.decoded_assets.get(asset_ref) else {
            // Asset not registered — silently skip (no render, no error).
            return Ok(());
        };

        let region = atlas.allocate(decoded.width, decoded.height, device, queue)?;
        atlas.upload(queue, &region, &decoded.data);

        let cached = CachedImage {
            atlas_region: region,
            source_width: decoded.width,
            source_height: decoded.height,
        };
        self.cache.insert(asset_ref.to_owned(), cached);

        batch.push_pending(node, region, node.fill);
        Ok(())
    }

    /// Resets the image system, clearing decoded assets and cache.
    pub fn reset(&mut self) {
        self.decoded_assets.clear();
        self.cache.clear();
    }

    /// Returns the number of registered assets.
    #[must_use]
    pub fn asset_count(&self) -> usize {
        self.decoded_assets.len()
    }

    /// Returns the number of cached atlas entries.
    #[must_use]
    pub fn cache_count(&self) -> usize {
        self.cache.len()
    }
}

impl Default for ImageSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ImageSystem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageSystem")
            .field("assets", &self.decoded_assets.len())
            .field("cached", &self.cache.len())
            .finish()
    }
}
