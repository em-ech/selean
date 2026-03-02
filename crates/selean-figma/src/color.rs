//! Color and text alignment conversion from Figma types to engine types.
//!
//! Figma uses 0.0-1.0 RGBA (same as the engine), so color conversion is
//! direct. Paint opacity is multiplied into the alpha channel.

use selean_engine::scene::{Color, TextAlign};

use crate::api::{FigmaColor, FigmaPaint};

/// Finds the first visible paint of the given type.
///
/// Iterates paints in order, skipping invisible ones, and returns the
/// first match for `paint_type`.
#[must_use]
pub fn find_visible_paint<'a>(
    paints: &'a [FigmaPaint],
    paint_type: &str,
) -> Option<&'a FigmaPaint> {
    paints
        .iter()
        .find(|p| p.visible && p.paint_type == paint_type)
}

/// Converts a Figma RGBA color to an engine `Color`.
///
/// The paint-level `opacity` is multiplied into the alpha channel.
#[must_use]
pub fn figma_color_to_engine(color: &FigmaColor, opacity: f32) -> Color {
    Color::new(color.r, color.g, color.b, color.a * opacity)
}

/// Extracts the first visible SOLID fill color from a list of paints.
///
/// Returns `None` if no visible solid fill exists.
#[must_use]
pub fn first_solid_color(paints: &[FigmaPaint]) -> Option<Color> {
    let paint = find_visible_paint(paints, "SOLID")?;
    let color = paint.color.as_ref()?;
    Some(figma_color_to_engine(color, paint.opacity))
}

/// Finds the first visible IMAGE paint's `imageRef` from a list of paints.
#[must_use]
pub fn first_image_ref(paints: &[FigmaPaint]) -> Option<String> {
    let paint = find_visible_paint(paints, "IMAGE")?;
    paint.image_ref.clone()
}

/// Maps a Figma text alignment string to an engine `TextAlign`.
#[must_use]
pub fn figma_text_align(align: &str) -> TextAlign {
    match align {
        "CENTER" => TextAlign::Center,
        "RIGHT" => TextAlign::Right,
        "JUSTIFIED" => TextAlign::Justify,
        _ => TextAlign::Left,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::api::FigmaColor;

    fn solid_paint(r: f32, g: f32, b: f32, a: f32) -> FigmaPaint {
        FigmaPaint {
            paint_type: "SOLID".to_string(),
            color: Some(FigmaColor { r, g, b, a }),
            ..Default::default()
        }
    }

    #[test]
    fn color_conversion_direct() {
        let fc = FigmaColor {
            r: 0.5,
            g: 0.25,
            b: 0.75,
            a: 1.0,
        };
        let c = figma_color_to_engine(&fc, 1.0);
        assert_eq!(c.r, 0.5);
        assert_eq!(c.g, 0.25);
        assert_eq!(c.b, 0.75);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn color_conversion_with_paint_opacity() {
        let fc = FigmaColor {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let c = figma_color_to_engine(&fc, 0.5);
        assert_eq!(c.a, 0.5);
    }

    #[test]
    fn color_conversion_combined_opacity() {
        let fc = FigmaColor {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 0.8,
        };
        let c = figma_color_to_engine(&fc, 0.5);
        assert!((c.a - 0.4).abs() < 1e-6);
    }

    #[test]
    fn find_visible_paint_returns_first_match() {
        let paints = vec![
            FigmaPaint {
                paint_type: "GRADIENT_LINEAR".to_string(),
                ..Default::default()
            },
            solid_paint(1.0, 0.0, 0.0, 1.0),
            solid_paint(0.0, 1.0, 0.0, 1.0),
        ];
        let found = find_visible_paint(&paints, "SOLID").unwrap();
        assert_eq!(found.color.unwrap().r, 1.0);
    }

    #[test]
    fn find_visible_paint_skips_invisible() {
        let paints = vec![FigmaPaint {
            paint_type: "SOLID".to_string(),
            visible: false,
            ..Default::default()
        }];
        assert!(find_visible_paint(&paints, "SOLID").is_none());
    }

    #[test]
    fn find_visible_paint_no_match() {
        let paints = vec![solid_paint(1.0, 0.0, 0.0, 1.0)];
        assert!(find_visible_paint(&paints, "IMAGE").is_none());
    }

    #[test]
    fn first_solid_color_picks_first() {
        let paints = vec![
            solid_paint(1.0, 0.0, 0.0, 1.0),
            solid_paint(0.0, 1.0, 0.0, 1.0),
        ];
        let c = first_solid_color(&paints).unwrap();
        assert_eq!(c.r, 1.0);
        assert_eq!(c.g, 0.0);
    }

    #[test]
    fn first_solid_color_skips_invisible() {
        let paints = vec![
            FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    r: 1.0,
                    ..Default::default()
                }),
                visible: false,
                ..Default::default()
            },
            solid_paint(0.0, 0.0, 1.0, 1.0),
        ];
        let c = first_solid_color(&paints).unwrap();
        assert_eq!(c.b, 1.0);
        assert_eq!(c.r, 0.0);
    }

    #[test]
    fn first_solid_color_skips_gradient() {
        let paints = vec![FigmaPaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            ..Default::default()
        }];
        assert!(first_solid_color(&paints).is_none());
    }

    #[test]
    fn first_solid_color_empty_list() {
        assert!(first_solid_color(&[]).is_none());
    }

    #[test]
    fn first_image_ref_found() {
        let paints = vec![FigmaPaint {
            paint_type: "IMAGE".to_string(),
            image_ref: Some("hash123".to_string()),
            ..Default::default()
        }];
        assert_eq!(first_image_ref(&paints).as_deref(), Some("hash123"));
    }

    #[test]
    fn first_image_ref_skips_invisible() {
        let paints = vec![FigmaPaint {
            paint_type: "IMAGE".to_string(),
            visible: false,
            image_ref: Some("hash123".to_string()),
            ..Default::default()
        }];
        assert!(first_image_ref(&paints).is_none());
    }

    #[test]
    fn text_align_center() {
        assert_eq!(figma_text_align("CENTER"), TextAlign::Center);
    }

    #[test]
    fn text_align_right() {
        assert_eq!(figma_text_align("RIGHT"), TextAlign::Right);
    }

    #[test]
    fn text_align_justified() {
        assert_eq!(figma_text_align("JUSTIFIED"), TextAlign::Justify);
    }

    #[test]
    fn text_align_left_default() {
        assert_eq!(figma_text_align("LEFT"), TextAlign::Left);
        assert_eq!(figma_text_align("UNKNOWN"), TextAlign::Left);
    }
}
