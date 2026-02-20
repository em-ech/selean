//! Generic GPU texture atlas parameterized by channel count.
//!
//! `TextureAtlas<1>` stores single-channel data (SDF glyphs, `R8Unorm`).
//! `TextureAtlas<4>` stores RGBA data (images, rasterized vectors, `Rgba8UnormSrgb`).
//! Both use a shelf-packing algorithm for allocation and support dynamic growth.

use selean_common::error::EngineError;
use tracing::{debug, info};

use crate::text::packer::{PackResult, ShelfPacker};

/// Default initial atlas dimensions (1024×1024).
const DEFAULT_ATLAS_SIZE: u32 = 1024;

/// Padding between entries in texels (prevents texture filtering bleed).
const ATLAS_PADDING: u32 = 1;

/// A rectangular region within a texture atlas.
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

/// A generic GPU texture atlas parameterized by channel count.
///
/// - `CHANNELS = 1`: `R8Unorm` format (SDF glyphs)
/// - `CHANNELS = 4`: `Rgba8UnormSrgb` format (images, rasterized vectors)
///
/// Uses a shelf-packing algorithm for allocation and supports growing the
/// backing texture when full.
pub struct TextureAtlas<const CHANNELS: u32> {
    /// GPU texture.
    texture: wgpu::Texture,
    /// Texture view for shader binding.
    texture_view: wgpu::TextureView,
    /// Sampler with linear filtering.
    sampler: wgpu::Sampler,
    /// Bind group layout for the atlas (texture + sampler).
    bind_group_layout: wgpu::BindGroupLayout,
    /// Bind group for the atlas texture + sampler.
    bind_group: wgpu::BindGroup,
    /// Shelf-packing allocator for rectangle placement.
    packer: ShelfPacker,
    /// Maximum texture dimension (from GPU limits).
    max_dimension: u32,
}

impl<const CHANNELS: u32> TextureAtlas<CHANNELS> {
    /// Returns the `wgpu` texture format for this atlas.
    fn texture_format() -> wgpu::TextureFormat {
        match CHANNELS {
            1 => wgpu::TextureFormat::R8Unorm,
            4 => wgpu::TextureFormat::Rgba8UnormSrgb,
            _ => unreachable!("TextureAtlas only supports 1 or 4 channels"),
        }
    }

    /// Returns a label prefix for debug names.
    fn label_prefix() -> &'static str {
        match CHANNELS {
            1 => "glyph_atlas",
            4 => "image_atlas",
            _ => "atlas",
        }
    }

    /// Returns the error variant for this atlas type.
    fn atlas_full_error(old_w: u32, old_h: u32) -> EngineError {
        match CHANNELS {
            1 => EngineError::Font {
                reason: format!(
                    "glyph atlas at maximum size {old_w}x{old_h}, cannot grow further"
                ),
            },
            4 => EngineError::Image {
                reason: format!(
                    "image atlas at maximum size {old_w}x{old_h}, cannot grow further"
                ),
            },
            _ => unreachable!(),
        }
    }

    /// Creates a new texture atlas with the default initial size.
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
            channels = CHANNELS,
            "{} created",
            Self::label_prefix()
        );

        Self {
            texture,
            texture_view,
            sampler,
            bind_group_layout,
            bind_group,
            packer: ShelfPacker::new(initial_size, initial_size, ATLAS_PADDING),
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
        self.packer.dimensions()
    }

    /// Resets the atlas to its initial empty state.
    ///
    /// Creates a fresh texture at the default size and resets all packing state.
    /// Any caches referencing atlas regions should also be cleared.
    pub fn reset(&mut self, device: &wgpu::Device) {
        let initial_size = DEFAULT_ATLAS_SIZE.min(self.max_dimension);

        let (texture, texture_view) = Self::create_texture(device, initial_size, initial_size);
        let bind_group =
            Self::create_bind_group(device, &self.bind_group_layout, &texture_view, &self.sampler);

        self.texture = texture;
        self.texture_view = texture_view;
        self.bind_group = bind_group;
        self.packer.reset(initial_size, initial_size);

        info!(
            width = initial_size,
            height = initial_size,
            "{} reset",
            Self::label_prefix()
        );
    }

    /// Allocates a region in the atlas for an entry of the given dimensions.
    ///
    /// Grows the atlas if necessary. Returns the allocated region on success.
    ///
    /// # Errors
    ///
    /// Returns an error if the atlas cannot grow to accommodate the entry.
    pub fn allocate(
        &mut self,
        width: u32,
        height: u32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> Result<AtlasRegion, EngineError> {
        match self.packer.allocate(width, height) {
            PackResult::Placed { x, y } => Ok(AtlasRegion {
                x,
                y,
                width,
                height,
            }),
            PackResult::Full => {
                self.grow(device, queue)?;
                self.allocate(width, height, device, queue)
            }
        }
    }

    /// Uploads pixel data to a region in the atlas texture.
    ///
    /// The data must have `region.width * region.height * CHANNELS` bytes.
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
                bytes_per_row: Some(region.width * CHANNELS),
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
        let (old_w, old_h) = self.packer.dimensions();
        let new_width = (old_w * 2).min(self.max_dimension);
        let new_height = (old_h * 2).min(self.max_dimension);

        if new_width == old_w && new_height == old_h {
            return Err(Self::atlas_full_error(old_w, old_h));
        }

        info!(
            old_w,
            old_h,
            new_w = new_width,
            new_h = new_height,
            "Growing {}",
            Self::label_prefix()
        );

        let (new_texture, new_texture_view) = Self::create_texture(device, new_width, new_height);

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
                width: old_w,
                height: old_h,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(std::iter::once(encoder.finish()));

        let bind_group = Self::create_bind_group(
            device,
            &self.bind_group_layout,
            &new_texture_view,
            &self.sampler,
        );

        self.texture = new_texture;
        self.texture_view = new_texture_view;
        self.bind_group = bind_group;
        self.packer.grow(new_width, new_height);

        debug!(
            width = new_width,
            height = new_height,
            "Atlas grown"
        );

        Ok(())
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let label = match CHANNELS {
            1 => "glyph_atlas_texture",
            4 => "image_atlas_texture",
            _ => "atlas_texture",
        };
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
            format: Self::texture_format(),
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
            label: Some("atlas_sampler"),
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
impl<const CHANNELS: u32> std::fmt::Debug for TextureAtlas<CHANNELS> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextureAtlas")
            .field("channels", &CHANNELS)
            .field("packer", &self.packer)
            .field("max_dimension", &self.max_dimension)
            .finish()
    }
}

/// A single-channel atlas for SDF glyphs. Backward-compatible type alias.
pub type GlyphAtlas = TextureAtlas<1>;

/// A four-channel RGBA atlas for images and rasterized vectors.
pub type ImageAtlas = TextureAtlas<4>;

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

    #[test]
    fn texture_format_channels_1() {
        assert_eq!(
            TextureAtlas::<1>::texture_format(),
            wgpu::TextureFormat::R8Unorm
        );
    }

    #[test]
    fn texture_format_channels_4() {
        assert_eq!(
            TextureAtlas::<4>::texture_format(),
            wgpu::TextureFormat::Rgba8UnormSrgb
        );
    }
}
