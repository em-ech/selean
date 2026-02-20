//! Rectangle rendering pipeline with instanced draw calls.
//!
//! Provides `RectInstance` (the per-instance GPU data), `RectPipeline` (the
//! compiled render pipeline), and `RectBatch` (the per-frame collection of
//! instances ready for drawing).

use bytemuck::{Pod, Zeroable};
use tracing::warn;

use super::quad::{QUAD_INDICES, QuadVertex};
use super::shared::{
    BLEND_STATE_ADD, PersistentInstanceBuffer, SharedPipelineResources, create_pipeline_with_blend,
};
use crate::scene::{BlendMode, Color, SceneNode, SceneNodeKind, TransformColumns};

/// Maximum number of rectangle instances per draw call.
///
/// 16,384 instances * 96 bytes = ~1.5 MB, well within GPU buffer limits.
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
    /// 2D affine transform columns: `[c0, c1, c2]` (24 bytes).
    pub transform: TransformColumns,
}

impl RectInstance {
    /// Returns the vertex buffer layout descriptor for instanced attributes.
    #[must_use]
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        // Transform columns at locations 8, 9, 10 starting at byte offset 72.
        const TRANSFORM_ATTRS: [wgpu::VertexAttribute; 3] =
            TransformColumns::vertex_attributes(8, 72);

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
            // location(8): transform_c0
            TRANSFORM_ATTRS[0],
            // location(9): transform_c1
            TRANSFORM_ATTRS[1],
            // location(10): transform_c2
            TRANSFORM_ATTRS[2],
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
            transform: node.world_transform.to_gpu_columns(),
        })
    }
}

/// A batch of rectangle instances ready to be rendered.
///
/// Collects `RectInstance` data on the CPU side, partitioned by blend mode.
/// Normal instances come first, followed by Add instances. The GPU upload
/// buffer is contiguous so both groups share a single instance buffer.
pub struct RectBatch {
    /// Normal blend mode instances.
    normal_instances: Vec<RectInstance>,
    /// Additive blend mode instances.
    add_instances: Vec<RectInstance>,
    /// Combined byte buffer for GPU upload (only used when `add_instances` is non-empty).
    upload_cache: Vec<u8>,
}

impl RectBatch {
    /// Creates a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            normal_instances: Vec::with_capacity(1024),
            add_instances: Vec::new(),
            upload_cache: Vec::new(),
        }
    }

    /// Clears the batch for a new frame.
    pub fn clear(&mut self) {
        self.normal_instances.clear();
        self.add_instances.clear();
        self.upload_cache.clear();
    }

    /// Adds a rectangle instance to the batch (Normal blend mode).
    pub fn push(&mut self, instance: RectInstance) {
        self.normal_instances.push(instance);
    }

    /// Adds a scene node to the batch if it's a rectangle type.
    ///
    /// Routes the instance to the Normal or Add sub-batch based on blend mode.
    /// Non-native blend modes (Multiply, Screen, etc.) fall back to Normal
    /// with a warning.
    ///
    /// Returns `true` if the node was added, `false` if it was skipped
    /// (non-rectangle node type, invisible, or zero-size).
    pub fn push_node(&mut self, node: &SceneNode) -> bool {
        if !node.visible || node.bounds.is_empty() {
            return false;
        }

        if let Some(instance) = RectInstance::from_scene_node(node) {
            match node.blend_mode {
                BlendMode::Add => self.add_instances.push(instance),
                BlendMode::Normal => self.normal_instances.push(instance),
                other => {
                    warn!(?other, node_id = %node.id, "Non-native blend mode, falling back to Normal");
                    self.normal_instances.push(instance);
                }
            }
            true
        } else {
            false
        }
    }

    /// Returns the total number of instances in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.normal_instances.len() + self.add_instances.len()
    }

    /// Returns the number of Normal blend mode instances.
    #[must_use]
    pub fn normal_len(&self) -> usize {
        self.normal_instances.len()
    }

    /// Returns the number of Add blend mode instances.
    #[must_use]
    pub fn add_len(&self) -> usize {
        self.add_instances.len()
    }

    /// Returns `true` if the batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.normal_instances.is_empty() && self.add_instances.is_empty()
    }

    /// Prepares the combined upload buffer for GPU upload.
    ///
    /// Must be called after all instances are pushed and before `as_bytes()`.
    /// When there are no Add instances (the common case), `as_bytes()` returns
    /// a zero-copy view of the normal instances. Otherwise, this builds a
    /// contiguous byte buffer with Normal instances first, then Add.
    pub fn finalize_blend(&mut self) {
        if !self.add_instances.is_empty() {
            self.upload_cache.clear();
            let normal_bytes: &[u8] = bytemuck::cast_slice(&self.normal_instances);
            let add_bytes: &[u8] = bytemuck::cast_slice(&self.add_instances);
            self.upload_cache.reserve(normal_bytes.len() + add_bytes.len());
            self.upload_cache.extend_from_slice(normal_bytes);
            self.upload_cache.extend_from_slice(add_bytes);
        }
    }

    /// Returns the instance data as a byte slice for GPU upload.
    ///
    /// Call `finalize_blend()` first if Add instances are present.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        if self.add_instances.is_empty() {
            bytemuck::cast_slice(&self.normal_instances)
        } else {
            &self.upload_cache
        }
    }

    /// Returns the number of draw calls needed to render all instances.
    #[must_use]
    pub fn draw_call_count(&self) -> usize {
        let total = self.len();
        if total == 0 {
            return 0;
        }
        // Each blend mode group gets its own draw call(s).
        let normal_calls = if self.normal_instances.is_empty() {
            0
        } else {
            self.normal_instances.len().div_ceil(MAX_INSTANCES_PER_BATCH)
        };
        let add_calls = if self.add_instances.is_empty() {
            0
        } else {
            self.add_instances.len().div_ceil(MAX_INSTANCES_PER_BATCH)
        };
        normal_calls + add_calls
    }
}

impl Default for RectBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The compiled rectangle rendering pipeline with Normal + Add blend mode variants.
///
/// Shared resources (vertex/index buffers, camera uniform) are provided by
/// [`SharedPipelineResources`] during creation and draw calls.
pub struct RectPipeline {
    /// Pipeline variant for Normal (alpha) blending.
    pipeline_normal: wgpu::RenderPipeline,
    /// Pipeline variant for Additive blending.
    pipeline_add: wgpu::RenderPipeline,
}

impl RectPipeline {
    /// Creates the rectangle pipeline variants for the given device and output format.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        shared: &SharedPipelineResources,
    ) -> Self {
        let shader_source = include_str!("shaders/rect.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rect_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rect_pipeline_layout"),
            bind_group_layouts: &[shared.camera_bind_group_layout()],
            push_constant_ranges: &[],
        });

        let buffers = [QuadVertex::layout(), RectInstance::layout()];

        let pipeline_normal = create_pipeline_with_blend(
            device,
            "rect_pipeline_normal",
            &pipeline_layout,
            &shader_module,
            &buffers,
            target_format,
            wgpu::BlendState::ALPHA_BLENDING,
        );

        let pipeline_add = create_pipeline_with_blend(
            device,
            "rect_pipeline_add",
            &pipeline_layout,
            &shader_module,
            &buffers,
            target_format,
            BLEND_STATE_ADD,
        );

        Self {
            pipeline_normal,
            pipeline_add,
        }
    }

    /// Records draw commands for a batch of rectangle instances.
    ///
    /// Instance data must already be uploaded to `instance_buf` via
    /// [`PersistentInstanceBuffer::upload`] before calling this method.
    /// Normal instances occupy indices `0..normal_len`, Add instances
    /// occupy `normal_len..total_len` in the instance buffer.
    ///
    /// Does nothing if the batch is empty.
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        batch: &RectBatch,
        instance_buf: &'a PersistentInstanceBuffer,
        shared: &'a SharedPipelineResources,
    ) {
        if batch.is_empty() {
            return;
        }

        pass.set_bind_group(0, shared.camera_bind_group(), &[]);
        pass.set_vertex_buffer(0, shared.vertex_buffer().slice(..));
        pass.set_vertex_buffer(1, instance_buf.buffer().slice(..));
        pass.set_index_buffer(shared.index_buffer().slice(..), wgpu::IndexFormat::Uint16);

        #[allow(clippy::cast_possible_truncation)]
        let index_count = QUAD_INDICES.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let normal_count = batch.normal_len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let add_count = batch.add_len() as u32;

        // Draw Normal instances.
        if normal_count > 0 {
            pass.set_pipeline(&self.pipeline_normal);
            pass.draw_indexed(0..index_count, 0, 0..normal_count);
        }

        // Draw Add instances (offset by normal_count in the instance buffer).
        if add_count > 0 {
            pass.set_pipeline(&self.pipeline_add);
            pass.draw_indexed(0..index_count, 0, normal_count..normal_count + add_count);
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;
    use crate::renderer::QUAD_VERTICES;
    use crate::scene::{BoundingBox, SceneNodeKind};
    use selean_common::types::NodeId;

    #[test]
    fn rect_instance_size_is_96_bytes() {
        // 2 + 2 + 4 + 4 + 2 + 4 + 2 + 2 + 2 = 24 floats * 4 bytes = 96 bytes.
        assert_eq!(std::mem::size_of::<RectInstance>(), 96);
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
        batch.finalize_blend();
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
    fn batch_push_node_skips_text() {
        let mut batch = RectBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "Hello".to_string(),
                font_size: 16.0,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 30.0),
        );
        assert!(!batch.push_node(&node));
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_push_node_skips_image() {
        let mut batch = RectBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Photo".to_string(),
            SceneNodeKind::Image {
                asset_ref: "img.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        assert!(!batch.push_node(&node));
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_push_node_skips_vector() {
        let mut batch = RectBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Icon".to_string(),
            SceneNodeKind::Vector {
                path_data: "M0 0 L10 10".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 24.0, 24.0),
        );
        assert!(!batch.push_node(&node));
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_push_node_accepts_group() {
        let mut batch = RectBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Group".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        );
        assert!(batch.push_node(&node));
        assert_eq!(batch.len(), 1);
    }

    #[test]
    fn batch_push_node_routes_by_blend_mode() {
        let mut batch = RectBatch::new();

        // Normal blend mode node.
        let node_normal = SceneNode::new(
            NodeId::new(),
            "Normal".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        assert!(batch.push_node(&node_normal));
        assert_eq!(batch.normal_len(), 1);
        assert_eq!(batch.add_len(), 0);

        // Add blend mode node.
        let mut node_add = SceneNode::new(
            NodeId::new(),
            "Additive".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node_add.blend_mode = BlendMode::Add;
        assert!(batch.push_node(&node_add));
        assert_eq!(batch.normal_len(), 1);
        assert_eq!(batch.add_len(), 1);
        assert_eq!(batch.len(), 2);
    }

    #[test]
    fn batch_finalize_blend_produces_contiguous_bytes() {
        let mut batch = RectBatch::new();
        batch.push(bytemuck::Zeroable::zeroed()); // normal

        // Manually add to add_instances.
        let mut node_add = SceneNode::new(
            NodeId::new(),
            "Add".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node_add.blend_mode = BlendMode::Add;
        batch.push_node(&node_add);

        batch.finalize_blend();
        assert_eq!(
            batch.as_bytes().len(),
            2 * std::mem::size_of::<RectInstance>()
        );
    }

    #[test]
    fn batch_non_native_falls_back_to_normal() {
        let mut batch = RectBatch::new();
        let mut node = SceneNode::new(
            NodeId::new(),
            "Multiply".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.blend_mode = BlendMode::Multiply;
        assert!(batch.push_node(&node));
        assert_eq!(batch.normal_len(), 1);
        assert_eq!(batch.add_len(), 0);
    }
}
