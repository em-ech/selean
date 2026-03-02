//! Coordinate and color conversion between IDML and engine types.
//!
//! IDML uses points (72 per inch). At 96 DPI, 1 pt = 1.333 px.
//! IDML positions are relative to the spread center; Selean uses top-left origin.

use selean_engine::scene::Color;

/// Points per pixel at 96 DPI.
const PT_PER_PX: f32 = 72.0 / 96.0;

/// Pixels per point at 96 DPI.
const PX_PER_PT: f32 = 96.0 / 72.0;

/// Converts IDML points to logical pixels.
#[must_use]
pub fn pt_to_px(pt: f32) -> f32 {
    pt * PX_PER_PT
}

/// Converts logical pixels to IDML points.
#[must_use]
pub fn px_to_pt(px: f32) -> f32 {
    px * PT_PER_PX
}

/// Converts IDML spread-center coordinates to top-left origin.
///
/// IDML places (0,0) at the center of the spread. `half_w` and `half_h`
/// are half the page dimensions in points.
#[must_use]
pub fn spread_to_topleft(x: f32, y: f32, half_w: f32, half_h: f32) -> (f32, f32) {
    (x + half_w, y + half_h)
}

/// Converts top-left origin coordinates to IDML spread-center.
#[must_use]
pub fn topleft_to_spread(x: f32, y: f32, half_w: f32, half_h: f32) -> (f32, f32) {
    (x - half_w, y - half_h)
}

/// Parses an IDML color from 0.0..1.0 float component strings.
///
/// Returns `None` if any component fails to parse.
#[must_use]
pub fn parse_idml_color(r: &str, g: &str, b: &str) -> Option<Color> {
    let r_val: f32 = r.parse().ok()?;
    let g_val: f32 = g.parse().ok()?;
    let b_val: f32 = b.parse().ok()?;
    Some(Color::new(r_val, g_val, b_val, 1.0))
}

/// Converts a Color to IDML float RGB strings.
#[must_use]
pub fn color_to_idml_rgb(color: &Color) -> (String, String, String) {
    (
        format!("{:.6}", color.r),
        format!("{:.6}", color.g),
        format!("{:.6}", color.b),
    )
}

/// Parses an IDML font size (points string) to pixels.
///
/// Returns the default 16.0 px if parsing fails.
#[must_use]
pub fn parse_idml_font_size(pt_str: &str) -> f32 {
    pt_str.parse::<f32>().map_or(16.0, pt_to_px)
}

/// Converts pixels to IDML font size in points.
#[must_use]
pub fn px_to_idml_font_size(px: f32) -> f32 {
    px_to_pt(px)
}

/// Parses an IDML `ItemTransform` attribute string into a 6-element array.
///
/// The format is `"a b c d tx ty"` representing a 2D affine matrix.
/// Returns identity `[1, 0, 0, 1, 0, 0]` on parse failure.
#[must_use]
pub fn parse_item_transform(attr: &str) -> [f32; 6] {
    let parts: Vec<f32> = attr
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect();
    if parts.len() == 6 {
        [parts[0], parts[1], parts[2], parts[3], parts[4], parts[5]]
    } else {
        [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]
    }
}

/// Builds an IDML `ItemTransform` attribute string from position.
///
/// Uses identity rotation/scale with the given translation.
#[must_use]
pub fn build_item_transform(tx: f32, ty: f32) -> String {
    format!("1 0 0 1 {tx} {ty}")
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn pt_to_px_basic() {
        let px = pt_to_px(72.0);
        assert!((px - 96.0).abs() < 0.01);
    }

    #[test]
    fn px_to_pt_basic() {
        let pt = px_to_pt(96.0);
        assert!((pt - 72.0).abs() < 0.01);
    }

    #[test]
    fn pt_px_roundtrip() {
        let original = 36.0_f32;
        let px = pt_to_px(original);
        let back = px_to_pt(px);
        assert!((original - back).abs() < 0.001);
    }

    #[test]
    fn pt_to_px_zero() {
        assert!((pt_to_px(0.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn pt_to_px_negative() {
        let px = pt_to_px(-72.0);
        assert!((px - (-96.0)).abs() < 0.01);
    }

    #[test]
    fn spread_to_topleft_center_is_half() {
        let (x, y) = spread_to_topleft(0.0, 0.0, 306.0, 396.0);
        assert!((x - 306.0).abs() < 0.001);
        assert!((y - 396.0).abs() < 0.001);
    }

    #[test]
    fn topleft_to_spread_roundtrip() {
        let (sx, sy) = topleft_to_spread(100.0, 200.0, 306.0, 396.0);
        let (tx, ty) = spread_to_topleft(sx, sy, 306.0, 396.0);
        assert!((tx - 100.0).abs() < 0.001);
        assert!((ty - 200.0).abs() < 0.001);
    }

    #[test]
    fn parse_color_valid() {
        let c = parse_idml_color("1.0", "0.5", "0.0").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 0.5).abs() < 0.01);
        assert!(c.b.abs() < 0.01);
    }

    #[test]
    fn parse_color_invalid() {
        assert!(parse_idml_color("abc", "0.5", "0.0").is_none());
    }

    #[test]
    fn color_to_idml_roundtrip() {
        let color = Color::new(0.8, 0.2, 0.5, 1.0);
        let (r, g, b) = color_to_idml_rgb(&color);
        let back = parse_idml_color(&r, &g, &b).unwrap();
        assert!((color.r - back.r).abs() < 0.001);
        assert!((color.g - back.g).abs() < 0.001);
        assert!((color.b - back.b).abs() < 0.001);
    }

    #[test]
    fn parse_font_size_valid() {
        let px = parse_idml_font_size("12");
        assert!((px - 16.0).abs() < 0.01);
    }

    #[test]
    fn parse_font_size_invalid_returns_default() {
        let px = parse_idml_font_size("abc");
        assert!((px - 16.0).abs() < f32::EPSILON);
    }

    #[test]
    fn px_to_idml_font_size_roundtrip() {
        let px = 32.0;
        let pt = px_to_idml_font_size(px);
        let back = parse_idml_font_size(&format!("{pt}"));
        assert!((px - back).abs() < 0.01);
    }

    #[test]
    fn parse_item_transform_valid() {
        let t = parse_item_transform("1 0 0 1 100.5 -200.3");
        assert!((t[0] - 1.0).abs() < 0.001);
        assert!((t[4] - 100.5).abs() < 0.001);
        assert!((t[5] - (-200.3)).abs() < 0.001);
    }

    #[test]
    fn parse_item_transform_invalid_returns_identity() {
        let t = parse_item_transform("bad data");
        assert!((t[0] - 1.0).abs() < f32::EPSILON);
        assert!((t[3] - 1.0).abs() < f32::EPSILON);
        assert!(t[4].abs() < f32::EPSILON);
    }

    #[test]
    fn build_item_transform_format() {
        let s = build_item_transform(10.5, -20.3);
        assert_eq!(s, "1 0 0 1 10.5 -20.3");
    }
}
