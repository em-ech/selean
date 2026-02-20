//! Text rendering pipeline with instanced glyph quads.
//!
//! Provides `GlyphInstance` (per-glyph GPU data), `TextBatch` (per-frame
//! collection with two-pass UV normalization), and `TextPipeline` (compiled
//! GPU pipeline for drawing glyphs).

use bytemuck::{Pod, Zeroable};
use tracing::warn;

use crate::renderer::texture_atlas::AtlasRegion;
use crate::renderer::{
    BLEND_STATE_ADD, QUAD_INDICES, PersistentInstanceBuffer, QuadVertex, SharedPipelineResources,
    create_pipeline_with_blend,
};
use crate::scene::{BlendMode, Color, SceneNode, TransformColumns};

use super::atlas::GlyphAtlas;
use super::cache::{GlyphCache, GlyphCacheKey};
use super::layout::PositionedGlyph;

/// Maximum number of glyph instances per draw call.
const MAX_INSTANCES_PER_BATCH: usize = 16_384;

/// GPU-compatible per-instance data for a single glyph quad.
///
/// Each glyph is rendered as an instanced unit quad, similar to `RectInstance`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct GlyphInstance {
    /// Top-left position in world space.
    pub pos: [f32; 2],
    /// Width and height in world space.
    pub size: [f32; 2],
    /// UV rectangle in the atlas: `[u_min, v_min, u_max, v_max]`.
    pub uv_rect: [f32; 4],
    /// Text color (RGBA, linear).
    pub color: [f32; 4],
    /// `[opacity, _pad, _pad, _pad]`. Padding for 16-byte alignment.
    pub opacity_pad: [f32; 4],
    /// 2D affine transform columns: `[c0, c1, c2]` (24 bytes).
    pub transform: TransformColumns,
}

impl GlyphInstance {
    /// Returns the vertex buffer layout descriptor for instanced glyph attributes.
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
            // location(5): color
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

    /// Creates a `GlyphInstance` from a positioned glyph and rendering properties.
    #[must_use]
    pub fn from_positioned(
        glyph: &PositionedGlyph,
        uv_rect: [f32; 4],
        color: &Color,
        opacity: f32,
        transform: TransformColumns,
    ) -> Self {
        Self {
            pos: [glyph.x, glyph.y],
            size: [glyph.width, glyph.height],
            uv_rect,
            color: [color.r, color.g, color.b, color.a],
            opacity_pad: [opacity, 0.0, 0.0, 0.0],
            transform,
        }
    }
}

/// A pending glyph stored during the prepare phase before UV normalization.
#[derive(Debug, Clone, Copy)]
struct PendingGlyph {
    pos: [f32; 2],
    size: [f32; 2],
    atlas_region: AtlasRegion,
    color: [f32; 4],
    opacity: f32,
    transform: TransformColumns,
    is_add: bool,
}

/// A batch of glyph instances with two-pass UV normalization.
///
/// During the prepare phase, glyphs are added with pixel-space atlas regions.
/// After all text nodes are processed and the atlas dimensions are final,
/// `finalize_uvs()` converts pixel regions to normalized UV coordinates
/// and partitions instances by blend mode (Normal first, then Add).
pub struct TextBatch {
    /// Pending glyphs with pixel-space atlas regions (pre-finalization).
    pending: Vec<PendingGlyph>,
    /// Finalized Normal blend mode instances ready for GPU upload.
    normal_instances: Vec<GlyphInstance>,
    /// Finalized Add blend mode instances.
    add_instances: Vec<GlyphInstance>,
    /// Combined byte buffer for GPU upload (only used when `add_instances` is non-empty).
    upload_cache: Vec<u8>,
}

impl TextBatch {
    /// Creates a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(512),
            normal_instances: Vec::with_capacity(512),
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

    /// Adds a glyph instance to the batch (already finalized, Normal blend mode).
    pub fn push(&mut self, instance: GlyphInstance) {
        self.normal_instances.push(instance);
    }

    /// Adds all glyph instances for a text node with pixel-space atlas regions.
    ///
    /// Atlas regions are stored in pixel coordinates. Call `finalize_uvs()`
    /// after all nodes are processed to normalize to UV coordinates.
    pub fn push_text_node(
        &mut self,
        node: &SceneNode,
        positioned: &[PositionedGlyph],
        cache: &GlyphCache,
        sdf_size: u16,
    ) {
        if !node.visible || positioned.is_empty() {
            return;
        }

        let color = node.fill.unwrap_or(Color::BLACK);
        let transform = node.world_transform.to_gpu_columns();
        let is_add = match node.blend_mode {
            BlendMode::Add => true,
            BlendMode::Normal => false,
            other => {
                warn!(?other, node_id = %node.id, "Non-native blend mode on text node, falling back to Normal");
                false
            }
        };

        for glyph in positioned {
            let key = GlyphCacheKey {
                glyph_id: glyph.glyph_id,
                sdf_size,
            };

            if let Some(cached) = cache.get(&key) {
                self.pending.push(PendingGlyph {
                    pos: [glyph.x, glyph.y],
                    size: [glyph.width, glyph.height],
                    atlas_region: cached.atlas_region,
                    color: [color.r, color.g, color.b, color.a],
                    opacity: node.opacity,
                    transform,
                    is_add,
                });
            }
        }
    }

    /// Converts all pending glyphs to finalized instances by normalizing
    /// atlas regions to UV coordinates, partitioned by blend mode.
    ///
    /// Call this after all text nodes are processed and the atlas dimensions
    /// are final for this frame.
    #[allow(clippy::cast_precision_loss)]
    pub fn finalize_uvs(&mut self, atlas_width: u32, atlas_height: u32) {
        self.normal_instances.clear();
        self.add_instances.clear();
        self.normal_instances.reserve(self.pending.len());

        for pg in &self.pending {
            let uv_rect = pg.atlas_region.uv_rect(atlas_width, atlas_height);
            let inst = GlyphInstance {
                pos: pg.pos,
                size: pg.size,
                uv_rect,
                color: pg.color,
                opacity_pad: [pg.opacity, 0.0, 0.0, 0.0],
                transform: pg.transform,
            };
            if pg.is_add {
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

    /// Returns the total number of finalized instances in the batch.
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

    /// Returns `true` if the batch is empty (no finalized instances).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.normal_instances.is_empty() && self.add_instances.is_empty()
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

impl Default for TextBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The compiled text rendering pipeline with Normal + Add blend mode variants.
///
/// Shared resources (vertex/index buffers, camera uniform) are provided by
/// [`SharedPipelineResources`] during creation and draw calls.
pub struct TextPipeline {
    /// Pipeline variant for Normal (alpha) blending.
    pipeline_normal: wgpu::RenderPipeline,
    /// Pipeline variant for Additive blending.
    pipeline_add: wgpu::RenderPipeline,
}

impl TextPipeline {
    /// Creates the text pipeline variants for the given device and output format.
    #[must_use]
    pub fn new(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        atlas_bind_group_layout: &wgpu::BindGroupLayout,
        shared: &SharedPipelineResources,
    ) -> Self {
        let shader_source = include_str!("../renderer/shaders/text.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[shared.camera_bind_group_layout(), atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        let buffers = [QuadVertex::layout(), GlyphInstance::layout()];

        let pipeline_normal = create_pipeline_with_blend(
            device,
            "text_pipeline_normal",
            &pipeline_layout,
            &shader_module,
            &buffers,
            target_format,
            wgpu::BlendState::ALPHA_BLENDING,
        );

        let pipeline_add = create_pipeline_with_blend(
            device,
            "text_pipeline_add",
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

    /// Records draw commands for a batch of glyph instances.
    ///
    /// Does nothing if the batch is empty.
    pub fn draw<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        batch: &TextBatch,
        instance_buf: &'a PersistentInstanceBuffer,
        atlas: &'a GlyphAtlas,
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
    fn glyph_instance_size_is_88_bytes() {
        // 2 + 2 + 4 + 4 + 4 + 2 + 2 + 2 = 22 floats * 4 bytes = 88 bytes.
        assert_eq!(std::mem::size_of::<GlyphInstance>(), 88);
    }

    #[test]
    fn glyph_instance_is_pod() {
        let _zeroed: GlyphInstance = bytemuck::Zeroable::zeroed();
    }

    #[test]
    fn batch_starts_empty() {
        let batch = TextBatch::new();
        assert!(batch.is_empty());
        assert_eq!(batch.len(), 0);
        assert_eq!(batch.draw_call_count(), 0);
    }

    #[test]
    fn batch_push_and_len() {
        let mut batch = TextBatch::new();
        batch.push(bytemuck::Zeroable::zeroed());
        batch.push(bytemuck::Zeroable::zeroed());
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    #[test]
    fn batch_clear() {
        let mut batch = TextBatch::new();
        batch.push(bytemuck::Zeroable::zeroed());
        batch.clear();
        assert!(batch.is_empty());
    }

    #[test]
    fn batch_as_bytes_size() {
        let mut batch = TextBatch::new();
        batch.push(bytemuck::Zeroable::zeroed());
        batch.push(bytemuck::Zeroable::zeroed());
        assert_eq!(
            batch.as_bytes().len(),
            2 * std::mem::size_of::<GlyphInstance>()
        );
    }

    #[test]
    fn batch_draw_call_count() {
        let mut batch = TextBatch::new();
        for _ in 0..MAX_INSTANCES_PER_BATCH {
            batch.push(bytemuck::Zeroable::zeroed());
        }
        assert_eq!(batch.draw_call_count(), 1);

        batch.push(bytemuck::Zeroable::zeroed());
        assert_eq!(batch.draw_call_count(), 2);
    }

    #[test]
    fn from_positioned_creates_correct_instance() {
        let glyph = PositionedGlyph {
            glyph_id: 42,
            x: 100.0,
            y: 200.0,
            width: 10.0,
            height: 15.0,
        };
        let uv_rect = [0.0, 0.1, 0.05, 0.15];
        let color = Color::BLACK;

        let instance =
            GlyphInstance::from_positioned(&glyph, uv_rect, &color, 0.8, TransformColumns::identity());

        assert_eq!(instance.pos, [100.0, 200.0]);
        assert_eq!(instance.size, [10.0, 15.0]);
        assert_eq!(instance.uv_rect, uv_rect);
        assert!((instance.opacity_pad[0] - 0.8).abs() < f32::EPSILON);
        assert_eq!(instance.transform, TransformColumns::identity());
    }

    #[test]
    fn finalize_uvs_normalizes_correctly() {
        use crate::scene::{BoundingBox, SceneNodeKind};
        use selean_common::types::NodeId;

        let mut batch = TextBatch::new();
        let node = SceneNode::new(
            NodeId::new(),
            "Label".to_string(),
            SceneNodeKind::Text {
                content: "A".to_string(),
                font_size: 16.0,
            },
            BoundingBox::new(10.0, 20.0, 100.0, 30.0),
        );

        // Simulate adding a pending glyph with a pixel-space region.
        let mut cache = GlyphCache::new();
        let key = GlyphCacheKey {
            glyph_id: 42,
            sdf_size: 48,
        };
        cache.insert(
            key,
            super::super::cache::CachedGlyph {
                atlas_region: AtlasRegion {
                    x: 100,
                    y: 200,
                    width: 48,
                    height: 48,
                },
                bearing_x: 2.0,
                bearing_y: 40.0,
                glyph_width_funits: 600,
                glyph_height_funits: 800,
            },
        );

        let positioned = vec![PositionedGlyph {
            glyph_id: 42,
            x: 15.0,
            y: 25.0,
            width: 10.0,
            height: 12.0,
        }];

        batch.push_text_node(&node, &positioned, &cache, 48);

        // Before finalization, instances should be empty.
        assert!(batch.is_empty());

        // Finalize with atlas size 1024x1024.
        batch.finalize_uvs(1024, 1024);
        assert_eq!(batch.len(), 1);

        let instance = &batch.normal_instances[0];
        assert_eq!(instance.pos, [15.0, 25.0]);
        // UVs should be normalized: 100/1024, 200/1024, 148/1024, 248/1024
        assert!((instance.uv_rect[0] - 100.0 / 1024.0).abs() < f32::EPSILON);
        assert!((instance.uv_rect[1] - 200.0 / 1024.0).abs() < f32::EPSILON);
        assert!((instance.uv_rect[2] - 148.0 / 1024.0).abs() < f32::EPSILON);
        assert!((instance.uv_rect[3] - 248.0 / 1024.0).abs() < f32::EPSILON);
    }
}
