//! 2D affine transform (3x2 matrix) for rotation, scale, skew, and translation.
//!
//! The transform is stored as a row-major 3x2 matrix with an implicit third row `[0, 0, 1]`:
//!
//! ```text
//! | m[0]  m[1]  m[4] |     | a  b  tx |
//! | m[2]  m[3]  m[5] |  =  | c  d  ty |
//! | 0     0     1    |     | 0  0  1  |
//! ```
//!
//! This representation uses 6 floats (24 bytes), saving 12 bytes over a full `mat3x3`.

use bytemuck::{Pod, Zeroable};

use super::node::BoundingBox;

/// A 2D affine transform represented as a 3x2 matrix (6 floats).
///
/// The matrix maps a point `(x, y)` as follows:
/// ```text
/// x' = a * x + b * y + tx
/// y' = c * x + d * y + ty
/// ```
///
/// Where `[a, b, c, d, tx, ty]` = `[m[0], m[1], m[2], m[3], m[4], m[5]]`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform2D {
    /// `[a, b, c, d, tx, ty]` — row-major affine matrix coefficients.
    m: [f32; 6],
}

impl Transform2D {
    /// The identity transform (no rotation, scale, or translation).
    pub const IDENTITY: Self = Self {
        m: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
    };

    /// Creates an identity transform.
    #[must_use]
    pub const fn identity() -> Self {
        Self::IDENTITY
    }

    /// Creates a transform from raw matrix coefficients `[a, b, c, d, tx, ty]`.
    #[must_use]
    pub const fn from_raw(m: [f32; 6]) -> Self {
        Self { m }
    }

    /// Returns the raw matrix coefficients `[a, b, c, d, tx, ty]`.
    #[must_use]
    pub const fn raw(&self) -> &[f32; 6] {
        &self.m
    }

    /// Creates a pure translation transform.
    #[must_use]
    pub fn translation(tx: f32, ty: f32) -> Self {
        Self {
            m: [1.0, 0.0, 0.0, 1.0, tx, ty],
        }
    }

    /// Creates a pure rotation transform (counter-clockwise, in radians).
    #[must_use]
    pub fn rotation(angle_rad: f32) -> Self {
        let (sin, cos) = angle_rad.sin_cos();
        Self {
            m: [cos, -sin, sin, cos, 0.0, 0.0],
        }
    }

    /// Creates a pure scale transform.
    #[must_use]
    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            m: [sx, 0.0, 0.0, sy, 0.0, 0.0],
        }
    }

    /// Creates a pure skew transform (angles in radians).
    ///
    /// `skew_x` shears the X axis (horizontal), `skew_y` shears the Y axis.
    #[must_use]
    pub fn skew(skew_x: f32, skew_y: f32) -> Self {
        Self {
            m: [1.0, skew_x.tan(), skew_y.tan(), 1.0, 0.0, 0.0],
        }
    }

    /// Creates a rotation around a specific center point.
    ///
    /// Equivalent to `translate(cx, cy) * rotate(angle) * translate(-cx, -cy)`.
    #[must_use]
    pub fn from_rotation_around(angle_rad: f32, cx: f32, cy: f32) -> Self {
        let translate_to_origin = Self::translation(-cx, -cy);
        let rotate = Self::rotation(angle_rad);
        let translate_back = Self::translation(cx, cy);
        translate_back
            .compose(&rotate)
            .compose(&translate_to_origin)
    }

    /// Creates a scale around a specific center point.
    ///
    /// Equivalent to `translate(cx, cy) * scale(sx, sy) * translate(-cx, -cy)`.
    #[must_use]
    pub fn from_scale_around(sx: f32, sy: f32, cx: f32, cy: f32) -> Self {
        let translate_to_origin = Self::translation(-cx, -cy);
        let scale = Self::scale(sx, sy);
        let translate_back = Self::translation(cx, cy);
        translate_back.compose(&scale).compose(&translate_to_origin)
    }

    /// Composes this transform with another: `self * other`.
    ///
    /// The result applies `other` first, then `self`.
    #[must_use]
    pub fn compose(&self, other: &Self) -> Self {
        let a = &self.m;
        let b = &other.m;
        Self {
            m: [
                a[0] * b[0] + a[1] * b[2],
                a[0] * b[1] + a[1] * b[3],
                a[2] * b[0] + a[3] * b[2],
                a[2] * b[1] + a[3] * b[3],
                a[0] * b[4] + a[1] * b[5] + a[4],
                a[2] * b[4] + a[3] * b[5] + a[5],
            ],
        }
    }

    /// Returns the determinant of the linear part of the transform.
    #[must_use]
    pub fn determinant(&self) -> f32 {
        self.m[0] * self.m[3] - self.m[1] * self.m[2]
    }

    /// Computes the inverse transform, or `None` if the matrix is singular.
    ///
    /// A singular matrix has a determinant at or near zero, meaning the transform
    /// collapses the 2D plane to a line or point (e.g., `scale(0, 0)`).
    #[must_use]
    pub fn inverse(&self) -> Option<Self> {
        let det = self.determinant();
        if det.abs() < 1e-10 {
            return None;
        }
        let inv_det = 1.0 / det;
        let a = self.m[0];
        let b = self.m[1];
        let c = self.m[2];
        let d = self.m[3];
        let tx = self.m[4];
        let ty = self.m[5];
        Some(Self {
            m: [
                d * inv_det,
                -b * inv_det,
                -c * inv_det,
                a * inv_det,
                (b * ty - d * tx) * inv_det,
                (c * tx - a * ty) * inv_det,
            ],
        })
    }

    /// Transforms a point `(x, y)` through this affine matrix.
    #[must_use]
    pub fn transform_point(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.m[0] * x + self.m[1] * y + self.m[4],
            self.m[2] * x + self.m[3] * y + self.m[5],
        )
    }

    /// Transforms a direction vector `(dx, dy)` through the linear part only (no translation).
    #[must_use]
    pub fn transform_vector(&self, dx: f32, dy: f32) -> (f32, f32) {
        (
            self.m[0] * dx + self.m[1] * dy,
            self.m[2] * dx + self.m[3] * dy,
        )
    }

    /// Transforms an axis-aligned bounding box, returning the enclosing AABB.
    ///
    /// Transforms all 4 corners and computes the min/max enclosing rectangle.
    /// The result is always axis-aligned (it may be larger than the transformed shape).
    #[must_use]
    pub fn transform_aabb(&self, bb: &BoundingBox) -> BoundingBox {
        let x0 = bb.x;
        let y0 = bb.y;
        let x1 = bb.x + bb.width;
        let y1 = bb.y + bb.height;

        let (p0x, p0y) = self.transform_point(x0, y0);
        let (p1x, p1y) = self.transform_point(x1, y0);
        let (p2x, p2y) = self.transform_point(x0, y1);
        let (p3x, p3y) = self.transform_point(x1, y1);

        let min_x = p0x.min(p1x).min(p2x).min(p3x);
        let min_y = p0y.min(p1y).min(p2y).min(p3y);
        let max_x = p0x.max(p1x).max(p2x).max(p3x);
        let max_y = p0y.max(p1y).max(p2y).max(p3y);

        BoundingBox::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// Returns `true` if this is the identity transform (within floating-point tolerance).
    #[must_use]
    pub fn is_identity(&self) -> bool {
        let id = Self::IDENTITY.m;
        self.m
            .iter()
            .zip(id.iter())
            .all(|(a, b)| (a - b).abs() < 1e-7)
    }

    /// Converts the transform to column-major GPU format for three `vec2<f32>` vertex attributes.
    ///
    /// Returns `[a, c, b, d, tx, ty]` as a `TransformColumns` — column-major order:
    /// - Column 0 (`c0`): `[a, c]` — how the +X basis vector maps
    /// - Column 1 (`c1`): `[b, d]` — how the +Y basis vector maps
    /// - Column 2 (`c2`): `[tx, ty]` — translation
    #[must_use]
    pub fn to_gpu_columns(&self) -> TransformColumns {
        TransformColumns {
            c0: [self.m[0], self.m[2]],
            c1: [self.m[1], self.m[3]],
            c2: [self.m[4], self.m[5]],
        }
    }
}

impl Default for Transform2D {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Column-major transform data for GPU vertex attributes.
///
/// Three `vec2<f32>` columns representing a 2D affine transform.
/// Embed this in instance structs to pass transforms to the vertex shader.
///
/// Layout:
/// - `c0 = [a, c]` — how +X maps
/// - `c1 = [b, d]` — how +Y maps
/// - `c2 = [tx, ty]` — translation
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct TransformColumns {
    /// Column 0: `[a, c]` — X basis vector.
    pub c0: [f32; 2],
    /// Column 1: `[b, d]` — Y basis vector.
    pub c1: [f32; 2],
    /// Column 2: `[tx, ty]` — Translation.
    pub c2: [f32; 2],
}

impl TransformColumns {
    /// Identity transform columns: `[1,0], [0,1], [0,0]`.
    pub const IDENTITY: Self = Self {
        c0: [1.0, 0.0],
        c1: [0.0, 1.0],
        c2: [0.0, 0.0],
    };

    /// Returns identity transform columns.
    #[must_use]
    pub const fn identity() -> Self {
        Self::IDENTITY
    }

    /// Creates transform columns from a `Transform2D`.
    #[must_use]
    pub fn from_transform(t: &Transform2D) -> Self {
        t.to_gpu_columns()
    }

    /// Returns `wgpu::VertexAttribute` descriptors for the three transform columns.
    ///
    /// The attributes are placed at `base_location`, `base_location + 1`, `base_location + 2`,
    /// with byte offsets starting at `base_offset`.
    #[must_use]
    pub const fn vertex_attributes(
        base_location: u32,
        base_offset: u64,
    ) -> [wgpu::VertexAttribute; 3] {
        [
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: base_offset,
                shader_location: base_location,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: base_offset + 8,
                shader_location: base_location + 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: base_offset + 16,
                shader_location: base_location + 2,
            },
        ]
    }
}

impl Default for TransformColumns {
    fn default() -> Self {
        Self::IDENTITY
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used, clippy::uninlined_format_args)]
mod tests {
    use super::*;
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    fn transforms_approx_eq(a: &Transform2D, b: &Transform2D) -> bool {
        a.m.iter().zip(b.m.iter()).all(|(x, y)| approx_eq(*x, *y))
    }

    // --- Identity ---

    #[test]
    fn identity_is_identity() {
        let t = Transform2D::identity();
        assert!(t.is_identity());
    }

    #[test]
    fn identity_default() {
        assert_eq!(Transform2D::default(), Transform2D::identity());
    }

    #[test]
    fn identity_transform_point() {
        let t = Transform2D::identity();
        let (x, y) = t.transform_point(42.0, 17.0);
        assert_eq!(x, 42.0);
        assert_eq!(y, 17.0);
    }

    #[test]
    fn identity_determinant_is_one() {
        assert!(approx_eq(Transform2D::identity().determinant(), 1.0));
    }

    // --- Translation ---

    #[test]
    fn translation_moves_point() {
        let t = Transform2D::translation(10.0, 20.0);
        let (x, y) = t.transform_point(5.0, 3.0);
        assert!(approx_eq(x, 15.0));
        assert!(approx_eq(y, 23.0));
    }

    #[test]
    fn translation_does_not_affect_vector() {
        let t = Transform2D::translation(100.0, 200.0);
        let (dx, dy) = t.transform_vector(5.0, 3.0);
        assert!(approx_eq(dx, 5.0));
        assert!(approx_eq(dy, 3.0));
    }

    #[test]
    fn translation_determinant_is_one() {
        assert!(approx_eq(
            Transform2D::translation(100.0, 200.0).determinant(),
            1.0
        ));
    }

    // --- Rotation ---

    #[test]
    fn rotation_90_degrees() {
        let t = Transform2D::rotation(FRAC_PI_2);
        let (x, y) = t.transform_point(1.0, 0.0);
        assert!(approx_eq(x, 0.0));
        assert!(approx_eq(y, 1.0));
    }

    #[test]
    fn rotation_180_degrees() {
        let t = Transform2D::rotation(PI);
        let (x, y) = t.transform_point(1.0, 0.0);
        assert!(approx_eq(x, -1.0));
        assert!(approx_eq(y, 0.0));
    }

    #[test]
    fn rotation_preserves_distance() {
        let t = Transform2D::rotation(FRAC_PI_4);
        let (x, y) = t.transform_point(3.0, 4.0);
        let original_dist = (3.0_f32 * 3.0 + 4.0 * 4.0).sqrt();
        let transformed_dist = (x * x + y * y).sqrt();
        assert!(approx_eq(original_dist, transformed_dist));
    }

    #[test]
    fn rotation_determinant_is_one() {
        assert!(approx_eq(
            Transform2D::rotation(FRAC_PI_4).determinant(),
            1.0
        ));
    }

    // --- Scale ---

    #[test]
    fn scale_multiplies_point() {
        let t = Transform2D::scale(2.0, 3.0);
        let (x, y) = t.transform_point(5.0, 7.0);
        assert!(approx_eq(x, 10.0));
        assert!(approx_eq(y, 21.0));
    }

    #[test]
    fn uniform_scale_determinant() {
        assert!(approx_eq(Transform2D::scale(3.0, 3.0).determinant(), 9.0));
    }

    #[test]
    fn non_uniform_scale_determinant() {
        assert!(approx_eq(Transform2D::scale(2.0, 5.0).determinant(), 10.0));
    }

    #[test]
    fn zero_scale_determinant_is_zero() {
        assert!(approx_eq(Transform2D::scale(0.0, 1.0).determinant(), 0.0));
    }

    // --- Skew ---

    #[test]
    fn skew_x_shears_horizontally() {
        let t = Transform2D::skew(FRAC_PI_4, 0.0);
        let (x, y) = t.transform_point(0.0, 1.0);
        assert!(approx_eq(x, 1.0)); // tan(45) = 1.0
        assert!(approx_eq(y, 1.0));
    }

    // --- from_rotation_around ---

    #[test]
    fn rotation_around_center_preserves_center() {
        let cx = 50.0;
        let cy = 50.0;
        let t = Transform2D::from_rotation_around(FRAC_PI_4, cx, cy);
        let (x, y) = t.transform_point(cx, cy);
        assert!(approx_eq(x, cx));
        assert!(approx_eq(y, cy));
    }

    #[test]
    fn rotation_around_center_rotates_corner() {
        let cx = 50.0;
        let cy = 50.0;
        let t = Transform2D::from_rotation_around(FRAC_PI_2, cx, cy);
        // Point at (100, 50) — 50 units right of center.
        // After 90-degree rotation, should be at (50, 100) — 50 units below center.
        let (x, y) = t.transform_point(100.0, 50.0);
        assert!(approx_eq(x, 50.0));
        assert!(approx_eq(y, 100.0));
    }

    // --- from_scale_around ---

    #[test]
    fn scale_around_center_preserves_center() {
        let cx = 50.0;
        let cy = 50.0;
        let t = Transform2D::from_scale_around(2.0, 2.0, cx, cy);
        let (x, y) = t.transform_point(cx, cy);
        assert!(approx_eq(x, cx));
        assert!(approx_eq(y, cy));
    }

    #[test]
    fn scale_around_center_scales_point() {
        let cx = 50.0;
        let cy = 50.0;
        let t = Transform2D::from_scale_around(2.0, 3.0, cx, cy);
        // Point at (70, 60) is 20 right, 10 below center.
        // After scale(2,3): 40 right, 30 below center → (90, 80).
        let (x, y) = t.transform_point(70.0, 60.0);
        assert!(approx_eq(x, 90.0));
        assert!(approx_eq(y, 80.0));
    }

    // --- Composition ---

    #[test]
    fn compose_identity_left() {
        let t = Transform2D::translation(10.0, 20.0);
        let composed = Transform2D::identity().compose(&t);
        assert!(transforms_approx_eq(&composed, &t));
    }

    #[test]
    fn compose_identity_right() {
        let t = Transform2D::translation(10.0, 20.0);
        let composed = t.compose(&Transform2D::identity());
        assert!(transforms_approx_eq(&composed, &t));
    }

    #[test]
    fn compose_translation_accumulates() {
        let t1 = Transform2D::translation(10.0, 0.0);
        let t2 = Transform2D::translation(0.0, 20.0);
        let composed = t1.compose(&t2);
        let (x, y) = composed.transform_point(0.0, 0.0);
        assert!(approx_eq(x, 10.0));
        assert!(approx_eq(y, 20.0));
    }

    #[test]
    fn compose_scale_then_translate() {
        // Scale first, then translate: T * S
        // Point (1,1) → scale(2,2) → (2,2) → translate(10,10) → (12,12)
        let t = Transform2D::translation(10.0, 10.0);
        let s = Transform2D::scale(2.0, 2.0);
        let composed = t.compose(&s);
        let (x, y) = composed.transform_point(1.0, 1.0);
        assert!(approx_eq(x, 12.0));
        assert!(approx_eq(y, 12.0));
    }

    #[test]
    fn compose_translate_then_scale() {
        // Translate first, then scale: S * T
        // Point (1,1) → translate(10,10) → (11,11) → scale(2,2) → (22,22)
        let t = Transform2D::translation(10.0, 10.0);
        let s = Transform2D::scale(2.0, 2.0);
        let composed = s.compose(&t);
        let (x, y) = composed.transform_point(1.0, 1.0);
        assert!(approx_eq(x, 22.0));
        assert!(approx_eq(y, 22.0));
    }

    // --- Inverse ---

    #[test]
    fn inverse_of_identity_is_identity() {
        let inv = Transform2D::identity().inverse().unwrap();
        assert!(inv.is_identity());
    }

    #[test]
    fn inverse_of_translation() {
        let t = Transform2D::translation(10.0, 20.0);
        let inv = t.inverse().unwrap();
        let (x, y) = inv.transform_point(10.0, 20.0);
        assert!(approx_eq(x, 0.0));
        assert!(approx_eq(y, 0.0));
    }

    #[test]
    fn inverse_of_scale() {
        let t = Transform2D::scale(2.0, 4.0);
        let inv = t.inverse().unwrap();
        let (x, y) = inv.transform_point(10.0, 20.0);
        assert!(approx_eq(x, 5.0));
        assert!(approx_eq(y, 5.0));
    }

    #[test]
    fn inverse_of_rotation() {
        let t = Transform2D::rotation(FRAC_PI_4);
        let inv = t.inverse().unwrap();
        let composed = t.compose(&inv);
        assert!(composed.is_identity());
    }

    #[test]
    fn inverse_round_trip() {
        let t = Transform2D::translation(30.0, 40.0)
            .compose(&Transform2D::rotation(0.7))
            .compose(&Transform2D::scale(2.0, 0.5));
        let inv = t.inverse().unwrap();
        let round_trip = t.compose(&inv);
        assert!(
            round_trip.is_identity(),
            "round trip should be identity, got {round_trip:?}"
        );
    }

    #[test]
    fn inverse_of_zero_scale_returns_none() {
        let t = Transform2D::scale(0.0, 1.0);
        assert!(t.inverse().is_none());
    }

    #[test]
    fn inverse_of_singular_returns_none() {
        // Matrix where both rows are parallel (determinant = 0).
        let t = Transform2D::from_raw([2.0, 4.0, 1.0, 2.0, 0.0, 0.0]);
        assert!(t.inverse().is_none());
    }

    #[test]
    fn inverse_of_zero_zero_scale_returns_none() {
        let t = Transform2D::scale(0.0, 0.0);
        assert!(t.inverse().is_none());
    }

    // --- transform_aabb ---

    #[test]
    fn transform_aabb_identity() {
        let bb = BoundingBox::new(10.0, 20.0, 100.0, 50.0);
        let result = Transform2D::identity().transform_aabb(&bb);
        assert!(approx_eq(result.x, 10.0));
        assert!(approx_eq(result.y, 20.0));
        assert!(approx_eq(result.width, 100.0));
        assert!(approx_eq(result.height, 50.0));
    }

    #[test]
    fn transform_aabb_translation() {
        let bb = BoundingBox::new(0.0, 0.0, 100.0, 50.0);
        let result = Transform2D::translation(10.0, 20.0).transform_aabb(&bb);
        assert!(approx_eq(result.x, 10.0));
        assert!(approx_eq(result.y, 20.0));
        assert!(approx_eq(result.width, 100.0));
        assert!(approx_eq(result.height, 50.0));
    }

    #[test]
    fn transform_aabb_scale() {
        let bb = BoundingBox::new(10.0, 10.0, 100.0, 50.0);
        let result = Transform2D::scale(2.0, 3.0).transform_aabb(&bb);
        assert!(approx_eq(result.x, 20.0));
        assert!(approx_eq(result.y, 30.0));
        assert!(approx_eq(result.width, 200.0));
        assert!(approx_eq(result.height, 150.0));
    }

    #[test]
    fn transform_aabb_rotation_90_expands() {
        // A 100x50 rect rotated 90 degrees becomes 50x100.
        let bb = BoundingBox::new(0.0, 0.0, 100.0, 50.0);
        let result = Transform2D::rotation(FRAC_PI_2).transform_aabb(&bb);
        assert!(approx_eq(result.width, 50.0));
        assert!(approx_eq(result.height, 100.0));
    }

    #[test]
    fn transform_aabb_empty_stays_empty() {
        let bb = BoundingBox::new(10.0, 20.0, 0.0, 0.0);
        let result = Transform2D::rotation(FRAC_PI_4).transform_aabb(&bb);
        assert!(approx_eq(result.width, 0.0));
        assert!(approx_eq(result.height, 0.0));
    }

    // --- to_gpu_columns ---

    #[test]
    fn identity_gpu_columns() {
        let cols = Transform2D::identity().to_gpu_columns();
        assert_eq!(cols.c0, [1.0, 0.0]);
        assert_eq!(cols.c1, [0.0, 1.0]);
        assert_eq!(cols.c2, [0.0, 0.0]);
    }

    #[test]
    fn translation_gpu_columns() {
        let cols = Transform2D::translation(10.0, 20.0).to_gpu_columns();
        assert_eq!(cols.c0, [1.0, 0.0]);
        assert_eq!(cols.c1, [0.0, 1.0]);
        assert_eq!(cols.c2, [10.0, 20.0]);
    }

    #[test]
    fn scale_gpu_columns() {
        let cols = Transform2D::scale(2.0, 3.0).to_gpu_columns();
        assert_eq!(cols.c0, [2.0, 0.0]);
        assert_eq!(cols.c1, [0.0, 3.0]);
        assert_eq!(cols.c2, [0.0, 0.0]);
    }

    // --- TransformColumns ---

    #[test]
    fn transform_columns_size_is_24_bytes() {
        assert_eq!(std::mem::size_of::<TransformColumns>(), 24);
    }

    #[test]
    fn transform_columns_is_pod() {
        let _zeroed: TransformColumns = bytemuck::Zeroable::zeroed();
    }

    #[test]
    fn transform_columns_identity() {
        let cols = TransformColumns::identity();
        assert_eq!(cols.c0, [1.0, 0.0]);
        assert_eq!(cols.c1, [0.0, 1.0]);
        assert_eq!(cols.c2, [0.0, 0.0]);
    }

    #[test]
    fn transform_columns_from_transform() {
        let t = Transform2D::translation(5.0, 10.0);
        let cols = TransformColumns::from_transform(&t);
        assert_eq!(cols, t.to_gpu_columns());
    }

    #[test]
    fn transform_columns_default_is_identity() {
        assert_eq!(TransformColumns::default(), TransformColumns::IDENTITY);
    }

    // --- is_identity edge cases ---

    #[test]
    fn non_identity_not_identity() {
        let t = Transform2D::translation(0.001, 0.0);
        assert!(!t.is_identity());
    }

    #[test]
    fn near_identity_within_tolerance() {
        let t = Transform2D::from_raw([1.0, 0.0, 0.0, 1.0, 1e-8, 1e-8]);
        assert!(t.is_identity());
    }

    // --- from_raw ---

    #[test]
    fn from_raw_round_trip() {
        let raw = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let t = Transform2D::from_raw(raw);
        assert_eq!(*t.raw(), raw);
    }

    // --- Property-based tests ---

    #[allow(clippy::expect_used)]
    mod proptests {
        use super::*;
        use proptest::prelude::*;

        fn arb_transform() -> impl Strategy<Value = Transform2D> {
            (
                -100.0_f32..100.0,
                -100.0_f32..100.0,
                -100.0_f32..100.0,
                -100.0_f32..100.0,
                -1000.0_f32..1000.0,
                -1000.0_f32..1000.0,
            )
                .prop_map(|(a, b, c, d, tx, ty)| Transform2D::from_raw([a, b, c, d, tx, ty]))
        }

        fn arb_invertible_transform() -> impl Strategy<Value = Transform2D> {
            // Use rotation + scale + translation to guarantee invertibility.
            (
                -PI..PI,
                0.1_f32..10.0,
                0.1_f32..10.0,
                -1000.0_f32..1000.0,
                -1000.0_f32..1000.0,
            )
                .prop_map(|(angle, sx, sy, tx, ty)| {
                    Transform2D::translation(tx, ty)
                        .compose(&Transform2D::rotation(angle))
                        .compose(&Transform2D::scale(sx, sy))
                })
        }

        fn arb_bounding_box() -> impl Strategy<Value = BoundingBox> {
            (
                0.0_f32..1000.0,
                0.0_f32..1000.0,
                1.0_f32..500.0,
                1.0_f32..500.0,
            )
                .prop_map(|(x, y, w, h)| BoundingBox::new(x, y, w, h))
        }

        proptest! {
            #![proptest_config(ProptestConfig::with_cases(100))]

            #[test]
            fn identity_compose_is_noop(t in arb_transform()) {
                let composed = Transform2D::identity().compose(&t);
                for (a, b) in composed.m.iter().zip(t.m.iter()) {
                    prop_assert!((a - b).abs() < 1e-4, "identity.compose(t) != t");
                }
            }

            #[test]
            fn inverse_round_trip_property(t in arb_invertible_transform()) {
                let inv = t.inverse().expect("invertible transform should have inverse");
                let round_trip = t.compose(&inv);
                let id = Transform2D::IDENTITY;
                for (a, b) in round_trip.m.iter().zip(id.m.iter()) {
                    prop_assert!(
                        (a - b).abs() < 1e-3,
                        "t.compose(t.inverse()) should be identity, got {:?}",
                        round_trip.m
                    );
                }
            }

            #[test]
            fn transform_aabb_contains_all_corners(
                bb in arb_bounding_box(),
                t in arb_invertible_transform(),
            ) {
                let aabb = t.transform_aabb(&bb);

                // All 4 transformed corners should be inside the enclosing AABB.
                let corners = [
                    (bb.x, bb.y),
                    (bb.x + bb.width, bb.y),
                    (bb.x, bb.y + bb.height),
                    (bb.x + bb.width, bb.y + bb.height),
                ];
                for (cx, cy) in corners {
                    let (tx, ty) = t.transform_point(cx, cy);
                    // Use a small tolerance for floating-point.
                    prop_assert!(
                        tx >= aabb.x - 1e-3
                            && tx <= aabb.x + aabb.width + 1e-3
                            && ty >= aabb.y - 1e-3
                            && ty <= aabb.y + aabb.height + 1e-3,
                        "Transformed corner ({tx}, {ty}) outside AABB {aabb:?}"
                    );
                }
            }

            #[test]
            fn composition_is_associative(
                a in arb_invertible_transform(),
                b in arb_invertible_transform(),
                c in arb_invertible_transform(),
            ) {
                let ab_c = a.compose(&b).compose(&c);
                let a_bc = a.compose(&b.compose(&c));
                for (x, y) in ab_c.m.iter().zip(a_bc.m.iter()) {
                    let tol = 1e-2 + 1e-4 * x.abs().max(y.abs());
                    prop_assert!(
                        (x - y).abs() < tol,
                        "(a*b)*c != a*(b*c): {:?} vs {:?}",
                        ab_c.m,
                        a_bc.m
                    );
                }
            }
        }
    }
}
