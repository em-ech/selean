//! WebGPU rendering pipeline.
//!
//! Handles GPU device initialization, surface management, shader pipelines,
//! and draw call batching for the Selean canvas.

mod gpu;

pub use gpu::{GpuContext, GpuContextDescriptor};
