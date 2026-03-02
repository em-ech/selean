//! Top-level renderer orchestrator.
//!
//! The `Renderer` owns the GPU context, camera, pipelines, and manages the
//! per-frame render loop: collecting visible nodes, updating GPU buffers,
//! and executing render passes.

use selean_common::error::EngineError;
use selean_common::types::NodeId;
use tracing::{debug, info, warn};

use super::blend_pipeline::BlendResources;
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

        let is_scroll = node.scroll_offset != [0.0, 0.0];
        let effective_clip = if node.clip_mode != ClipMode::None {
            node.clip_mode
        } else if is_scroll {
            ClipMode::Scissor
        } else {
            ClipMode::None
        };
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
    #[allow(clippy::too_many_lines)]
    fn build_draw_list(&mut self) {
        let (viewport_w, viewport_h) = self.camera.viewport_size();
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let vp_size = (viewport_w as u32, viewport_h as u32);

        // Take render_order temporarily to avoid borrow conflict with self.draw_list.
        let render_order = std::mem::take(&mut self.render_order);

        for entry in &render_order {
            match entry {
                RenderOrderEntry::Rect {
                    instance_start,
                    instance_count,
                    blend,
                    stencil_test,
                } => {
                    if !blend.is_native() {
                        self.draw_list.push(DrawCommand::CopyFramebuffer);
                    }
                    self.draw_list.push(DrawCommand::DrawRects {
                        instance_start: *instance_start,
                        instance_count: *instance_count,
                        blend: *blend,
                        stencil_test: *stencil_test,
                    });
                    if !blend.is_native() {
                        self.draw_list
                            .push(DrawCommand::ApplyBlend { mode: *blend });
                    }
                }
                RenderOrderEntry::Text {
                    pending_start,
                    pending_count,
                    blend,
                    stencil_test,
                    shader_clip_rect,
                } => {
                    // Patch clip rects on ordered text instances.
                    let start = *pending_start as usize;
                    let count = *pending_count as usize;
                    let clip_arr = shader_clip_rect.to_array();
                    let instances = self.text_batch.ordered_instances_mut();
                    for inst in instances.iter_mut().skip(start).take(count) {
                        inst.clip_rect = clip_arr;
                    }
                    if !blend.is_native() {
                        self.draw_list.push(DrawCommand::CopyFramebuffer);
                    }
                    self.draw_list.push(DrawCommand::DrawGlyphs {
                        instance_start: *pending_start,
                        instance_count: *pending_count,
                        blend: *blend,
                        stencil_test: *stencil_test,
                    });
                    if !blend.is_native() {
                        self.draw_list
                            .push(DrawCommand::ApplyBlend { mode: *blend });
                    }
                }
                RenderOrderEntry::Image {
                    pending_start,
                    pending_count,
                    blend,
                    stencil_test,
                    shader_clip_rect,
                } => {
                    let start = *pending_start as usize;
                    let count = *pending_count as usize;
                    let clip_arr = shader_clip_rect.to_array();
                    let instances = self.image_batch.ordered_instances_mut();
                    for inst in instances.iter_mut().skip(start).take(count) {
                        inst.clip_rect = clip_arr;
                    }
                    if !blend.is_native() {
                        self.draw_list.push(DrawCommand::CopyFramebuffer);
                    }
                    self.draw_list.push(DrawCommand::DrawImages {
                        instance_start: *pending_start,
                        instance_count: *pending_count,
                        blend: *blend,
                        stencil_test: *stencil_test,
                    });
                    if !blend.is_native() {
                        self.draw_list
                            .push(DrawCommand::ApplyBlend { mode: *blend });
                    }
                }
                RenderOrderEntry::Vector {
                    pending_start,
                    pending_count,
                    blend,
                    stencil_test,
                    shader_clip_rect,
                } => {
                    let start = *pending_start as usize;
                    let count = *pending_count as usize;
                    let clip_arr = shader_clip_rect.to_array();
                    let instances = self.vector_batch.ordered_instances_mut();
                    for inst in instances.iter_mut().skip(start).take(count) {
                        inst.clip_rect = clip_arr;
                    }
                    if !blend.is_native() {
                        self.draw_list.push(DrawCommand::CopyFramebuffer);
                    }
                    self.draw_list.push(DrawCommand::DrawVectors {
                        instance_start: *pending_start,
                        instance_count: *pending_count,
                        blend: *blend,
                        stencil_test: *stencil_test,
                    });
                    if !blend.is_native() {
                        self.draw_list
                            .push(DrawCommand::ApplyBlend { mode: *blend });
                    }
                }
                RenderOrderEntry::PushScissor { clip_rect } => {
                    if let Some((x, y, w, h)) = clip_rect.to_scissor_rect(vp_size.0, vp_size.1) {
                        self.draw_list.push(DrawCommand::SetScissor {
                            x,
                            y,
                            width: w,
                            height: h,
                        });
                    }
                }
                RenderOrderEntry::PopScissor { parent_scissor } => {
                    if let Some(parent) = parent_scissor {
                        if let Some((x, y, w, h)) = parent.to_scissor_rect(vp_size.0, vp_size.1) {
                            self.draw_list.push(DrawCommand::SetScissor {
                                x,
                                y,
                                width: w,
                                height: h,
                            });
                        } else {
                            self.draw_list.push(DrawCommand::ResetScissor);
                        }
                    } else {
                        self.draw_list.push(DrawCommand::ResetScissor);
                    }
                }
                RenderOrderEntry::PushStencil {
                    write_instance_start,
                    write_instance_count,
                    new_stencil_ref,
                } => {
                    self.draw_list.push(DrawCommand::StencilWrite {
                        instance_start: *write_instance_start,
                        instance_count: *write_instance_count,
                    });
                    self.draw_list
                        .push(DrawCommand::SetStencilRef(*new_stencil_ref));
                }
                RenderOrderEntry::PopStencil {
                    decrement_instance_start,
                    decrement_instance_count,
                    restored_stencil_ref,
                } => {
                    self.draw_list.push(DrawCommand::StencilDecrement {
                        instance_start: *decrement_instance_start,
                        instance_count: *decrement_instance_count,
                    });
                    self.draw_list
                        .push(DrawCommand::SetStencilRef(*restored_stencil_ref));
                }
            }
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
    })
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
