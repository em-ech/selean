//! Pure Rust blend mode formulas per W3C Compositing and Blending Level 1.
//!
//! Provides CPU-side reference implementations for all 11 non-native blend modes.
//! These formulas match the WGSL implementations in `blend_lib.wgsl` and serve
//! as the ground truth for testing.

use crate::scene::BlendMode;

/// Applies the blend formula for a single color channel.
///
/// Both `src` and `dst` are expected to be in `[0.0, 1.0]` (straight/un-premultiplied).
/// Returns the blended channel value, clamped to `[0.0, 1.0]`.
#[must_use]
pub fn blend_channel(mode: BlendMode, src: f32, dst: f32) -> f32 {
    let result = match mode {
        BlendMode::Multiply => src * dst,
        BlendMode::Screen => src + dst - src * dst,
        BlendMode::Overlay => overlay_channel(src, dst),
        BlendMode::Darken => src.min(dst),
        BlendMode::Lighten => src.max(dst),
        BlendMode::ColorDodge => color_dodge_channel(src, dst),
        BlendMode::ColorBurn => color_burn_channel(src, dst),
        BlendMode::HardLight => overlay_channel(dst, src),
        BlendMode::SoftLight => soft_light_channel(src, dst),
        BlendMode::Difference => (src - dst).abs(),
        BlendMode::Exclusion => src + dst - 2.0 * src * dst,
        BlendMode::Normal | BlendMode::Add => src,
    };
    result.clamp(0.0, 1.0)
}

/// Applies the blend formula to RGB channels.
///
/// Both `src_rgb` and `dst_rgb` are expected to be straight (un-premultiplied)
/// values in `[0.0, 1.0]`. Returns the blended RGB, clamped per channel.
#[must_use]
pub fn blend_rgb(mode: BlendMode, src_rgb: [f32; 3], dst_rgb: [f32; 3]) -> [f32; 3] {
    [
        blend_channel(mode, src_rgb[0], dst_rgb[0]),
        blend_channel(mode, src_rgb[1], dst_rgb[1]),
        blend_channel(mode, src_rgb[2], dst_rgb[2]),
    ]
}

/// Composites premultiplied RGBA using Porter-Duff source-over with a custom
/// blend formula applied to the un-premultiplied RGB channels.
///
/// Input and output are premultiplied RGBA in `[0.0, 1.0]`.
#[must_use]
pub fn composite_blend(mode: BlendMode, src: [f32; 4], dst: [f32; 4]) -> [f32; 4] {
    let a_src = src[3];
    let a_dst = dst[3];
    let a_out = a_src + a_dst * (1.0 - a_src);

    if a_out < 1e-6 {
        return [0.0, 0.0, 0.0, 0.0];
    }

    // Un-premultiply.
    let src_rgb = if a_src > 1e-6 {
        [src[0] / a_src, src[1] / a_src, src[2] / a_src]
    } else {
        [0.0, 0.0, 0.0]
    };
    let dst_rgb = if a_dst > 1e-6 {
        [dst[0] / a_dst, dst[1] / a_dst, dst[2] / a_dst]
    } else {
        [0.0, 0.0, 0.0]
    };

    let blended = blend_rgb(mode, src_rgb, dst_rgb);

    // Porter-Duff source-over composite with blended RGB.
    let r = (1.0 - a_dst) * src_rgb[0] * a_src
        + (1.0 - a_src) * dst_rgb[0] * a_dst
        + a_src * a_dst * blended[0];
    let g = (1.0 - a_dst) * src_rgb[1] * a_src
        + (1.0 - a_src) * dst_rgb[1] * a_dst
        + a_src * a_dst * blended[1];
    let b = (1.0 - a_dst) * src_rgb[2] * a_src
        + (1.0 - a_src) * dst_rgb[2] * a_dst
        + a_src * a_dst * blended[2];

    [
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
        a_out.clamp(0.0, 1.0),
    ]
}

// --- Per-mode helper functions ---

/// Overlay: `if dst < 0.5 { 2*src*dst } else { 1 - 2*(1-src)*(1-dst) }`.
fn overlay_channel(src: f32, dst: f32) -> f32 {
    if dst < 0.5 {
        2.0 * src * dst
    } else {
        1.0 - 2.0 * (1.0 - src) * (1.0 - dst)
    }
}

/// `ColorDodge`: `if src >= 1.0 { 1.0 } else { min(1.0, dst / (1.0 - src)) }`.
fn color_dodge_channel(src: f32, dst: f32) -> f32 {
    if dst <= 0.0 {
        0.0
    } else if src >= 1.0 {
        1.0
    } else {
        (dst / (1.0 - src)).min(1.0)
    }
}

/// `ColorBurn`: `if src <= 0.0 { 0.0 } else { 1.0 - min(1.0, (1.0 - dst) / src) }`.
fn color_burn_channel(src: f32, dst: f32) -> f32 {
    if dst >= 1.0 {
        1.0
    } else if src <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - dst) / src).min(1.0)
    }
}

/// `SoftLight` per W3C spec:
/// ```text
/// if src <= 0.5:
///     dst - (1 - 2*src) * dst * (1 - dst)
/// else:
///     dst + (2*src - 1) * (d(dst) - dst)
/// where d(x) = if x <= 0.25 { ((16*x - 12)*x + 4)*x } else { sqrt(x) }
/// ```
fn soft_light_channel(src: f32, dst: f32) -> f32 {
    if src <= 0.5 {
        dst - (1.0 - 2.0 * src) * dst * (1.0 - dst)
    } else {
        let d = if dst <= 0.25 {
            ((16.0 * dst - 12.0) * dst + 4.0) * dst
        } else {
            dst.sqrt()
        };
        dst + (2.0 * src - 1.0) * (d - dst)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    const EPS: f32 = 1e-5;

    fn assert_close(actual: f32, expected: f32, msg: &str) {
        assert!(
            (actual - expected).abs() < EPS,
            "{msg}: expected {expected}, got {actual}"
        );
    }

    // --- Multiply ---

    #[test]
    fn multiply_both_zero() {
        assert_close(blend_channel(BlendMode::Multiply, 0.0, 0.0), 0.0, "0*0");
    }

    #[test]
    fn multiply_both_one() {
        assert_close(blend_channel(BlendMode::Multiply, 1.0, 1.0), 1.0, "1*1");
    }

    #[test]
    fn multiply_half() {
        assert_close(
            blend_channel(BlendMode::Multiply, 0.5, 0.5),
            0.25,
            "0.5*0.5",
        );
    }

    #[test]
    fn multiply_one_zero() {
        assert_close(blend_channel(BlendMode::Multiply, 1.0, 0.0), 0.0, "1*0");
    }

    // --- Screen ---

    #[test]
    fn screen_both_zero() {
        assert_close(
            blend_channel(BlendMode::Screen, 0.0, 0.0),
            0.0,
            "screen 0,0",
        );
    }

    #[test]
    fn screen_both_one() {
        assert_close(
            blend_channel(BlendMode::Screen, 1.0, 1.0),
            1.0,
            "screen 1,1",
        );
    }

    #[test]
    fn screen_half() {
        assert_close(
            blend_channel(BlendMode::Screen, 0.5, 0.5),
            0.75,
            "screen 0.5,0.5",
        );
    }

    #[test]
    fn screen_one_zero() {
        assert_close(
            blend_channel(BlendMode::Screen, 1.0, 0.0),
            1.0,
            "screen 1,0",
        );
    }

    // --- Overlay ---

    #[test]
    fn overlay_both_zero() {
        assert_close(
            blend_channel(BlendMode::Overlay, 0.0, 0.0),
            0.0,
            "overlay 0,0",
        );
    }

    #[test]
    fn overlay_both_one() {
        assert_close(
            blend_channel(BlendMode::Overlay, 1.0, 1.0),
            1.0,
            "overlay 1,1",
        );
    }

    #[test]
    fn overlay_half() {
        // dst=0.5 is the boundary, uses the else branch: 1 - 2*(1-0.5)*(1-0.5) = 0.5
        assert_close(
            blend_channel(BlendMode::Overlay, 0.5, 0.5),
            0.5,
            "overlay 0.5,0.5",
        );
    }

    #[test]
    fn overlay_low_dst() {
        // dst=0.25 < 0.5: 2*0.8*0.25 = 0.4
        assert_close(
            blend_channel(BlendMode::Overlay, 0.8, 0.25),
            0.4,
            "overlay 0.8,0.25",
        );
    }

    // --- Darken ---

    #[test]
    fn darken_picks_min() {
        assert_close(
            blend_channel(BlendMode::Darken, 0.3, 0.7),
            0.3,
            "darken min",
        );
    }

    #[test]
    fn darken_equal() {
        assert_close(
            blend_channel(BlendMode::Darken, 0.5, 0.5),
            0.5,
            "darken equal",
        );
    }

    // --- Lighten ---

    #[test]
    fn lighten_picks_max() {
        assert_close(
            blend_channel(BlendMode::Lighten, 0.3, 0.7),
            0.7,
            "lighten max",
        );
    }

    #[test]
    fn lighten_equal() {
        assert_close(
            blend_channel(BlendMode::Lighten, 0.5, 0.5),
            0.5,
            "lighten equal",
        );
    }

    // --- ColorDodge ---

    #[test]
    fn color_dodge_src_one() {
        assert_close(
            blend_channel(BlendMode::ColorDodge, 1.0, 0.5),
            1.0,
            "dodge src=1",
        );
    }

    #[test]
    fn color_dodge_dst_zero() {
        assert_close(
            blend_channel(BlendMode::ColorDodge, 0.5, 0.0),
            0.0,
            "dodge dst=0",
        );
    }

    #[test]
    fn color_dodge_half() {
        // 0.5 / (1 - 0.5) = 1.0, clamped to 1.0
        assert_close(
            blend_channel(BlendMode::ColorDodge, 0.5, 0.5),
            1.0,
            "dodge 0.5,0.5",
        );
    }

    #[test]
    fn color_dodge_low() {
        // 0.2 / (1 - 0.25) = 0.2 / 0.75 = 0.26667
        assert_close(
            blend_channel(BlendMode::ColorDodge, 0.25, 0.2),
            0.2 / 0.75,
            "dodge 0.25,0.2",
        );
    }

    // --- ColorBurn ---

    #[test]
    fn color_burn_src_zero() {
        assert_close(
            blend_channel(BlendMode::ColorBurn, 0.0, 0.5),
            0.0,
            "burn src=0",
        );
    }

    #[test]
    fn color_burn_dst_one() {
        assert_close(
            blend_channel(BlendMode::ColorBurn, 0.5, 1.0),
            1.0,
            "burn dst=1",
        );
    }

    #[test]
    fn color_burn_half() {
        // 1 - (1 - 0.5) / 0.5 = 1 - 1 = 0.0
        assert_close(
            blend_channel(BlendMode::ColorBurn, 0.5, 0.5),
            0.0,
            "burn 0.5,0.5",
        );
    }

    #[test]
    fn color_burn_low() {
        // 1 - (1 - 0.8) / 0.5 = 1 - 0.4 = 0.6
        assert_close(
            blend_channel(BlendMode::ColorBurn, 0.5, 0.8),
            0.6,
            "burn 0.5,0.8",
        );
    }

    // --- HardLight ---

    #[test]
    fn hard_light_both_zero() {
        assert_close(
            blend_channel(BlendMode::HardLight, 0.0, 0.0),
            0.0,
            "hardlight 0,0",
        );
    }

    #[test]
    fn hard_light_both_one() {
        assert_close(
            blend_channel(BlendMode::HardLight, 1.0, 1.0),
            1.0,
            "hardlight 1,1",
        );
    }

    #[test]
    fn hard_light_is_overlay_swapped() {
        // HardLight(src, dst) = Overlay(dst, src)
        let src = 0.3;
        let dst = 0.7;
        let hl = blend_channel(BlendMode::HardLight, src, dst);
        let ov = blend_channel(BlendMode::Overlay, dst, src);
        assert_close(hl, ov, "hardlight == overlay swapped");
    }

    // --- SoftLight ---

    #[test]
    fn soft_light_both_half() {
        // src=0.5 <= 0.5: dst - (1 - 2*0.5)*dst*(1-dst) = 0.5 - 0 = 0.5
        assert_close(
            blend_channel(BlendMode::SoftLight, 0.5, 0.5),
            0.5,
            "softlight 0.5,0.5",
        );
    }

    #[test]
    fn soft_light_src_zero() {
        // src=0: dst - (1 - 0)*dst*(1-dst) = 0.5 - 0.5*0.5 = 0.25
        assert_close(
            blend_channel(BlendMode::SoftLight, 0.0, 0.5),
            0.25,
            "softlight 0,0.5",
        );
    }

    #[test]
    fn soft_light_src_one() {
        // src=1 > 0.5, dst=0.5 > 0.25: d(0.5) = sqrt(0.5) ~ 0.7071
        // result = 0.5 + (2*1 - 1)*(0.7071 - 0.5) = 0.5 + 0.2071 ~ 0.7071
        let result = blend_channel(BlendMode::SoftLight, 1.0, 0.5);
        assert_close(result, 0.5_f32.sqrt(), "softlight 1,0.5");
    }

    #[test]
    fn soft_light_low_dst() {
        // src=0.8 > 0.5, dst=0.1 <= 0.25: d(0.1) = ((16*0.1 - 12)*0.1 + 4)*0.1
        // = ((1.6 - 12)*0.1 + 4)*0.1 = (-1.04 + 4)*0.1 = 0.296
        // result = 0.1 + (2*0.8 - 1)*(0.296 - 0.1) = 0.1 + 0.6*0.196 = 0.2176
        let d = ((16.0 * 0.1 - 12.0) * 0.1 + 4.0) * 0.1;
        let expected = 0.1 + (2.0 * 0.8 - 1.0) * (d - 0.1);
        assert_close(
            blend_channel(BlendMode::SoftLight, 0.8, 0.1),
            expected,
            "softlight low dst",
        );
    }

    // --- Difference ---

    #[test]
    fn difference_equal() {
        assert_close(
            blend_channel(BlendMode::Difference, 0.5, 0.5),
            0.0,
            "diff equal",
        );
    }

    #[test]
    fn difference_basic() {
        assert_close(
            blend_channel(BlendMode::Difference, 0.8, 0.3),
            0.5,
            "diff 0.8-0.3",
        );
    }

    #[test]
    fn difference_reversed() {
        assert_close(
            blend_channel(BlendMode::Difference, 0.3, 0.8),
            0.5,
            "diff 0.3-0.8",
        );
    }

    // --- Exclusion ---

    #[test]
    fn exclusion_both_zero() {
        assert_close(
            blend_channel(BlendMode::Exclusion, 0.0, 0.0),
            0.0,
            "excl 0,0",
        );
    }

    #[test]
    fn exclusion_both_one() {
        // 1 + 1 - 2*1*1 = 0
        assert_close(
            blend_channel(BlendMode::Exclusion, 1.0, 1.0),
            0.0,
            "excl 1,1",
        );
    }

    #[test]
    fn exclusion_half() {
        // 0.5 + 0.5 - 2*0.5*0.5 = 0.5
        assert_close(
            blend_channel(BlendMode::Exclusion, 0.5, 0.5),
            0.5,
            "excl 0.5,0.5",
        );
    }

    // --- blend_rgb ---

    #[test]
    fn blend_rgb_multiply() {
        let result = blend_rgb(BlendMode::Multiply, [0.5, 0.25, 1.0], [0.5, 1.0, 0.0]);
        assert_close(result[0], 0.25, "rgb multiply r");
        assert_close(result[1], 0.25, "rgb multiply g");
        assert_close(result[2], 0.0, "rgb multiply b");
    }

    #[test]
    fn blend_rgb_screen() {
        let result = blend_rgb(BlendMode::Screen, [0.5, 0.0, 1.0], [0.5, 1.0, 0.0]);
        assert_close(result[0], 0.75, "rgb screen r");
        assert_close(result[1], 1.0, "rgb screen g");
        assert_close(result[2], 1.0, "rgb screen b");
    }

    // --- composite_blend ---

    #[test]
    fn composite_opaque_multiply() {
        // Both fully opaque: a_out=1, un-premul is identity.
        // Porter-Duff: 0*src + 0*dst + 1*1*blend = blend
        let src = [0.5, 0.5, 0.5, 1.0];
        let dst = [0.5, 0.5, 0.5, 1.0];
        let result = composite_blend(BlendMode::Multiply, src, dst);
        assert_close(result[0], 0.25, "composite multiply r");
        assert_close(result[3], 1.0, "composite multiply a");
    }

    #[test]
    fn composite_transparent_src() {
        let src = [0.0, 0.0, 0.0, 0.0];
        let dst = [0.5, 0.5, 0.5, 1.0];
        let result = composite_blend(BlendMode::Multiply, src, dst);
        // Fully transparent src: result should be dst.
        assert_close(result[0], 0.5, "transparent src r");
        assert_close(result[3], 1.0, "transparent src a");
    }

    #[test]
    fn composite_transparent_dst() {
        let src = [0.5, 0.5, 0.5, 1.0];
        let dst = [0.0, 0.0, 0.0, 0.0];
        let result = composite_blend(BlendMode::Multiply, src, dst);
        // Fully transparent dst: result should be src.
        assert_close(result[0], 0.5, "transparent dst r");
        assert_close(result[3], 1.0, "transparent dst a");
    }

    #[test]
    fn composite_both_transparent() {
        let src = [0.0, 0.0, 0.0, 0.0];
        let dst = [0.0, 0.0, 0.0, 0.0];
        let result = composite_blend(BlendMode::Screen, src, dst);
        assert_close(result[3], 0.0, "both transparent a");
    }

    // --- shader_index ---

    #[test]
    fn shader_index_native_modes() {
        assert_eq!(BlendMode::Normal.shader_index(), u32::MAX);
        assert_eq!(BlendMode::Add.shader_index(), u32::MAX);
    }

    #[test]
    fn shader_index_non_native_contiguous() {
        assert_eq!(BlendMode::Multiply.shader_index(), 0);
        assert_eq!(BlendMode::Screen.shader_index(), 1);
        assert_eq!(BlendMode::Overlay.shader_index(), 2);
        assert_eq!(BlendMode::Darken.shader_index(), 3);
        assert_eq!(BlendMode::Lighten.shader_index(), 4);
        assert_eq!(BlendMode::ColorDodge.shader_index(), 5);
        assert_eq!(BlendMode::ColorBurn.shader_index(), 6);
        assert_eq!(BlendMode::HardLight.shader_index(), 7);
        assert_eq!(BlendMode::SoftLight.shader_index(), 8);
        assert_eq!(BlendMode::Difference.shader_index(), 9);
        assert_eq!(BlendMode::Exclusion.shader_index(), 10);
    }
}
