//! Tailwind CSS class generation from scene node properties.
//!
//! Maps [`SceneNode`] visual properties to Tailwind CSS utility classes
//! using arbitrary value syntax (e.g., `bg-[#ff0000]`, `w-[200px]`).

use selean_engine::scene::{
    BlendMode, ClipMode, Color, Effect, Gradient, GradientStop, SceneNode, SceneNodeKind,
};

/// Converts a [`Color`] to a hex string (`#RRGGBB` or `#RRGGBBAA`).
#[must_use]
pub fn color_to_hex(color: &Color) -> String {
    let r = float_to_byte(color.r);
    let g = float_to_byte(color.g);
    let b = float_to_byte(color.b);
    if (color.a - 1.0).abs() < f32::EPSILON {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        let a = float_to_byte(color.a);
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

/// Converts a float in [0.0, 1.0] to a byte in [0, 255].
fn float_to_byte(v: f32) -> u8 {
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let byte = (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    byte
}

/// Returns Tailwind arbitrary color syntax like `[#ff0000]`.
#[must_use]
pub fn color_to_tailwind_style(color: &Color) -> String {
    format!("[{}]", color_to_hex(color))
}

/// Generates space-separated Tailwind classes for a [`SceneNode`].
///
/// Covers positioning, sizing, fill, stroke, opacity, corner radius,
/// visibility, blend mode, clipping, and effects.
#[must_use]
pub fn node_classes(node: &SceneNode) -> String {
    let mut classes = Vec::new();
    classes.push("absolute".to_string());

    append_position_classes(&mut classes, node);
    append_fill_classes(&mut classes, node);
    append_stroke_classes(&mut classes, node);
    append_opacity_class(&mut classes, node);
    append_corner_radius_classes(&mut classes, node);
    append_visibility_class(&mut classes, node);
    append_blend_mode_class(&mut classes, node);
    append_clip_class(&mut classes, node);
    append_effect_classes(&mut classes, node);
    append_text_classes(&mut classes, node);

    classes.join(" ")
}

/// Appends position and size classes.
fn append_position_classes(classes: &mut Vec<String>, node: &SceneNode) {
    let b = &node.bounds;
    classes.push(format!("left-[{}px]", format_f32(b.x)));
    classes.push(format!("top-[{}px]", format_f32(b.y)));
    classes.push(format!("w-[{}px]", format_f32(b.width)));
    classes.push(format!("h-[{}px]", format_f32(b.height)));
}

/// Appends fill/background classes. Gradients use inline style (not classes).
fn append_fill_classes(classes: &mut Vec<String>, node: &SceneNode) {
    // Solid fill only when no gradient is present.
    if node.fill_gradient.is_none() {
        if let Some(ref fill) = node.fill {
            classes.push(format!("bg-{}", color_to_tailwind_style(fill)));
        }
    }
}

/// Appends stroke/border classes.
fn append_stroke_classes(classes: &mut Vec<String>, node: &SceneNode) {
    if let Some(ref stroke) = node.stroke {
        let width = node.stroke_width.max(1.0);
        classes.push("border".to_string());
        classes.push(format!("border-{}", color_to_tailwind_style(stroke)));
        if (width - 1.0).abs() > f32::EPSILON {
            classes.push(format!("border-[{}px]", format_f32(width)));
        }
    }
}

/// Appends opacity class (skip if fully opaque).
fn append_opacity_class(classes: &mut Vec<String>, node: &SceneNode) {
    if (node.opacity - 1.0).abs() > f32::EPSILON {
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
        let percent = (node.opacity * 100.0).round() as u32;
        classes.push(format!("opacity-{percent}"));
    }
}

/// Appends corner radius classes for `Frame` nodes.
fn append_corner_radius_classes(classes: &mut Vec<String>, node: &SceneNode) {
    if let SceneNodeKind::Frame { corner_radius } = &node.kind {
        let all_same = corner_radius
            .iter()
            .all(|r| (r - corner_radius[0]).abs() < f32::EPSILON);
        if all_same {
            if corner_radius[0] > 0.0 {
                classes.push(format!("rounded-[{}px]", format_f32(corner_radius[0])));
            }
        } else {
            append_individual_corners(classes, corner_radius);
        }
    }
}

/// Appends per-corner radius classes (TL, TR, BR, BL order).
fn append_individual_corners(classes: &mut Vec<String>, radii: &[f32; 4]) {
    let labels = ["rounded-tl", "rounded-tr", "rounded-br", "rounded-bl"];
    for (label, &r) in labels.iter().zip(radii.iter()) {
        if r > 0.0 {
            classes.push(format!("{label}-[{}px]", format_f32(r)));
        }
    }
}

/// Appends `hidden` class for invisible nodes.
fn append_visibility_class(classes: &mut Vec<String>, node: &SceneNode) {
    if !node.visible {
        classes.push("hidden".to_string());
    }
}

/// Appends `mix-blend-{mode}` class (skip `Normal`).
fn append_blend_mode_class(classes: &mut Vec<String>, node: &SceneNode) {
    let class = blend_mode_class(node.blend_mode);
    if let Some(c) = class {
        classes.push(c.to_string());
    }
}

/// Maps a [`BlendMode`] to a Tailwind `mix-blend-*` class.
fn blend_mode_class(mode: BlendMode) -> Option<&'static str> {
    match mode {
        BlendMode::Normal | BlendMode::Add => None,
        BlendMode::Multiply => Some("mix-blend-multiply"),
        BlendMode::Screen => Some("mix-blend-screen"),
        BlendMode::Overlay => Some("mix-blend-overlay"),
        BlendMode::Darken => Some("mix-blend-darken"),
        BlendMode::Lighten => Some("mix-blend-lighten"),
        BlendMode::ColorDodge => Some("mix-blend-color-dodge"),
        BlendMode::ColorBurn => Some("mix-blend-color-burn"),
        BlendMode::HardLight => Some("mix-blend-hard-light"),
        BlendMode::SoftLight => Some("mix-blend-soft-light"),
        BlendMode::Difference => Some("mix-blend-difference"),
        BlendMode::Exclusion => Some("mix-blend-exclusion"),
    }
}

/// Appends `overflow-hidden` for clipped nodes.
fn append_clip_class(classes: &mut Vec<String>, node: &SceneNode) {
    if node.clip_mode != ClipMode::None {
        classes.push("overflow-hidden".to_string());
    }
}

/// Appends effect classes (drop shadow, blur).
fn append_effect_classes(classes: &mut Vec<String>, node: &SceneNode) {
    for effect in &node.effects {
        match effect {
            Effect::DropShadow {
                color,
                offset_x,
                offset_y,
                blur_radius,
            } => {
                let hex = color_to_hex(color);
                classes.push(format!(
                    "shadow-[{}px_{}px_{}px_{}]",
                    format_f32(*offset_x),
                    format_f32(*offset_y),
                    format_f32(*blur_radius),
                    hex,
                ));
            }
            Effect::Blur { radius } => {
                classes.push(format!("blur-[{}px]", format_f32(*radius)));
            }
        }
    }
}

/// Appends text-specific classes for `Text` nodes.
fn append_text_classes(classes: &mut Vec<String>, node: &SceneNode) {
    if let SceneNodeKind::Text {
        font_size,
        font_weight,
        font_family,
        font_style,
        text_align,
        line_height,
        text_color,
        ..
    } = &node.kind
    {
        classes.push(format!("text-[{}px]", format_f32(*font_size)));
        if *font_weight != 400 {
            classes.push(format!("font-[{font_weight}]"));
        }
        if font_family != "Inter" && !font_family.is_empty() {
            classes.push(format!("font-['{font_family}']"));
        }
        if *font_style == selean_engine::scene::FontStyle::Italic {
            classes.push("italic".to_string());
        }
        append_text_align_class(classes, *text_align);
        if (*line_height - 1.2).abs() > f32::EPSILON {
            classes.push(format!("leading-[{}]", format_f32(*line_height)));
        }
        if let Some(tc) = text_color {
            classes.push(format!("text-{}", color_to_tailwind_style(tc)));
        } else if let Some(ref fill) = node.fill {
            classes.push(format!("text-{}", color_to_tailwind_style(fill)));
        }
    }
}

/// Appends text alignment class.
fn append_text_align_class(classes: &mut Vec<String>, align: selean_engine::scene::TextAlign) {
    use selean_engine::scene::TextAlign;
    match align {
        TextAlign::Left => {} // default, skip
        TextAlign::Center => classes.push("text-center".to_string()),
        TextAlign::Right => classes.push("text-right".to_string()),
        TextAlign::Justify => classes.push("text-justify".to_string()),
    }
}

/// Generates an inline `style` attribute string for gradient fills.
///
/// Returns `None` if the node has no gradient fill.
#[must_use]
pub fn gradient_style(node: &SceneNode) -> Option<String> {
    let gradient = node.fill_gradient.as_ref()?;
    let css = match gradient {
        Gradient::Linear { start, end, stops } => {
            let angle = compute_gradient_angle(*start, *end);
            let stop_str = format_gradient_stops(stops);
            format!("background: linear-gradient({angle}deg, {stop_str})")
        }
        Gradient::Radial { stops, .. } => {
            let stop_str = format_gradient_stops(stops);
            format!("background: radial-gradient(circle, {stop_str})")
        }
    };
    Some(css)
}

/// Computes the CSS gradient angle from start/end fraction points.
fn compute_gradient_angle(start: [f32; 2], end: [f32; 2]) -> i32 {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    // CSS gradient angle: 0deg = bottom-to-top, 90deg = left-to-right
    #[allow(clippy::cast_possible_truncation)]
    let angle = (dy.atan2(dx).to_degrees() + 90.0).round() as i32;
    ((angle % 360) + 360) % 360
}

/// Formats gradient stops as a CSS color-stop list.
fn format_gradient_stops(stops: &[GradientStop]) -> String {
    stops
        .iter()
        .map(|s| {
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let pct = (s.position * 100.0).round() as u32;
            format!("{} {pct}%", color_to_hex(&s.color))
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Formats an f32 as a clean string (no trailing zeros).
fn format_f32(v: f32) -> String {
    if (v - v.round()).abs() < 0.001 {
        #[allow(clippy::cast_possible_truncation)]
        return format!("{}", v.round() as i32);
    }
    format!("{v:.1}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, FontStyle, TextAlign};

    use super::*;

    fn make_frame(x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            "Frame".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(x, y, w, h),
        )
    }

    fn make_text(content: &str) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            "Text".to_string(),
            SceneNodeKind::Text {
                content: content.to_string(),
                font_size: 16.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: FontStyle::Normal,
                text_align: TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(0.0, 0.0, 200.0, 40.0),
        )
    }

    // --- color_to_hex tests ---

    #[test]
    fn hex_opaque_red() {
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        assert_eq!(color_to_hex(&c), "#ff0000");
    }

    #[test]
    fn hex_opaque_white() {
        assert_eq!(color_to_hex(&Color::WHITE), "#ffffff");
    }

    #[test]
    fn hex_opaque_black() {
        assert_eq!(color_to_hex(&Color::BLACK), "#000000");
    }

    #[test]
    fn hex_transparent() {
        let c = Color::new(1.0, 0.0, 0.0, 0.5);
        assert_eq!(color_to_hex(&c), "#ff000080");
    }

    #[test]
    fn hex_fully_transparent() {
        assert_eq!(color_to_hex(&Color::TRANSPARENT), "#00000000");
    }

    #[test]
    fn hex_mid_gray() {
        let c = Color::new(0.5, 0.5, 0.5, 1.0);
        let hex = color_to_hex(&c);
        // 0.5 * 255 = 127.5, rounds to 128 = 0x80
        assert_eq!(hex, "#808080");
    }

    // --- node_classes tests ---

    #[test]
    fn frame_with_fill_and_stroke() {
        let mut node = make_frame(10.0, 20.0, 300.0, 200.0);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        node.stroke = Some(Color::new(0.0, 0.0, 0.0, 1.0));
        node.stroke_width = 2.0;
        let cls = node_classes(&node);
        assert!(cls.contains("absolute"));
        assert!(cls.contains("left-[10px]"));
        assert!(cls.contains("top-[20px]"));
        assert!(cls.contains("w-[300px]"));
        assert!(cls.contains("h-[200px]"));
        assert!(cls.contains("bg-[#ff0000]"));
        assert!(cls.contains("border"));
        assert!(cls.contains("border-[#000000]"));
        assert!(cls.contains("border-[2px]"));
    }

    #[test]
    fn frame_with_corner_radius() {
        let mut node = SceneNode::new(
            NodeId::new(),
            "Rounded".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [8.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        node.fill = Some(Color::WHITE);
        let cls = node_classes(&node);
        assert!(cls.contains("rounded-[8px]"));
    }

    #[test]
    fn frame_with_mixed_corner_radii() {
        let node = SceneNode::new(
            NodeId::new(),
            "Mixed".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [4.0, 8.0, 0.0, 12.0],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 100.0),
        );
        let cls = node_classes(&node);
        assert!(cls.contains("rounded-tl-[4px]"));
        assert!(cls.contains("rounded-tr-[8px]"));
        assert!(!cls.contains("rounded-br"));
        assert!(cls.contains("rounded-bl-[12px]"));
    }

    #[test]
    fn text_node_classes() {
        let mut node = make_text("Hello");
        if let SceneNodeKind::Text {
            ref mut font_weight,
            ref mut font_style,
            ref mut text_align,
            ref mut text_color,
            ..
        } = node.kind
        {
            *font_weight = 700;
            *font_style = FontStyle::Italic;
            *text_align = TextAlign::Center;
            *text_color = Some(Color::new(0.0, 0.0, 1.0, 1.0));
        }
        let cls = node_classes(&node);
        assert!(cls.contains("text-[16px]"));
        assert!(cls.contains("font-[700]"));
        assert!(cls.contains("italic"));
        assert!(cls.contains("text-center"));
        assert!(cls.contains("text-[#0000ff]"));
    }

    #[test]
    fn hidden_node_has_hidden_class() {
        let mut node = make_frame(0.0, 0.0, 50.0, 50.0);
        node.visible = false;
        let cls = node_classes(&node);
        assert!(cls.contains("hidden"));
    }

    #[test]
    fn blend_mode_multiply() {
        let mut node = make_frame(0.0, 0.0, 50.0, 50.0);
        node.blend_mode = BlendMode::Multiply;
        let cls = node_classes(&node);
        assert!(cls.contains("mix-blend-multiply"));
    }

    #[test]
    fn blend_mode_normal_skipped() {
        let node = make_frame(0.0, 0.0, 50.0, 50.0);
        let cls = node_classes(&node);
        assert!(!cls.contains("mix-blend"));
    }

    #[test]
    fn drop_shadow_effect() {
        let mut node = make_frame(0.0, 0.0, 100.0, 100.0);
        node.effects.push(Effect::DropShadow {
            color: Color::new(0.0, 0.0, 0.0, 0.5),
            offset_x: 4.0,
            offset_y: 4.0,
            blur_radius: 8.0,
        });
        let cls = node_classes(&node);
        assert!(cls.contains("shadow-[4px_4px_8px_#00000080]"));
    }

    #[test]
    fn blur_effect() {
        let mut node = make_frame(0.0, 0.0, 100.0, 100.0);
        node.effects.push(Effect::Blur { radius: 10.0 });
        let cls = node_classes(&node);
        assert!(cls.contains("blur-[10px]"));
    }

    #[test]
    fn no_fill_no_stroke_minimal_classes() {
        let node = make_frame(0.0, 0.0, 100.0, 100.0);
        let cls = node_classes(&node);
        assert!(cls.contains("absolute"));
        assert!(cls.contains("left-[0px]"));
        assert!(!cls.contains("bg-"));
        assert!(!cls.contains("border"));
    }

    #[test]
    fn opacity_class() {
        let mut node = make_frame(0.0, 0.0, 50.0, 50.0);
        node.opacity = 0.5;
        let cls = node_classes(&node);
        assert!(cls.contains("opacity-50"));
    }

    #[test]
    fn clip_mode_overflow_hidden() {
        let mut node = make_frame(0.0, 0.0, 50.0, 50.0);
        node.clip_mode = ClipMode::Scissor;
        let cls = node_classes(&node);
        assert!(cls.contains("overflow-hidden"));
    }

    #[test]
    fn format_f32_integer() {
        assert_eq!(format_f32(10.0), "10");
    }

    #[test]
    fn format_f32_decimal() {
        assert_eq!(format_f32(10.5), "10.5");
    }

    // --- gradient_style tests ---

    #[test]
    fn gradient_style_none_without_gradient() {
        let node = make_frame(0.0, 0.0, 100.0, 100.0);
        assert!(gradient_style(&node).is_none());
    }

    #[test]
    fn gradient_style_linear() {
        let mut node = make_frame(0.0, 0.0, 100.0, 100.0);
        node.fill_gradient = Some(Gradient::Linear {
            start: [0.0, 0.0],
            end: [1.0, 0.0],
            stops: vec![
                GradientStop {
                    position: 0.0,
                    color: Color::new(1.0, 0.0, 0.0, 1.0),
                },
                GradientStop {
                    position: 1.0,
                    color: Color::new(0.0, 0.0, 1.0, 1.0),
                },
            ],
        });
        let style = gradient_style(&node).expect("should have gradient");
        assert!(style.contains("linear-gradient"));
        assert!(style.contains("#ff0000"));
        assert!(style.contains("#0000ff"));
    }

    #[test]
    fn gradient_style_radial() {
        let mut node = make_frame(0.0, 0.0, 100.0, 100.0);
        node.fill_gradient = Some(Gradient::Radial {
            center: [0.5, 0.5],
            radius: 1.0,
            stops: vec![GradientStop {
                position: 0.0,
                color: Color::WHITE,
            }],
        });
        let style = gradient_style(&node).expect("should have gradient");
        assert!(style.contains("radial-gradient"));
    }
}
