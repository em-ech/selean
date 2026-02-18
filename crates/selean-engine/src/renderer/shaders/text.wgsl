// Text shader with SDF-based glyph rendering.
//
// Each glyph is rendered as an instanced unit quad that samples from the
// SDF glyph atlas texture. The SDF value determines whether a pixel is
// inside or outside the glyph shape, with smooth anti-aliasing at edges.
//
// Coordinate system:
// - World space: y-axis points down (screen convention).
// - The vertex shader transforms from world space to clip space using the camera matrix.
// - The fragment shader samples the SDF atlas and applies threshold + AA.

// --- Uniforms ---

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// --- Atlas texture ---

@group(1) @binding(0)
var atlas_texture: texture_2d<f32>;

@group(1) @binding(1)
var atlas_sampler: sampler;

// --- Instance data ---

struct GlyphInstance {
    // Glyph quad position (top-left corner) in world space.
    @location(2) pos: vec2<f32>,
    // Glyph quad size (width, height) in world space.
    @location(3) size: vec2<f32>,
    // UV rectangle in the atlas: [u_min, v_min, u_max, v_max].
    @location(4) uv_rect: vec4<f32>,
    // Text color (RGBA, linear, non-premultiplied).
    @location(5) color: vec4<f32>,
    // x: opacity, yzw: unused.
    @location(6) opacity_pad: vec4<f32>,
};

// --- Vertex input/output ---

struct VertexInput {
    // Unit quad position: (0,0), (1,0), (0,1), (1,1).
    @location(0) quad_pos: vec2<f32>,
    // Unit quad UV: same as quad_pos.
    @location(1) quad_uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    // Interpolated UV in the atlas.
    @location(0) atlas_uv: vec2<f32>,
    // Text color.
    @location(1) color: vec4<f32>,
    // Opacity.
    @location(2) opacity: f32,
};

// --- Vertex shader ---

@vertex
fn vs_main(vert: VertexInput, inst: GlyphInstance) -> VertexOutput {
    var out: VertexOutput;

    // Scale and translate the unit quad to the glyph's world-space rectangle.
    let world_pos = inst.pos + vert.quad_pos * inst.size;
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);

    // Interpolate UV within the atlas region.
    let uv_min = inst.uv_rect.xy;
    let uv_max = inst.uv_rect.zw;
    out.atlas_uv = mix(uv_min, uv_max, vert.quad_uv);

    out.color = inst.color;
    out.opacity = inst.opacity_pad.x;

    return out;
}

// --- Fragment shader ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the SDF value from the atlas (R channel, [0..1]).
    let sdf_val = textureSample(atlas_texture, atlas_sampler, in.atlas_uv).r;

    // SDF threshold: 0.5 (128/255) is the glyph edge.
    // Use fwidth for screen-space anti-aliasing width.
    let dist = sdf_val - 0.5;
    let aa = fwidth(dist);
    let alpha = smoothstep(-aa, aa, dist);

    var color = in.color;
    color.a *= alpha * in.opacity;

    return color;
}
