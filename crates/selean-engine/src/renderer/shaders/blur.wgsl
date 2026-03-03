// Dual Kawase blur shader.
//
// Two-pass approach:
// - Downsample: takes a full-res texture and writes to a half-res target,
//   sampling 5 taps (center + 4 diagonal offsets).
// - Upsample: takes a half-res texture and writes back to full-res,
//   sampling 8 neighbors at fractional offsets.
//
// Multiple iterations produce wider blur. Each iteration doubles the effective
// radius. Iterations = max(1, ceil(blur_radius / 4.0)).

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

struct BlurParams {
    // x: 1.0/width, y: 1.0/height of the source texture.
    texel_size: vec2<f32>,
    // Padding to align to 16 bytes.
    _pad: vec2<f32>,
};

@group(0) @binding(2) var<uniform> params: BlurParams;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

// Full-screen triangle (no vertex buffer needed).
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4f(x, y, 0.0, 1.0);
    out.uv = vec2f((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// Downsample: 5-tap filter.
// Center + 4 diagonal taps at half-texel offsets.
@fragment
fn fs_downsample(in: VertexOutput) -> @location(0) vec4f {
    let ht = params.texel_size * 0.5;

    let center = textureSample(src_texture, src_sampler, in.uv);
    let tl = textureSample(src_texture, src_sampler, in.uv + vec2f(-ht.x, -ht.y));
    let tr = textureSample(src_texture, src_sampler, in.uv + vec2f( ht.x, -ht.y));
    let bl = textureSample(src_texture, src_sampler, in.uv + vec2f(-ht.x,  ht.y));
    let br = textureSample(src_texture, src_sampler, in.uv + vec2f( ht.x,  ht.y));

    return (center * 4.0 + tl + tr + bl + br) / 8.0;
}

// Upsample: 8-tap tent filter.
// Samples 8 neighbors at full-texel offsets for smooth reconstruction.
@fragment
fn fs_upsample(in: VertexOutput) -> @location(0) vec4f {
    let t = params.texel_size;

    var color = vec4f(0.0);
    // Cardinal neighbors (weight 2 each).
    color += textureSample(src_texture, src_sampler, in.uv + vec2f(-t.x, 0.0)) * 2.0;
    color += textureSample(src_texture, src_sampler, in.uv + vec2f( t.x, 0.0)) * 2.0;
    color += textureSample(src_texture, src_sampler, in.uv + vec2f(0.0, -t.y)) * 2.0;
    color += textureSample(src_texture, src_sampler, in.uv + vec2f(0.0,  t.y)) * 2.0;
    // Diagonal neighbors (weight 1 each).
    color += textureSample(src_texture, src_sampler, in.uv + vec2f(-t.x, -t.y));
    color += textureSample(src_texture, src_sampler, in.uv + vec2f( t.x, -t.y));
    color += textureSample(src_texture, src_sampler, in.uv + vec2f(-t.x,  t.y));
    color += textureSample(src_texture, src_sampler, in.uv + vec2f( t.x,  t.y));

    return color / 12.0;
}
