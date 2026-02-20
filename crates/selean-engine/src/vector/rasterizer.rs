//! Vector path rasterizer using `tiny-skia`.
//!
//! Converts `tiny_skia::Path` objects into RGBA pixel data by rendering
//! them to a `tiny_skia::Pixmap`.

use selean_common::error::EngineError;

use crate::scene::Color;

/// A rasterized vector path as RGBA pixel data.
#[derive(Debug, Clone)]
pub struct RasterizedVector {
    /// RGBA pixel data (4 bytes per pixel, row-major, premultiplied alpha).
    pub data: Vec<u8>,
    /// Rasterized width in pixels.
    pub width: u32,
    /// Rasterized height in pixels.
    pub height: u32,
}

/// Rasterizes a `tiny_skia::Path` to RGBA pixels at the given dimensions.
///
/// The path is scaled to fit within `width × height`. If `fill` is provided,
/// the path is filled with that color. If `stroke` and `stroke_width` are
/// provided, the path is also stroked.
///
/// # Errors
///
/// Returns `EngineError::Vector` if rasterization fails (e.g., zero dimensions).
pub fn rasterize_path(
    path: &tiny_skia::Path,
    width: u32,
    height: u32,
    fill: Option<Color>,
    stroke: Option<Color>,
    stroke_width: f32,
) -> Result<RasterizedVector, EngineError> {
    if width == 0 || height == 0 {
        return Err(EngineError::Vector {
            reason: "cannot rasterize to zero dimensions".to_string(),
        });
    }

    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or_else(|| EngineError::Vector {
        reason: format!("failed to create pixmap {width}x{height}"),
    })?;

    // Compute transform to scale path to fit within the target dimensions.
    let bounds = path.bounds();
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 {
        return Err(EngineError::Vector {
            reason: "path has zero-area bounds".to_string(),
        });
    }

    #[allow(clippy::cast_precision_loss)]
    let scale_x = width as f32 / bounds.width();
    #[allow(clippy::cast_precision_loss)]
    let scale_y = height as f32 / bounds.height();
    let scale = scale_x.min(scale_y);

    let transform = tiny_skia::Transform::from_scale(scale, scale)
        .post_translate(-bounds.x() * scale, -bounds.y() * scale);

    // Fill the path.
    if let Some(fill_color) = fill {
        let paint = color_to_paint(fill_color);
        pixmap.fill_path(
            path,
            &paint,
            tiny_skia::FillRule::Winding,
            transform,
            None,
        );
    }

    // Stroke the path.
    if let Some(stroke_color) = stroke {
        if stroke_width > 0.0 {
            let paint = color_to_paint(stroke_color);
            let stroke_opts = tiny_skia::Stroke {
                width: stroke_width * scale,
                ..tiny_skia::Stroke::default()
            };
            pixmap.stroke_path(path, &paint, &stroke_opts, transform, None);
        }
    }

    Ok(RasterizedVector {
        data: pixmap.data().to_vec(),
        width,
        height,
    })
}

/// Converts a `Color` to a `tiny_skia::Paint`.
fn color_to_paint(color: Color) -> tiny_skia::Paint<'static> {
    let mut paint = tiny_skia::Paint::default();
    paint.set_color(tiny_skia::Color::from_rgba(color.r, color.g, color.b, color.a).unwrap_or(tiny_skia::Color::BLACK));
    paint.anti_alias = true;
    paint
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::parser::parse_path_data;

    #[test]
    fn rasterize_simple_line() {
        let path = parse_path_data("M 0 0 L 10 10").expect("parse");
        let result = rasterize_path(
            &path,
            32,
            32,
            None,
            Some(Color::BLACK),
            2.0,
        )
        .expect("rasterize");

        assert_eq!(result.width, 32);
        assert_eq!(result.height, 32);
        assert_eq!(result.data.len(), 32 * 32 * 4);
        // Should have some non-zero pixels from the stroke.
        assert!(result.data.iter().any(|&b| b != 0));
    }

    #[test]
    fn rasterize_filled_triangle() {
        let path = parse_path_data("M 0 0 L 50 0 L 25 50 Z").expect("parse");
        let result = rasterize_path(
            &path,
            64,
            64,
            Some(Color::new(1.0, 0.0, 0.0, 1.0)),
            None,
            0.0,
        )
        .expect("rasterize");

        assert_eq!(result.width, 64);
        assert_eq!(result.height, 64);
        // Should have non-zero pixels from the fill.
        assert!(result.data.iter().any(|&b| b != 0));
    }

    #[test]
    fn rasterize_zero_dimensions_fails() {
        let path = parse_path_data("M 0 0 L 10 10").expect("parse");
        assert!(rasterize_path(&path, 0, 64, Some(Color::BLACK), None, 0.0).is_err());
        assert!(rasterize_path(&path, 64, 0, Some(Color::BLACK), None, 0.0).is_err());
    }

    #[test]
    fn rasterize_with_fill_and_stroke() {
        let path = parse_path_data("M 0 0 L 50 0 L 50 50 L 0 50 Z").expect("parse");
        let result = rasterize_path(
            &path,
            64,
            64,
            Some(Color::new(0.0, 0.0, 1.0, 1.0)),
            Some(Color::BLACK),
            2.0,
        )
        .expect("rasterize");

        assert_eq!(result.data.len(), 64 * 64 * 4);
        assert!(result.data.iter().any(|&b| b != 0));
    }
}
