//! GPU texture atlas for SDF glyphs.
//!
//! This module re-exports the generic `TextureAtlas<1>` as `GlyphAtlas` and
//! the `AtlasRegion` type from the shared texture atlas module. All atlas
//! logic lives in `renderer::texture_atlas`.

pub use crate::renderer::texture_atlas::{AtlasRegion, GlyphAtlas};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_region_uv_rect() {
        let region = AtlasRegion {
            x: 100,
            y: 200,
            width: 50,
            height: 60,
        };

        let uv = region.uv_rect(1024, 1024);
        let expected = [
            100.0 / 1024.0,
            200.0 / 1024.0,
            150.0 / 1024.0,
            260.0 / 1024.0,
        ];
        for (a, b) in uv.iter().zip(expected.iter()) {
            assert!((a - b).abs() < f32::EPSILON, "uv {a} != expected {b}");
        }
    }

    #[test]
    fn atlas_region_uv_full_texture() {
        let region = AtlasRegion {
            x: 0,
            y: 0,
            width: 512,
            height: 512,
        };

        let uv = region.uv_rect(512, 512);
        assert!((uv[0]).abs() < f32::EPSILON);
        assert!((uv[1]).abs() < f32::EPSILON);
        assert!((uv[2] - 1.0).abs() < f32::EPSILON);
        assert!((uv[3] - 1.0).abs() < f32::EPSILON);
    }
}
