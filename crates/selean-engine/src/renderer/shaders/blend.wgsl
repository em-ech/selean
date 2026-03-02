// Full-screen blend composite shader.
//
// Reads the src texture (intermediate RT after element render) and dst texture
// (snapshot before element render), applies the blend formula, and writes the
// composited result. Uses Porter-Duff source-over with custom blend RGB.

// --- Blend function library (inlined) ---

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

// --- Bindings ---

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var dst_texture: texture_2d<f32>;
@group(0) @binding(2) var tex_sampler: sampler;

struct BlendUniform {
    mode: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(3) var<uniform> blend_params: BlendUniform;

// --- Vertex shader: full-screen triangle from vertex index ---

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Full-screen triangle: vertices at (-1,-1), (3,-1), (-1,3)
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4f(x, y, 0.0, 1.0);
    // Map to UV: [0,1] range with Y flipped for texture sampling
    out.uv = vec2f((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// --- Fragment shader ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let src_pm = textureSample(src_texture, tex_sampler, in.uv);
    let dst_pm = textureSample(dst_texture, tex_sampler, in.uv);

    let a_src = src_pm.a;
    let a_dst = dst_pm.a;

    // Un-premultiply
    var src_rgb: vec3f;
    if a_src > 0.001 {
        src_rgb = src_pm.rgb / a_src;
    } else {
        src_rgb = vec3f(0.0);
    }

    var dst_rgb: vec3f;
    if a_dst > 0.001 {
        dst_rgb = dst_pm.rgb / a_dst;
    } else {
        dst_rgb = vec3f(0.0);
    }

    // Apply blend formula
    let blended = apply_blend(blend_params.mode, src_rgb, dst_rgb);

    // Porter-Duff source-over composite
    let a_out = a_src + a_dst * (1.0 - a_src);

    if a_out < 0.001 {
        return vec4f(0.0);
    }

    let rgb_out = (1.0 - a_dst) * src_rgb * a_src
                + (1.0 - a_src) * dst_rgb * a_dst
                + a_src * a_dst * blended;

    return vec4f(clamp(rgb_out, vec3f(0.0), vec3f(1.0)), a_out);
}
