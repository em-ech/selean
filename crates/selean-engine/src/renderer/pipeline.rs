//! Top-level renderer orchestrator.
//!
//! The `Renderer` owns the GPU context, camera, pipelines, and manages the
//! per-frame render loop: collecting visible nodes, updating GPU buffers,
//! and executing render passes.

use selean_common::error::EngineError;
use selean_common::types::NodeId;
use tracing::{debug, info, warn};

use super::blend_pipeline::BlendResources;
use super::blur_pipeline::BlurResources;
use super::camera::Camera;
use super::clip_stack::ClipStack;
use super::draw_list::{DrawCommand, DrawList};
use super::gpu::{GpuContext, GpuContextDescriptor};
use super::quad::QUAD_INDICES;
use super::rect_pipeline::{RectBatch, RectInstance, RectPipeline};
use super::shared::{PersistentInstanceBuffer, SharedPipelineResources, create_stencil_texture};
use super::texture_atlas::TextureAtlas;
use super::textured_quad::{TexturedQuadBatch, TexturedQuadPipeline};
use crate::image::ImageSystem;
use crate::scene::{BlendMode, ClipMode, ClipRect, SceneGraph, SceneNode, SceneNodeKind};
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

/// An entry in the render order produced by the DFS traversal.
///
/// Phase 1 (DFS) populates these entries. Phase 2 (after UV finalization)
/// iterates them to patch clip rects and emit [`DrawCommand`]s.
#[derive(Debug, Clone)]
enum RenderOrderEntry {
    Rect {
        instance_start: u32,
        instance_count: u32,
        blend: BlendMode,
        stencil_test: bool,
    },
    Text {
        pending_start: u32,
        pending_count: u32,
        blend: BlendMode,
        stencil_test: bool,
        shader_clip_rect: ClipRect,
    },
    Image {
        pending_start: u32,
        pending_count: u32,
        blend: BlendMode,
        stencil_test: bool,
        shader_clip_rect: ClipRect,
    },
    Vector {
        pending_start: u32,
        pending_count: u32,
        blend: BlendMode,
        stencil_test: bool,
        shader_clip_rect: ClipRect,
    },
    PushScissor {
        clip_rect: ClipRect,
    },
    PopScissor {
        parent_scissor: Option<ClipRect>,
    },
    PushStencil {
        write_instance_start: u32,
        write_instance_count: u32,
        new_stencil_ref: u32,
    },
    PopStencil {
        decrement_instance_start: u32,
        decrement_instance_count: u32,
        restored_stencil_ref: u32,
    },
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
    /// Stencil texture for clip masking (used only when blend resources are bypassed).
    #[allow(dead_code)]
    stencil_texture: wgpu::Texture,
    /// Stencil texture view for render pass attachment (used only when blend resources are bypassed).
    #[allow(dead_code)]
    stencil_view: wgpu::TextureView,
    /// GPU resources for shader-based blend mode compositing.
    blend_resources: BlendResources,
    /// GPU resources for dual Kawase blur effects.
    blur_resources: BlurResources,
    /// Background clear color.
    clear_color: wgpu::Color,
    /// The texture format used for the render target.
    target_format: wgpu::TextureFormat,
    /// Draw command list for the current frame (reused across frames).
    draw_list: DrawList,
    /// Render order entries from DFS traversal (reused across frames).
    render_order: Vec<RenderOrderEntry>,
    /// Clip state stack for DFS traversal (reused across frames).
    clip_stack: ClipStack,
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

        // Create blend resources for shader-based blend mode compositing.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let blend_resources = BlendResources::new(
            &gpu.device,
            target_format,
            descriptor.viewport_width as u32,
            descriptor.viewport_height as u32,
        );

        // Create blur resources for drop shadow and layer blur effects.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let blur_resources = BlurResources::new(
            &gpu.device,
            target_format,
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
            blend_resources,
            blur_resources,
            clear_color: descriptor.clear_color,
            target_format,
            draw_list: DrawList::new(),
            render_order: Vec::with_capacity(256),
            clip_stack: ClipStack::new(),
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

        // Resize blend resources (intermediate RT, snapshot, stencil).
        self.blend_resources
            .resize(&self.gpu.device, width as u32, height as u32);

        // Resize blur resources (half-res textures).
        self.blur_resources
            .resize(&self.gpu.device, width as u32, height as u32);

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

    /// Registers a new font family for text rendering.
    ///
    /// Returns the assigned `FontId`. If the family is already registered,
    /// returns the existing ID.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the font bytes cannot be parsed.
    pub fn register_font(&mut self, family: &str, data: Vec<u8>) -> Result<(), EngineError> {
        self.text_system.register_font(family, data)?;
        Ok(())
    }

    /// Renders a complete frame using hierarchical DFS traversal.
    ///
    /// Replaces the flat `prepare()` + `render()` flow. Walks the scene tree
    /// depth-first, respecting clip node boundaries, and produces a draw list
    /// that is executed in a single render pass.
    ///
    /// # Arguments
    /// * `scene` — The scene graph (world transforms are recomputed if dirty).
    /// * `view` — The texture view to render into.
    pub fn render_frame(&mut self, scene: &mut SceneGraph, view: &wgpu::TextureView) {
        // Phase 0: Recompute cached world transforms.
        scene.recompute_world_transforms();

        // Clear per-frame state.
        self.rect_batch.clear();
        self.text_batch.clear();
        self.image_batch.clear();
        self.vector_batch.clear();
        self.render_order.clear();
        self.draw_list.clear();
        self.clip_stack.clear();

        // Phase 1: DFS traversal from each root.
        let roots = scene.roots().to_vec();
        for root_id in roots {
            self.dfs_visit(scene, root_id);
        }

        // Phase 2: Finalize UVs (atlas dimensions are now stable).
        let (atlas_w, atlas_h) = self.image_atlas.dimensions();
        self.image_batch.finalize_uvs_ordered(atlas_w, atlas_h);
        self.vector_batch.finalize_uvs_ordered(atlas_w, atlas_h);

        let (glyph_atlas_w, glyph_atlas_h) = self.text_system.atlas().dimensions();
        self.text_batch
            .finalize_uvs_ordered(glyph_atlas_w, glyph_atlas_h);

        // Phase 2b: Iterate render order, patch clip rects, emit draw commands.
        self.build_draw_list();

        // Merge adjacent compatible draw commands.
        self.draw_list.merge_adjacent();

        // Upload camera uniform.
        let camera_uniform = self.camera.build_uniform();
        self.shared.update_camera(&self.gpu.queue, &camera_uniform);

        // Upload ordered instance buffers.
        self.rect_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.rect_batch.ordered_as_bytes(),
        );
        self.text_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.text_batch.ordered_as_bytes(),
        );
        self.image_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.image_batch.ordered_as_bytes(),
        );
        self.vector_instance_buf.upload(
            &self.gpu.device,
            &self.gpu.queue,
            self.vector_batch.ordered_as_bytes(),
        );

        debug!(
            rects = self.rect_batch.ordered_len(),
            glyphs = self.text_batch.ordered_len(),
            images = self.image_batch.ordered_len(),
            vectors = self.vector_batch.ordered_len(),
            draw_commands = self.draw_list.len(),
            "Frame prepared (hierarchical DFS)"
        );

        // Phase 3: Execute the draw list in a render pass.
        self.execute_draw_list(view);
    }

    /// DFS visit for a single node and its descendants.
    #[allow(clippy::too_many_lines)]
    fn dfs_visit(&mut self, scene: &SceneGraph, node_id: NodeId) {
        let Some(node) = scene.get(node_id) else {
            return;
        };
        if !node.visible {
            return;
        }

        let effective_clip = compute_effective_clip_mode(node.clip_mode, node.scroll_offset);
        let blend_mode = node.blend_mode;
        let is_clip = effective_clip != ClipMode::None;

        // Enter clip: push clip state and emit render order entry.
        let parent_scissor = self.clip_stack.current_scissor().copied();
        if is_clip {
            let world_bb = node.world_transform.transform_aabb(&node.bounds);
            let node_clip = ClipRect::new(
                world_bb.x,
                world_bb.y,
                world_bb.x + world_bb.width,
                world_bb.y + world_bb.height,
            );
            match effective_clip {
                ClipMode::Scissor => {
                    self.clip_stack.push_scissor(node_clip);
                    self.render_order.push(RenderOrderEntry::PushScissor {
                        clip_rect: self
                            .clip_stack
                            .current_scissor()
                            .copied()
                            .unwrap_or(node_clip),
                    });
                }
                ClipMode::Stencil => {
                    // Create stencil write instance for the clip shape.
                    if let Some(inst) =
                        create_stencil_rect_instance(node, self.clip_stack.current_shader_rect())
                    {
                        #[allow(clippy::cast_possible_truncation)]
                        let write_start = self.rect_batch.ordered_len() as u32;
                        self.rect_batch.push_ordered(inst);
                        self.clip_stack.push_stencil(node_id);
                        let new_ref = self.clip_stack.stencil_ref();
                        self.render_order.push(RenderOrderEntry::PushStencil {
                            write_instance_start: write_start,
                            write_instance_count: 1,
                            new_stencil_ref: new_ref,
                        });
                    }
                }
                ClipMode::ShaderRect => {
                    self.clip_stack.push_shader_rect(node_clip);
                }
                ClipMode::None => unreachable!(),
            }
        }

        // Process node content: create instances or record pending ranges.
        let resolved = self.clip_stack.resolve();
        match &node.kind {
            SceneNodeKind::Frame { .. } | SceneNodeKind::Group => {
                if !node.bounds.is_empty() {
                    if let Some(mut inst) = RectInstance::from_scene_node(node) {
                        inst.clip_rect = resolved.shader_clip_rect.to_array();
                        #[allow(clippy::cast_possible_truncation)]
                        let start = self.rect_batch.ordered_len() as u32;
                        self.rect_batch.push_ordered(inst);
                        self.render_order.push(RenderOrderEntry::Rect {
                            instance_start: start,
                            instance_count: 1,
                            blend: blend_mode,
                            stencil_test: resolved.stencil_ref > 0,
                        });
                    }
                }
            }
            SceneNodeKind::Text {
                content, font_size, ..
            } => {
                #[allow(clippy::cast_possible_truncation)]
                let pending_start = self.text_batch.pending_len() as u32;
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
                #[allow(clippy::cast_possible_truncation)]
                let pending_count = self.text_batch.pending_len() as u32 - pending_start;
                if pending_count > 0 {
                    self.render_order.push(RenderOrderEntry::Text {
                        pending_start,
                        pending_count,
                        blend: blend_mode,
                        stencil_test: resolved.stencil_ref > 0,
                        shader_clip_rect: resolved.shader_clip_rect,
                    });
                }
            }
            SceneNodeKind::Image { asset_ref } => {
                #[allow(clippy::cast_possible_truncation)]
                let pending_start = self.image_batch.pending_len() as u32;
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
                #[allow(clippy::cast_possible_truncation)]
                let pending_count = self.image_batch.pending_len() as u32 - pending_start;
                if pending_count > 0 {
                    self.render_order.push(RenderOrderEntry::Image {
                        pending_start,
                        pending_count,
                        blend: blend_mode,
                        stencil_test: resolved.stencil_ref > 0,
                        shader_clip_rect: resolved.shader_clip_rect,
                    });
                }
            }
            SceneNodeKind::Vector { path_data } => {
                #[allow(clippy::cast_possible_truncation)]
                let pending_start = self.vector_batch.pending_len() as u32;
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
                #[allow(clippy::cast_possible_truncation)]
                let pending_count = self.vector_batch.pending_len() as u32 - pending_start;
                if pending_count > 0 {
                    self.render_order.push(RenderOrderEntry::Vector {
                        pending_start,
                        pending_count,
                        blend: blend_mode,
                        stencil_test: resolved.stencil_ref > 0,
                        shader_clip_rect: resolved.shader_clip_rect,
                    });
                }
            }
        }

        // Recurse into children (depth-first, back-to-front order).
        let children = scene.children(node_id).unwrap_or(&[]).to_vec();
        for child_id in children {
            self.dfs_visit(scene, child_id);
        }

        // Exit clip: emit pop entry and pop the clip stack.
        if is_clip {
            match effective_clip {
                ClipMode::Scissor => {
                    self.clip_stack.pop_scissor();
                    self.render_order
                        .push(RenderOrderEntry::PopScissor { parent_scissor });
                }
                ClipMode::Stencil => {
                    // Create decrement instance (same shape as write).
                    if let Some(node) = scene.get(node_id) {
                        if let Some(inst) = create_stencil_rect_instance(
                            node,
                            self.clip_stack.current_shader_rect(),
                        ) {
                            #[allow(clippy::cast_possible_truncation)]
                            let dec_start = self.rect_batch.ordered_len() as u32;
                            self.rect_batch.push_ordered(inst);
                            self.clip_stack.pop_stencil();
                            let restored_ref = self.clip_stack.stencil_ref();
                            self.render_order.push(RenderOrderEntry::PopStencil {
                                decrement_instance_start: dec_start,
                                decrement_instance_count: 1,
                                restored_stencil_ref: restored_ref,
                            });
                        }
                    }
                }
                ClipMode::ShaderRect => {
                    self.clip_stack.pop_shader_rect();
                }
                ClipMode::None => unreachable!(),
            }
        }
    }

    /// Phase 2b: Iterate render order entries, patch clip rects on ordered
    /// instances, and emit draw commands to the draw list.
    fn build_draw_list(&mut self) {
        let (viewport_w, viewport_h) = self.camera.viewport_size();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let vp_size = (viewport_w as u32, viewport_h as u32);

        // Take render_order temporarily to avoid borrow conflict with self.draw_list.
        let render_order = std::mem::take(&mut self.render_order);

        // Patch clip rects on batched instances (requires mutable batch access).
        for entry in &render_order {
            match entry {
                RenderOrderEntry::Text {
                    pending_start,
                    pending_count,
                    shader_clip_rect,
                    ..
                } => {
                    let start = *pending_start as usize;
                    let count = *pending_count as usize;
                    let clip_arr = shader_clip_rect.to_array();
                    let instances = self.text_batch.ordered_instances_mut();
                    for inst in instances.iter_mut().skip(start).take(count) {
                        inst.clip_rect = clip_arr;
                    }
                }
                RenderOrderEntry::Image {
                    pending_start,
                    pending_count,
                    shader_clip_rect,
                    ..
                } => {
                    let start = *pending_start as usize;
                    let count = *pending_count as usize;
                    let clip_arr = shader_clip_rect.to_array();
                    let instances = self.image_batch.ordered_instances_mut();
                    for inst in instances.iter_mut().skip(start).take(count) {
                        inst.clip_rect = clip_arr;
                    }
                }
                RenderOrderEntry::Vector {
                    pending_start,
                    pending_count,
                    shader_clip_rect,
                    ..
                } => {
                    let start = *pending_start as usize;
                    let count = *pending_count as usize;
                    let clip_arr = shader_clip_rect.to_array();
                    let instances = self.vector_batch.ordered_instances_mut();
                    for inst in instances.iter_mut().skip(start).take(count) {
                        inst.clip_rect = clip_arr;
                    }
                }
                _ => {}
            }
        }

        // Generate draw commands from render order entries (pure CPU logic).
        let commands = build_draw_commands_from_render_order(&render_order, vp_size);
        for cmd in commands {
            self.draw_list.push(cmd);
        }

        // Restore render_order for next frame reuse.
        self.render_order = render_order;
    }

    /// Executes the draw list, rendering to an intermediate texture and blitting to the swapchain.
    ///
    /// Non-native blend modes are handled by breaking the render pass:
    /// 1. `CopyFramebuffer`: snapshot the intermediate to capture the destination.
    /// 2. The element is drawn with REPLACE blend (overwriting the intermediate).
    /// 3. `ApplyBlend`: a full-screen composite pass reads src+dst and writes the blended result.
    ///
    /// All native-blend commands render directly to the intermediate. At the end,
    /// the intermediate is blitted to the swapchain.
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    fn execute_draw_list(&self, view: &wgpu::TextureView) {
        let has_non_native = self.draw_list.commands().iter().any(|cmd| {
            matches!(
                cmd,
                DrawCommand::CopyFramebuffer | DrawCommand::ApplyBlend { .. }
            )
        });

        if has_non_native {
            self.execute_draw_list_blended(view);
        } else {
            self.execute_draw_list_direct(view);
        }
    }

    /// Fast path: no non-native blend modes, render directly to swapchain (single pass).
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    fn execute_draw_list_direct(&self, view: &wgpu::TextureView) {
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
                    view: self.blend_resources.stencil_view(),
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.execute_scene_commands(&mut pass);
        }

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Slow path: non-native blend modes present, render to intermediate and blit.
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    fn execute_draw_list_blended(&self, view: &wgpu::TextureView) {
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("blend_render_encoder"),
            });

        let (vp_w, vp_h) = self.camera.viewport_size();
        let index_count = QUAD_INDICES.len() as u32;
        let mut current_stencil_ref: u32 = 0;

        // Track which commands need render pass breaks.
        // We iterate commands, breaking the pass at CopyFramebuffer/ApplyBlend.
        let commands: Vec<DrawCommand> = self.draw_list.commands().to_vec();
        let mut cmd_idx = 0;
        let mut is_first_pass = true;

        while cmd_idx < commands.len() {
            // Find the next CopyFramebuffer or ApplyBlend.
            let break_at = commands[cmd_idx..]
                .iter()
                .position(|cmd| {
                    matches!(
                        cmd,
                        DrawCommand::CopyFramebuffer | DrawCommand::ApplyBlend { .. }
                    )
                })
                .map_or(commands.len(), |offset| cmd_idx + offset);

            // Render scene commands [cmd_idx..break_at] in a render pass on the intermediate.
            if break_at > cmd_idx {
                let load_op = if is_first_pass {
                    wgpu::LoadOp::Clear(self.clear_color)
                } else {
                    wgpu::LoadOp::Load
                };
                let stencil_load = if is_first_pass {
                    wgpu::LoadOp::Clear(0)
                } else {
                    wgpu::LoadOp::Load
                };

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("intermediate_render_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: self.blend_resources.intermediate_view(),
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: load_op,
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: self.blend_resources.stencil_view(),
                            depth_ops: None,
                            stencil_ops: Some(wgpu::Operations {
                                load: stencil_load,
                                store: wgpu::StoreOp::Store,
                            }),
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    for cmd in &commands[cmd_idx..break_at] {
                        self.execute_single_command(
                            cmd,
                            &mut pass,
                            &mut current_stencil_ref,
                            vp_w,
                            vp_h,
                            index_count,
                        );
                    }
                }
                is_first_pass = false;
            }

            cmd_idx = break_at;

            // Handle the break command if present.
            if cmd_idx < commands.len() {
                match &commands[cmd_idx] {
                    DrawCommand::CopyFramebuffer => {
                        // If this is the very first command, clear the intermediate first.
                        if is_first_pass {
                            // Empty pass to clear the intermediate before snapshot.
                            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("initial_clear_pass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: self.blend_resources.intermediate_view(),
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(self.clear_color),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: self.blend_resources.stencil_view(),
                                        depth_ops: None,
                                        stencil_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                    },
                                ),
                                timestamp_writes: None,
                                occlusion_query_set: None,
                            });
                            is_first_pass = false;
                        }
                        self.blend_resources.snapshot(&mut encoder);
                        cmd_idx += 1;
                    }
                    DrawCommand::ApplyBlend { mode } => {
                        self.blend_resources.blend_pass(
                            &mut encoder,
                            &self.gpu.device,
                            &self.gpu.queue,
                            *mode,
                        );
                        cmd_idx += 1;
                    }
                    _ => unreachable!(),
                }
            }
        }

        // If no commands were executed (empty draw list), still clear the intermediate.
        if is_first_pass {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("empty_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.blend_resources.intermediate_view(),
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
        }

        // Final blit: intermediate -> swapchain.
        self.blend_resources
            .blit_to_target(&mut encoder, &self.gpu.device, view);

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Executes all scene commands in a single render pass (no blend breaking).
    #[allow(clippy::cast_possible_truncation)]
    fn execute_scene_commands<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        let (vp_w, vp_h) = self.camera.viewport_size();
        let index_count = QUAD_INDICES.len() as u32;
        let mut current_stencil_ref: u32 = 0;

        for cmd in self.draw_list.commands() {
            self.execute_single_command(
                cmd,
                pass,
                &mut current_stencil_ref,
                vp_w,
                vp_h,
                index_count,
            );
        }
    }

    /// Executes a single draw command in the given render pass.
    #[allow(clippy::cast_possible_truncation, clippy::too_many_lines)]
    fn execute_single_command<'a>(
        &'a self,
        cmd: &DrawCommand,
        pass: &mut wgpu::RenderPass<'a>,
        current_stencil_ref: &mut u32,
        vp_w: f32,
        vp_h: f32,
        index_count: u32,
    ) {
        match cmd {
            DrawCommand::SetScissor {
                x,
                y,
                width,
                height,
            } => {
                pass.set_scissor_rect(*x, *y, *width, *height);
            }
            DrawCommand::ResetScissor => {
                #[allow(clippy::cast_sign_loss)]
                pass.set_scissor_rect(0, 0, vp_w as u32, vp_h as u32);
            }
            DrawCommand::SetStencilRef(ref_val) => {
                *current_stencil_ref = *ref_val;
            }
            DrawCommand::StencilWrite {
                instance_start,
                instance_count,
            } => {
                pass.set_pipeline(self.rect_pipeline.stencil_write_pipeline());
                pass.set_bind_group(0, self.shared.camera_bind_group(), &[]);
                pass.set_vertex_buffer(0, self.shared.vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, self.rect_instance_buf.buffer().slice(..));
                pass.set_index_buffer(
                    self.shared.index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.set_stencil_reference(*current_stencil_ref);
                pass.draw_indexed(
                    0..index_count,
                    0,
                    *instance_start..*instance_start + *instance_count,
                );
            }
            DrawCommand::StencilDecrement {
                instance_start,
                instance_count,
            } => {
                pass.set_pipeline(self.rect_pipeline.stencil_decrement_pipeline());
                pass.set_bind_group(0, self.shared.camera_bind_group(), &[]);
                pass.set_vertex_buffer(0, self.shared.vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, self.rect_instance_buf.buffer().slice(..));
                pass.set_index_buffer(
                    self.shared.index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.set_stencil_reference(*current_stencil_ref);
                pass.draw_indexed(
                    0..index_count,
                    0,
                    *instance_start..*instance_start + *instance_count,
                );
            }
            DrawCommand::DrawRects {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => {
                pass.set_pipeline(self.rect_pipeline.select_pipeline(*blend, *stencil_test));
                pass.set_bind_group(0, self.shared.camera_bind_group(), &[]);
                pass.set_vertex_buffer(0, self.shared.vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, self.rect_instance_buf.buffer().slice(..));
                pass.set_index_buffer(
                    self.shared.index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.set_stencil_reference(*current_stencil_ref);
                pass.draw_indexed(
                    0..index_count,
                    0,
                    *instance_start..*instance_start + *instance_count,
                );
            }
            DrawCommand::DrawGlyphs {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => {
                pass.set_pipeline(self.text_pipeline.select_pipeline(*blend, *stencil_test));
                pass.set_bind_group(0, self.shared.camera_bind_group(), &[]);
                pass.set_bind_group(1, self.text_system.atlas().bind_group(), &[]);
                pass.set_vertex_buffer(0, self.shared.vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, self.text_instance_buf.buffer().slice(..));
                pass.set_index_buffer(
                    self.shared.index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.set_stencil_reference(*current_stencil_ref);
                pass.draw_indexed(
                    0..index_count,
                    0,
                    *instance_start..*instance_start + *instance_count,
                );
            }
            DrawCommand::DrawImages {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => {
                pass.set_pipeline(
                    self.textured_quad_pipeline
                        .select_pipeline(*blend, *stencil_test),
                );
                pass.set_bind_group(0, self.shared.camera_bind_group(), &[]);
                pass.set_bind_group(1, self.image_atlas.bind_group(), &[]);
                pass.set_vertex_buffer(0, self.shared.vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, self.image_instance_buf.buffer().slice(..));
                pass.set_index_buffer(
                    self.shared.index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.set_stencil_reference(*current_stencil_ref);
                pass.draw_indexed(
                    0..index_count,
                    0,
                    *instance_start..*instance_start + *instance_count,
                );
            }
            DrawCommand::DrawVectors {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => {
                pass.set_pipeline(
                    self.textured_quad_pipeline
                        .select_pipeline(*blend, *stencil_test),
                );
                pass.set_bind_group(0, self.shared.camera_bind_group(), &[]);
                pass.set_bind_group(1, self.image_atlas.bind_group(), &[]);
                pass.set_vertex_buffer(0, self.shared.vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, self.vector_instance_buf.buffer().slice(..));
                pass.set_index_buffer(
                    self.shared.index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );
                pass.set_stencil_reference(*current_stencil_ref);
                pass.draw_indexed(
                    0..index_count,
                    0,
                    *instance_start..*instance_start + *instance_count,
                );
            }
            // CopyFramebuffer and ApplyBlend are handled by the caller
            // (execute_draw_list_blended) by breaking render passes.
            DrawCommand::CopyFramebuffer | DrawCommand::ApplyBlend { .. } => {}
        }
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
            rect_count: self.rect_batch.ordered_len(),
            glyph_count: self.text_batch.ordered_len(),
            image_count: self.image_batch.ordered_len(),
            vector_count: self.vector_batch.ordered_len(),
            draw_calls: self.draw_list.len(),
            total_nodes_processed: 0,
        }
    }
}

/// Computes the effective clip mode for a node.
///
/// If the node has an explicit clip mode (not `None`), that mode is used.
/// Otherwise, if the node has a non-zero scroll offset, an implicit `Scissor`
/// clip is applied to prevent scroll content from overflowing.
/// Returns `ClipMode::None` if neither condition applies.
#[must_use]
fn compute_effective_clip_mode(clip_mode: ClipMode, scroll_offset: [f32; 2]) -> ClipMode {
    if clip_mode != ClipMode::None {
        clip_mode
    } else if scroll_offset != [0.0, 0.0] {
        ClipMode::Scissor
    } else {
        ClipMode::None
    }
}

/// Collects node IDs from a scene graph in DFS (depth-first, pre-order) traversal order.
///
/// Walks each root in order, visiting the node before its children. Invisible
/// nodes and their entire subtrees are skipped. Returns the node IDs in the
/// order they would be visited during rendering.
#[cfg(test)]
#[must_use]
fn dfs_traversal_order(scene: &SceneGraph) -> Vec<NodeId> {
    fn collect(scene: &SceneGraph, node_id: NodeId, result: &mut Vec<NodeId>) {
        let Some(node) = scene.get(node_id) else {
            return;
        };
        if !node.visible {
            return;
        }
        result.push(node_id);
        for &child_id in &node.children {
            collect(scene, child_id, result);
        }
    }

    let mut result = Vec::new();
    for &root_id in scene.roots() {
        collect(scene, root_id, &mut result);
    }
    result
}

/// Converts render order entries into draw commands.
///
/// This is the CPU-side draw list construction logic extracted from `build_draw_list`.
/// It translates `RenderOrderEntry` items into `DrawCommand` items, inserting
/// `CopyFramebuffer`/`ApplyBlend` around non-native blend mode draws, and
/// converting scissor/stencil push/pop entries into the corresponding GPU commands.
///
/// The `viewport` tuple is `(width_u32, height_u32)` used for scissor rect clamping.
#[allow(clippy::too_many_lines)]
fn build_draw_commands_from_render_order(
    render_order: &[RenderOrderEntry],
    viewport: (u32, u32),
) -> Vec<DrawCommand> {
    let mut commands = Vec::new();
    for entry in render_order {
        match entry {
            RenderOrderEntry::Rect {
                instance_start,
                instance_count,
                blend,
                stencil_test,
            } => {
                if !blend.is_native() {
                    commands.push(DrawCommand::CopyFramebuffer);
                }
                commands.push(DrawCommand::DrawRects {
                    instance_start: *instance_start,
                    instance_count: *instance_count,
                    blend: *blend,
                    stencil_test: *stencil_test,
                });
                if !blend.is_native() {
                    commands.push(DrawCommand::ApplyBlend { mode: *blend });
                }
            }
            RenderOrderEntry::Text {
                pending_start,
                pending_count,
                blend,
                stencil_test,
                ..
            } => {
                if !blend.is_native() {
                    commands.push(DrawCommand::CopyFramebuffer);
                }
                commands.push(DrawCommand::DrawGlyphs {
                    instance_start: *pending_start,
                    instance_count: *pending_count,
                    blend: *blend,
                    stencil_test: *stencil_test,
                });
                if !blend.is_native() {
                    commands.push(DrawCommand::ApplyBlend { mode: *blend });
                }
            }
            RenderOrderEntry::Image {
                pending_start,
                pending_count,
                blend,
                stencil_test,
                ..
            } => {
                if !blend.is_native() {
                    commands.push(DrawCommand::CopyFramebuffer);
                }
                commands.push(DrawCommand::DrawImages {
                    instance_start: *pending_start,
                    instance_count: *pending_count,
                    blend: *blend,
                    stencil_test: *stencil_test,
                });
                if !blend.is_native() {
                    commands.push(DrawCommand::ApplyBlend { mode: *blend });
                }
            }
            RenderOrderEntry::Vector {
                pending_start,
                pending_count,
                blend,
                stencil_test,
                ..
            } => {
                if !blend.is_native() {
                    commands.push(DrawCommand::CopyFramebuffer);
                }
                commands.push(DrawCommand::DrawVectors {
                    instance_start: *pending_start,
                    instance_count: *pending_count,
                    blend: *blend,
                    stencil_test: *stencil_test,
                });
                if !blend.is_native() {
                    commands.push(DrawCommand::ApplyBlend { mode: *blend });
                }
            }
            RenderOrderEntry::PushScissor { clip_rect } => {
                if let Some((x, y, w, h)) = clip_rect.to_scissor_rect(viewport.0, viewport.1) {
                    commands.push(DrawCommand::SetScissor {
                        x,
                        y,
                        width: w,
                        height: h,
                    });
                }
            }
            RenderOrderEntry::PopScissor { parent_scissor } => {
                if let Some(parent) = parent_scissor {
                    if let Some((x, y, w, h)) = parent.to_scissor_rect(viewport.0, viewport.1) {
                        commands.push(DrawCommand::SetScissor {
                            x,
                            y,
                            width: w,
                            height: h,
                        });
                    } else {
                        commands.push(DrawCommand::ResetScissor);
                    }
                } else {
                    commands.push(DrawCommand::ResetScissor);
                }
            }
            RenderOrderEntry::PushStencil {
                write_instance_start,
                write_instance_count,
                new_stencil_ref,
            } => {
                commands.push(DrawCommand::StencilWrite {
                    instance_start: *write_instance_start,
                    instance_count: *write_instance_count,
                });
                commands.push(DrawCommand::SetStencilRef(*new_stencil_ref));
            }
            RenderOrderEntry::PopStencil {
                decrement_instance_start,
                decrement_instance_count,
                restored_stencil_ref,
            } => {
                commands.push(DrawCommand::StencilDecrement {
                    instance_start: *decrement_instance_start,
                    instance_count: *decrement_instance_count,
                });
                commands.push(DrawCommand::SetStencilRef(*restored_stencil_ref));
            }
        }
    }
    commands
}

/// Creates a `RectInstance` suitable for stencil write/decrement operations.
///
/// The instance represents the clip shape: fill area with no stroke, full opacity,
/// and the given shader clip rect. Color values are irrelevant since stencil
/// pipelines use `ColorWrites::empty()`.
fn create_stencil_rect_instance(
    node: &SceneNode,
    shader_clip_rect: ClipRect,
) -> Option<RectInstance> {
    let corner_radii = match &node.kind {
        SceneNodeKind::Frame { corner_radius } => *corner_radius,
        SceneNodeKind::Group => [0.0; 4],
        _ => return None,
    };

    Some(RectInstance {
        pos: [node.bounds.x, node.bounds.y],
        size: [node.bounds.width, node.bounds.height],
        fill_color: [1.0, 1.0, 1.0, 1.0],
        stroke_color: [0.0, 0.0, 0.0, 0.0],
        stroke_width_opacity: [0.0, 1.0],
        corner_radii,
        transform: node.world_transform.to_gpu_columns(),
        clip_rect: shader_clip_rect.to_array(),
        gradient_meta: [0.0; 4],
        gradient_points: [0.0; 4],
        gradient_stops: [0.0; 20],
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp, clippy::unwrap_used)]

    use super::*;
    use crate::scene::{BoundingBox, FontStyle, TextAlign};

    // --- RendererDescriptor / FrameStats ---

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

    // --- compute_effective_clip_mode ---

    #[test]
    fn effective_clip_mode_explicit_scissor() {
        assert_eq!(
            compute_effective_clip_mode(ClipMode::Scissor, [0.0, 0.0]),
            ClipMode::Scissor
        );
    }

    #[test]
    fn effective_clip_mode_explicit_stencil() {
        assert_eq!(
            compute_effective_clip_mode(ClipMode::Stencil, [0.0, 0.0]),
            ClipMode::Stencil
        );
    }

    #[test]
    fn effective_clip_mode_explicit_shader_rect() {
        assert_eq!(
            compute_effective_clip_mode(ClipMode::ShaderRect, [0.0, 0.0]),
            ClipMode::ShaderRect
        );
    }

    #[test]
    fn effective_clip_mode_explicit_overrides_scroll() {
        // Explicit clip mode takes precedence over scroll offset.
        assert_eq!(
            compute_effective_clip_mode(ClipMode::Stencil, [10.0, 20.0]),
            ClipMode::Stencil
        );
    }

    #[test]
    fn effective_clip_mode_scroll_implies_scissor() {
        assert_eq!(
            compute_effective_clip_mode(ClipMode::None, [0.0, 5.0]),
            ClipMode::Scissor
        );
    }

    #[test]
    fn effective_clip_mode_scroll_x_only() {
        assert_eq!(
            compute_effective_clip_mode(ClipMode::None, [3.0, 0.0]),
            ClipMode::Scissor
        );
    }

    #[test]
    fn effective_clip_mode_no_clip_no_scroll() {
        assert_eq!(
            compute_effective_clip_mode(ClipMode::None, [0.0, 0.0]),
            ClipMode::None
        );
    }

    // --- create_stencil_rect_instance ---

    fn make_frame_node(x: f32, y: f32, w: f32, h: f32, corner_radius: [f32; 4]) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            "test_frame".to_string(),
            SceneNodeKind::Frame { corner_radius },
            BoundingBox::new(x, y, w, h),
        )
    }

    #[test]
    fn stencil_instance_from_frame_node() {
        let node = make_frame_node(10.0, 20.0, 100.0, 50.0, [4.0, 8.0, 12.0, 16.0]);
        let clip = ClipRect::new(0.0, 0.0, 200.0, 200.0);
        let inst = create_stencil_rect_instance(&node, clip);
        assert!(inst.is_some());

        let inst = inst.unwrap();
        assert_eq!(inst.pos, [10.0, 20.0]);
        assert_eq!(inst.size, [100.0, 50.0]);
        assert_eq!(inst.fill_color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(inst.stroke_color, [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(inst.stroke_width_opacity, [0.0, 1.0]);
        assert_eq!(inst.corner_radii, [4.0, 8.0, 12.0, 16.0]);
        assert_eq!(inst.clip_rect, clip.to_array());
    }

    #[test]
    fn stencil_instance_from_group_node() {
        let node = SceneNode::new(
            NodeId::new(),
            "group".to_string(),
            SceneNodeKind::Group,
            BoundingBox::new(0.0, 0.0, 200.0, 200.0),
        );
        let inst = create_stencil_rect_instance(&node, ClipRect::INFINITE);
        assert!(inst.is_some());
        assert_eq!(inst.unwrap().corner_radii, [0.0; 4]);
    }

    #[test]
    fn stencil_instance_from_text_node_returns_none() {
        let node = SceneNode::new(
            NodeId::new(),
            "text".to_string(),
            SceneNodeKind::Text {
                content: "hello".to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 100.0, 20.0),
        );
        assert!(create_stencil_rect_instance(&node, ClipRect::INFINITE).is_none());
    }

    #[test]
    fn stencil_instance_from_image_node_returns_none() {
        let node = SceneNode::new(
            NodeId::new(),
            "img".to_string(),
            SceneNodeKind::Image {
                asset_ref: "test.png".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        assert!(create_stencil_rect_instance(&node, ClipRect::INFINITE).is_none());
    }

    #[test]
    fn stencil_instance_from_vector_node_returns_none() {
        let node = SceneNode::new(
            NodeId::new(),
            "vec".to_string(),
            SceneNodeKind::Vector {
                path_data: "M0 0 L10 10".to_string(),
            },
            BoundingBox::new(0.0, 0.0, 24.0, 24.0),
        );
        assert!(create_stencil_rect_instance(&node, ClipRect::INFINITE).is_none());
    }

    #[test]
    fn stencil_instance_uses_shader_clip_rect() {
        let node = make_frame_node(0.0, 0.0, 50.0, 50.0, [0.0; 4]);
        let clip = ClipRect::new(5.0, 10.0, 45.0, 40.0);
        let inst = create_stencil_rect_instance(&node, clip).unwrap();
        assert_eq!(inst.clip_rect, [5.0, 10.0, 45.0, 40.0]);
    }

    #[test]
    fn stencil_instance_gradient_fields_zeroed() {
        let node = make_frame_node(0.0, 0.0, 50.0, 50.0, [0.0; 4]);
        let inst = create_stencil_rect_instance(&node, ClipRect::INFINITE).unwrap();
        assert_eq!(inst.gradient_meta, [0.0; 4]);
        assert_eq!(inst.gradient_points, [0.0; 4]);
        assert_eq!(inst.gradient_stops, [0.0; 20]);
    }

    // --- dfs_traversal_order ---

    #[test]
    fn dfs_empty_scene() {
        let scene = SceneGraph::new();
        let order = dfs_traversal_order(&scene);
        assert!(order.is_empty());
    }

    #[test]
    fn dfs_single_root() {
        let mut scene = SceneGraph::new();
        let node = make_frame_node(0.0, 0.0, 100.0, 100.0, [0.0; 4]);
        let id = node.id;
        scene.add_root(node);

        let order = dfs_traversal_order(&scene);
        assert_eq!(order, vec![id]);
    }

    #[test]
    fn dfs_multiple_roots_in_insertion_order() {
        let mut scene = SceneGraph::new();
        let n1 = make_frame_node(0.0, 0.0, 100.0, 100.0, [0.0; 4]);
        let n2 = make_frame_node(200.0, 0.0, 100.0, 100.0, [0.0; 4]);
        let n3 = make_frame_node(400.0, 0.0, 100.0, 100.0, [0.0; 4]);
        let id1 = n1.id;
        let id2 = n2.id;
        let id3 = n3.id;
        scene.add_root(n1);
        scene.add_root(n2);
        scene.add_root(n3);

        let order = dfs_traversal_order(&scene);
        assert_eq!(order, vec![id1, id2, id3]);
    }

    #[test]
    fn dfs_parent_before_children() {
        let mut scene = SceneGraph::new();
        let parent = make_frame_node(0.0, 0.0, 200.0, 200.0, [0.0; 4]);
        let child1 = make_frame_node(10.0, 10.0, 50.0, 50.0, [0.0; 4]);
        let child2 = make_frame_node(70.0, 10.0, 50.0, 50.0, [0.0; 4]);
        let pid = parent.id;
        let cid1 = child1.id;
        let cid2 = child2.id;

        scene.add_root(parent);
        scene.add_child(pid, child1);
        scene.add_child(pid, child2);

        let order = dfs_traversal_order(&scene);
        assert_eq!(order, vec![pid, cid1, cid2]);
    }

    #[test]
    fn dfs_deep_hierarchy() {
        let mut scene = SceneGraph::new();
        let root = make_frame_node(0.0, 0.0, 500.0, 500.0, [0.0; 4]);
        let mid = make_frame_node(10.0, 10.0, 200.0, 200.0, [0.0; 4]);
        let leaf = make_frame_node(20.0, 20.0, 50.0, 50.0, [0.0; 4]);
        let root_id = root.id;
        let mid_id = mid.id;
        let leaf_id = leaf.id;

        scene.add_root(root);
        scene.add_child(root_id, mid);
        scene.add_child(mid_id, leaf);

        let order = dfs_traversal_order(&scene);
        assert_eq!(order, vec![root_id, mid_id, leaf_id]);
    }

    #[test]
    fn dfs_skips_invisible_nodes_and_subtrees() {
        let mut scene = SceneGraph::new();
        let root = make_frame_node(0.0, 0.0, 500.0, 500.0, [0.0; 4]);
        let visible_child = make_frame_node(10.0, 10.0, 100.0, 100.0, [0.0; 4]);
        let mut invisible_child = make_frame_node(120.0, 10.0, 100.0, 100.0, [0.0; 4]);
        invisible_child.visible = false;
        let grandchild_of_invisible = make_frame_node(130.0, 20.0, 50.0, 50.0, [0.0; 4]);

        let root_id = root.id;
        let vis_id = visible_child.id;
        let invis_id = invisible_child.id;
        let gc_id = grandchild_of_invisible.id;

        scene.add_root(root);
        scene.add_child(root_id, visible_child);
        scene.add_child(root_id, invisible_child);
        scene.add_child(invis_id, grandchild_of_invisible);

        let order = dfs_traversal_order(&scene);
        assert_eq!(order, vec![root_id, vis_id]);
        assert!(!order.contains(&invis_id));
        assert!(!order.contains(&gc_id));
    }

    #[test]
    fn dfs_mixed_roots_and_children() {
        let mut scene = SceneGraph::new();
        let r1 = make_frame_node(0.0, 0.0, 100.0, 100.0, [0.0; 4]);
        let r1c1 = make_frame_node(10.0, 10.0, 30.0, 30.0, [0.0; 4]);
        let r2 = make_frame_node(200.0, 0.0, 100.0, 100.0, [0.0; 4]);
        let r2c1 = make_frame_node(210.0, 10.0, 30.0, 30.0, [0.0; 4]);
        let r2c2 = make_frame_node(250.0, 10.0, 30.0, 30.0, [0.0; 4]);

        let r1_id = r1.id;
        let r1c1_id = r1c1.id;
        let r2_id = r2.id;
        let r2c1_id = r2c1.id;
        let r2c2_id = r2c2.id;

        scene.add_root(r1);
        scene.add_child(r1_id, r1c1);
        scene.add_root(r2);
        scene.add_child(r2_id, r2c1);
        scene.add_child(r2_id, r2c2);

        let order = dfs_traversal_order(&scene);
        assert_eq!(order, vec![r1_id, r1c1_id, r2_id, r2c1_id, r2c2_id]);
    }

    // --- build_draw_commands_from_render_order ---

    #[test]
    fn draw_commands_empty_render_order() {
        let commands = build_draw_commands_from_render_order(&[], (1920, 1080));
        assert!(commands.is_empty());
    }

    #[test]
    fn draw_commands_single_rect_normal_blend() {
        let entries = vec![RenderOrderEntry::Rect {
            instance_start: 0,
            instance_count: 3,
            blend: BlendMode::Normal,
            stencil_test: false,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DrawCommand::DrawRects {
                instance_start: 0,
                instance_count: 3,
                blend: BlendMode::Normal,
                stencil_test: false,
            }
        ));
    }

    #[test]
    fn draw_commands_rect_non_native_blend_wraps_with_copy_and_apply() {
        let entries = vec![RenderOrderEntry::Rect {
            instance_start: 0,
            instance_count: 1,
            blend: BlendMode::Multiply,
            stencil_test: false,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 3);
        assert!(matches!(&commands[0], DrawCommand::CopyFramebuffer));
        assert!(matches!(
            &commands[1],
            DrawCommand::DrawRects {
                blend: BlendMode::Multiply,
                ..
            }
        ));
        assert!(matches!(
            &commands[2],
            DrawCommand::ApplyBlend {
                mode: BlendMode::Multiply
            }
        ));
    }

    #[test]
    fn draw_commands_text_normal_blend() {
        let entries = vec![RenderOrderEntry::Text {
            pending_start: 5,
            pending_count: 10,
            blend: BlendMode::Normal,
            stencil_test: true,
            shader_clip_rect: ClipRect::INFINITE,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DrawCommand::DrawGlyphs {
                instance_start: 5,
                instance_count: 10,
                blend: BlendMode::Normal,
                stencil_test: true,
            }
        ));
    }

    #[test]
    fn draw_commands_image_non_native_blend() {
        let entries = vec![RenderOrderEntry::Image {
            pending_start: 0,
            pending_count: 2,
            blend: BlendMode::Screen,
            stencil_test: false,
            shader_clip_rect: ClipRect::INFINITE,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 3);
        assert!(matches!(&commands[0], DrawCommand::CopyFramebuffer));
        assert!(matches!(
            &commands[1],
            DrawCommand::DrawImages {
                blend: BlendMode::Screen,
                ..
            }
        ));
        assert!(matches!(
            &commands[2],
            DrawCommand::ApplyBlend {
                mode: BlendMode::Screen
            }
        ));
    }

    #[test]
    fn draw_commands_vector_add_blend_is_native() {
        let entries = vec![RenderOrderEntry::Vector {
            pending_start: 0,
            pending_count: 4,
            blend: BlendMode::Add,
            stencil_test: false,
            shader_clip_rect: ClipRect::INFINITE,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        // Add is native, so no CopyFramebuffer/ApplyBlend.
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DrawCommand::DrawVectors {
                blend: BlendMode::Add,
                ..
            }
        ));
    }

    #[test]
    fn draw_commands_push_scissor() {
        let entries = vec![RenderOrderEntry::PushScissor {
            clip_rect: ClipRect::new(10.0, 20.0, 110.0, 120.0),
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DrawCommand::SetScissor {
                x: 10,
                y: 20,
                width: 100,
                height: 100,
            }
        ));
    }

    #[test]
    fn draw_commands_push_scissor_outside_viewport_emits_nothing() {
        let entries = vec![RenderOrderEntry::PushScissor {
            clip_rect: ClipRect::new(-100.0, -100.0, -10.0, -10.0),
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert!(commands.is_empty());
    }

    #[test]
    fn draw_commands_pop_scissor_with_parent() {
        let entries = vec![RenderOrderEntry::PopScissor {
            parent_scissor: Some(ClipRect::new(0.0, 0.0, 500.0, 500.0)),
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DrawCommand::SetScissor {
                x: 0,
                y: 0,
                width: 500,
                height: 500,
            }
        ));
    }

    #[test]
    fn draw_commands_pop_scissor_no_parent_resets() {
        let entries = vec![RenderOrderEntry::PopScissor {
            parent_scissor: None,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], DrawCommand::ResetScissor));
    }

    #[test]
    fn draw_commands_pop_scissor_invalid_parent_resets() {
        // Parent scissor rect is outside viewport, so to_scissor_rect returns None.
        let entries = vec![RenderOrderEntry::PopScissor {
            parent_scissor: Some(ClipRect::new(-200.0, -200.0, -100.0, -100.0)),
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], DrawCommand::ResetScissor));
    }

    #[test]
    fn draw_commands_push_stencil() {
        let entries = vec![RenderOrderEntry::PushStencil {
            write_instance_start: 5,
            write_instance_count: 1,
            new_stencil_ref: 2,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            DrawCommand::StencilWrite {
                instance_start: 5,
                instance_count: 1,
            }
        ));
        assert!(matches!(&commands[1], DrawCommand::SetStencilRef(2)));
    }

    #[test]
    fn draw_commands_pop_stencil() {
        let entries = vec![RenderOrderEntry::PopStencil {
            decrement_instance_start: 10,
            decrement_instance_count: 1,
            restored_stencil_ref: 0,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            DrawCommand::StencilDecrement {
                instance_start: 10,
                instance_count: 1,
            }
        ));
        assert!(matches!(&commands[1], DrawCommand::SetStencilRef(0)));
    }

    #[test]
    fn draw_commands_complex_sequence() {
        // Simulates: scissor push, rect, text, scissor pop, stencil push/pop.
        let entries = vec![
            RenderOrderEntry::PushScissor {
                clip_rect: ClipRect::new(0.0, 0.0, 800.0, 600.0),
            },
            RenderOrderEntry::Rect {
                instance_start: 0,
                instance_count: 1,
                blend: BlendMode::Normal,
                stencil_test: false,
            },
            RenderOrderEntry::Text {
                pending_start: 0,
                pending_count: 5,
                blend: BlendMode::Normal,
                stencil_test: false,
                shader_clip_rect: ClipRect::INFINITE,
            },
            RenderOrderEntry::PopScissor {
                parent_scissor: None,
            },
            RenderOrderEntry::PushStencil {
                write_instance_start: 1,
                write_instance_count: 1,
                new_stencil_ref: 1,
            },
            RenderOrderEntry::Rect {
                instance_start: 2,
                instance_count: 1,
                blend: BlendMode::Normal,
                stencil_test: true,
            },
            RenderOrderEntry::PopStencil {
                decrement_instance_start: 3,
                decrement_instance_count: 1,
                restored_stencil_ref: 0,
            },
        ];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        // SetScissor, DrawRects, DrawGlyphs, ResetScissor,
        // StencilWrite, SetStencilRef(1), DrawRects, StencilDecrement, SetStencilRef(0)
        assert_eq!(commands.len(), 9);
        assert!(matches!(&commands[0], DrawCommand::SetScissor { .. }));
        assert!(matches!(&commands[1], DrawCommand::DrawRects { .. }));
        assert!(matches!(&commands[2], DrawCommand::DrawGlyphs { .. }));
        assert!(matches!(&commands[3], DrawCommand::ResetScissor));
        assert!(matches!(&commands[4], DrawCommand::StencilWrite { .. }));
        assert!(matches!(&commands[5], DrawCommand::SetStencilRef(1)));
        assert!(matches!(
            &commands[6],
            DrawCommand::DrawRects {
                stencil_test: true,
                ..
            }
        ));
        assert!(matches!(&commands[7], DrawCommand::StencilDecrement { .. }));
        assert!(matches!(&commands[8], DrawCommand::SetStencilRef(0)));
    }

    #[test]
    fn draw_commands_mixed_blend_modes() {
        // Normal rect, then Multiply rect, then Normal rect.
        let entries = vec![
            RenderOrderEntry::Rect {
                instance_start: 0,
                instance_count: 1,
                blend: BlendMode::Normal,
                stencil_test: false,
            },
            RenderOrderEntry::Rect {
                instance_start: 1,
                instance_count: 1,
                blend: BlendMode::Multiply,
                stencil_test: false,
            },
            RenderOrderEntry::Rect {
                instance_start: 2,
                instance_count: 1,
                blend: BlendMode::Normal,
                stencil_test: false,
            },
        ];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        // DrawRects(Normal), CopyFramebuffer, DrawRects(Multiply), ApplyBlend, DrawRects(Normal)
        assert_eq!(commands.len(), 5);
        assert!(matches!(
            &commands[0],
            DrawCommand::DrawRects {
                blend: BlendMode::Normal,
                ..
            }
        ));
        assert!(matches!(&commands[1], DrawCommand::CopyFramebuffer));
        assert!(matches!(
            &commands[2],
            DrawCommand::DrawRects {
                blend: BlendMode::Multiply,
                ..
            }
        ));
        assert!(matches!(
            &commands[3],
            DrawCommand::ApplyBlend {
                mode: BlendMode::Multiply
            }
        ));
        assert!(matches!(
            &commands[4],
            DrawCommand::DrawRects {
                blend: BlendMode::Normal,
                ..
            }
        ));
    }

    #[test]
    fn draw_commands_stencil_test_flag_preserved() {
        let entries = vec![RenderOrderEntry::Rect {
            instance_start: 0,
            instance_count: 1,
            blend: BlendMode::Normal,
            stencil_test: true,
        }];
        let commands = build_draw_commands_from_render_order(&entries, (1920, 1080));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            DrawCommand::DrawRects {
                stencil_test: true,
                ..
            }
        ));
    }
}
