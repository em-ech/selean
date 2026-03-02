//! Color and text alignment conversion from Figma types to engine types.
//!
//! Figma uses 0.0-1.0 RGBA (same as the engine), so color conversion is
//! direct. Paint opacity is multiplied into the alpha channel.

use selean_engine::scene::{Color, TextAlign};

use crate::api::{FigmaColor, FigmaPaint};

/// Converts a Figma RGBA color to an engine `Color`.
///
/// The paint-level `opacity` is multiplied into the alpha channel.
#[must_use]
pub fn figma_color_to_engine(color: &FigmaColor, opacity: f32) -> Color {
    Color::new(color.r, color.g, color.b, color.a * opacity)
}

/// Extracts the first visible SOLID fill color from a list of paints.
///
/// Returns `None` if no visible solid fill exists. Gradients and image
/// fills are skipped with a trace-level log.
#[must_use]
pub fn first_solid_color(paints: &[FigmaPaint]) -> Option<Color> {
    for paint in paints {
        if !paint.visible {
            continue;
        }
        if paint.paint_type == "SOLID" {
            if let Some(ref color) = paint.color {
                return Some(figma_color_to_engine(color, paint.opacity));
            }
        } else {
            tracing::trace!(paint_type = %paint.paint_type, "skipping non-solid paint");
        }
    }
    None
}

/// Finds the first IMAGE paint's `imageRef` from a list of paints.
#[must_use]
pub fn first_image_ref(paints: &[FigmaPaint]) -> Option<String> {
    for paint in paints {
        if !paint.visible {
            continue;
        }
        if paint.paint_type == "IMAGE" {
            if let Some(ref img_ref) = paint.image_ref {
                return Some(img_ref.clone());
            }
        }
    }
    None
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
    fn first_solid_color_picks_first() {
        let paints = vec![
            FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                opacity: 1.0,
                visible: true,
                image_ref: None,
            },
            FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    r: 0.0,
                    g: 1.0,
                    b: 0.0,
                    a: 1.0,
                }),
                opacity: 1.0,
                visible: true,
                image_ref: None,
            },
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
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                }),
                opacity: 1.0,
                visible: false,
                image_ref: None,
            },
            FigmaPaint {
                paint_type: "SOLID".to_string(),
                color: Some(FigmaColor {
                    r: 0.0,
                    g: 0.0,
                    b: 1.0,
                    a: 1.0,
                }),
                opacity: 1.0,
                visible: true,
                image_ref: None,
            },
        ];
        let c = first_solid_color(&paints).unwrap();
        assert_eq!(c.b, 1.0);
        assert_eq!(c.r, 0.0);
    }

    #[test]
    fn first_solid_color_skips_gradient() {
        let paints = vec![FigmaPaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            color: None,
            opacity: 1.0,
            visible: true,
            image_ref: None,
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
            color: None,
            opacity: 1.0,
            visible: true,
            image_ref: Some("hash123".to_string()),
        }];
        assert_eq!(first_image_ref(&paints).as_deref(), Some("hash123"));
    }

    #[test]
    fn first_image_ref_skips_invisible() {
        let paints = vec![FigmaPaint {
            paint_type: "IMAGE".to_string(),
            color: None,
            opacity: 1.0,
            visible: false,
            image_ref: Some("hash123".to_string()),
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
