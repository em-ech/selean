//! Image decoding and resizing.
//!
//! Provides eager decode + optional resize at asset registration time.
//! Keeps the render loop free of I/O and decode work.

use selean_common::error::EngineError;

/// A decoded RGBA image ready for atlas upload.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    /// RGBA pixel data (4 bytes per pixel, row-major).
    pub data: Vec<u8>,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
}

/// Decodes raw image bytes (PNG, JPEG, or WebP) into RGBA pixel data.
///
/// # Errors
///
/// Returns `EngineError::Image` if the bytes cannot be decoded.
pub fn decode_image(bytes: &[u8]) -> Result<DecodedImage, EngineError> {
    let img = image::load_from_memory(bytes).map_err(|e| EngineError::Image {
        reason: format!("failed to decode image: {e}"),
    })?;

    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();

    if width == 0 || height == 0 {
        return Err(EngineError::Image {
            reason: "decoded image has zero dimensions".to_string(),
        });
    }

    Ok(DecodedImage {
        data: rgba.into_raw(),
        width,
        height,
    })
}

/// Decodes and resizes an image so its largest dimension does not exceed `max_dim`.
///
/// If the image is already within bounds, no resizing occurs.
///
/// # Errors
///
/// Returns `EngineError::Image` if the bytes cannot be decoded.
pub fn decode_image_resized(bytes: &[u8], max_dim: u32) -> Result<DecodedImage, EngineError> {
    let img = image::load_from_memory(bytes).map_err(|e| EngineError::Image {
        reason: format!("failed to decode image: {e}"),
    })?;

    let width = img.width();
    let height = img.height();

    if width == 0 || height == 0 {
        return Err(EngineError::Image {
            reason: "decoded image has zero dimensions".to_string(),
        });
    }

    let resized = if width > max_dim || height > max_dim {
        img.resize(max_dim, max_dim, image::imageops::FilterType::Lanczos3)
    } else {
        img
    };

    let rgba = resized.to_rgba8();
    let w = rgba.width();
    let h = rgba.height();
    Ok(DecodedImage {
        data: rgba.into_raw(),
        width: w,
        height: h,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// Creates a minimal valid 2x2 RGBA PNG for testing.
    fn make_test_png(width: u32, height: u32) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            // Create solid red pixels.
            let data: Vec<u8> = (0..(width * height))
                .flat_map(|_| [255u8, 0, 0, 255])
                .collect();
            image::ImageEncoder::write_image(
                encoder,
                &data,
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .expect("encode test png");
        }
        buf
    }

    #[test]
    fn decode_valid_png() {
        let png = make_test_png(4, 4);
        let decoded = decode_image(&png).expect("should decode");
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 4);
        assert_eq!(decoded.data.len(), 4 * 4 * 4); // 4x4 RGBA
    }

    #[test]
    fn decode_invalid_bytes_fails() {
        let result = decode_image(b"not an image");
        assert!(result.is_err());
    }

    #[test]
    fn decode_resized_within_bounds() {
        let png = make_test_png(4, 4);
        let decoded = decode_image_resized(&png, 1024).expect("should decode");
        // Should not resize since 4 <= 1024.
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 4);
    }

    #[test]
    fn decode_resized_exceeds_max() {
        let png = make_test_png(100, 50);
        let decoded = decode_image_resized(&png, 40).expect("should decode");
        // Largest dim (100) should be reduced to 40, maintaining aspect ratio.
        assert!(decoded.width <= 40);
        assert!(decoded.height <= 40);
        assert!(decoded.width > 0);
        assert!(decoded.height > 0);
    }

    #[test]
    fn decode_preserves_alpha() {
        // Create a 1x1 PNG with semi-transparent pixel.
        let mut buf = Vec::new();
        {
            let encoder = image::codecs::png::PngEncoder::new(&mut buf);
            image::ImageEncoder::write_image(
                encoder,
                &[128, 64, 32, 200],
                1,
                1,
                image::ExtendedColorType::Rgba8,
            )
            .expect("encode");
        }
        let decoded = decode_image(&buf).expect("decode");
        assert_eq!(decoded.data, vec![128, 64, 32, 200]);
    }
}
