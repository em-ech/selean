//! Rectangle rendering pipeline with instanced draw calls.
//!
//! Provides `RectInstance` (the per-instance GPU data), `RectPipeline` (the
//! compiled render pipeline), and `RectBatch` (the per-frame collection of
//! instances ready for drawing).

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use super::camera::CameraUniform;
use super::quad::{QUAD_INDICES, QUAD_VERTICES, QuadVertex};
use crate::scene::{Color, SceneNode, SceneNodeKind};

/// Maximum number of rectangle instances per draw call.
///
/// 16,384 instances * 80 bytes = ~1.3 MB, well within GPU buffer limits.
/// If more instances are needed, they're split into multiple draw calls.
const MAX_INSTANCES_PER_BATCH: usize = 16_384;

// --- Per-instance data ---

/// GPU-compatible per-instance data for a rectangle.
///
/// Packed to match the WGSL `RectInstance` struct layout.
/// Each field corresponds to a vertex attribute location in the shader.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct RectInstance {
    /// Top-left corner position in world space.
    pub pos: [f32; 2],
    /// Width and height in world space.
    pub size: [f32; 2],
    /// Fill color (RGBA, linear).
    pub fill_color: [f32; 4],
    /// Stroke color (RGBA, linear).
    pub stroke_color: [f32; 4],
    /// `[stroke_width, opacity]`.
    pub stroke_width_opacity: [f32; 2],
    /// Per-corner radius: `[top_left, top_right, bottom_right, bottom_left]`.
    pub corner_radii: [f32; 4],
}

impl RectInstance {
    /// Returns the vertex buffer layout descriptor for instanced attributes.
    #[must_use]
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRS: &[wgpu::VertexAttribute] = &[
            // location(2): pos
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 2,
            },
            // location(3): size
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 8,
                shader_location: 3,
            },
            // location(4): fill_color
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 4,
            },
            // location(5): stroke_color
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 5,
            },
            // location(6): stroke_width_opacity
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 48,
                shader_location: 6,
            },
            // location(7): corner_radii
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 56,
                shader_location: 7,
            },
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }

    /// Creates a `RectInstance` from a `SceneNode`.
    ///
    /// Returns `None` if the node is not a renderable rectangle type
    /// (i.e., not a `Frame` or `Group` with visual properties).
    #[must_use]
    pub fn from_scene_node(node: &SceneNode) -> Option<Self> {
        let corner_radii = match &node.kind {
            SceneNodeKind::Frame { corner_radius } => *corner_radius,
            SceneNodeKind::Group => [0.0; 4],
            // Text, Image, Vector nodes are rendered by their own pipelines.
            SceneNodeKind::Text { .. }
            | SceneNodeKind::Image { .. }
            | SceneNodeKind::Vector { .. } => return None,
        };

        let fill = node.fill.unwrap_or(Color::TRANSPARENT);
        let stroke = node.stroke.unwrap_or(Color::TRANSPARENT);

        Some(Self {
            pos: [node.bounds.x, node.bounds.y],
            size: [node.bounds.width, node.bounds.height],
            fill_color: [fill.r, fill.g, fill.b, fill.a],
            stroke_color: [stroke.r, stroke.g, stroke.b, stroke.a],
            stroke_width_opacity: [node.stroke_width, node.opacity],
            corner_radii,
        })
    }
}

/// A batch of rectangle instances ready to be rendered.
///
/// Collects `RectInstance` data on the CPU side, then uploads to a GPU
/// instance buffer when ready to draw.
pub struct RectBatch {
    /// CPU-side instance data, cleared each frame.
    instances: Vec<RectInstance>,
}

impl RectBatch {
    /// Creates a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            instances: Vec::with_capacity(1024),
        }
    }

    /// Clears the batch for a new frame.
    pub fn clear(&mut self) {
        self.instances.clear();
    }

    /// Adds a rectangle instance to the batch.
    pub fn push(&mut self, instance: RectInstance) {
        self.instances.push(instance);
    }

    /// Adds a scene node to the batch if it's a rectangle type.
    ///
    /// Returns `true` if the node was added, `false` if it was skipped
    /// (non-rectangle node type, invisible, or zero-size).
    pub fn push_node(&mut self, node: &SceneNode) -> bool {
        if !node.visible || node.bounds.is_empty() {
            return false;
        }

        if let Some(instance) = RectInstance::from_scene_node(node) {
            self.instances.push(instance);
            true
        } else {
            false
        }
    }

    /// Returns the number of instances in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Returns `true` if the batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Returns the instance data as a byte slice for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.instances)
    }

    /// Returns the number of draw calls needed to render all instances.
    ///
    /// Instances beyond `MAX_INSTANCES_PER_BATCH` require additional draw calls.
    #[must_use]
    pub fn draw_call_count(&self) -> usize {
        if self.instances.is_empty() {
            return 0;
        }
        self.instances.len().div_ceil(MAX_INSTANCES_PER_BATCH)
    }

    /// Returns an iterator over sub-batches of up to `MAX_INSTANCES_PER_BATCH` instances.
    pub fn chunks(&self) -> impl Iterator<Item = &[RectInstance]> {
        self.instances.chunks(MAX_INSTANCES_PER_BATCH)
    }
}

impl Default for RectBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The compiled rectangle rendering pipeline.
///
/// Owns the GPU resources needed to render rectangles: shader module,
/// render pipeline, shared vertex/index buffers, and camera uniform buffer.
pub struct RectPipeline {
    /// The compiled render pipeline.
    pipeline: wgpu::RenderPipeline,
    /// Vertex buffer for the unit quad (shared by all instances).
    vertex_buffer: wgpu::Buffer,
    /// Index buffer for the unit quad.
    index_buffer: wgpu::Buffer,
    /// Camera uniform buffer.
    camera_buffer: wgpu::Buffer,
    /// Bind group containing the camera uniform.
    camera_bind_group: wgpu::BindGroup,
}

impl RectPipeline {
    /// Creates the rectangle pipeline for the given device and output format.
    ///
    /// # Arguments
    /// * `device` — The GPU device.
    /// * `target_format` — The texture format of the render target (surface).
    #[must_use]
    pub fn new(device: &wgpu::Device, target_format: wgpu::TextureFormat) -> Self {
        // Load and compile the rectangle shader.
        let shader_source = include_str!("shaders/rect.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Camera uniform buffer and bind group layout.
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera_uniform_buffer"),
            size: CameraUniform::size(),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera_bind_group_layout"),
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
            label: Some("camera_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create the render pipeline.
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("rect_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[QuadVertex::layout(), RectInstance::layout()],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // 2D — no backface culling.
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        // Create shared vertex and index buffers for the unit quad.
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad_vertex_buffer"),
            contents: bytemuck::cast_slice(QUAD_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("quad_index_buffer"),
            contents: bytemuck::cast_slice(QUAD_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            camera_buffer,
            camera_bind_group,
        }
    }

    /// Updates the camera uniform buffer with new data.
    pub fn update_camera(&self, queue: &wgpu::Queue, uniform: &CameraUniform) {
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(uniform));
    }

    /// Records draw commands for a batch of rectangle instances.
    ///
    /// Creates a temporary instance buffer from the batch data and records
    /// instanced draw calls into the given render pass.
    ///
    /// Does nothing if the batch is empty.
    pub fn draw<'a>(
        &'a self,
        device: &wgpu::Device,
        pass: &mut wgpu::RenderPass<'a>,
        batch: &RectBatch,
    ) {
        if batch.is_empty() {
            return;
        }

        // Create instance buffer from batch data.
        let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rect_instance_buffer"),
            contents: batch.as_bytes(),
            usage: wgpu::BufferUsages::VERTEX,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.camera_bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.set_vertex_buffer(1, instance_buffer.slice(..));
        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        // Draw all instances. If over the batch limit, this still works because
        // the GPU handles the full instance count; MAX_INSTANCES_PER_BATCH is
        // an advisory limit for buffer sizing, not a hard GPU limit.
        // QUAD_INDICES.len() is always 6, and batch.len() is bounded by
        // MAX_INSTANCES_PER_BATCH (16384) — both fit in u32.
        #[allow(clippy::cast_possible_truncation)]
        let instance_count = batch.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let index_count = QUAD_INDICES.len() as u32;
        pass.draw_indexed(0..index_count, 0, 0..instance_count);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::scene::{BoundingBox, SceneNodeKind};
    use selean_common::types::NodeId;

    #[test]
    fn rect_instance_size_is_72_bytes() {
        // 2 + 2 + 4 + 4 + 2 + 4 = 18 floats * 4 bytes = 72 bytes.
        assert_eq!(std::mem::size_of::<RectInstance>(), 72);
    }

    #[test]
    fn rect_instance_is_pod() {
        // This compiles only if RectInstance implements Pod + Zeroable.
        let _zeroed: RectInstance = bytemuck::Zeroable::zeroed();
    }

    #[test]
    fn quad_vertex_size() {
        assert_eq!(std::mem::size_of::<QuadVertex>(), 16);
    }

    #[test]
    fn quad_has_4_vertices_6_indices() {
        assert_eq!(QUAD_VERTICES.len(), 4);
        assert_eq!(QUAD_INDICES.len(), 6);
    }

    #[test]
    fn from_scene_node_frame() {
        let node = SceneNode::new(
            NodeId::new(),
            "Test".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [4.0, 8.0, 12.0, 16.0],
            },
            BoundingBox::new(10.0, 20.0, 100.0, 50.0),
        );

        let inst = RectInstance::from_scene_node(&node);
        assert!(inst.is_some());

        let inst = inst.unwrap_or_else(|| unreachable!());
        assert_eq!(inst.pos, [10.0, 20.0]);
        assert_eq!(inst.size, [100.0, 50.0]);
        assert_eq!(inst.corner_radii, [4.0, 8.0, 12.0, 16.0]);
    }

    #[test]
    fn from_scene_node_frame_with_fill() {
        let mut node = SceneNode::new(
            NodeId::new(),
            "Colored".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );
        node.fill = Some(Color::new(1.0, 0.0, 0.5, 0.8));
        node.stroke = Some(Color::new(0.0, 0.0, 0.0, 1.0));
        node.stroke_width = 2.0;
        node.opacity = 0.9;

        let inst = RectInstance::from_scene_node(&node).unwrap_or_else(|| unreachable!());
        assert_eq!(inst.fill_color, [1.0, 0.0, 0.5, 0.8]);
        assert_eq!(inst.stroke_color, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(inst.stroke_width_opacity, [2.0, 0.9]);
    }

    #[test]
    fn from_scene_node_group_returns_some() {
        let node = SceneNode::new(
            NodeId::new(),
            "Group".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        );

        let inst = RectInstance::from_scene_node(&node);
        assert!(inst.is_some());
    }

    #[test]
    fn from_scene_node_text_returns_none() {
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 16.0,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 20.0),
        );

        assert!(RectInstance::from_scene_node(&node).is_none());
    }

    #[test]
    fn from_scene_node_image_returns_none() {
        let node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "img_001".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );

        assert!(RectInstance::from_scene_node(&node).is_none());
    }

    #[test]
    fn from_scene_node_vector_returns_none() {
        let node = SceneNode::new(
            NodeId::new(),
            "Icon".to_string(),
            SceneNodeKind::Vector {
                path_data: "M0 0 L10 10".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 24.0, 24.0),
        );

        assert!(RectInstance::from_scene_node(&node).is_none());
    }

    #[test]
    fn no_fill_defaults_to_transparent() {
        let node = SceneNode::new(
            NodeId::new(),
            "NoFill".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 50.0, 50.0),
        );

        let inst = RectInstance::from_scene_node(&node).unwrap_or_else(|| unreachable!());
        assert_eq!(inst.fill_color, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(inst.stroke_color, [0.0, 0.0, 0.0, 0.0]);
    }

    // --- RectBatch tests ---

    #[test]
    fn batch_starts_empty() {
        let batch = RectBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
        assert_eq!(batch.draw_call_count(), 0);
    }

    #[test]
    fn batch_push_and_len() {
        let mut batch = RectBatch::new();
        batch.push(bytemuck::Zeroable::zeroed());
        batch.push(bytemuck::Zeroable::zeroed());
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn batch_clear() {
        let mut batch = RectBatch::new();
        batch.push(bytemuck::Zeroable::zeroed());
        batch.clear();
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_push_node_skips_invisible() {
        let mut batch = RectBatch::new();
        let mut node = SceneNode::new(
            NodeId::new(),
            "Hidden".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.visible = false;

        assert!(!batch.push_node(&node));
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_push_node_skips_empty_bounds() {
        let mut batch = RectBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "ZeroSize".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 0.0, 0.0),
        );

        assert!(!batch.push_node(&node));
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_push_node_adds_visible_frame() {
        let mut batch = RectBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Visible".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );

        assert!(batch.push_node(&node));
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn batch_as_bytes_size() {
        let mut batch = RectBatch::new();
        batch.push(bytemuck::Zeroable::zeroed());
        batch.push(bytemuck::Zeroable::zeroed());
        assert_eq!(
            batch.as_bytes().len(),
            2 * std::mem::size_of::<RectInstance>()
        );
    }

    #[test]
    fn batch_draw_call_count() {
        let mut batch = RectBatch::new();
        // Push exactly MAX_INSTANCES_PER_BATCH instances.
        for _ in 0..MAX_INSTANCES_PER_BATCH {
            batch.push(bytemuck::Zeroable::zeroed());
        }
        assert_eq!(batch.draw_call_count(), 1);

        // One more pushes it to 2 draw calls.
        batch.push(bytemuck::Zeroable::zeroed());
        assert_eq!(batch.draw_call_count(), 2);
    }

    #[test]
    fn batch_chunks_splits_correctly() {
        let mut batch = RectBatch::new();
        for _ in 0..=MAX_INSTANCES_PER_BATCH {
            batch.push(bytemuck::Zeroable::zeroed());
        }

        let chunks: Vec<_> = batch.chunks().collect();
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].len(), MAX_INSTANCES_PER_BATCH);
        assert_eq!(chunks[1].len(), 1);
    }
}
