//! Shared unit quad geometry used by all instanced rendering pipelines.
//!
//! Both `RectPipeline` and `TextPipeline` render instanced quads. This module
//! provides the shared vertex data and index buffer layout.

use bytemuck::{Pod, Zeroable};

/// A vertex of the unit quad (position + UV).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct QuadVertex {
    /// Position on the unit quad: (0,0), (1,0), (0,1), or (1,1).
    pub position: [f32; 2],
    /// UV coordinates (same as position for rectangles).
    pub uv: [f32; 2],
}

impl QuadVertex {
    /// Returns the vertex buffer layout descriptor for `QuadVertex`.
    #[must_use]
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // location(0): quad_pos
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            },
            // location(1): quad_uv
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 1,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRS,
        }
    }
}

/// The four vertices of the unit quad.
pub const QUAD_VERTICES: &[QuadVertex] = &[
    QuadVertex {
        position: [0.0, 0.0],
        uv: [0.0, 0.0],
    }, // top-left
    QuadVertex {
        position: [1.0, 0.0],
        uv: [1.0, 0.0],
    }, // top-right
    QuadVertex {
        position: [0.0, 1.0],
        uv: [0.0, 1.0],
    }, // bottom-left
    QuadVertex {
        position: [1.0, 1.0],
        uv: [1.0, 1.0],
    }, // bottom-right
];

/// The six indices forming two triangles for the unit quad.
pub const QUAD_INDICES: &[u16] = &[
    0, 1, 2, // top-left triangle
    1, 3, 2, // bottom-right triangle
];
