//! Text rendering pipeline with instanced glyph quads.
//!
//! Provides `GlyphInstance` (per-glyph GPU data), `TextBatch` (per-frame
//! collection with two-pass UV normalization), and `TextPipeline` (compiled
//! GPU pipeline for drawing glyphs).

use bytemuck::{Pod, Zeroable};

use crate::renderer::texture_atlas::AtlasRegion;
use crate::renderer::{QUAD_INDICES, PersistentInstanceBuffer, QuadVertex, SharedPipelineResources};
use crate::scene::{Color, SceneNode};

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
}

impl GlyphInstance {
    /// Returns the vertex buffer layout descriptor for instanced glyph attributes.
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
    ) -> Self {
        Self {
            pos: [glyph.x, glyph.y],
            size: [glyph.width, glyph.height],
            uv_rect,
            color: [color.r, color.g, color.b, color.a],
            opacity_pad: [opacity, 0.0, 0.0, 0.0],
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
}

/// A batch of glyph instances with two-pass UV normalization.
///
/// During the prepare phase, glyphs are added with pixel-space atlas regions.
/// After all text nodes are processed and the atlas dimensions are final,
/// `finalize_uvs()` converts pixel regions to normalized UV coordinates.
/// This prevents stale UVs when the atlas grows mid-loop.
pub struct TextBatch {
    /// Pending glyphs with pixel-space atlas regions (pre-finalization).
    pending: Vec<PendingGlyph>,
    /// Finalized instance data ready for GPU upload.
    instances: Vec<GlyphInstance>,
}

impl TextBatch {
    /// Creates a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(512),
            instances: Vec::with_capacity(512),
        }
    }

    /// Clears the batch for a new frame.
    pub fn clear(&mut self) {
        self.pending.clear();
        self.instances.clear();
    }

    /// Adds a glyph instance to the batch (already finalized, for backward compat).
    pub fn push(&mut self, instance: GlyphInstance) {
        self.instances.push(instance);
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
                });
            }
        }
    }

    /// Converts all pending glyphs to finalized instances by normalizing
    /// atlas regions to UV coordinates.
    ///
    /// Call this after all text nodes are processed and the atlas dimensions
    /// are final for this frame.
    #[allow(clippy::cast_precision_loss)]
    pub fn finalize_uvs(&mut self, atlas_width: u32, atlas_height: u32) {
        self.instances.clear();
        self.instances.reserve(self.pending.len());

        for pg in &self.pending {
            let uv_rect = pg.atlas_region.uv_rect(atlas_width, atlas_height);
            self.instances.push(GlyphInstance {
                pos: pg.pos,
                size: pg.size,
                uv_rect,
                color: pg.color,
                opacity_pad: [pg.opacity, 0.0, 0.0, 0.0],
            });
        }
    }

    /// Returns the number of finalized instances in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.instances.len()
    }

    /// Returns `true` if the batch is empty (no finalized instances).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instances.is_empty()
    }

    /// Returns the instance data as a byte slice for GPU upload.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.instances)
    }

    /// Returns the number of draw calls needed.
    #[must_use]
    pub fn draw_call_count(&self) -> usize {
        if self.instances.is_empty() {
            return 0;
        }
        self.instances.len().div_ceil(MAX_INSTANCES_PER_BATCH)
    }
}

impl Default for TextBatch {
    fn default() -> Self {
        Self::new()
    }
}

/// The compiled text rendering pipeline.
///
/// Owns only the pipeline-specific GPU render pipeline. Shared resources
/// (vertex/index buffers, camera uniform) are provided by
/// [`SharedPipelineResources`] during creation and draw calls.
pub struct TextPipeline {
    /// The compiled render pipeline.
    pipeline: wgpu::RenderPipeline,
}

impl TextPipeline {
    /// Creates the text pipeline for the given device and output format.
    ///
    /// # Arguments
    /// * `device` — The GPU device.
    /// * `target_format` — The texture format of the render target (surface).
    /// * `atlas_bind_group_layout` — The bind group layout for the glyph atlas.
    /// * `shared` — Shared resources providing the camera bind group layout.
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

        // Pipeline layout: group(0) = camera (shared), group(1) = atlas.
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text_pipeline_layout"),
            bind_group_layouts: &[shared.camera_bind_group_layout(), atlas_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[QuadVertex::layout(), GlyphInstance::layout()],
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

        Self { pipeline }
    }

    /// Records draw commands for a batch of glyph instances.
    ///
    /// Instance data must already be uploaded to `instance_buf` via
    /// [`PersistentInstanceBuffer::upload`] before calling this method.
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

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, shared.camera_bind_group(), &[]);
        pass.set_bind_group(1, atlas.bind_group(), &[]);
        pass.set_vertex_buffer(0, shared.vertex_buffer().slice(..));
        pass.set_vertex_buffer(1, instance_buf.buffer().slice(..));
        pass.set_index_buffer(shared.index_buffer().slice(..), wgpu::IndexFormat::Uint16);

        #[allow(clippy::cast_possible_truncation)]
        let instance_count = batch.len() as u32;
        #[allow(clippy::cast_possible_truncation)]
        let index_count = QUAD_INDICES.len() as u32;
        pass.draw_indexed(0..index_count, 0, 0..instance_count);
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;

    #[test]
    fn glyph_instance_size_is_64_bytes() {
        // 2 + 2 + 4 + 4 + 4 = 16 floats * 4 bytes = 64 bytes.
        assert_eq!(std::mem::size_of::<GlyphInstance>(), 64);
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

        let instance = GlyphInstance::from_positioned(&glyph, uv_rect, &color, 0.8);

        assert_eq!(instance.pos, [100.0, 200.0]);
        assert_eq!(instance.size, [10.0, 15.0]);
        assert_eq!(instance.uv_rect, uv_rect);
        assert!((instance.opacity_pad[0] - 0.8).abs() < f32::EPSILON);
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

        let instance = &batch.instances[0];
        assert_eq!(instance.pos, [15.0, 25.0]);
        // UVs should be normalized: 100/1024, 200/1024, 148/1024, 248/1024
        assert!((instance.uv_rect[0] - 100.0 / 1024.0).abs() < f32::EPSILON);
        assert!((instance.uv_rect[1] - 200.0 / 1024.0).abs() < f32::EPSILON);
        assert!((instance.uv_rect[2] - 148.0 / 1024.0).abs() < f32::EPSILON);
        assert!((instance.uv_rect[3] - 248.0 / 1024.0).abs() < f32::EPSILON);
    }
}
