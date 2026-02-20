//! WebGPU rendering pipeline.
//!
//! Handles GPU device initialization, surface management, shader pipelines,
//! and draw call batching for the Selean canvas.

mod camera;
mod gpu;
mod pipeline;
mod quad;
mod rect_pipeline;
mod shared;
/// Generic texture atlas parameterized by channel count.
pub mod texture_atlas;
/// Textured quad pipeline shared by image and vector rendering.
pub mod textured_quad;

pub use camera::{Camera, CameraUniform};
pub use gpu::{GpuContext, GpuContextDescriptor};
pub use pipeline::{FrameStats, Renderer, RendererDescriptor};
pub use quad::{QUAD_INDICES, QUAD_VERTICES, QuadVertex};
pub use rect_pipeline::{RectBatch, RectInstance, RectPipeline};
pub use shared::{
    BLEND_STATE_ADD, PersistentInstanceBuffer, STENCIL_NOOP, SharedPipelineResources,
    create_pipeline_with_blend, create_stencil_texture,
};
pub use texture_atlas::{AtlasRegion, GlyphAtlas, ImageAtlas, TextureAtlas};
pub use textured_quad::{TexturedQuadBatch, TexturedQuadInstance, TexturedQuadPipeline};
