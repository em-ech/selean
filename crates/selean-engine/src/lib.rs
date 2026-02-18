//! GPU-accelerated 2D rendering engine for the Selean design platform.
//!
//! This crate provides:
//! - WebGPU-based rendering pipeline with draw call batching
//! - Scene graph with dirty flag tracking for incremental updates
//! - R-tree spatial index for viewport virtualization
//! - SDF text rendering with `HarfBuzz` shaping

pub mod renderer;
pub mod scene;
pub mod spatial;
pub mod text;
