//! Camera and viewport management for the 2D canvas.
//!
//! The camera defines an orthographic projection from world-space coordinates
//! (where design nodes live) to clip space (what the GPU renders). It supports
//! pan (translation) and zoom (scale) operations.

use bytemuck::{Pod, Zeroable};

/// The minimum allowed zoom level (10% zoom).
const MIN_ZOOM: f32 = 0.1;
/// The maximum allowed zoom level (100x zoom).
const MAX_ZOOM: f32 = 100.0;

/// GPU-compatible uniform buffer data for the camera transform.
///
/// Contains a 4x4 orthographic projection matrix in column-major order,
/// ready to be uploaded to a uniform buffer and used in shaders.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    /// 4x4 view-projection matrix in column-major order.
    ///
    /// Transforms world-space coordinates to clip space:
    /// `clip_pos = view_proj * world_pos`
    pub view_proj: [f32; 16],
}

impl CameraUniform {
    /// Returns the size of this struct in bytes, for buffer allocation.
    #[must_use]
    pub const fn size() -> u64 {
        std::mem::size_of::<Self>() as u64
    }
}

/// 2D camera state for the design canvas.
///
/// The camera maps a rectangular region of world space (the viewport) to the
/// screen. Pan shifts the viewport center. Zoom changes the visible area size.
///
/// Coordinate system:
/// - World space: origin at (0, 0), y-axis points down (screen convention).
/// - Clip space: x in [-1, 1], y in [-1, 1], with (-1, -1) at top-left.
#[derive(Debug, Clone)]
pub struct Camera {
    /// X coordinate of the viewport center in world space.
    pan_x: f32,
    /// Y coordinate of the viewport center in world space.
    pan_y: f32,
    /// Zoom level. 1.0 = 100%, 2.0 = 200% (zoomed in), 0.5 = 50% (zoomed out).
    zoom: f32,
    /// Viewport width in physical pixels (screen resolution).
    viewport_width: f32,
    /// Viewport height in physical pixels (screen resolution).
    viewport_height: f32,
}

impl Camera {
    /// Creates a new camera centered at the origin with 1:1 zoom.
    ///
    /// # Arguments
    /// * `viewport_width` — Width of the render target in physical pixels.
    /// * `viewport_height` — Height of the render target in physical pixels.
    ///
    /// Both dimensions are clamped to a minimum of 1.0 to prevent
    /// degenerate projection matrices.
    #[must_use]
    pub fn new(viewport_width: f32, viewport_height: f32) -> Self {
        Self {
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            viewport_width: viewport_width.max(1.0),
            viewport_height: viewport_height.max(1.0),
        }
    }

    /// Updates the viewport dimensions (e.g., on window resize).
    ///
    /// Dimensions are clamped to a minimum of 1.0.
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        self.viewport_width = width.max(1.0);
        self.viewport_height = height.max(1.0);
    }

    /// Sets the pan offset (viewport center in world space).
    pub fn set_pan(&mut self, x: f32, y: f32) {
        self.pan_x = x;
        self.pan_y = y;
    }

    /// Translates the viewport by the given delta in world-space units.
    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    /// Sets the zoom level, clamped to [`MIN_ZOOM`, `MAX_ZOOM`].
    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    }

    /// Multiplies the current zoom by a factor, clamped to [`MIN_ZOOM`, `MAX_ZOOM`].
    ///
    /// Use values > 1.0 to zoom in, < 1.0 to zoom out.
    pub fn zoom_by(&mut self, factor: f32) {
        self.set_zoom(self.zoom * factor);
    }

    /// Returns the current zoom level.
    #[must_use]
    pub fn zoom(&self) -> f32 {
        self.zoom
    }

    /// Returns the current pan position (viewport center in world space).
    #[must_use]
    pub fn pan(&self) -> (f32, f32) {
        (self.pan_x, self.pan_y)
    }

    /// Returns the viewport dimensions in physical pixels.
    #[must_use]
    pub fn viewport_size(&self) -> (f32, f32) {
        (self.viewport_width, self.viewport_height)
    }

    /// Returns the visible world-space rectangle as `(left, top, right, bottom)`.
    ///
    /// This is the axis-aligned bounding box of the viewport in world coordinates,
    /// used for viewport culling (R-tree queries).
    #[must_use]
    pub fn visible_rect(&self) -> (f32, f32, f32, f32) {
        let half_w = self.viewport_width / (2.0 * self.zoom);
        let half_h = self.viewport_height / (2.0 * self.zoom);
        (
            self.pan_x - half_w,
            self.pan_y - half_h,
            self.pan_x + half_w,
            self.pan_y + half_h,
        )
    }

    /// Converts screen-space pixel coordinates to world-space coordinates.
    ///
    /// Screen origin is top-left `(0, 0)`, with `(viewport_width, viewport_height)`
    /// at the bottom-right. The viewport is centered on `(pan_x, pan_y)` in world space.
    #[must_use]
    pub fn screen_to_world(&self, screen_x: f32, screen_y: f32) -> (f32, f32) {
        let world_x = (screen_x - self.viewport_width / 2.0) / self.zoom + self.pan_x;
        let world_y = (screen_y - self.viewport_height / 2.0) / self.zoom + self.pan_y;
        (world_x, world_y)
    }

    /// Converts world-space coordinates to screen-space pixel coordinates.
    #[must_use]
    pub fn world_to_screen(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let screen_x = (world_x - self.pan_x) * self.zoom + self.viewport_width / 2.0;
        let screen_y = (world_y - self.pan_y) * self.zoom + self.viewport_height / 2.0;
        (screen_x, screen_y)
    }

    /// Zooms by `factor` while keeping the given screen-space point stationary.
    ///
    /// Adjusts pan so that the world point under `(screen_x, screen_y)` remains
    /// fixed after the zoom change.
    pub fn zoom_at(&mut self, factor: f32, screen_x: f32, screen_y: f32) {
        let (world_x, world_y) = self.screen_to_world(screen_x, screen_y);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        // Solve for new pan so screen_to_world(screen_x, screen_y) == (world_x, world_y).
        self.pan_x = world_x - (screen_x - self.viewport_width / 2.0) / self.zoom;
        self.pan_y = world_y - (screen_y - self.viewport_height / 2.0) / self.zoom;
    }

    /// Builds the GPU-compatible uniform data for this camera.
    ///
    /// The resulting matrix maps world-space coordinates to clip space using
    /// an orthographic projection:
    ///
    /// ```text
    /// | 2*zoom/w   0         0   -(2*zoom*pan_x)/w |
    /// | 0         -2*zoom/h  0    (2*zoom*pan_y)/h |
    /// | 0          0         1    0                 |
    /// | 0          0         0    1                 |
    /// ```
    ///
    /// The y-axis is flipped (negative scale) so that world-space y-down
    /// maps to clip-space y-up (GPU convention).
    #[must_use]
    pub fn build_uniform(&self) -> CameraUniform {
        let w = self.viewport_width;
        let h = self.viewport_height;
        let z = self.zoom;

        // Orthographic projection: maps viewport-sized region to [-1, 1] clip space.
        // The viewport is centered on (pan_x, pan_y) in world space.
        let sx = 2.0 * z / w;
        let sy = -2.0 * z / h; // Negative to flip y-axis (world y-down → clip y-up)
        let tx = -sx * self.pan_x;
        let ty = -sy * self.pan_y;

        // Column-major 4x4 matrix (wgpu/WebGPU convention).
        #[rustfmt::skip]
        let view_proj = [
            sx,  0.0, 0.0, 0.0, // column 0
            0.0, sy,  0.0, 0.0, // column 1
            0.0, 0.0, 1.0, 0.0, // column 2
            tx,  ty,  0.0, 1.0, // column 3
        ];

        CameraUniform { view_proj }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::float_cmp)] // Exact float comparisons are intentional with known values.

    use super::*;

    #[test]
    fn new_camera_defaults() {
        let cam = Camera::new(800.0, 600.0);
        assert_eq!(cam.pan(), (0.0, 0.0));
        assert_eq!(cam.zoom(), 1.0);
        assert_eq!(cam.viewport_size(), (800.0, 600.0));
    }

    #[test]
    fn viewport_size_clamped_to_minimum() {
        let cam = Camera::new(0.0, -100.0);
        assert_eq!(cam.viewport_size(), (1.0, 1.0));
    }

    #[test]
    fn set_viewport_size_clamps() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_viewport_size(-5.0, 0.0);
        assert_eq!(cam.viewport_size(), (1.0, 1.0));
    }

    #[test]
    fn pan_by_accumulates() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.pan_by(10.0, 20.0);
        cam.pan_by(5.0, -3.0);
        assert_eq!(cam.pan(), (15.0, 17.0));
    }

    #[test]
    fn set_pan_replaces() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.pan_by(100.0, 100.0);
        cam.set_pan(5.0, 10.0);
        assert_eq!(cam.pan(), (5.0, 10.0));
    }

    #[test]
    fn zoom_clamped_to_min() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(0.01);
        assert_eq!(cam.zoom(), MIN_ZOOM);
    }

    #[test]
    fn zoom_clamped_to_max() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(500.0);
        assert_eq!(cam.zoom(), MAX_ZOOM);
    }

    #[test]
    fn zoom_by_multiplies() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(2.0);
        cam.zoom_by(1.5);
        assert_eq!(cam.zoom(), 3.0);
    }

    #[test]
    fn zoom_by_clamps() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(MAX_ZOOM);
        cam.zoom_by(2.0); // Would be 200.0, clamped to MAX_ZOOM
        assert_eq!(cam.zoom(), MAX_ZOOM);
    }

    #[test]
    fn visible_rect_at_origin_zoom_1() {
        let cam = Camera::new(800.0, 600.0);
        let (left, top, right, bottom) = cam.visible_rect();
        assert_eq!(left, -400.0);
        assert_eq!(top, -300.0);
        assert_eq!(right, 400.0);
        assert_eq!(bottom, 300.0);
    }

    #[test]
    fn visible_rect_with_pan() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_pan(100.0, 50.0);
        let (left, top, right, bottom) = cam.visible_rect();
        assert_eq!(left, -300.0); // 100 - 400
        assert_eq!(top, -250.0); // 50 - 300
        assert_eq!(right, 500.0); // 100 + 400
        assert_eq!(bottom, 350.0); // 50 + 300
    }

    #[test]
    fn visible_rect_with_zoom_in() {
        let cam = {
            let mut c = Camera::new(800.0, 600.0);
            c.set_zoom(2.0); // 2x zoom = half the world visible
            c
        };
        let (left, top, right, bottom) = cam.visible_rect();
        assert_eq!(left, -200.0); // 800 / (2*2) = 200
        assert_eq!(top, -150.0);
        assert_eq!(right, 200.0);
        assert_eq!(bottom, 150.0);
    }

    #[test]
    fn uniform_identity_at_origin() {
        // At the origin with zoom=1 and an 800x600 viewport,
        // a point at (400, 300) should map to clip (1.0, -1.0) — bottom-right.
        let cam = Camera::new(800.0, 600.0);
        let u = cam.build_uniform();

        // Transform (400, 300) through the matrix.
        let clip = transform_point(&u.view_proj, 400.0, 300.0);
        assert!(
            (clip.0 - 1.0).abs() < 1e-6,
            "x should be 1.0, got {}",
            clip.0
        );
        assert!(
            (clip.1 - (-1.0)).abs() < 1e-6,
            "y should be -1.0, got {}",
            clip.1
        );
    }

    #[test]
    fn uniform_origin_maps_to_clip_origin() {
        // With no pan, world origin (0,0) should map to clip (0,0).
        let cam = Camera::new(800.0, 600.0);
        let u = cam.build_uniform();
        let clip = transform_point(&u.view_proj, 0.0, 0.0);
        assert!((clip.0).abs() < 1e-6, "x should be 0.0, got {}", clip.0);
        assert!((clip.1).abs() < 1e-6, "y should be 0.0, got {}", clip.1);
    }

    #[test]
    fn uniform_with_pan() {
        // Pan to (100, 50): world (100, 50) should map to clip (0, 0).
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_pan(100.0, 50.0);
        let u = cam.build_uniform();
        let clip = transform_point(&u.view_proj, 100.0, 50.0);
        assert!((clip.0).abs() < 1e-6, "x should be 0.0, got {}", clip.0);
        assert!((clip.1).abs() < 1e-6, "y should be 0.0, got {}", clip.1);
    }

    #[test]
    fn uniform_with_zoom() {
        // 2x zoom: world (200, 150) should map to clip (1.0, -1.0).
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(2.0);
        let u = cam.build_uniform();
        let clip = transform_point(&u.view_proj, 200.0, 150.0);
        assert!(
            (clip.0 - 1.0).abs() < 1e-6,
            "x should be 1.0, got {}",
            clip.0
        );
        assert!(
            (clip.1 - (-1.0)).abs() < 1e-6,
            "y should be -1.0, got {}",
            clip.1
        );
    }

    #[test]
    fn camera_uniform_size_is_64_bytes() {
        // 4x4 matrix of f32 = 16 * 4 = 64 bytes.
        assert_eq!(CameraUniform::size(), 64);
    }

    // --- Coordinate conversion tests ---

    #[test]
    fn screen_to_world_identity() {
        // No pan, zoom=1. Screen center (400, 300) maps to world origin (0, 0).
        let cam = Camera::new(800.0, 600.0);
        let (wx, wy) = cam.screen_to_world(400.0, 300.0);
        assert_eq!((wx, wy), (0.0, 0.0));
        // Top-left corner maps to visible_rect top-left.
        let (wx, wy) = cam.screen_to_world(0.0, 0.0);
        assert_eq!((wx, wy), (-400.0, -300.0));
    }

    #[test]
    fn screen_to_world_with_pan() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_pan(100.0, 50.0);
        // Screen center maps to the pan point.
        let (wx, wy) = cam.screen_to_world(400.0, 300.0);
        assert_eq!((wx, wy), (100.0, 50.0));
    }

    #[test]
    fn screen_to_world_with_zoom() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(2.0);
        // Screen center still maps to world origin.
        let (wx, wy) = cam.screen_to_world(400.0, 300.0);
        assert_eq!((wx, wy), (0.0, 0.0));
        // Bottom-right maps to half the distance at 2x zoom.
        let (wx, wy) = cam.screen_to_world(800.0, 600.0);
        assert_eq!((wx, wy), (200.0, 150.0));
    }

    #[test]
    fn screen_to_world_with_pan_and_zoom() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_pan(100.0, 50.0);
        cam.set_zoom(2.0);
        // Screen center maps to pan point.
        let (wx, wy) = cam.screen_to_world(400.0, 300.0);
        assert_eq!((wx, wy), (100.0, 50.0));
        // Top-left of screen.
        let (wx, wy) = cam.screen_to_world(0.0, 0.0);
        assert_eq!((wx, wy), (-100.0, -100.0));
    }

    #[test]
    fn world_to_screen_identity() {
        let cam = Camera::new(800.0, 600.0);
        // World origin maps to screen center.
        let (sx, sy) = cam.world_to_screen(0.0, 0.0);
        assert_eq!((sx, sy), (400.0, 300.0));
    }

    #[test]
    fn world_to_screen_roundtrip() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_pan(77.0, -33.0);
        cam.set_zoom(3.5);
        let screen = (123.4, 567.8);
        let (wx, wy) = cam.screen_to_world(screen.0, screen.1);
        let (sx, sy) = cam.world_to_screen(wx, wy);
        assert!(
            (sx - screen.0).abs() < 1e-3,
            "x: expected {}, got {}",
            screen.0,
            sx
        );
        assert!(
            (sy - screen.1).abs() < 1e-3,
            "y: expected {}, got {}",
            screen.1,
            sy
        );
    }

    #[test]
    fn zoom_at_center_only_changes_zoom() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.zoom_at(2.0, 400.0, 300.0);
        assert_eq!(cam.zoom(), 2.0);
        assert_eq!(cam.pan(), (0.0, 0.0));
    }

    #[test]
    fn zoom_at_corner_adjusts_pan() {
        let mut cam = Camera::new(800.0, 600.0);
        // Before: world under screen (0,0) = (-400, -300).
        cam.zoom_at(2.0, 0.0, 0.0);
        assert_eq!(cam.zoom(), 2.0);
        // After: screen (0,0) should still map to world (-400, -300).
        let (wx, wy) = cam.screen_to_world(0.0, 0.0);
        assert!((wx - (-400.0)).abs() < 1e-3, "x: expected -400, got {}", wx);
        assert!((wy - (-300.0)).abs() < 1e-3, "y: expected -300, got {}", wy);
    }

    #[test]
    fn zoom_at_clamps_to_bounds() {
        let mut cam = Camera::new(800.0, 600.0);
        // Zoom to extremely large factor.
        cam.zoom_at(1000.0, 400.0, 300.0);
        assert_eq!(cam.zoom(), MAX_ZOOM);
        // Zoom to extremely small factor.
        cam.zoom_at(0.0001, 400.0, 300.0);
        assert_eq!(cam.zoom(), MIN_ZOOM);
    }

    #[test]
    fn zoom_clamp_existing_methods() {
        let mut cam = Camera::new(800.0, 600.0);
        cam.set_zoom(0.001);
        assert_eq!(cam.zoom(), MIN_ZOOM);
        cam.set_zoom(1.0);
        cam.zoom_by(0.001);
        assert_eq!(cam.zoom(), MIN_ZOOM);
    }

    /// Helper: multiply a 4x4 column-major matrix by a 2D point (z=0, w=1).
    /// Returns `(clip_x, clip_y)`.
    fn transform_point(m: &[f32; 16], x: f32, y: f32) -> (f32, f32) {
        let clip_x = m[0] * x + m[4] * y + m[12];
        let clip_y = m[1] * x + m[5] * y + m[13];
        (clip_x, clip_y)
    }
}
