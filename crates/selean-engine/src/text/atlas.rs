//! GPU texture atlas for SDF glyphs.
//!
//! This module re-exports the generic `TextureAtlas<1>` as `GlyphAtlas` and
//! the `AtlasRegion` type from the shared texture atlas module. All atlas
//! logic lives in `renderer::texture_atlas`.

pub use crate::renderer::texture_atlas::{AtlasRegion, GlyphAtlas};
