// Rectangle shader with SDF-based rounded corners, fill, stroke, opacity, and gradients.
//
// Uses instanced rendering: one shared unit quad is drawn once per rectangle instance.
// Each instance provides its own position, size, colors, corner radius, stroke, opacity,
// and optional gradient fill (linear or radial, up to 4 color stops).
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
    // Gradient metadata: [type, stop_count, 0, 0]. type: 0=none, 1=linear, 2=radial.
    @location(12) gradient_meta: vec4<f32>,
    // Gradient points: [start_x/center_x, start_y/center_y, end_x/radius, end_y/0].
    @location(13) gradient_points: vec4<f32>,
    // Gradient stops: 4 stops * 5 floats = 20 floats packed as 5 vec4s.
    // Each stop: [position, r, g, b] then [a, next_position, next_r, next_g] interleaved.
    // Actually packed as: stop0=[pos,r,g,b], stop0_a+stop1=[a,pos,r,g], etc.
    // Layout per stop (5 floats): position, r, g, b, a spread across vec4s.
    @location(14) gradient_stops_0: vec4<f32>,
    @location(15) gradient_stops_1: vec4<f32>,
    @location(16) gradient_stops_2: vec4<f32>,
    @location(17) gradient_stops_3: vec4<f32>,
    @location(18) gradient_stops_4: vec4<f32>,
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
    // Gradient metadata passthrough.
    @location(8) gradient_meta: vec4<f32>,
    // Gradient points passthrough.
    @location(9) gradient_points: vec4<f32>,
    // Gradient stops passthrough (5 vec4s).
    @location(10) gradient_stops_0: vec4<f32>,
    @location(11) gradient_stops_1: vec4<f32>,
    @location(12) gradient_stops_2: vec4<f32>,
    @location(13) gradient_stops_3: vec4<f32>,
    @location(14) gradient_stops_4: vec4<f32>,
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
    out.gradient_meta = inst.gradient_meta;
    out.gradient_points = inst.gradient_points;
    out.gradient_stops_0 = inst.gradient_stops_0;
    out.gradient_stops_1 = inst.gradient_stops_1;
    out.gradient_stops_2 = inst.gradient_stops_2;
    out.gradient_stops_3 = inst.gradient_stops_3;
    out.gradient_stops_4 = inst.gradient_stops_4;

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

// Read a gradient stop from the packed 20-float array (5 vec4s).
// Each stop is 5 consecutive floats: [position, r, g, b, a].
// stop_index: 0..3
fn read_gradient_stop_pos(
    s0: vec4<f32>, s1: vec4<f32>, s2: vec4<f32>, s3: vec4<f32>, s4: vec4<f32>,
    stop_index: i32
) -> f32 {
    // Stop 0: s0.x
    // Stop 1: s1.x (offset 5 -> vec4 index 1, component 0... actually floats 5,6,7,8,9)
    // Layout: [p0 r0 g0 b0 | a0 p1 r1 g1 | b1 a1 p2 r2 | g2 b2 a2 p3 | r3 g3 b3 a3]
    switch stop_index {
        case 0: { return s0.x; }
        case 1: { return s1.y; }
        case 2: { return s2.z; }
        case 3: { return s3.w; }
        default: { return 0.0; }
    }
}

fn read_gradient_stop_color(
    s0: vec4<f32>, s1: vec4<f32>, s2: vec4<f32>, s3: vec4<f32>, s4: vec4<f32>,
    stop_index: i32
) -> vec4<f32> {
    // Stop 0 color: r=s0.y, g=s0.z, b=s0.w, a=s1.x
    // Stop 1 color: r=s1.z, g=s1.w, b=s2.x, a=s2.y
    // Stop 2 color: r=s2.w, g=s3.x, b=s3.y, a=s3.z
    // Stop 3 color: r=s4.x, g=s4.y, b=s4.z, a=s4.w
    switch stop_index {
        case 0: { return vec4<f32>(s0.y, s0.z, s0.w, s1.x); }
        case 1: { return vec4<f32>(s1.z, s1.w, s2.x, s2.y); }
        case 2: { return vec4<f32>(s2.w, s3.x, s3.y, s3.z); }
        case 3: { return vec4<f32>(s4.x, s4.y, s4.z, s4.w); }
        default: { return vec4<f32>(0.0); }
    }
}

// Evaluate a gradient at parameter t [0..1] given packed stop data.
fn evaluate_gradient(
    t_raw: f32, stop_count: i32,
    s0: vec4<f32>, s1: vec4<f32>, s2: vec4<f32>, s3: vec4<f32>, s4: vec4<f32>,
) -> vec4<f32> {
    let t = clamp(t_raw, 0.0, 1.0);

    if stop_count <= 0 {
        return vec4<f32>(0.0);
    }
    if stop_count == 1 {
        return read_gradient_stop_color(s0, s1, s2, s3, s4, 0);
    }

    // Find the two stops surrounding t and interpolate.
    var color = read_gradient_stop_color(s0, s1, s2, s3, s4, 0);

    for (var i = 1; i < 4; i++) {
        if i >= stop_count {
            break;
        }
        let prev_pos = read_gradient_stop_pos(s0, s1, s2, s3, s4, i - 1);
        let curr_pos = read_gradient_stop_pos(s0, s1, s2, s3, s4, i);
        let prev_color = read_gradient_stop_color(s0, s1, s2, s3, s4, i - 1);
        let curr_color = read_gradient_stop_color(s0, s1, s2, s3, s4, i);

        if t <= curr_pos {
            let range = curr_pos - prev_pos;
            if range > 0.0 {
                let local_t = (t - prev_pos) / range;
                color = mix(prev_color, curr_color, local_t);
            } else {
                color = curr_color;
            }
            return color;
        }
    }

    // t is past all stops; return last stop color.
    return read_gradient_stop_color(s0, s1, s2, s3, s4, stop_count - 1);
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

    // Determine fill color: gradient takes priority over solid.
    let grad_type = i32(in.gradient_meta.x);
    let grad_stop_count = i32(in.gradient_meta.y);
    var fill = in.fill_color;

    if grad_type == 1 {
        // Linear gradient: project local_pos onto gradient axis.
        let uv = in.local_pos / in.rect_size;
        let grad_start = vec2<f32>(in.gradient_points.x, in.gradient_points.y);
        let grad_end = vec2<f32>(in.gradient_points.z, in.gradient_points.w);
        let axis = grad_end - grad_start;
        let axis_len_sq = dot(axis, axis);
        var t = 0.0;
        if axis_len_sq > 0.0 {
            t = dot(uv - grad_start, axis) / axis_len_sq;
        }
        fill = evaluate_gradient(
            t, grad_stop_count,
            in.gradient_stops_0, in.gradient_stops_1, in.gradient_stops_2,
            in.gradient_stops_3, in.gradient_stops_4,
        );
    } else if grad_type == 2 {
        // Radial gradient: distance from center.
        let uv = in.local_pos / in.rect_size;
        let grad_center = vec2<f32>(in.gradient_points.x, in.gradient_points.y);
        let grad_radius = in.gradient_points.z;
        let dist = length(uv - grad_center);
        var t = 0.0;
        if grad_radius > 0.0 {
            t = dist / grad_radius;
        }
        fill = evaluate_gradient(
            t, grad_stop_count,
            in.gradient_stops_0, in.gradient_stops_1, in.gradient_stops_2,
            in.gradient_stops_3, in.gradient_stops_4,
        );
    }

    // If no stroke, just render fill.
    if stroke_w <= 0.0 {
        var color = fill;
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
    let fill_contrib = fill * inner_alpha;

    var color = stroke_contrib + fill_contrib;
    color.a *= opacity;
    return color;
}
