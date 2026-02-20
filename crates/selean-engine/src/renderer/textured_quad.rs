//! Textured quad rendering pipeline shared by image and vector rendering.
//!
//! Provides `TexturedQuadInstance` (per-instance GPU data), `TexturedQuadBatch`
//! (per-frame collection with two-pass UV normalization), and `TexturedQuadPipeline`
//! (compiled GPU pipeline for drawing textured quads).

use bytemuck::{Pod, Zeroable};
use tracing::warn;

use crate::renderer::texture_atlas::AtlasRegion;
use crate::renderer::{
    BLEND_STATE_ADD, QUAD_INDICES, PersistentInstanceBuffer, QuadVertex, SharedPipelineResources,
    create_pipeline_with_blend,
};
use crate::scene::{BlendMode, Color, SceneNode, TransformColumns};

/// Maximum number of textured quad instances per draw call.
const MAX_INSTANCES_PER_BATCH: usize = 16_384;

/// GPU-compatible per-instance data for a single textured quad.
///
/// Each image or rasterized vector is rendered as an instanced unit quad
/// that samples from the RGBA atlas texture.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct TexturedQuadInstance {
    /// Top-left position in world space.
    pub pos: [f32; 2],
    /// Width and height in world space.
    pub size: [f32; 2],
    /// UV rectangle in the atlas: `[u_min, v_min, u_max, v_max]`.
    pub uv_rect: [f32; 4],
    /// Tint color (RGBA, linear). Multiplied with texture color.
    pub tint_color: [f32; 4],
    /// `[opacity, _pad, _pad, _pad]`. Padding for 16-byte alignment.
    pub opacity_pad: [f32; 4],
    /// 2D affine transform columns: `[c0, c1, c2]` (24 bytes).
    pub transform: TransformColumns,
}

impl TexturedQuadInstance {
    /// Returns the vertex buffer layout descriptor for instanced attributes.
    #[must_use]
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        // Transform columns at locations 7, 8, 9 starting at byte offset 64.
        const TRANSFORM_ATTRS: [wgpu::VertexAttribute; 3] =
            TransformColumns::vertex_attributes(7, 64);

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
            // location(4): uv_rect
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 4,
            },
            // location(5): tint_color
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 32,
                shader_location: 5,
            },
            // location(6): opacity_pad
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 48,
                shader_location: 6,
            },
            // location(7): transform_c0
            TRANSFORM_ATTRS[0],
            // location(8): transform_c1
            TRANSFORM_ATTRS[1],
            // location(9): transform_c2
            TRANSFORM_ATTRS[2],
        ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: ATTRS,
        }
    }
}

/// A pending quad stored during the prepare phase before UV normalization.
#[derive(Debug, Clone, Copy)]
struct PendingQuad {
    pos: [f32; 2],
    size: [f32; 2],
    atlas_region: AtlasRegion,
    tint_color: [f32; 4],
    opacity: f32,
    transform: TransformColumns,
    is_add: bool,
}

/// A batch of textured quad instances with two-pass UV normalization.
///
/// During the prepare phase, quads are added with pixel-space atlas regions.
/// After all nodes are processed and the atlas dimensions are final,
/// `finalize_uvs()` converts pixel regions to normalized UV coordinates
/// and partitions instances by blend mode (Normal first, then Add).
pub struct TexturedQuadBatch {
    /// Pending quads with pixel-space atlas regions (pre-finalization).
    pending: Vec<PendingQuad>,
    /// Finalized Normal blend mode instances.
    normal_instances: Vec<TexturedQuadInstance>,
    /// Finalized Add blend mode instances.
    add_instances: Vec<TexturedQuadInstance>,
    /// Combined byte buffer for GPU upload (only used when `add_instances` is non-empty).
    upload_cache: Vec<u8>,
}

impl TexturedQuadBatch {
    /// Creates a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(256),
            normal_instances: Vec::with_capacity(256),
            add_instances: Vec::new(),
            upload_cache: Vec::new(),
        }
    }

    /// Clears the batch for a new frame.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.normal_instances.clear();
        self.add_instances.clear();
        self.upload_cache.clear();
    }

    /// Adds a textured quad with pixel-space atlas region.
    ///
    /// Non-native blend modes fall back to Normal with a warning.
    pub fn push_pending(
        &mut self,
        node: &SceneNode,
        atlas_region: AtlasRegion,
        tint: Option<Color>,
    ) {
        if !node.visible || node.bounds.is_empty() {
            return;
        }

        let tint_color = tint.unwrap_or(Color::WHITE);
        let is_add = match node.blend_mode {
            BlendMode::Add => true,
            BlendMode::Normal => false,
            other => {
                warn!(?other, node_id = %node.id, "Non-native blend mode on textured quad, falling back to Normal");
                false
            }
        };

        self.pending.push(PendingQuad {
            pos: [node.bounds.x, node.bounds.y],
            size: [node.bounds.width, node.bounds.height],
            atlas_region,
            tint_color: [tint_color.r, tint_color.g, tint_color.b, tint_color.a],
            opacity: node.opacity,
            transform: node.world_transform.to_gpu_columns(),
            is_add,
        });
    }

    /// Converts all pending quads to finalized instances by normalizing
    /// atlas regions to UV coordinates, partitioned by blend mode.
    #[allow(clippy::cast_precision_loss)]
    pub fn finalize_uvs(&mut self, atlas_width: u32, atlas_height: u32) {
        self.normal_instances.clear();
        self.add_instances.clear();
        self.normal_instances.reserve(self.pending.len());

        for pq in &self.pending {
            let uv_rect = pq.atlas_region.uv_rect(atlas_width, atlas_height);
            let inst = TexturedQuadInstance {
                pos: pq.pos,
                size: pq.size,
                uv_rect,
                tint_color: pq.tint_color,
                opacity_pad: [pq.opacity, 0.0, 0.0, 0.0],
                transform: pq.transform,
            };
            if pq.is_add {
                self.add_instances.push(inst);
            } else {
                self.normal_instances.push(inst);
            }
        }

        // Build upload cache if needed.
        if !self.add_instances.is_empty() {
            self.upload_cache.clear();
            let normal_bytes: &[u8] = bytemuck::cast_slice(&self.normal_instances);
            let add_bytes: &[u8] = bytemuck::cast_slice(&self.add_instances);
            self.upload_cache.reserve(normal_bytes.len() + add_bytes.len());
            self.upload_cache.extend_from_slice(normal_bytes);
            self.upload_cache.extend_from_slice(add_bytes);
        }
    }

    /// Returns the total number of finalized instances.
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

    /// Returns `true` if the batch has no finalized instances.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.normal_instances.is_empty() && self.add_instances.is_empty()
    }

    /// Returns the number of pending (pre-finalization) quads.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Returns the instance data as a byte slice for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        if self.add_instances.is_empty() {
            bytemuck::cast_slice(&self.normal_instances)
        } else {
            &self.upload_cache
        }
    }

    /// Returns the number of draw calls needed.
    #[must_use]
    pub fn draw_call_count(&self) -> usize {
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

impl Default for TexturedQuadBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The compiled textured quad rendering pipeline with Normal + Add blend mode variants.
///
/// Shared by both image and vector rendering. Samples from an RGBA atlas
/// texture, multiplies by tint color, and applies opacity.
pub struct TexturedQuadPipeline {
    /// Pipeline variant for Normal (alpha) blending.
    pipeline_normal: wgpu::RenderPipeline,
    /// Pipeline variant for Additive blending.
    pipeline_add: wgpu::RenderPipeline,
}

impl TexturedQuadPipeline {
    /// Creates the textured quad pipeline variants for the given device and output format.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        atlas_bind_group_layout: &wgpu::BindGroupLayout,
        shared: &SharedPipelineResources,
    ) -> Self {
        let shader_source = include_str!("shaders/textured_quad.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("textured_quad_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("textured_quad_pipeline_layout"),
            bind_group_layouts: &[shared.camera_bind_group_layout(), atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        let buffers = [QuadVertex::layout(), TexturedQuadInstance::layout()];

        let pipeline_normal = create_pipeline_with_blend(
            device,
            "textured_quad_pipeline_normal",
            &pipeline_layout,
            &shader_module,
            &buffers,
            target_format,
            wgpu::BlendState::ALPHA_BLENDING,
        );

        let pipeline_add = create_pipeline_with_blend(
            device,
            "textured_quad_pipeline_add",
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

    /// Records draw commands for a batch of textured quad instances.
    ///
    /// `finalize_uvs()` must have been called on the batch before drawing.
    /// Does nothing if the batch is empty.
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        batch: &TexturedQuadBatch,
        instance_buf: &'a PersistentInstanceBuffer,
        atlas: &'a crate::renderer::texture_atlas::TextureAtlas<4>,
        shared: &'a SharedPipelineResources,
    ) {
        if batch.is_empty() {
            return;
        }

        pass.set_bind_group(0, shared.camera_bind_group(), &[]);
        pass.set_bind_group(1, atlas.bind_group(), &[]);
        pass.set_vertex_buffer(0, shared.vertex_buffer().slice(..));
        pass.set_vertex_buffer(1, instance_buf.buffer().slice(..));
        pass.set_index_buffer(shared.index_buffer().slice(..), wgpu::IndexFormat::Uint16);

        #[allow(clippy::cast_possible_truncation)]
        let index_count = QUAD_INDICES.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let normal_count = batch.normal_len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let add_count = batch.add_len() as u32;

        if normal_count > 0 {
            pass.set_pipeline(&self.pipeline_normal);
            pass.draw_indexed(0..index_count, 0, 0..normal_count);
        }

        if add_count > 0 {
            pass.set_pipeline(&self.pipeline_add);
            pass.draw_indexed(0..index_count, 0, normal_count..normal_count + add_count);
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn textured_quad_instance_size_is_88_bytes() {
        // 2 + 2 + 4 + 4 + 4 + 2 + 2 + 2 = 22 floats * 4 bytes = 88 bytes.
        assert_eq!(std::mem::size_of::<TexturedQuadInstance>(), 88);
    }

    #[test]
    fn textured_quad_instance_is_pod() {
        let _zeroed: TexturedQuadInstance = bytemuck::Zeroable::zeroed();
    }

    #[test]
    fn batch_starts_empty() {
        let batch = TexturedQuadBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
        assert_eq!(batch.pending_count(), 0);
        assert_eq!(batch.draw_call_count(), 0);
    }

    #[test]
    fn batch_clear() {
        use crate::scene::{BoundingBox, SceneNodeKind};
        use selean_common::types::NodeId;

        let mut batch = TexturedQuadBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Img".to_string(),
            SceneNodeKind::Image {
                asset_ref: "test.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );

        batch.push_pending(
            &node,
            AtlasRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            None,
        );
        assert_eq!(batch.pending_count(), 1);

        batch.clear();
        assert_eq!(batch.pending_count(), 0);
        assert!(batch.is_empty());
    }

    #[test]
    fn finalize_uvs_normalizes_correctly() {
        use crate::scene::{BoundingBox, SceneNodeKind};
        use selean_common::types::NodeId;

        let mut batch = TexturedQuadBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Img".to_string(),
            SceneNodeKind::Image {
                asset_ref: "test.png".to_string(),
            },
            BoundingBox::new(10.0, 20.0, 100.0, 80.0),
        );

        batch.push_pending(
            &node,
            AtlasRegion {
                x: 0,
                y: 0,
                width: 256,
                height: 256,
            },
            None,
        );

        batch.finalize_uvs(1024, 1024);
        assert_eq!(batch.len(), 1);

        let instance = &batch.normal_instances[0];
        assert_eq!(instance.pos, [10.0, 20.0]);
        assert_eq!(instance.size, [100.0, 80.0]);
        assert!((instance.uv_rect[0]).abs() < f32::EPSILON); // u_min = 0/1024
        assert!((instance.uv_rect[1]).abs() < f32::EPSILON); // v_min = 0/1024
        assert!((instance.uv_rect[2] - 0.25).abs() < f32::EPSILON); // u_max = 256/1024
        assert!((instance.uv_rect[3] - 0.25).abs() < f32::EPSILON); // v_max = 256/1024
    }

    #[test]
    fn push_pending_skips_invisible() {
        use crate::scene::{BoundingBox, SceneNodeKind};
        use selean_common::types::NodeId;

        let mut batch = TexturedQuadBatch::new();
        let mut node = SceneNode::new(
            NodeId::new(),
            "Hidden".to_string(),
            SceneNodeKind::Image {
                asset_ref: "test.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.visible = false;

        batch.push_pending(
            &node,
            AtlasRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            None,
        );
        assert_eq!(batch.pending_count(), 0);
    }

    #[test]
    fn batch_draw_call_count() {
        use crate::scene::{BoundingBox, SceneNodeKind};
        use selean_common::types::NodeId;

        let mut batch = TexturedQuadBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Img".to_string(),
            SceneNodeKind::Image {
                asset_ref: "test.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );

        for _ in 0..MAX_INSTANCES_PER_BATCH {
            batch.push_pending(
                &node,
                AtlasRegion {
                    x: 0,
                    y: 0,
                    width: 64,
                    height: 64,
                },
                None,
            );
        }
        batch.finalize_uvs(1024, 1024);
        assert_eq!(batch.draw_call_count(), 1);

        batch.push_pending(
            &node,
            AtlasRegion {
                x: 0,
                y: 0,
                width: 64,
                height: 64,
            },
            None,
        );
        batch.finalize_uvs(1024, 1024);
        assert_eq!(batch.draw_call_count(), 2);
    }
}
