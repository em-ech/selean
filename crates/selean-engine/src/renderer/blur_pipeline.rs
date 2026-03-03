//! Dual Kawase blur pipeline for drop shadow and layer blur effects.
//!
//! Uses a two-pass approach (downsample + upsample) with iterative
//! ping-pong between two half-resolution textures to achieve wide blurs
//! efficiently. Iterations = max(1, ceil(blur_radius / 4.0)).

use wgpu::util::DeviceExt;

/// GPU resources for the dual Kawase blur effect.
pub struct BlurResources {
    /// Half-resolution ping texture (ping-pong target).
    tex_a: wgpu::Texture,
    view_a: wgpu::TextureView,
    /// Half-resolution pong texture (ping-pong target).
    tex_b: wgpu::Texture,
    view_b: wgpu::TextureView,
    /// Downsample pipeline (full-res -> half-res, or half -> half).
    downsample_pipeline: wgpu::RenderPipeline,
    /// Upsample pipeline (half-res -> full-res, or half -> half).
    upsample_pipeline: wgpu::RenderPipeline,
    /// Bind group layout shared by both passes.
    bind_group_layout: wgpu::BindGroupLayout,
    /// Uniform buffer for `texel_size` parameter.
    params_buffer: wgpu::Buffer,
    /// Linear sampler for texture reads.
    sampler: wgpu::Sampler,
    /// Current half-resolution dimensions.
    half_size: (u32, u32),
    /// Texture format.
    format: wgpu::TextureFormat,
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
impl BlurResources {
    /// Creates blur resources for the given device, format, and viewport dimensions.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let hw = (width / 2).max(1);
        let hh = (height / 2).max(1);

        let (tex_a, view_a) = create_blur_texture(device, format, hw, hh, "blur_a");
        let (tex_b, view_b) = create_blur_texture(device, format, hw, hh, "blur_b");

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("blur_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 16-byte uniform: `vec2<f32>` texel_size + `vec2<f32>` padding
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("blur_params_buffer"),
            contents: &[0u8; 16],
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blur_bind_group_layout"),
            entries: &[
                // binding 0: source texture
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
                // binding 1: sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // binding 2: uniform params
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blur_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let shader_source = include_str!("shaders/blur.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blur_shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let downsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur_downsample_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
                entry_point: Some("fs_downsample"),
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

        let upsample_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blur_upsample_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
                entry_point: Some("fs_upsample"),
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
            tex_a,
            view_a,
            tex_b,
            view_b,
            downsample_pipeline,
            upsample_pipeline,
            bind_group_layout,
            params_buffer,
            sampler,
            half_size: (hw, hh),
            format,
        }
    }

    /// Recreates the half-resolution textures when the viewport changes.
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        let hw = (width / 2).max(1);
        let hh = (height / 2).max(1);
        if (hw, hh) == self.half_size {
            return;
        }
        let (pt, pv) = create_blur_texture(device, self.format, hw, hh, "blur_a");
        let (qt, qv) = create_blur_texture(device, self.format, hw, hh, "blur_b");
        self.tex_a = pt;
        self.view_a = pv;
        self.tex_b = qt;
        self.view_b = qv;
        self.half_size = (hw, hh);
    }

    /// Returns the number of blur iterations for a given blur radius.
    #[must_use]
    pub fn iterations_for_radius(blur_radius: f32) -> u32 {
        (blur_radius / 4.0).ceil().max(1.0) as u32
    }

    /// Applies a blur to `source_view`, writing the result to `target_view`.
    ///
    /// The source is first downsampled to the internal half-res textures,
    /// iteratively blurred, then upsampled back to the target.
    pub fn blur(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
        target_view: &wgpu::TextureView,
        blur_radius: f32,
    ) {
        let iterations = Self::iterations_for_radius(blur_radius);
        let (hw, hh) = self.half_size;

        // Pass 1: Downsample source -> ping
        self.write_texel_size(queue, true);
        self.run_pass(
            device,
            encoder,
            &self.downsample_pipeline,
            source_view,
            &self.view_a,
        );

        // Iterative blur passes on half-res (ping-pong).
        for i in 0..iterations.saturating_sub(1) {
            let half_texel = [1.0 / hw as f32, 1.0 / hh as f32, 0.0f32, 0.0f32];
            queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&half_texel));

            if i % 2 == 0 {
                self.run_pass(
                    device,
                    encoder,
                    &self.downsample_pipeline,
                    &self.view_a,
                    &self.view_b,
                );
            } else {
                self.run_pass(
                    device,
                    encoder,
                    &self.downsample_pipeline,
                    &self.view_b,
                    &self.view_a,
                );
            }
        }

        // Final upsample: half -> target
        let last_view = if (iterations.saturating_sub(1)) % 2 == 0 {
            &self.view_a
        } else {
            &self.view_b
        };
        let half_texel = [1.0 / hw as f32, 1.0 / hh as f32, 0.0f32, 0.0f32];
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&half_texel));
        self.run_pass(
            device,
            encoder,
            &self.upsample_pipeline,
            last_view,
            target_view,
        );
    }

    /// Writes the texel size into the params buffer.
    ///
    /// If `full_res` is true, uses the full viewport size (2x `half_size`).
    /// Otherwise uses `half_size`.
    fn write_texel_size(&self, queue: &wgpu::Queue, full_res: bool) {
        let (w, h) = if full_res {
            (self.half_size.0 * 2, self.half_size.1 * 2)
        } else {
            self.half_size
        };
        let texel = [1.0 / w as f32, 1.0 / h as f32, 0.0f32, 0.0f32];
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&texel));
    }

    /// Runs a single fullscreen pass: reads from `source`, writes to `target`.
    fn run_pass(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        source: &wgpu::TextureView,
        target: &wgpu::TextureView,
    ) {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blur_pass_bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(source),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer.as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("blur_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

/// Creates a color texture for blur ping-pong.
fn create_blur_texture(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterations_for_small_radius() {
        assert_eq!(BlurResources::iterations_for_radius(1.0), 1);
        assert_eq!(BlurResources::iterations_for_radius(0.5), 1);
        assert_eq!(BlurResources::iterations_for_radius(4.0), 1);
    }

    #[test]
    fn iterations_for_medium_radius() {
        assert_eq!(BlurResources::iterations_for_radius(5.0), 2);
        assert_eq!(BlurResources::iterations_for_radius(8.0), 2);
    }

    #[test]
    fn iterations_for_large_radius() {
        assert_eq!(BlurResources::iterations_for_radius(12.0), 3);
        assert_eq!(BlurResources::iterations_for_radius(16.0), 4);
        assert_eq!(BlurResources::iterations_for_radius(20.0), 5);
    }

    #[test]
    fn iterations_for_zero_radius() {
        assert_eq!(BlurResources::iterations_for_radius(0.0), 1);
    }
}
