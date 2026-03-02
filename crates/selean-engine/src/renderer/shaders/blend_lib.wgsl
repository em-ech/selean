// Blend mode function library per W3C Compositing and Blending Level 1.
//
// Each function takes un-premultiplied src and dst RGB and returns the blended result.
// These are called by the dispatch function `apply_blend()`.

fn blend_multiply(src: vec3f, dst: vec3f) -> vec3f {
    return src * dst;
}

fn blend_screen(src: vec3f, dst: vec3f) -> vec3f {
    return src + dst - src * dst;
}

fn overlay_channel(src: f32, dst: f32) -> f32 {
    if dst < 0.5 {
        return 2.0 * src * dst;
    } else {
        return 1.0 - 2.0 * (1.0 - src) * (1.0 - dst);
    }
}

fn blend_overlay(src: vec3f, dst: vec3f) -> vec3f {
    return vec3f(
        overlay_channel(src.x, dst.x),
        overlay_channel(src.y, dst.y),
        overlay_channel(src.z, dst.z),
    );
}

fn blend_darken(src: vec3f, dst: vec3f) -> vec3f {
    return min(src, dst);
}

fn blend_lighten(src: vec3f, dst: vec3f) -> vec3f {
    return max(src, dst);
}

fn color_dodge_channel(src: f32, dst: f32) -> f32 {
    if dst <= 0.0 {
        return 0.0;
    } else if src >= 1.0 {
        return 1.0;
    } else {
        return min(1.0, dst / (1.0 - src));
    }
}

fn blend_color_dodge(src: vec3f, dst: vec3f) -> vec3f {
    return vec3f(
        color_dodge_channel(src.x, dst.x),
        color_dodge_channel(src.y, dst.y),
        color_dodge_channel(src.z, dst.z),
    );
}

fn color_burn_channel(src: f32, dst: f32) -> f32 {
    if dst >= 1.0 {
        return 1.0;
    } else if src <= 0.0 {
        return 0.0;
    } else {
        return 1.0 - min(1.0, (1.0 - dst) / src);
    }
}

fn blend_color_burn(src: vec3f, dst: vec3f) -> vec3f {
    return vec3f(
        color_burn_channel(src.x, dst.x),
        color_burn_channel(src.y, dst.y),
        color_burn_channel(src.z, dst.z),
    );
}

fn blend_hard_light(src: vec3f, dst: vec3f) -> vec3f {
    // HardLight(src, dst) = Overlay(dst, src)
    return vec3f(
        overlay_channel(dst.x, src.x),
        overlay_channel(dst.y, src.y),
        overlay_channel(dst.z, src.z),
    );
}

fn soft_light_channel(src: f32, dst: f32) -> f32 {
    if src <= 0.5 {
        return dst - (1.0 - 2.0 * src) * dst * (1.0 - dst);
    } else {
        var d: f32;
        if dst <= 0.25 {
            d = ((16.0 * dst - 12.0) * dst + 4.0) * dst;
        } else {
            d = sqrt(dst);
        }
        return dst + (2.0 * src - 1.0) * (d - dst);
    }
}

fn blend_soft_light(src: vec3f, dst: vec3f) -> vec3f {
    return vec3f(
        soft_light_channel(src.x, dst.x),
        soft_light_channel(src.y, dst.y),
        soft_light_channel(src.z, dst.z),
    );
}

fn blend_difference(src: vec3f, dst: vec3f) -> vec3f {
    return abs(src - dst);
}

fn blend_exclusion(src: vec3f, dst: vec3f) -> vec3f {
    return src + dst - 2.0 * src * dst;
}

/// Dispatches to the appropriate blend function based on mode index.
///
/// Mode indices: 0=Multiply, 1=Screen, 2=Overlay, 3=Darken, 4=Lighten,
/// 5=ColorDodge, 6=ColorBurn, 7=HardLight, 8=SoftLight, 9=Difference, 10=Exclusion.
fn apply_blend(mode: u32, src: vec3f, dst: vec3f) -> vec3f {
    switch mode {
        case 0u: { return blend_multiply(src, dst); }
        case 1u: { return blend_screen(src, dst); }
        case 2u: { return blend_overlay(src, dst); }
        case 3u: { return blend_darken(src, dst); }
        case 4u: { return blend_lighten(src, dst); }
        case 5u: { return blend_color_dodge(src, dst); }
        case 6u: { return blend_color_burn(src, dst); }
        case 7u: { return blend_hard_light(src, dst); }
        case 8u: { return blend_soft_light(src, dst); }
        case 9u: { return blend_difference(src, dst); }
        case 10u: { return blend_exclusion(src, dst); }
        default: { return src; }
    }
}
