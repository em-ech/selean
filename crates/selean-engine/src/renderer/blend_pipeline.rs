//! GPU resources for shader-based blend mode compositing.
//!
//! Manages the intermediate render target, snapshot texture, and blend/blit
//! pipelines required for non-native blend modes (Multiply, Screen, etc.).
//!
//! The workflow for a non-native blend element:
//! 1. All preceding commands render to the intermediate texture.
//! 2. `snapshot()` copies the intermediate to the snapshot texture.
//! 3. The element renders to the intermediate with `BlendState::REPLACE`.
//! 4. `blend_pass()` composites src (intermediate) and dst (snapshot) using the
//!    blend formula, writing the result back to the intermediate.
//! 5. After all commands, `blit_to_target()` copies intermediate to swapchain.

use wgpu::util::DeviceExt;

use crate::scene::BlendMode;

/// GPU resources for intermediate-target blend mode compositing.
pub struct BlendResources {
    /// Intermediate render target (all scene rendering goes here).
    intermediate_texture: wgpu::Texture,
    /// View into the intermediate render target.
    intermediate_view: wgpu::TextureView,
    /// Snapshot of intermediate (captured before non-native blend element).
    snapshot_texture: wgpu::Texture,
    /// View into the snapshot texture.
    snapshot_view: wgpu::TextureView,
    /// Depth/stencil texture for the intermediate render target.
    stencil_texture: wgpu::Texture,
    /// Depth/stencil view for render pass attachment.
    stencil_view: wgpu::TextureView,
    /// Pipeline for the blend composite pass.
    blend_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for the blend pass.
    blend_bind_group_layout: wgpu::BindGroupLayout,
    /// Uniform buffer for blend mode index.
    blend_uniform_buffer: wgpu::Buffer,
    /// Pipeline for the final blit to swapchain.
    blit_pipeline: wgpu::RenderPipeline,
    /// Bind group layout for the blit pass.
    blit_bind_group_layout: wgpu::BindGroupLayout,
    /// Shared sampler for texture reads.
    sampler: wgpu::Sampler,
    /// Current texture dimensions.
    size: (u32, u32),
    /// Texture format (matches swapchain).
    format: wgpu::TextureFormat,
}

impl BlendResources {
    /// Creates blend resources for the given device, format, and dimensions.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let w = width.max(1);
        let h = height.max(1);

        let (intermediate_texture, intermediate_view) =
            create_color_texture(device, format, w, h, "blend_intermediate");
        let (snapshot_texture, snapshot_view) =
            create_color_texture(device, format, w, h, "blend_snapshot");
        let (stencil_texture, stencil_view) = create_stencil_texture(device, w, h);

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blend_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let blend_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blend_uniform_buffer"),
            contents: &[0u8; 16], // u32 mode + 3x u32 padding = 16 bytes
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Blend pass: bind group layout with src texture, dst texture, sampler, uniform.
        let blend_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blend_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let blend_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("blend_pipeline_layout"),
                bind_group_layouts: &[&blend_bind_group_layout],
                push_constant_ranges: &[],
            });

        let blend_shader_source = include_str!("shaders/blend.wgsl");
        let blend_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blend_shader"),
            source: wgpu::ShaderSource::Wgsl(blend_shader_source.into()),
        });

        let blend_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blend_pipeline"),
            layout: Some(&blend_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blend_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blend_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        // Blit pass: bind group layout with texture + sampler.
        let blit_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("blit_bind_group_layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let blit_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit_pipeline_layout"),
            bind_group_layouts: &[&blit_bind_group_layout],
            push_constant_ranges: &[],
        });

        let blit_shader_source = include_str!("shaders/blit.wgsl");
        let blit_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit_shader"),
            source: wgpu::ShaderSource::Wgsl(blit_shader_source.into()),
        });

        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit_pipeline"),
            layout: Some(&blit_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blit_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &blit_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            intermediate_texture,
            intermediate_view,
            snapshot_texture,
            snapshot_view,
            stencil_texture,
            stencil_view,
            blend_pipeline,
            blend_bind_group_layout,
            blend_uniform_buffer,
            blit_pipeline,
            blit_bind_group_layout,
            sampler,
            size: (w, h),
            format,
        }
    }

    /// Recreates textures on viewport resize.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let w = width.max(1);
        let h = height.max(1);
        if (w, h) == self.size {
            return;
        }

        let (intermediate_texture, intermediate_view) =
            create_color_texture(device, self.format, w, h, "blend_intermediate");
        let (snapshot_texture, snapshot_view) =
            create_color_texture(device, self.format, w, h, "blend_snapshot");
        let (stencil_texture, stencil_view) = create_stencil_texture(device, w, h);

        self.intermediate_texture = intermediate_texture;
        self.intermediate_view = intermediate_view;
        self.snapshot_texture = snapshot_texture;
        self.snapshot_view = snapshot_view;
        self.stencil_texture = stencil_texture;
        self.stencil_view = stencil_view;
        self.size = (w, h);
    }

    /// Returns the intermediate render target view (used as the color attachment).
    #[must_use]
    pub fn intermediate_view(&self) -> &wgpu::TextureView {
        &self.intermediate_view
    }

    /// Returns the stencil view for the intermediate target.
    #[must_use]
    pub fn stencil_view(&self) -> &wgpu::TextureView {
        &self.stencil_view
    }

    /// Returns the current texture dimensions.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Copies the intermediate texture to the snapshot texture.
    ///
    /// Call this before rendering a non-native blend element to capture the
    /// current framebuffer state as the "destination" for the blend formula.
    pub fn snapshot(&self, encoder: &mut wgpu::CommandEncoder) {
        let (w, h) = self.size;
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.intermediate_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &self.snapshot_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Runs the blend composite pass.
    ///
    /// Reads `snapshot_view` (dst) and `intermediate_view` (src after element render),
    /// applies the blend formula for the given mode, writes result to intermediate.
    ///
    /// The intermediate texture cannot be simultaneously read and written in the same
    /// render pass. This method uses a temporary copy: the intermediate is already in
    /// `snapshot_view` (captured before the element), and the element was drawn on top.
    /// We read both and write to the intermediate with REPLACE blend.
    pub fn blend_pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mode: BlendMode,
    ) {
        // Write blend mode index to uniform buffer.
        let mode_bytes = mode.shader_index().to_le_bytes();
        let mut uniform_data = [0u8; 16];
        uniform_data[..4].copy_from_slice(&mode_bytes);
        queue.write_buffer(&self.blend_uniform_buffer, 0, &uniform_data);

        // We need the intermediate as a texture input, but we also need to write
        // back to it. Solution: copy intermediate to a temporary read texture,
        // then composite temp_read (src) + snapshot (dst) -> intermediate.
        let (w, h) = self.size;
        let (temp_texture, temp_view) =
            create_color_texture(device, self.format, w, h, "blend_temp_src");
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.intermediate_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &temp_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );

        // Create bind group: src=temp_read, dst=snapshot.
        let blend_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blend_bind_group"),
            layout: &self.blend_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&temp_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.snapshot_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.blend_uniform_buffer.as_entire_binding(),
                },
            ],
        });

        // Render pass targeting intermediate with REPLACE blend.
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blend_composite_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.intermediate_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.blend_pipeline);
        pass.set_bind_group(0, &blend_bind_group, &[]);
        pass.draw(0..3, 0..1); // Full-screen triangle
    }

    /// Blits the intermediate texture to the swapchain target view.
    pub fn blit_to_target(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        target_view: &wgpu::TextureView,
    ) {
        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bind_group"),
            layout: &self.blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.intermediate_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blit_to_swapchain_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&self.blit_pipeline);
        pass.set_bind_group(0, &blit_bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Creates a color texture suitable for use as both a render target and texture input.
fn create_color_texture(
    device: &wgpu::Device,
    format: wgpu::TextureFormat,
    width: u32,
    height: u32,
    label: &str,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}

/// Creates a depth/stencil texture for the intermediate render target.
fn create_stencil_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("blend_stencil_texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth24PlusStencil8,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
