//! Coordinate and color conversion between OOXML and engine types.
//!
//! OOXML uses English Metric Units (EMU) where 1 inch = 914400 EMU.
//! At 96 DPI, 1 pixel = 9525 EMU.

use selean_engine::scene::Color;

/// EMU per pixel at 96 DPI.
pub const EMU_PER_PIXEL: f64 = 9525.0;

/// Converts EMU to logical pixels.
#[must_use]
#[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
pub fn emu_to_px(emu: i64) -> f32 {
    (emu as f64 / EMU_PER_PIXEL) as f32
}

/// Converts logical pixels to EMU.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn px_to_emu(px: f32) -> i64 {
    (f64::from(px) * EMU_PER_PIXEL) as i64
}

/// Parses an OOXML 6-digit hex color string (e.g. "FF0000") to a Color.
///
/// Returns None if the string is not a valid 6-character hex color.
#[must_use]
pub fn parse_ooxml_color(hex: &str) -> Option<Color> {
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        1.0,
    ))
}

/// Converts a Color to OOXML 6-digit hex string (e.g. "FF0000").
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn color_to_ooxml_hex(color: &Color) -> String {
    let r = (color.r * 255.0).round() as u8;
    let g = (color.g * 255.0).round() as u8;
    let b = (color.b * 255.0).round() as u8;
    format!("{r:02X}{g:02X}{b:02X}")
}

/// Converts OOXML font size (hundredths of a point) to pixels.
///
/// OOXML `sz` attribute is in hundredths of a point. At 96 DPI, 1pt = 1.333px.
#[must_use]
#[allow(clippy::cast_precision_loss)]
pub fn ooxml_font_size_to_px(hundredths: i32) -> f32 {
    let points = hundredths as f32 / 100.0;
    points * 96.0 / 72.0
}

/// Converts pixels to OOXML font size (hundredths of a point).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn px_to_ooxml_font_size(px: f32) -> i32 {
    let points = px * 72.0 / 96.0;
    (points * 100.0).round() as i32
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn emu_to_px_basic() {
        // 1 inch = 914400 EMU = 96 px at 96 DPI
        let px = emu_to_px(914_400);
        assert!((px - 96.0).abs() < 0.01);
    }

    #[test]
    fn px_to_emu_basic() {
        let emu = px_to_emu(96.0);
        assert_eq!(emu, 914_400);
    }

    #[test]
    fn emu_px_roundtrip() {
        let original = 500_000_i64;
        let px = emu_to_px(original);
        let back = px_to_emu(px);
        assert!((original - back).abs() <= 1);
    }

    #[test]
    fn emu_zero() {
        assert!((emu_to_px(0) - 0.0).abs() < f32::EPSILON);
        assert_eq!(px_to_emu(0.0), 0);
    }

    #[test]
    fn emu_negative() {
        let px = emu_to_px(-914_400);
        assert!((px - (-96.0)).abs() < 0.01);
    }

    #[test]
    fn parse_color_red() {
        let c = parse_ooxml_color("FF0000").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!(c.g.abs() < 0.01);
        assert!(c.b.abs() < 0.01);
        assert!((c.a - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_white() {
        let c = parse_ooxml_color("FFFFFF").unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
        assert!((c.g - 1.0).abs() < 0.01);
        assert!((c.b - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_color_black() {
        let c = parse_ooxml_color("000000").unwrap();
        assert!(c.r.abs() < 0.01);
        assert!(c.g.abs() < 0.01);
        assert!(c.b.abs() < 0.01);
    }

    #[test]
    fn parse_color_invalid_length() {
        assert!(parse_ooxml_color("FF00").is_none());
        assert!(parse_ooxml_color("FF0000FF").is_none());
    }

    #[test]
    fn parse_color_invalid_hex() {
        assert!(parse_ooxml_color("GGHHII").is_none());
    }

    #[test]
    fn parse_color_empty() {
        assert!(parse_ooxml_color("").is_none());
    }

    #[test]
    fn color_to_hex_roundtrip() {
        let color = Color::new(1.0, 0.0, 0.5, 1.0);
        let hex = color_to_ooxml_hex(&color);
        let back = parse_ooxml_color(&hex).unwrap();
        assert!((color.r - back.r).abs() < 0.01);
        assert!((color.g - back.g).abs() < 0.01);
        assert!((color.b - back.b).abs() < 0.01);
    }

    #[test]
    fn color_to_hex_black() {
        let hex = color_to_ooxml_hex(&Color::new(0.0, 0.0, 0.0, 1.0));
        assert_eq!(hex, "000000");
    }

    #[test]
    fn color_to_hex_white() {
        let hex = color_to_ooxml_hex(&Color::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(hex, "FFFFFF");
    }

    #[test]
    fn font_size_conversion() {
        // 1200 hundredths = 12pt = 16px at 96 DPI
        let px = ooxml_font_size_to_px(1200);
        assert!((px - 16.0).abs() < 0.01);
    }

    #[test]
    fn font_size_roundtrip() {
        let original = 2400;
        let px = ooxml_font_size_to_px(original);
        let back = px_to_ooxml_font_size(px);
        assert_eq!(back, original);
    }

    #[test]
    fn font_size_small() {
        // 800 hundredths = 8pt
        let px = ooxml_font_size_to_px(800);
        let expected = 8.0 * 96.0 / 72.0; // ~10.67px
        assert!((px - expected).abs() < 0.01);
    }
}
