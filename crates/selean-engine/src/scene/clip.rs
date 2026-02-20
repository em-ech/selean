//! Clip mode and clip rectangle types for node-level clipping.
//!
//! Each scene node can optionally clip its children using one of three modes:
//! - **Scissor**: axis-aligned GPU scissor rect, zero shader overhead, no rounded corners.
//! - **Stencil**: stencil buffer writes, supports rounded corners and nesting.
//! - **`ShaderRect`**: per-instance `clip_rect` checked in the fragment shader with `discard`.

use std::fmt;

/// Clipping mode for a scene node.
///
/// When a node has a clip mode other than `None`, all descendants are clipped
/// to the node's bounds (or a shape derived from them). The mode determines
/// which GPU mechanism is used for clipping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClipMode {
    /// No clipping. Children render their full content regardless of parent bounds.
    #[default]
    None,
    /// Axis-aligned scissor rectangle. Zero GPU overhead but no rounded corners.
    /// Uses `set_scissor_rect()` on the render pass. Rotated nodes use the
    /// world-space AABB (conservative).
    Scissor,
    /// Stencil buffer clipping. Supports rounded corners (on Frame nodes) and
    /// arbitrary nesting via stencil increment/decrement.
    Stencil,
    /// Per-instance shader clip rectangle. The fragment shader discards pixels
    /// outside the clip bounds. Supports rotated rectangular clips via
    /// world-space AABB (conservative).
    ShaderRect,
}

/// An axis-aligned clip rectangle in world-space coordinates.
///
/// Represents the intersection of all active clip regions. Used for both
/// scissor rects and shader `clip_rect` values.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipRect {
    /// Left edge (minimum x).
    pub min_x: f32,
    /// Top edge (minimum y).
    pub min_y: f32,
    /// Right edge (maximum x).
    pub max_x: f32,
    /// Bottom edge (maximum y).
    pub max_y: f32,
}

impl ClipRect {
    /// Infinite clip rect (no clipping). Used as the sentinel value in shader
    /// `clip_rect` fields. The always-true comparison is optimized away by the
    /// GPU compiler.
    pub const INFINITE: Self = Self {
        min_x: f32::NEG_INFINITY,
        min_y: f32::NEG_INFINITY,
        max_x: f32::INFINITY,
        max_y: f32::INFINITY,
    };

    /// Creates a new clip rect from bounds.
    #[must_use]
    pub const fn new(min_x: f32, min_y: f32, max_x: f32, max_y: f32) -> Self {
        Self {
            min_x,
            min_y,
            max_x,
            max_y,
        }
    }

    /// Returns the intersection of this clip rect with another.
    ///
    /// The result is the region visible in both rects. If the rects do not
    /// overlap, the result will be empty (`is_empty()` returns `true`).
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            min_x: self.min_x.max(other.min_x),
            min_y: self.min_y.max(other.min_y),
            max_x: self.max_x.min(other.max_x),
            max_y: self.max_y.min(other.max_y),
        }
    }

    /// Returns `true` if the clip rect has zero or negative area.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.max_x <= self.min_x || self.max_y <= self.min_y
    }

    /// Returns `true` if the given point is inside this clip rect.
    #[must_use]
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x <= self.max_x && y >= self.min_y && y <= self.max_y
    }

    /// Converts to a wgpu scissor rect `(x, y, width, height)` in physical pixels.
    ///
    /// Clamps to viewport bounds. Returns `None` if the resulting rect is empty
    /// (fully outside the viewport or zero size).
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn to_scissor_rect(
        &self,
        viewport_w: u32,
        viewport_h: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        let x0 = self.min_x.max(0.0) as u32;
        let y0 = self.min_y.max(0.0) as u32;
        let x1 = (self.max_x.max(0.0) as u32).min(viewport_w);
        let y1 = (self.max_y.max(0.0) as u32).min(viewport_h);

        if x1 <= x0 || y1 <= y0 {
            return None;
        }

        Some((x0, y0, x1 - x0, y1 - y0))
    }

    /// Returns the clip rect as a `[f32; 4]` array suitable for shader uniforms.
    /// Format: `[min_x, min_y, max_x, max_y]`.
    #[must_use]
    pub const fn to_array(&self) -> [f32; 4] {
        [self.min_x, self.min_y, self.max_x, self.max_y]
    }
}

impl fmt::Display for ClipRect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ClipRect({:.1}, {:.1}, {:.1}, {:.1})",
            self.min_x, self.min_y, self.max_x, self.max_y
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)]

    use super::*;

    // --- ClipMode tests ---

    #[test]
    fn clip_mode_default_is_none() {
        assert_eq!(ClipMode::default(), ClipMode::None);
    }

    #[test]
    fn clip_mode_variants_are_distinct() {
        assert_ne!(ClipMode::None, ClipMode::Scissor);
        assert_ne!(ClipMode::Scissor, ClipMode::Stencil);
        assert_ne!(ClipMode::Stencil, ClipMode::ShaderRect);
    }

    // --- ClipRect intersection tests ---

    #[test]
    fn intersect_overlapping_rects() {
        let a = ClipRect::new(0.0, 0.0, 100.0, 100.0);
        let b = ClipRect::new(50.0, 50.0, 150.0, 150.0);
        let c = a.intersect(&b);
        assert_eq!(c.min_x, 50.0);
        assert_eq!(c.min_y, 50.0);
        assert_eq!(c.max_x, 100.0);
        assert_eq!(c.max_y, 100.0);
        assert!(!c.is_empty());
    }

    #[test]
    fn intersect_non_overlapping_rects() {
        let a = ClipRect::new(0.0, 0.0, 50.0, 50.0);
        let b = ClipRect::new(100.0, 100.0, 200.0, 200.0);
        let c = a.intersect(&b);
        assert!(c.is_empty());
    }

    #[test]
    fn intersect_contained_rect() {
        let outer = ClipRect::new(0.0, 0.0, 200.0, 200.0);
        let inner = ClipRect::new(50.0, 50.0, 100.0, 100.0);
        let c = outer.intersect(&inner);
        assert_eq!(c.min_x, 50.0);
        assert_eq!(c.min_y, 50.0);
        assert_eq!(c.max_x, 100.0);
        assert_eq!(c.max_y, 100.0);
    }

    #[test]
    fn intersect_touching_edges() {
        let a = ClipRect::new(0.0, 0.0, 50.0, 50.0);
        let b = ClipRect::new(50.0, 0.0, 100.0, 50.0);
        let c = a.intersect(&b);
        assert!(c.is_empty()); // touching at edge = zero width
    }

    #[test]
    fn intersect_with_infinite() {
        let a = ClipRect::new(10.0, 20.0, 300.0, 400.0);
        let c = a.intersect(&ClipRect::INFINITE);
        assert_eq!(c.min_x, 10.0);
        assert_eq!(c.min_y, 20.0);
        assert_eq!(c.max_x, 300.0);
        assert_eq!(c.max_y, 400.0);
    }

    #[test]
    fn intersect_is_commutative() {
        let a = ClipRect::new(0.0, 0.0, 100.0, 100.0);
        let b = ClipRect::new(25.0, 25.0, 75.0, 75.0);
        let ab = a.intersect(&b);
        let ba = b.intersect(&a);
        assert_eq!(ab.min_x, ba.min_x);
        assert_eq!(ab.min_y, ba.min_y);
        assert_eq!(ab.max_x, ba.max_x);
        assert_eq!(ab.max_y, ba.max_y);
    }

    // --- ClipRect is_empty tests ---

    #[test]
    fn empty_rect_zero_width() {
        assert!(ClipRect::new(10.0, 10.0, 10.0, 50.0).is_empty());
    }

    #[test]
    fn empty_rect_zero_height() {
        assert!(ClipRect::new(10.0, 10.0, 50.0, 10.0).is_empty());
    }

    #[test]
    fn empty_rect_inverted() {
        assert!(ClipRect::new(100.0, 100.0, 50.0, 50.0).is_empty());
    }

    #[test]
    fn non_empty_rect() {
        assert!(!ClipRect::new(0.0, 0.0, 1.0, 1.0).is_empty());
    }

    #[test]
    fn infinite_is_not_empty() {
        assert!(!ClipRect::INFINITE.is_empty());
    }

    // --- ClipRect contains_point tests ---

    #[test]
    fn contains_point_inside() {
        let r = ClipRect::new(0.0, 0.0, 100.0, 100.0);
        assert!(r.contains_point(50.0, 50.0));
    }

    #[test]
    fn contains_point_on_edge() {
        let r = ClipRect::new(0.0, 0.0, 100.0, 100.0);
        assert!(r.contains_point(0.0, 0.0));
        assert!(r.contains_point(100.0, 100.0));
    }

    #[test]
    fn contains_point_outside() {
        let r = ClipRect::new(0.0, 0.0, 100.0, 100.0);
        assert!(!r.contains_point(101.0, 50.0));
        assert!(!r.contains_point(-1.0, 50.0));
    }

    // --- ClipRect to_scissor_rect tests ---

    #[test]
    fn to_scissor_rect_normal() {
        let r = ClipRect::new(10.0, 20.0, 110.0, 120.0);
        let s = r.to_scissor_rect(1920, 1080);
        assert_eq!(s, Some((10, 20, 100, 100)));
    }

    #[test]
    fn to_scissor_rect_clamped_to_viewport() {
        let r = ClipRect::new(-10.0, -10.0, 2000.0, 1200.0);
        let s = r.to_scissor_rect(1920, 1080);
        assert_eq!(s, Some((0, 0, 1920, 1080)));
    }

    #[test]
    fn to_scissor_rect_fully_outside() {
        let r = ClipRect::new(-100.0, -100.0, -10.0, -10.0);
        let s = r.to_scissor_rect(1920, 1080);
        assert_eq!(s, None);
    }

    #[test]
    fn to_scissor_rect_zero_size() {
        let r = ClipRect::new(50.0, 50.0, 50.0, 50.0);
        let s = r.to_scissor_rect(1920, 1080);
        assert_eq!(s, None);
    }

    // --- ClipRect to_array ---

    #[test]
    fn to_array_matches_fields() {
        let r = ClipRect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(r.to_array(), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn infinite_to_array() {
        let arr = ClipRect::INFINITE.to_array();
        assert_eq!(arr[0], f32::NEG_INFINITY);
        assert_eq!(arr[1], f32::NEG_INFINITY);
        assert_eq!(arr[2], f32::INFINITY);
        assert_eq!(arr[3], f32::INFINITY);
    }
}
