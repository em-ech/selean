//! WebGPU rendering pipeline.
//!
//! Handles GPU device initialization, surface management, shader pipelines,
//! and draw call batching for the Selean canvas.

/// Pure Rust blend mode formulas per W3C Compositing and Blending Level 1.
pub mod blend;
/// GPU resources for shader-based blend mode compositing.
pub mod blend_pipeline;
mod camera;
/// Hierarchical clip state stack for DFS scene traversal.
pub mod clip_stack;
/// Draw command list for the hierarchical rendering pipeline.
pub mod draw_list;
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
pub use clip_stack::{ClipStack, ResolvedClipState};
pub use draw_list::{DrawCommand, DrawList};
pub use gpu::{GpuContext, GpuContextDescriptor};
pub use pipeline::{FrameStats, Renderer, RendererDescriptor};
pub use quad::{QUAD_INDICES, QUAD_VERTICES, QuadVertex};
pub use rect_pipeline::{RectBatch, RectInstance, RectPipeline};
pub use shared::{
    BLEND_STATE_ADD, BLEND_STATE_REPLACE, PersistentInstanceBuffer, STENCIL_DECREMENT,
    STENCIL_NOOP, STENCIL_TEST, STENCIL_WRITE, SharedPipelineResources, create_pipeline_with_blend,
    create_stencil_texture,
};
pub use texture_atlas::{AtlasRegion, GlyphAtlas, ImageAtlas, TextureAtlas};
pub use textured_quad::{TexturedQuadBatch, TexturedQuadInstance, TexturedQuadPipeline};
