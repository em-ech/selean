//! Shared GPU resources used by all instanced rendering pipelines.
//!
//! All instanced pipelines use the same unit quad geometry and camera uniform.
//! This module provides those resources once, avoiding duplication and ensuring
//! a single `queue.write_buffer` call per frame for the camera.
//!
//! Also provides the shared blend state constants and pipeline creation helper
//! used by all pipeline types to create Normal + Add blend mode variants.

use wgpu::util::DeviceExt;

use super::camera::CameraUniform;
use super::quad::{QUAD_INDICES, QUAD_VERTICES};

/// GPU resources shared across all instanced rendering pipelines.
///
/// Created once at renderer initialization. Both `RectPipeline` and
/// `TextPipeline` reference these during pipeline creation (for the camera
/// bind group layout) and during draw calls (for the vertex/index/camera
/// buffers).
pub struct SharedPipelineResources {
    /// Vertex buffer for the unit quad (shared by all instances).
    vertex_buffer: wgpu::Buffer,
    /// Index buffer for the unit quad.
    index_buffer: wgpu::Buffer,
    /// Camera uniform buffer.
    camera_buffer: wgpu::Buffer,
    /// Bind group layout for the camera uniform.
    camera_bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group containing the camera uniform.
    camera_bind_group: wgpu::BindGroup,
}

impl SharedPipelineResources {
    /// Creates shared GPU resources.
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shared_quad_vertex_buffer"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("shared_quad_index_buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shared_camera_uniform_buffer"),
            size: CameraUniform::size(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("shared_camera_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shared_camera_bind_group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        Self {
            vertex_buffer,
            index_buffer,
            camera_buffer,
            camera_bind_group_layout,
            camera_bind_group,
        }
    }

    /// Returns the camera bind group layout (for pipeline creation).
    #[must_use]
    pub fn camera_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.camera_bind_group_layout
    }

    /// Returns the camera bind group (for draw calls).
    #[must_use]
    pub fn camera_bind_group(&self) -> &wgpu::BindGroup {
        &self.camera_bind_group
    }

    /// Returns the shared vertex buffer.
    #[must_use]
    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    /// Returns the shared index buffer.
    #[must_use]
    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }

    /// Updates the camera uniform buffer. Called once per frame.
    pub fn update_camera(&self, queue: &wgpu::Queue, uniform: &CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(uniform));
    }
}

/// A GPU vertex buffer that persists across frames, growing only when needed.
///
/// Avoids per-frame `device.create_buffer_init()` by reusing a single buffer
/// and writing data via `queue.write_buffer()`. The buffer doubles in capacity
/// when the data exceeds the current size.
pub struct PersistentInstanceBuffer {
    /// The GPU buffer.
    buffer: wgpu::Buffer,
    /// Current buffer capacity in bytes.
    capacity: usize,
    /// Label for debug purposes.
    label: &'static str,
}

impl PersistentInstanceBuffer {
    /// Creates a new persistent instance buffer.
    #[must_use]
    pub fn new(device: &wgpu::Device, label: &'static str, initial_capacity: usize) -> Self {
        let capacity = initial_capacity.max(64); // Minimum 64 bytes.
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity,
            label,
        }
    }

    /// Uploads instance data to the GPU buffer, growing if necessary.
    ///
    /// If the data exceeds the current capacity, a new buffer is allocated
    /// at the next power-of-two size. Otherwise, data is written in-place.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        if data.len() > self.capacity {
            let new_capacity = data.len().next_power_of_two();
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: new_capacity as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.capacity = new_capacity;
        }

        queue.write_buffer(&self.buffer, 0, data);
    }

    /// Returns a reference to the underlying GPU buffer.
    #[must_use]
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }
}

// --- Blend state constants ---

/// Replace blending: source color overwrites destination entirely.
///
/// Used when rendering elements with non-native blend modes. The element is
/// written with no blending; the actual compositing is performed by a subsequent
/// shader-based blend pass.
pub const BLEND_STATE_REPLACE: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent::REPLACE,
    alpha: wgpu::BlendComponent::REPLACE,
};

/// Additive blending: source color is added to destination.
///
/// Used for the `BlendMode::Add` pipeline variant.
pub const BLEND_STATE_ADD: wgpu::BlendState = wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    },
};

// --- Shared pipeline creation helper ---

/// No-op stencil state: always passes, never writes. Used by pipelines that
/// don't interact with stencil but must be compatible with the stencil attachment.
pub const STENCIL_NOOP: wgpu::DepthStencilState = wgpu::DepthStencilState {
    format: wgpu::TextureFormat::Depth24PlusStencil8,
    depth_write_enabled: false,
    depth_compare: wgpu::CompareFunction::Always,
    stencil: wgpu::StencilState {
        front: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        },
        back: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        },
        read_mask: 0xFF,
        write_mask: 0x00,
    },
    bias: wgpu::DepthBiasState {
        constant: 0,
        slope_scale: 0.0,
        clamp: 0.0,
    },
};

/// Stencil-test state: only passes where stencil equals the reference value.
/// Used by pipelines that render content inside a stencil clip region.
pub const STENCIL_TEST: wgpu::DepthStencilState = wgpu::DepthStencilState {
    format: wgpu::TextureFormat::Depth24PlusStencil8,
    depth_write_enabled: false,
    depth_compare: wgpu::CompareFunction::Always,
    stencil: wgpu::StencilState {
        front: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        },
        back: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Equal,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        },
        read_mask: 0xFF,
        write_mask: 0x00,
    },
    bias: wgpu::DepthBiasState {
        constant: 0,
        slope_scale: 0.0,
        clamp: 0.0,
    },
};

/// Stencil-write state: increments stencil buffer on pass (clip push).
/// Used to write clip shapes into the stencil buffer.
pub const STENCIL_WRITE: wgpu::DepthStencilState = wgpu::DepthStencilState {
    format: wgpu::TextureFormat::Depth24PlusStencil8,
    depth_write_enabled: false,
    depth_compare: wgpu::CompareFunction::Always,
    stencil: wgpu::StencilState {
        front: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::IncrementClamp,
        },
        back: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::IncrementClamp,
        },
        read_mask: 0xFF,
        write_mask: 0xFF,
    },
    bias: wgpu::DepthBiasState {
        constant: 0,
        slope_scale: 0.0,
        clamp: 0.0,
    },
};

/// Stencil-decrement state: decrements stencil buffer on pass (clip pop).
/// Used to restore the stencil buffer when popping a clip boundary.
pub const STENCIL_DECREMENT: wgpu::DepthStencilState = wgpu::DepthStencilState {
    format: wgpu::TextureFormat::Depth24PlusStencil8,
    depth_write_enabled: false,
    depth_compare: wgpu::CompareFunction::Always,
    stencil: wgpu::StencilState {
        front: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::DecrementClamp,
        },
        back: wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::DecrementClamp,
        },
        read_mask: 0xFF,
        write_mask: 0xFF,
    },
    bias: wgpu::DepthBiasState {
        constant: 0,
        slope_scale: 0.0,
        clamp: 0.0,
    },
};

/// Creates a render pipeline with the given blend state, depth/stencil state,
/// and color write mask.
///
/// This helper encapsulates the common pipeline descriptor structure shared by
/// all instanced rendering pipelines (rect, text, textured quad).
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn create_pipeline_with_blend(
    device: &wgpu::Device,
    label: &str,
    pipeline_layout: &wgpu::PipelineLayout,
    shader_module: &wgpu::ShaderModule,
    buffers: &[wgpu::VertexBufferLayout<'_>],
    target_format: wgpu::TextureFormat,
    blend_state: wgpu::BlendState,
    depth_stencil: Option<wgpu::DepthStencilState>,
    color_writes: wgpu::ColorWrites,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(pipeline_layout),
        vertex: wgpu::VertexState {
            module: shader_module,
            entry_point: Some("vs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        },
        depth_stencil,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader_module,
            entry_point: Some("fs_main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(blend_state),
                write_mask: color_writes,
            })],
        }),
        multiview: None,
        cache: None,
    })
}

/// Creates the stencil texture and view for the given dimensions.
#[must_use]
pub fn create_stencil_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("stencil_texture"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
