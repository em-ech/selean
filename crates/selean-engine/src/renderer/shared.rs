//! Shared GPU resources used by all instanced rendering pipelines.
//!
//! Both `RectPipeline` and `TextPipeline` use the same unit quad geometry and
//! camera uniform. This module provides those resources once, avoiding
//! duplication and ensuring a single `queue.write_buffer` call per frame
//! for the camera.

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
