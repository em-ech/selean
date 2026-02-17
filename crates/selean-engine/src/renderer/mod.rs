//! WebGPU rendering pipeline.
//!
//! Handles GPU device initialization, surface management, shader pipelines,
//! and draw call batching for the Selean canvas.

mod camera;
mod gpu;
mod pipeline;
mod rect_pipeline;

pub use camera::{Camera, CameraUniform};
pub use gpu::{GpuContext, GpuContextDescriptor};
pub use pipeline::{FrameStats, Renderer, RendererDescriptor};
pub use rect_pipeline::{RectBatch, RectInstance, RectPipeline};
