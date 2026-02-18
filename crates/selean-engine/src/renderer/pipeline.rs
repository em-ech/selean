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
use crate::scene::{SceneNode, SceneNodeKind};
use crate::text::{TextBatch, TextPipeline, TextSystem};

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
    /// Rectangle rendering pipeline.
    rect_pipeline: RectPipeline,
    /// Per-frame rectangle batch (reused across frames to avoid allocation).
    rect_batch: RectBatch,
    /// Text rendering pipeline.
    text_pipeline: TextPipeline,
    /// Per-frame text glyph batch (reused across frames).
    text_batch: TextBatch,
    /// Text subsystem (font, cache, atlas).
    text_system: TextSystem,
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
        let rect_pipeline = RectPipeline::new(&gpu.device, target_format);

        // Initialize text subsystem.
        let text_system = TextSystem::new(&gpu.device)?;
        let text_pipeline = TextPipeline::new(
            &gpu.device,
            target_format,
            text_system.atlas().bind_group_layout(),
        );

        info!(
            viewport_w = descriptor.viewport_width,
            viewport_h = descriptor.viewport_height,
            format = ?target_format,
            "Renderer initialized (with text support)"
        );

        Ok(Self {
            gpu,
            camera,
            rect_pipeline,
            rect_batch: RectBatch::new(),
            text_pipeline,
            text_batch: TextBatch::new(),
            text_system,
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
    pub fn resize(&mut self, width: f32, height: f32) {
        self.camera.set_viewport_size(width, height);
        debug!(width, height, "Viewport resized");
    }

    /// Prepares a frame for rendering by collecting visible scene nodes.
    ///
    /// Processes each node into the appropriate batch: rectangles go to
    /// `RectBatch`, text nodes are shaped, SDF-cached, and laid out into
    /// `TextBatch`.
    ///
    /// # Arguments
    /// * `visible_nodes` — Iterator of scene nodes within the viewport.
    ///   Nodes should be in back-to-front render order.
    pub fn prepare<'a>(&mut self, visible_nodes: impl Iterator<Item = &'a SceneNode>) {
        self.rect_batch.clear();
        self.text_batch.clear();

        let mut node_count = 0u32;
        for node in visible_nodes {
            // Try rect pipeline first.
            if self.rect_batch.push_node(node) {
                node_count += 1;
                continue;
            }

            // Try text pipeline.
            if let SceneNodeKind::Text {
                ref content,
                font_size,
            } = node.kind
            {
                if let Err(e) = self.text_system.prepare_text_node(
                    &self.gpu.device,
                    &self.gpu.queue,
                    node,
                    content,
                    font_size,
                    &mut self.text_batch,
                ) {
                    warn!(?e, node_id = %node.id, "Failed to prepare text node");
                }
                node_count += 1;
            }
        }

        // Upload the camera uniform to both pipelines.
        let camera_uniform = self.camera.build_uniform();
        self.rect_pipeline
            .update_camera(&self.gpu.queue, &camera_uniform);
        self.text_pipeline
            .update_camera(&self.gpu.queue, &camera_uniform);

        debug!(
            rects = self.rect_batch.len(),
            glyphs = self.text_batch.len(),
            total_nodes = node_count,
            rect_draws = self.rect_batch.draw_call_count(),
            text_draws = self.text_batch.draw_call_count(),
            "Frame prepared"
        );
    }

    /// Renders a frame to the given texture view.
    ///
    /// Executes the render pass with the prepared batch data. Call `prepare()`
    /// before this method each frame.
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
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw rectangles first (typically behind text).
            self.rect_pipeline
                .draw(&self.gpu.device, &mut pass, &self.rect_batch);

            // Draw text on top.
            self.text_pipeline.draw(
                &self.gpu.device,
                &mut pass,
                &self.text_batch,
                self.text_system.atlas(),
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
            draw_calls: self.rect_batch.draw_call_count() + self.text_batch.draw_call_count(),
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
        assert_eq!(stats.draw_calls, 0);
        assert_eq!(stats.total_nodes_processed, 0);
    }
}
