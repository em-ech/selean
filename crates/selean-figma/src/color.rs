//! Color and text alignment conversion from Figma types to engine types.
//!
//! Figma uses 0.0-1.0 RGBA (same as the engine), so color conversion is
//! direct. Paint opacity is multiplied into the alpha channel.

use selean_engine::scene::{Color, Gradient, GradientStop, TextAlign};

use crate::api::{FigmaColor, FigmaGradientStop, FigmaPaint, FigmaVector};

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

/// Maximum gradient stops supported by the engine shader.
const MAX_GRADIENT_STOPS: usize = 4;

/// Extracts the first visible gradient fill from a list of paints.
///
/// Supports `GRADIENT_LINEAR` and `GRADIENT_RADIAL`. Stops are capped at 4.
/// Returns `None` if no visible gradient fill exists.
#[must_use]
pub fn first_gradient(paints: &[FigmaPaint]) -> Option<Gradient> {
    let paint = find_visible_paint(paints, "GRADIENT_LINEAR")
        .or_else(|| find_visible_paint(paints, "GRADIENT_RADIAL"))?;

    let stops = convert_gradient_stops(&paint.gradient_stops, paint.opacity);
    if stops.is_empty() {
        return None;
    }

    match paint.paint_type.as_str() {
        "GRADIENT_LINEAR" => {
            let (start, end) = extract_linear_endpoints(&paint.gradient_handle_positions);
            Some(Gradient::Linear { start, end, stops })
        }
        "GRADIENT_RADIAL" => {
            let (center, radius) = extract_radial_params(&paint.gradient_handle_positions);
            Some(Gradient::Radial {
                center,
                radius,
                stops,
            })
        }
        _ => None,
    }
}

/// Converts Figma gradient stops to engine gradient stops, capped at 4.
fn convert_gradient_stops(figma_stops: &[FigmaGradientStop], opacity: f32) -> Vec<GradientStop> {
    figma_stops
        .iter()
        .take(MAX_GRADIENT_STOPS)
        .map(|s| GradientStop {
            position: s.position,
            color: figma_color_to_engine(&s.color, opacity),
        })
        .collect()
}

/// Extracts linear gradient start/end from Figma handle positions.
///
/// Figma provides 3 handle positions for linear gradients: [start, end, width].
/// We use the first two as the gradient axis.
fn extract_linear_endpoints(positions: &[FigmaVector]) -> ([f32; 2], [f32; 2]) {
    if positions.len() >= 2 {
        (
            [positions[0].x, positions[0].y],
            [positions[1].x, positions[1].y],
        )
    } else {
        ([0.0, 0.0], [1.0, 0.0])
    }
}

/// Extracts radial gradient center and radius from Figma handle positions.
///
/// Center is positions[0], radius is the distance from center to positions[1].
fn extract_radial_params(positions: &[FigmaVector]) -> ([f32; 2], f32) {
    if positions.len() >= 2 {
        let center = [positions[0].x, positions[0].y];
        let dx = positions[1].x - positions[0].x;
        let dy = positions[1].y - positions[0].y;
        let radius = (dx * dx + dy * dy).sqrt().max(0.001);
        (center, radius)
    } else {
        ([0.5, 0.5], 0.5)
    }
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
    use crate::api::{FigmaColor, FigmaGradientStop, FigmaVector};

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

    fn linear_gradient_paint() -> FigmaPaint {
        FigmaPaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            gradient_handle_positions: vec![
                FigmaVector { x: 0.0, y: 0.5 },
                FigmaVector { x: 1.0, y: 0.5 },
                FigmaVector { x: 0.0, y: 1.0 },
            ],
            gradient_stops: vec![
                FigmaGradientStop {
                    position: 0.0,
                    color: FigmaColor {
                        r: 1.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                },
                FigmaGradientStop {
                    position: 1.0,
                    color: FigmaColor {
                        r: 0.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    },
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn first_gradient_linear() {
        let paints = vec![linear_gradient_paint()];
        let grad = first_gradient(&paints).unwrap();
        match grad {
            Gradient::Linear { start, end, stops } => {
                assert_eq!(start, [0.0, 0.5]);
                assert_eq!(end, [1.0, 0.5]);
                assert_eq!(stops.len(), 2);
                assert_eq!(stops[0].position, 0.0);
                assert_eq!(stops[0].color.r, 1.0);
                assert_eq!(stops[1].position, 1.0);
                assert_eq!(stops[1].color.b, 1.0);
            }
            Gradient::Radial { .. } => panic!("expected Linear gradient"),
        }
    }

    #[test]
    fn first_gradient_radial() {
        let paints = vec![FigmaPaint {
            paint_type: "GRADIENT_RADIAL".to_string(),
            gradient_handle_positions: vec![
                FigmaVector { x: 0.5, y: 0.5 },
                FigmaVector { x: 1.0, y: 0.5 },
            ],
            gradient_stops: vec![
                FigmaGradientStop {
                    position: 0.0,
                    color: FigmaColor {
                        r: 1.0,
                        ..Default::default()
                    },
                },
                FigmaGradientStop {
                    position: 1.0,
                    color: FigmaColor {
                        b: 1.0,
                        ..Default::default()
                    },
                },
            ],
            ..Default::default()
        }];
        let grad = first_gradient(&paints).unwrap();
        match grad {
            Gradient::Radial {
                center,
                radius,
                stops,
            } => {
                assert_eq!(center, [0.5, 0.5]);
                assert!((radius - 0.5).abs() < 1e-6);
                assert_eq!(stops.len(), 2);
            }
            Gradient::Linear { .. } => panic!("expected Radial gradient"),
        }
    }

    #[test]
    fn first_gradient_skips_invisible() {
        let paints = vec![FigmaPaint {
            visible: false,
            ..linear_gradient_paint()
        }];
        assert!(first_gradient(&paints).is_none());
    }

    #[test]
    fn first_gradient_none_for_solid() {
        let paints = vec![solid_paint(1.0, 0.0, 0.0, 1.0)];
        assert!(first_gradient(&paints).is_none());
    }

    #[test]
    fn first_gradient_empty_stops_returns_none() {
        let paints = vec![FigmaPaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            gradient_handle_positions: vec![
                FigmaVector { x: 0.0, y: 0.0 },
                FigmaVector { x: 1.0, y: 1.0 },
            ],
            gradient_stops: vec![],
            ..Default::default()
        }];
        assert!(first_gradient(&paints).is_none());
    }

    #[test]
    fn first_gradient_caps_at_four_stops() {
        let stops: Vec<FigmaGradientStop> = (0..6)
            .map(|i: u8| FigmaGradientStop {
                position: f32::from(i) / 5.0,
                color: FigmaColor::default(),
            })
            .collect();
        let paints = vec![FigmaPaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            gradient_handle_positions: vec![
                FigmaVector { x: 0.0, y: 0.0 },
                FigmaVector { x: 1.0, y: 0.0 },
            ],
            gradient_stops: stops,
            ..Default::default()
        }];
        let grad = first_gradient(&paints).unwrap();
        match grad {
            Gradient::Linear { stops, .. } => assert_eq!(stops.len(), 4),
            Gradient::Radial { .. } => panic!("expected Linear"),
        }
    }

    #[test]
    fn first_gradient_opacity_applied_to_stops() {
        let paints = vec![FigmaPaint {
            opacity: 0.5,
            ..linear_gradient_paint()
        }];
        let grad = first_gradient(&paints).unwrap();
        match grad {
            Gradient::Linear { stops, .. } => {
                assert_eq!(stops[0].color.a, 0.5);
            }
            Gradient::Radial { .. } => panic!("expected Linear"),
        }
    }

    #[test]
    fn first_gradient_missing_handle_positions_uses_defaults() {
        let paints = vec![FigmaPaint {
            paint_type: "GRADIENT_LINEAR".to_string(),
            gradient_handle_positions: vec![],
            gradient_stops: vec![FigmaGradientStop {
                position: 0.0,
                color: FigmaColor::default(),
            }],
            ..Default::default()
        }];
        let grad = first_gradient(&paints).unwrap();
        match grad {
            Gradient::Linear { start, end, .. } => {
                assert_eq!(start, [0.0, 0.0]);
                assert_eq!(end, [1.0, 0.0]);
            }
            Gradient::Radial { .. } => panic!("expected Linear"),
        }
    }
}
