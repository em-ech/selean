// Rectangle shader with SDF-based rounded corners, fill, stroke, and opacity.
//
// Uses instanced rendering: one shared unit quad is drawn once per rectangle instance.
// Each instance provides its own position, size, colors, corner radius, stroke, and opacity.
//
// Coordinate system:
// - World space: y-axis points down (screen convention).
// - The vertex shader transforms from world space to clip space using the camera matrix.
// - The fragment shader computes a signed distance field for rounded rectangle edges.

// --- Uniforms ---

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

// --- Instance data ---

struct RectInstance {
    // Rectangle position (top-left corner) in local space.
    @location(2) pos: vec2<f32>,
    // Rectangle size (width, height) in local space.
    @location(3) size: vec2<f32>,
    // Fill color (RGBA, linear, non-premultiplied).
    @location(4) fill_color: vec4<f32>,
    // Stroke color (RGBA, linear, non-premultiplied).
    @location(5) stroke_color: vec4<f32>,
    // x: stroke width in world pixels, y: opacity [0..1].
    @location(6) stroke_width_opacity: vec2<f32>,
    // Per-corner radius: (top-left, top-right, bottom-right, bottom-left).
    @location(7) corner_radii: vec4<f32>,
    // 2D affine transform columns (world_transform).
    @location(8) transform_c0: vec2<f32>,
    @location(9) transform_c1: vec2<f32>,
    @location(10) transform_c2: vec2<f32>,
    // Clip rectangle: [min_x, min_y, max_x, max_y]. Fragments outside are discarded.
    @location(11) clip_rect: vec4<f32>,
};

// --- Vertex input/output ---

struct VertexInput {
    // Unit quad position: (0,0), (1,0), (0,1), (1,1).
    @location(0) quad_pos: vec2<f32>,
    // Unit quad UV: same as quad_pos for rectangles.
    @location(1) quad_uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_pos: vec4<f32>,
    // Local position within the rectangle in world pixels, origin at top-left.
    @location(0) local_pos: vec2<f32>,
    // Rectangle size for SDF computation.
    @location(1) rect_size: vec2<f32>,
    // Fill color.
    @location(2) fill_color: vec4<f32>,
    // Stroke color.
    @location(3) stroke_color: vec4<f32>,
    // x: stroke width, y: opacity.
    @location(4) stroke_width_opacity: vec2<f32>,
    // Corner radii.
    @location(5) corner_radii: vec4<f32>,
    // World-space position for clip rect testing.
    @location(6) world_pos: vec2<f32>,
    // Clip rectangle passthrough.
    @location(7) clip_rect: vec4<f32>,
};

// --- Vertex shader ---

@vertex
fn vs_main(vert: VertexInput, inst: RectInstance) -> VertexOutput {
    var out: VertexOutput;

    // Compute position in local (node) space.
    let local_pos = inst.pos + vert.quad_pos * inst.size;

    // Apply 2D affine transform to get world position.
    let world_pos = vec2<f32>(
        inst.transform_c0.x * local_pos.x + inst.transform_c1.x * local_pos.y + inst.transform_c2.x,
        inst.transform_c0.y * local_pos.x + inst.transform_c1.y * local_pos.y + inst.transform_c2.y,
    );
    out.clip_pos = camera.view_proj * vec4<f32>(world_pos, 0.0, 1.0);

    // Pass local position (in pixels within the rect) for SDF computation.
    // This stays in node-local space so SDF anti-aliasing works correctly with rotation.
    out.local_pos = vert.quad_pos * inst.size;
    out.rect_size = inst.size;
    out.fill_color = inst.fill_color;
    out.stroke_color = inst.stroke_color;
    out.stroke_width_opacity = inst.stroke_width_opacity;
    out.corner_radii = inst.corner_radii;
    out.world_pos = world_pos;
    out.clip_rect = inst.clip_rect;

    return out;
}

// --- Fragment shader ---

// Signed distance to a rounded rectangle.
// `p` is the position relative to the rectangle center.
// `half_size` is half the rectangle dimensions.
// `radius` is the corner radius for the nearest corner.
fn sd_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + vec2<f32>(radius);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - radius;
}

// Select the correct corner radius based on which quadrant of the rect we're in.
fn select_corner_radius(local_pos: vec2<f32>, rect_size: vec2<f32>, radii: vec4<f32>) -> f32 {
    let center = rect_size * 0.5;
    let is_right = local_pos.x >= center.x;
    let is_bottom = local_pos.y >= center.y;

    // radii: (top-left, top-right, bottom-right, bottom-left)
    if is_bottom {
        if is_right {
            return radii.z; // bottom-right
        } else {
            return radii.w; // bottom-left
        }
    } else {
        if is_right {
            return radii.y; // top-right
        } else {
            return radii.x; // top-left
        }
    }
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // ShaderRect clip: discard fragments outside the clip rectangle.
    if (in.world_pos.x < in.clip_rect.x || in.world_pos.x > in.clip_rect.z ||
        in.world_pos.y < in.clip_rect.y || in.world_pos.y > in.clip_rect.w) {
        discard;
    }

    let half_size = in.rect_size * 0.5;
    let center_pos = in.local_pos - half_size;
    let stroke_w = in.stroke_width_opacity.x;
    let opacity = in.stroke_width_opacity.y;

    // Select corner radius for this fragment's quadrant.
    // Clamp radius to half the smaller dimension to prevent degenerate shapes.
    let max_radius = min(half_size.x, half_size.y);
    let raw_radius = select_corner_radius(in.local_pos, in.rect_size, in.corner_radii);
    let radius = min(raw_radius, max_radius);

    // Signed distance to the outer edge of the rectangle.
    let dist_outer = sd_rounded_rect(center_pos, half_size, radius);

    // Anti-aliasing: smooth transition over ~1 pixel at edges.
    let aa = fwidth(dist_outer);
    let outer_alpha = 1.0 - smoothstep(-aa, aa, dist_outer);

    // Discard fully transparent fragments. Makes stencil writes respect
    // rounded corners: fragments outside the SDF shape are discarded and
    // do not trigger stencil increment. Harmless for normal rendering.
    if (outer_alpha <= 0.0) {
        discard;
    }

    // If no stroke, just render fill.
    if stroke_w <= 0.0 {
        var color = in.fill_color;
        color.a *= outer_alpha * opacity;
        return color;
    }

    // With stroke: compute inner edge for the fill region.
    let inner_half = half_size - vec2<f32>(stroke_w);
    let inner_radius = max(radius - stroke_w, 0.0);

    // If stroke is wider than half the rect, everything is stroke.
    if inner_half.x <= 0.0 || inner_half.y <= 0.0 {
        var color = in.stroke_color;
        color.a *= outer_alpha * opacity;
        return color;
    }

    let dist_inner = sd_rounded_rect(center_pos, inner_half, inner_radius);
    let inner_alpha = 1.0 - smoothstep(-aa, aa, dist_inner);

    // Blend: stroke in the border region, fill in the interior.
    let stroke_alpha = outer_alpha - inner_alpha;
    let stroke_contrib = in.stroke_color * stroke_alpha;
    let fill_contrib = in.fill_color * inner_alpha;

    var color = stroke_contrib + fill_contrib;
    color.a *= opacity;
    return color;
}
