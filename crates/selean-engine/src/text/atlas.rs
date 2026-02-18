//! GPU texture atlas for SDF glyphs.
//!
//! Manages a single-channel (`R8Unorm`) GPU texture that stores SDF bitmaps
//! for rendered glyphs. Uses a simple shelf-packing algorithm: glyphs are
//! packed left-to-right in rows, with new rows created when the current row
//! overflows.

use selean_common::error::EngineError;
use tracing::{debug, info};

/// Default initial atlas dimensions (1024x1024 = 1 MB).
const DEFAULT_ATLAS_SIZE: u32 = 1024;

/// Padding between glyphs in texels (prevents texture filtering bleed).
const GLYPH_PADDING: u32 = 1;

/// A rectangular region within the texture atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasRegion {
    /// Top-left X in texels.
    pub x: u32,
    /// Top-left Y in texels.
    pub y: u32,
    /// Width in texels.
    pub width: u32,
    /// Height in texels.
    pub height: u32,
}

impl AtlasRegion {
    /// Computes UV coordinates for this region given the full atlas dimensions.
    ///
    /// Returns `[u_min, v_min, u_max, v_max]` in the range [0.0, 1.0].
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn uv_rect(&self, atlas_width: u32, atlas_height: u32) -> [f32; 4] {
        let aw = atlas_width as f32;
        let ah = atlas_height as f32;
        [
            self.x as f32 / aw,
            self.y as f32 / ah,
            (self.x + self.width) as f32 / aw,
            (self.y + self.height) as f32 / ah,
        ]
    }
}

/// Manages a GPU texture atlas for SDF glyphs.
///
/// Uses a shelf-packing algorithm: rows of variable height, new glyphs packed
/// left-to-right, new rows created when the current row is full.
pub struct GlyphAtlas {
    /// GPU texture (`R8Unorm`, single channel).
    texture: wgpu::Texture,
    /// Texture view for shader binding.
    texture_view: wgpu::TextureView,
    /// Sampler with linear filtering for smooth SDF interpolation.
    sampler: wgpu::Sampler,
    /// Bind group layout for the atlas (texture + sampler).
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for the atlas texture + sampler.
    bind_group: wgpu::BindGroup,
    /// Current atlas width in texels.
    width: u32,
    /// Current atlas height in texels.
    height: u32,
    /// Current packing cursor: x position in the current row.
    cursor_x: u32,
    /// Current packing cursor: y position of the current row's top.
    cursor_y: u32,
    /// Height of the current row (max glyph height in this row).
    row_height: u32,
    /// Maximum texture dimension (from GPU limits).
    max_dimension: u32,
}

impl GlyphAtlas {
    /// Creates a new glyph atlas with the default initial size.
    ///
    /// # Arguments
    /// * `device` — The GPU device.
    #[must_use]
    pub fn new(device: &wgpu::Device) -> Self {
        let max_dimension = device.limits().max_texture_dimension_2d;
        let initial_size = DEFAULT_ATLAS_SIZE.min(max_dimension);

        let (texture, texture_view) = Self::create_texture(device, initial_size, initial_size);
        let sampler = Self::create_sampler(device);
        let bind_group_layout = Self::create_bind_group_layout(device);
        let bind_group =
            Self::create_bind_group(device, &bind_group_layout, &texture_view, &sampler);

        info!(
            width = initial_size,
            height = initial_size,
            "Glyph atlas created"
        );

        Self {
            texture,
            texture_view,
            sampler,
            bind_group_layout,
            bind_group,
            width: initial_size,
            height: initial_size,
            cursor_x: 0,
            cursor_y: 0,
            row_height: 0,
            max_dimension,
        }
    }

    /// Returns the bind group layout for the atlas (for pipeline creation).
    #[must_use]
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Returns the bind group for the atlas (for draw calls).
    #[must_use]
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Returns the atlas dimensions.
    #[must_use]
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Allocates a region in the atlas for a glyph of the given dimensions.
    ///
    /// Returns `None` if the atlas is full and cannot grow further.
    /// Uses shelf-packing: glyphs are placed left-to-right with padding,
    /// creating new rows as needed.
    ///
    /// # Errors
    ///
    /// Returns `EngineError::Font` if the atlas cannot grow to accommodate the glyph.
    pub fn allocate(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<AtlasRegion, EngineError> {
        let padded_w = width + GLYPH_PADDING;
        let padded_h = height + GLYPH_PADDING;

        // Try to fit in the current row.
        if self.cursor_x + padded_w <= self.width && self.cursor_y + padded_h <= self.height {
            let region = AtlasRegion {
                x: self.cursor_x,
                y: self.cursor_y,
                width,
                height,
            };
            self.cursor_x += padded_w;
            if padded_h > self.row_height {
                self.row_height = padded_h;
            }
            return Ok(region);
        }

        // Try starting a new row.
        let new_row_y = self.cursor_y + self.row_height;
        if padded_w <= self.width && new_row_y + padded_h <= self.height {
            self.cursor_x = padded_w;
            self.cursor_y = new_row_y;
            self.row_height = padded_h;

            return Ok(AtlasRegion {
                x: 0,
                y: new_row_y,
                width,
                height,
            });
        }

        // Atlas is full — try to grow.
        self.grow(device, queue)?;

        // Retry allocation after growth (recursive, bounded by max_dimension).
        self.allocate(width, height, device, queue)
    }

    /// Uploads SDF data to a region in the atlas texture.
    pub fn upload(&self, queue: &wgpu::Queue, region: &AtlasRegion, data: &[u8]) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: region.x,
                    y: region.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(region.width),
                rows_per_image: Some(region.height),
            },
            wgpu::Extent3d {
                width: region.width,
                height: region.height,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Doubles the atlas size, copying existing data to the new texture.
    fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<(), EngineError> {
        let new_width = (self.width * 2).min(self.max_dimension);
        let new_height = (self.height * 2).min(self.max_dimension);

        if new_width == self.width && new_height == self.height {
            return Err(EngineError::Font {
                reason: format!(
                    "glyph atlas at maximum size {}x{}, cannot grow further",
                    self.width, self.height
                ),
            });
        }

        info!(
            old_w = self.width,
            old_h = self.height,
            new_w = new_width,
            new_h = new_height,
            "Growing glyph atlas"
        );

        let (new_texture, new_texture_view) = Self::create_texture(device, new_width, new_height);

        // Copy old texture contents to the new texture.
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("atlas_grow_encoder"),
        });

        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &new_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        // Recreate the bind group with the new texture view.
        let bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &new_texture_view,
            &self.sampler,
        );

        self.texture = new_texture;
        self.texture_view = new_texture_view;
        self.bind_group = bind_group;
        self.width = new_width;
        self.height = new_height;

        debug!(width = new_width, height = new_height, "Atlas grown");

        Ok(())
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph_atlas_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph_atlas_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        })
    }

    fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("atlas_bind_group_layout"),
            entries: &[
                // binding(0): SDF atlas texture
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
                // binding(1): atlas sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("atlas_bind_group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for GlyphAtlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlyphAtlas")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("cursor_x", &self.cursor_x)
            .field("cursor_y", &self.cursor_y)
            .field("row_height", &self.row_height)
            .field("max_dimension", &self.max_dimension)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_region_uv_rect() {
        let region = AtlasRegion {
            x: 100,
            y: 200,
            width: 50,
            height: 60,
        };

        let uv = region.uv_rect(1024, 1024);
        let expected = [
            100.0 / 1024.0,
            200.0 / 1024.0,
            150.0 / 1024.0,
            260.0 / 1024.0,
        ];
        for (a, b) in uv.iter().zip(expected.iter()) {
            assert!((a - b).abs() < f32::EPSILON, "uv {a} != expected {b}");
        }
    }

    #[test]
    fn atlas_region_uv_full_texture() {
        let region = AtlasRegion {
            x: 0,
            y: 0,
            width: 512,
            height: 512,
        };

        let uv = region.uv_rect(512, 512);
        assert!((uv[0]).abs() < f32::EPSILON);
        assert!((uv[1]).abs() < f32::EPSILON);
        assert!((uv[2] - 1.0).abs() < f32::EPSILON);
        assert!((uv[3] - 1.0).abs() < f32::EPSILON);
    }
}
