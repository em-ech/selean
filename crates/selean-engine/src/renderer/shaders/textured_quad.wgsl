// Textured quad shader for RGBA image/vector rendering.
//
// Each image or rasterized vector is rendered as an instanced unit quad that
// samples from the RGBA atlas texture. The texture color is multiplied by a
// tint color and opacity.
//
// Coordinate system:
// - World space: y-axis points down (screen convention).
// - The vertex shader transforms from world space to clip space using the camera matrix.
// - The fragment shader samples RGBA, multiplies by tint, and applies opacity.

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

struct QuadInstance {
    // Quad position (top-left corner) in world space.
    @location(2) pos: vec2<f32>,
    // Quad size (width, height) in world space.
    @location(3) size: vec2<f32>,
    // UV rectangle in the atlas: [u_min, v_min, u_max, v_max].
    @location(4) uv_rect: vec4<f32>,
    // Tint color (RGBA, linear). Multiplied with texture color.
    @location(5) tint_color: vec4<f32>,
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
    // Tint color.
    @location(1) tint_color: vec4<f32>,
    // Opacity.
    @location(2) opacity: f32,
};

// --- Vertex shader ---

@vertex
fn vs_main(vert: VertexInput, inst: QuadInstance) -> VertexOutput {
    var out: VertexOutput;

    // Scale and translate the unit quad to the instance's world-space rectangle.
    let world_pos = inst.pos + vert.quad_pos * inst.size;
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);

    // Interpolate UV within the atlas region.
    let uv_min = inst.uv_rect.xy;
    let uv_max = inst.uv_rect.zw;
    out.atlas_uv = mix(uv_min, uv_max, vert.quad_uv);

    out.tint_color = inst.tint_color;
    out.opacity = inst.opacity_pad.x;

    return out;
}

// --- Fragment shader ---

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Sample the RGBA texture from the atlas.
    let tex_color = textureSample(atlas_texture, atlas_sampler, in.atlas_uv);

    // Multiply by tint color and apply opacity.
    var color = tex_color * in.tint_color;
    color.a *= in.opacity;

    return color;
}
