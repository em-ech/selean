//! Top-level renderer orchestrator.
//!
//! The `Renderer` owns the GPU context, camera, pipelines, and manages the
//! per-frame render loop: collecting visible nodes, updating GPU buffers,
//! and executing render passes.

use selean_common::error::EngineError;
use tracing::{debug, info, warn};

use super::camera::Camera;
use super::gpu::{GpuContext, GpuContextDescriptor};
use super::rect_pipeline::{RectBatch, RectPipeline};
use super::shared::{PersistentInstanceBuffer, SharedPipelineResources, create_stencil_texture};
use super::texture_atlas::TextureAtlas;
use super::textured_quad::{TexturedQuadBatch, TexturedQuadPipeline};
use crate::image::ImageSystem;
use crate::scene::{SceneNode, SceneNodeKind};
use crate::text::{TextBatch, TextPipeline, TextSystem};
use crate::vector::VectorSystem;

/// Configuration for renderer initialization.
#[derive(Debug, Clone)]
pub struct RendererDescriptor {
    /// GPU context configuration.
    pub gpu: GpuContextDescriptor,
    /// Initial viewport width in physical pixels.
    pub viewport_width: f32,
    /// Initial viewport height in physical pixels.
    pub viewport_height: f32,
    /// Background clear color (RGBA, linear).
    pub clear_color: wgpu::Color,
}

impl Default for RendererDescriptor {
    fn default() -> Self {
        Self {
            gpu: GpuContextDescriptor::default(),
            viewport_width: 1280.0,
            viewport_height: 720.0,
            clear_color: wgpu::Color {
                r: 0.95,
                g: 0.95,
                b: 0.95,
                a: 1.0,
            },
        }
    }
}

/// Top-level renderer that orchestrates the entire rendering pipeline.
///
/// Owns all GPU resources and provides the frame rendering interface.
/// Created once at application startup.
pub struct Renderer {
    /// GPU device, queue, and adapter info.
    gpu: GpuContext,
    /// Camera (view-projection) state.
    camera: Camera,
    /// Shared GPU resources (vertex/index buffers, camera uniform).
    shared: SharedPipelineResources,
    /// Rectangle rendering pipeline.
    rect_pipeline: RectPipeline,
    /// Per-frame rectangle batch (reused across frames to avoid allocation).
    rect_batch: RectBatch,
    /// Persistent GPU buffer for rectangle instance data.
    rect_instance_buf: PersistentInstanceBuffer,
    /// Text rendering pipeline.
    text_pipeline: TextPipeline,
    /// Per-frame text glyph batch (reused across frames).
    text_batch: TextBatch,
    /// Persistent GPU buffer for glyph instance data.
    text_instance_buf: PersistentInstanceBuffer,
    /// Text subsystem (font, cache, atlas).
    text_system: TextSystem,
    /// Textured quad pipeline (shared by image and vector rendering).
    textured_quad_pipeline: TexturedQuadPipeline,
    /// Per-frame image batch.
    image_batch: TexturedQuadBatch,
    /// Persistent GPU buffer for image instance data.
    image_instance_buf: PersistentInstanceBuffer,
    /// Image subsystem (decode, cache).
    image_system: ImageSystem,
    /// Per-frame vector batch.
    vector_batch: TexturedQuadBatch,
    /// Persistent GPU buffer for vector instance data.
    vector_instance_buf: PersistentInstanceBuffer,
    /// Vector subsystem (parse, rasterize, cache).
    vector_system: VectorSystem,
    /// RGBA atlas shared by images and vectors.
    image_atlas: TextureAtlas<4>,
    /// Stencil texture for clip masking.
    stencil_texture: wgpu::Texture,
    /// Stencil texture view for render pass attachment.
    stencil_view: wgpu::TextureView,
    /// Background clear color.
    clear_color: wgpu::Color,
    /// The texture format used for the render target.
    target_format: wgpu::TextureFormat,
}

impl Renderer {
    /// Creates a new renderer with the given configuration.
    ///
    /// This initializes the GPU device, compiles shaders, and sets up all
    /// render pipelines. The renderer is ready to render frames after this call.
    ///
    /// # Arguments
    /// * `descriptor` — Renderer configuration.
    /// * `target_format` — The texture format of the render target surface.
    ///   Typically obtained from `surface.get_capabilities(&adapter).formats[0]`.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::NoAdapter`, `EngineError::DeviceCreation`, or
    /// `EngineError::Font` if initialization fails.
    pub async fn new(
        descriptor: &RendererDescriptor,
        target_format: wgpu::TextureFormat,
    ) -> Result<Self, EngineError> {
        let gpu = GpuContext::new(&descriptor.gpu).await?;
        let camera = Camera::new(descriptor.viewport_width, descriptor.viewport_height);

        // Create shared resources first (camera uniform, quad buffers).
        let shared = SharedPipelineResources::new(&gpu.device);

        let rect_pipeline = RectPipeline::new(&gpu.device, target_format, &shared);

        // Initialize text subsystem.
        let text_system = TextSystem::new(&gpu.device)?;
        let text_pipeline = TextPipeline::new(
            &gpu.device,
            target_format,
            text_system.atlas().bind_group_layout(),
            &shared,
        );

        // Initialize RGBA atlas (shared by images and vectors).
        let image_atlas = TextureAtlas::<4>::new(&gpu.device);

        // Initialize textured quad pipeline (shared by images and vectors).
        let textured_quad_pipeline = TexturedQuadPipeline::new(
            &gpu.device,
            target_format,
            image_atlas.bind_group_layout(),
            &shared,
        );

        // Create stencil texture for clip masking.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let (stencil_texture, stencil_view) = create_stencil_texture(
            &gpu.device,
            descriptor.viewport_width as u32,
            descriptor.viewport_height as u32,
        );

        info!(
            viewport_w = descriptor.viewport_width,
            viewport_h = descriptor.viewport_height,
            format = ?target_format,
            "Renderer initialized (with text, image, and vector support)"
        );

        // Initial capacity: ~256 instances each (reasonable for typical scenes).
        let rect_instance_buf =
            PersistentInstanceBuffer::new(&gpu.device, "rect_instance_buffer", 256 * 96);
        let text_instance_buf =
            PersistentInstanceBuffer::new(&gpu.device, "glyph_instance_buffer", 256 * 88);
        let image_instance_buf =
            PersistentInstanceBuffer::new(&gpu.device, "image_instance_buffer", 256 * 88);
        let vector_instance_buf =
            PersistentInstanceBuffer::new(&gpu.device, "vector_instance_buffer", 256 * 88);

        Ok(Self {
            gpu,
            camera,
            shared,
            rect_pipeline,
            rect_batch: RectBatch::new(),
            rect_instance_buf,
            text_pipeline,
            text_batch: TextBatch::new(),
            text_instance_buf,
            text_system,
            textured_quad_pipeline,
            image_batch: TexturedQuadBatch::new(),
            image_instance_buf,
            image_system: ImageSystem::new(),
            vector_batch: TexturedQuadBatch::new(),
            vector_instance_buf,
            vector_system: VectorSystem::new(),
            image_atlas,
            stencil_texture,
            stencil_view,
            clear_color: descriptor.clear_color,
            target_format,
        })
    }

    /// Returns a reference to the GPU context.
    #[must_use]
    pub fn gpu(&self) -> &GpuContext {
        &self.gpu
    }

    /// Returns a mutable reference to the camera.
    #[must_use]
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    /// Returns a reference to the camera.
    #[must_use]
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Returns the texture format used for the render target.
    #[must_use]
    pub fn target_format(&self) -> wgpu::TextureFormat {
        self.target_format
    }

    /// Sets the background clear color.
    pub fn set_clear_color(&mut self, color: wgpu::Color) {
        self.clear_color = color;
    }

    /// Updates the viewport size (e.g., on window resize).
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn resize(&mut self, width: f32, height: f32) {
        self.camera.set_viewport_size(width, height);

        // Recreate stencil texture to match new viewport size.
        let (stencil_texture, stencil_view) =
            create_stencil_texture(&self.gpu.device, width as u32, height as u32);
        self.stencil_texture = stencil_texture;
        self.stencil_view = stencil_view;

        debug!(width, height, "Viewport resized");
    }

    /// Registers an image asset for later use in Image nodes.
    ///
    /// The image bytes are decoded and optionally resized eagerly.
    /// The decoded data is stored for atlas upload during the prepare phase.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Image` if the bytes cannot be decoded.
    pub fn register_image_asset(
        &mut self,
        asset_ref: &str,
        bytes: &[u8],
    ) -> Result<(), EngineError> {
        self.image_system.register_asset(asset_ref, bytes)
    }

    /// Prepares a frame for rendering by collecting visible scene nodes.
    ///
    /// Processes each node into the appropriate batch: rectangles go to
    /// `RectBatch`, text nodes to `TextBatch`, images and vectors to
    /// `TexturedQuadBatch` instances.
    ///
    /// # Arguments
    /// * `visible_nodes` — Iterator of scene nodes within the viewport.
    ///   Nodes should be in back-to-front render order.
    pub fn prepare<'a>(&mut self, visible_nodes: impl Iterator<Item = &'a SceneNode>) {
        self.rect_batch.clear();
        self.text_batch.clear();
        self.image_batch.clear();
        self.vector_batch.clear();

        let mut node_count = 0u32;
        for node in visible_nodes {
            node_count += 1;

            match &node.kind {
                SceneNodeKind::Frame { .. } | SceneNodeKind::Group => {
                    self.rect_batch.push_node(node);
                }
                SceneNodeKind::Text { content, font_size } => {
                    if let Err(e) = self.text_system.prepare_text_node(
                        &self.gpu.device,
                        &self.gpu.queue,
                        node,
                        content,
                        *font_size,
                        &mut self.text_batch,
                    ) {
                        warn!(?e, node_id = %node.id, "Failed to prepare text node");
                    }
                }
                SceneNodeKind::Image { asset_ref } => {
                    if let Err(e) = self.image_system.prepare_image_node(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &mut self.image_atlas,
                        node,
                        asset_ref,
                        &mut self.image_batch,
                    ) {
                        warn!(?e, node_id = %node.id, "Failed to prepare image node");
                    }
                }
                SceneNodeKind::Vector { path_data } => {
                    if let Err(e) = self.vector_system.prepare_vector_node(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &mut self.image_atlas,
                        node,
                        path_data,
                        &mut self.vector_batch,
                    ) {
                        warn!(?e, node_id = %node.id, "Failed to prepare vector node");
                    }
                }
            }
        }

        // Finalize UVs now that atlas dimensions are stable.
        let (atlas_w, atlas_h) = self.image_atlas.dimensions();
        self.image_batch.finalize_uvs(atlas_w, atlas_h);
        self.vector_batch.finalize_uvs(atlas_w, atlas_h);

        let (glyph_atlas_w, glyph_atlas_h) = self.text_system.atlas().dimensions();
        self.text_batch.finalize_uvs(glyph_atlas_w, glyph_atlas_h);

        // Finalize blend mode partitioning for rect batch (text/image/vector do it in finalize_uvs).
        self.rect_batch.finalize_blend();

        // Upload the camera uniform once (shared by all pipelines).
        let camera_uniform = self.camera.build_uniform();
        self.shared.update_camera(&self.gpu.queue, &camera_uniform);

        // Upload instance data to persistent GPU buffers.
        self.rect_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.rect_batch.as_bytes(),
        );
        self.text_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.text_batch.as_bytes(),
        );
        self.image_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.image_batch.as_bytes(),
        );
        self.vector_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.vector_batch.as_bytes(),
        );

        debug!(
            rects = self.rect_batch.len(),
            glyphs = self.text_batch.len(),
            images = self.image_batch.len(),
            vectors = self.vector_batch.len(),
            total_nodes = node_count,
            rect_draws = self.rect_batch.draw_call_count(),
            text_draws = self.text_batch.draw_call_count(),
            image_draws = self.image_batch.draw_call_count(),
            vector_draws = self.vector_batch.draw_call_count(),
            "Frame prepared"
        );
    }

    /// Renders a frame to the given texture view.
    ///
    /// Executes the render pass with the prepared batch data. Call `prepare()`
    /// before this method each frame.
    ///
    /// Draw order: rects → images → vectors → text (back to front).
    ///
    /// # Arguments
    /// * `view` — The texture view to render into (from a surface or offscreen target).
    pub fn render(&self, view: &wgpu::TextureView) {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.stencil_view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // 1. Draw rectangles (furthest back).
            self.rect_pipeline.draw(
                &mut pass,
                &self.rect_batch,
                &self.rect_instance_buf,
                &self.shared,
            );

            // 2. Draw images.
            self.textured_quad_pipeline.draw(
                &mut pass,
                &self.image_batch,
                &self.image_instance_buf,
                &self.image_atlas,
                &self.shared,
            );

            // 3. Draw vectors.
            self.textured_quad_pipeline.draw(
                &mut pass,
                &self.vector_batch,
                &self.vector_instance_buf,
                &self.image_atlas,
                &self.shared,
            );

            // 4. Draw text on top.
            self.text_pipeline.draw(
                &mut pass,
                &self.text_batch,
                &self.text_instance_buf,
                self.text_system.atlas(),
                &self.shared,
            );
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Convenience method: prepares and renders a frame in one call.
    ///
    /// # Arguments
    /// * `visible_nodes` — Iterator of scene nodes within the viewport.
    /// * `view` — The texture view to render into.
    pub fn render_frame<'a>(
        &mut self,
        visible_nodes: impl Iterator<Item = &'a SceneNode>,
        view: &wgpu::TextureView,
    ) {
        self.prepare(visible_nodes);
        self.render(view);
    }
}

/// Statistics from the most recent frame, useful for diagnostics and benchmarks.
#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    /// Number of rectangle instances submitted to the GPU.
    pub rect_count: usize,
    /// Number of glyph instances submitted to the GPU.
    pub glyph_count: usize,
    /// Number of image instances submitted to the GPU.
    pub image_count: usize,
    /// Number of vector instances submitted to the GPU.
    pub vector_count: usize,
    /// Number of draw calls issued.
    pub draw_calls: usize,
    /// Total nodes processed (including non-renderable types).
    pub total_nodes_processed: usize,
}

impl Renderer {
    /// Returns statistics about the current prepared frame.
    #[must_use]
    pub fn frame_stats(&self) -> FrameStats {
        FrameStats {
            rect_count: self.rect_batch.len(),
            glyph_count: self.text_batch.len(),
            image_count: self.image_batch.len(),
            vector_count: self.vector_batch.len(),
            draw_calls: self.rect_batch.draw_call_count()
                + self.text_batch.draw_call_count()
                + self.image_batch.draw_call_count()
                + self.vector_batch.draw_call_count(),
            total_nodes_processed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renderer_descriptor_defaults() {
        let desc = RendererDescriptor::default();
        assert!((desc.viewport_width - 1280.0).abs() < f32::EPSILON);
        assert!((desc.viewport_height - 720.0).abs() < f32::EPSILON);
    }

    #[test]
    fn frame_stats_default() {
        let stats = FrameStats::default();
        assert_eq!(stats.rect_count, 0);
        assert_eq!(stats.glyph_count, 0);
        assert_eq!(stats.image_count, 0);
        assert_eq!(stats.vector_count, 0);
        assert_eq!(stats.draw_calls, 0);
        assert_eq!(stats.total_nodes_processed, 0);
    }
}
