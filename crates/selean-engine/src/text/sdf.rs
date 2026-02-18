//! SDF (Signed Distance Field) generation for glyph bitmaps.
//!
//! Converts glyph outlines into SDF bitmaps using:
//! 1. Rasterization via `ab_glyph_rasterizer` to produce a coverage bitmap
//! 2. Felzenszwalb-Huttenlocher Euclidean Distance Transform (EDT) to compute distances
//! 3. Combination of inner/outer distance fields into a single SDF

use ab_glyph_rasterizer::{Point, Rasterizer, point};
use ttf_parser::OutlineBuilder;

/// Configuration for SDF generation.
#[derive(Debug, Clone, Copy)]
pub struct SdfParams {
    /// Size of the rasterization grid for each glyph (in pixels).
    /// The glyph is scaled to fit within this size, with `spread` pixels
    /// of padding on each side.
    pub render_size: u32,
    /// Number of pixels of SDF spread around the glyph edge.
    /// Determines how far the distance field extends beyond the glyph outline.
    pub spread: u32,
}

impl Default for SdfParams {
    fn default() -> Self {
        Self {
            render_size: 48,
            spread: 6,
        }
    }
}

/// A generated SDF bitmap for a single glyph.
#[derive(Debug, Clone)]
pub struct SdfBitmap {
    /// Single-channel SDF data. 128 = on the edge, >128 = inside, <128 = outside.
    pub data: Vec<u8>,
    /// Width of the bitmap in pixels.
    pub width: u32,
    /// Height of the bitmap in pixels.
    pub height: u32,
    /// Horizontal bearing: offset from glyph origin to left edge of bitmap,
    /// in SDF pixels (scaled to `render_size` coordinate space).
    pub bearing_x: f32,
    /// Vertical bearing: offset from baseline to top edge of bitmap,
    /// in SDF pixels (scaled to `render_size` coordinate space).
    pub bearing_y: f32,
    /// Glyph bounding box width in font units.
    pub glyph_width_funits: u16,
    /// Glyph bounding box height in font units.
    pub glyph_height_funits: u16,
}

/// Generates an SDF bitmap for a single glyph.
///
/// Returns `None` if the glyph has no outline (e.g., space character).
///
/// # Process
/// 1. Gets the glyph's bounding box and outline from the font.
/// 2. Scales the outline to fit within `render_size - 2*spread` pixels.
/// 3. Rasterizes to a coverage bitmap via `ab_glyph_rasterizer`.
/// 4. Runs the Felzenszwalb-Huttenlocher EDT on inner and outer regions.
/// 5. Combines into a signed distance field normalized to `[0, 255]`.
#[allow(clippy::similar_names)]
pub fn generate_glyph_sdf(
    face: &ttf_parser::Face<'_>,
    glyph_id: ttf_parser::GlyphId,
    params: &SdfParams,
) -> Option<SdfBitmap> {
    // Get the bounding box to determine glyph dimensions.
    let bbox = face.glyph_bounding_box(glyph_id)?;

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let glyph_w_funits = (bbox.x_max - bbox.x_min) as u16;
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    let glyph_h_funits = (bbox.y_max - bbox.y_min) as u16;

    if glyph_w_funits == 0 || glyph_h_funits == 0 {
        return None;
    }

    // Compute scale to fit the glyph within (render_size - 2*spread) pixels.
    let usable_size = params.render_size.saturating_sub(2 * params.spread);
    if usable_size == 0 {
        return None;
    }

    #[allow(clippy::cast_possible_truncation)]
    let usable_u16 = usable_size as u16;
    let scale_x = f32::from(usable_u16) / f32::from(glyph_w_funits);
    let scale_y = f32::from(usable_u16) / f32::from(glyph_h_funits);
    let scale = scale_x.min(scale_y);

    // Compute bitmap dimensions (glyph scaled + 2*spread padding).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let scaled_w = (f32::from(glyph_w_funits) * scale).ceil() as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let scaled_h = (f32::from(glyph_h_funits) * scale).ceil() as u32;
    let bmp_w = scaled_w + 2 * params.spread;
    let bmp_h = scaled_h + 2 * params.spread;

    // Rasterize the glyph outline.
    #[allow(clippy::cast_precision_loss)]
    let spread_f32 = params.spread as f32;
    let mut builder = OutlineToRasterizer {
        rasterizer: Rasterizer::new(bmp_w as usize, bmp_h as usize),
        current: point(0.0, 0.0),
        scale,
        // Offset: spread padding + glyph origin shift.
        offset_x: spread_f32 - f32::from(bbox.x_min) * scale,
        // Flip Y (font coords are Y-up, bitmap coords are Y-down).
        offset_y: spread_f32 + f32::from(bbox.y_max) * scale,
        flip_y: true,
    };

    face.outline_glyph(glyph_id, &mut builder)?;

    // Extract the coverage bitmap.
    #[allow(clippy::cast_possible_truncation)]
    let pixel_count = (bmp_w * bmp_h) as usize;
    let mut coverage = vec![0.0_f32; pixel_count];
    builder.rasterizer.for_each_pixel(|index, alpha| {
        if index < coverage.len() {
            coverage[index] = alpha;
        }
    });

    // Run EDT and produce the SDF.
    let sdf_data = coverage_to_sdf(&coverage, bmp_w as usize, bmp_h as usize, params.spread);

    // Compute bearings in SDF pixel space.
    let bearing_x = f32::from(bbox.x_min) * scale - spread_f32;
    let bearing_y = f32::from(bbox.y_max) * scale + spread_f32;

    Some(SdfBitmap {
        data: sdf_data,
        width: bmp_w,
        height: bmp_h,
        bearing_x,
        bearing_y,
        glyph_width_funits: glyph_w_funits,
        glyph_height_funits: glyph_h_funits,
    })
}

/// Converts a coverage bitmap to an SDF bitmap using the EDT algorithm.
///
/// The coverage values should be in [0.0, 1.0] where 0.0 is outside and 1.0 is inside.
/// Returns a Vec<u8> where 128 = edge, >128 = inside, <128 = outside.
fn coverage_to_sdf(coverage: &[f32], width: usize, height: usize, spread: u32) -> Vec<u8> {
    let n = width * height;
    #[allow(clippy::cast_precision_loss)]
    let spread_f = spread as f32;

    // Build inner and outer distance grids.
    // Inner: distance from inside pixels to the nearest outside pixel.
    // Outer: distance from outside pixels to the nearest inside pixel.
    let mut inner = vec![0.0_f32; n];
    let mut outer = vec![0.0_f32; n];

    #[allow(clippy::cast_precision_loss)]
    let inf = (width + height) as f32;

    for i in 0..n {
        let a = coverage[i];
        if a > 0.5 {
            // Inside the glyph.
            inner[i] = 0.0;
            outer[i] = inf;
        } else {
            // Outside the glyph.
            inner[i] = inf;
            outer[i] = 0.0;
        }

        // Sub-pixel refinement at edges.
        if a > 0.0 && a < 1.0 {
            // Approximate distance from coverage: d ≈ 0.5 - a
            let d = 0.5 - a;
            if d > 0.0 {
                outer[i] = d * d;
                inner[i] = 0.0;
            } else {
                inner[i] = d * d;
                outer[i] = 0.0;
            }
        }
    }

    // Run 2D EDT on both grids.
    edt_2d(&mut inner, width, height);
    edt_2d(&mut outer, width, height);

    // Combine into signed distance and normalize.
    // After EDT:
    //   inner[i] = squared distance from pixel i to nearest inside pixel
    //   outer[i] = squared distance from pixel i to nearest outside pixel
    // Signed distance: positive inside, negative outside.
    //   sd = sqrt(outer) - sqrt(inner)
    // Map to [0, 255]: 128 = edge, >128 = inside, <128 = outside.
    let mut result = vec![0u8; n];
    for i in 0..n {
        let sd = outer[i].sqrt() - inner[i].sqrt();
        let normalized = 0.5 + sd / (2.0 * spread_f);
        let clamped = normalized.clamp(0.0, 1.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        {
            result[i] = (clamped * 255.0) as u8;
        }
    }

    result
}

// --- Felzenszwalb-Huttenlocher EDT ---

/// 1D squared Euclidean distance transform using the parabola envelope method.
///
/// Operates in-place on `f`, which contains squared distances.
/// After this call, `f[q]` = min over all p of { `f_orig[p]` + (q-p)^2 }.
#[allow(clippy::many_single_char_names, clippy::needless_range_loop)]
fn edt_1d(f: &mut [f32]) {
    let n = f.len();
    if n <= 1 {
        return;
    }

    // v[k] = index of the k-th parabola in the lower envelope.
    // z[k] = left boundary of the k-th parabola's region.
    let mut v = vec![0_usize; n];
    let mut z = vec![0.0_f32; n + 1];
    let mut d = vec![0.0_f32; n]; // output

    let mut k = 0_usize;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;

    for q in 1..n {
        // Compute the intersection of the parabola at q with the parabola at v[k].
        loop {
            let vk = v[k];
            #[allow(clippy::cast_precision_loss)]
            let q_f = q as f32;
            #[allow(clippy::cast_precision_loss)]
            let vk_f = vk as f32;
            let s = ((f[q] + q_f * q_f) - (f[vk] + vk_f * vk_f)) / (2.0 * (q_f - vk_f));

            if s > z[k] {
                // The new parabola extends the envelope.
                k += 1;
                v[k] = q;
                z[k] = s;
                z[k + 1] = f32::INFINITY;
                break;
            }
            // The previous parabola is dominated; remove it.
            if k == 0 {
                v[0] = q;
                z[1] = f32::INFINITY;
                break;
            }
            k -= 1;
        }
    }

    // Fill in the distance values by scanning left to right.
    k = 0;
    for q in 0..n {
        #[allow(clippy::cast_precision_loss)]
        while z[k + 1] < q as f32 {
            k += 1;
        }
        #[allow(clippy::cast_precision_loss)]
        let diff = q as f32 - v[k] as f32;
        d[q] = diff * diff + f[v[k]];
    }

    f[..n].copy_from_slice(&d[..n]);
}

/// 2D squared Euclidean distance transform.
///
/// Applies the 1D EDT to each row, then to each column, giving the
/// squared Euclidean distance to the nearest zero-distance pixel.
fn edt_2d(grid: &mut [f32], width: usize, height: usize) {
    // Process rows.
    let mut row_buf = vec![0.0_f32; width];
    for y in 0..height {
        let start = y * width;
        row_buf.copy_from_slice(&grid[start..start + width]);
        edt_1d(&mut row_buf);
        grid[start..start + width].copy_from_slice(&row_buf);
    }

    // Process columns.
    let mut col_buf = vec![0.0_f32; height];
    for x in 0..width {
        for y in 0..height {
            col_buf[y] = grid[y * width + x];
        }
        edt_1d(&mut col_buf);
        for y in 0..height {
            grid[y * width + x] = col_buf[y];
        }
    }
}

// --- Outline builder bridge: ttf_parser → ab_glyph_rasterizer ---

/// Bridges `ttf_parser::OutlineBuilder` to `ab_glyph_rasterizer::Rasterizer`.
///
/// Transforms font-unit coordinates to pixel coordinates with Y-axis flipping.
struct OutlineToRasterizer {
    rasterizer: Rasterizer,
    current: Point,
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    flip_y: bool,
}

impl OutlineToRasterizer {
    fn transform(&self, x: f32, y: f32) -> Point {
        let px = x * self.scale + self.offset_x;
        let py = if self.flip_y {
            -y * self.scale + self.offset_y
        } else {
            y * self.scale + self.offset_y
        };
        point(px, py)
    }
}

impl OutlineBuilder for OutlineToRasterizer {
    fn move_to(&mut self, x: f32, y: f32) {
        self.current = self.transform(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let to = self.transform(x, y);
        self.rasterizer.draw_line(self.current, to);
        self.current = to;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let control = self.transform(x1, y1);
        let to = self.transform(x, y);
        self.rasterizer.draw_quad(self.current, control, to);
        self.current = to;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.transform(x1, y1);
        let c2 = self.transform(x2, y2);
        let to = self.transform(x, y);
        self.rasterizer.draw_cubic(self.current, c1, c2, to);
        self.current = to;
    }

    fn close(&mut self) {
        // Closing is implicit in the rasterizer (it works with individual segments).
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::font::FontData;

    #[test]
    fn edt_1d_all_zeros() {
        let mut f = vec![0.0; 5];
        edt_1d(&mut f);
        for v in &f {
            assert!((*v - 0.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn edt_1d_single_source() {
        // Only index 2 is zero, rest are "infinite".
        let mut f = vec![1000.0, 1000.0, 0.0, 1000.0, 1000.0];
        edt_1d(&mut f);
        // Expected squared distances: [4, 1, 0, 1, 4]
        assert!((f[0] - 4.0).abs() < 0.1);
        assert!((f[1] - 1.0).abs() < 0.1);
        assert!((f[2] - 0.0).abs() < 0.1);
        assert!((f[3] - 1.0).abs() < 0.1);
        assert!((f[4] - 4.0).abs() < 0.1);
    }

    #[test]
    fn edt_1d_single_element() {
        let mut f = vec![42.0];
        edt_1d(&mut f);
        assert!((f[0] - 42.0).abs() < f32::EPSILON);
    }

    #[test]
    fn edt_2d_center_source() {
        // 5x5 grid with a zero at center (2,2).
        let mut grid = vec![1000.0; 25];
        grid[2 * 5 + 2] = 0.0; // center

        edt_2d(&mut grid, 5, 5);

        // Center should be 0.
        assert!((grid[12] - 0.0).abs() < 0.1);
        // Adjacent cells should have squared distance 1.
        assert!((grid[11] - 1.0).abs() < 0.1); // left
        assert!((grid[13] - 1.0).abs() < 0.1); // right
        assert!((grid[7] - 1.0).abs() < 0.1); // above
        assert!((grid[17] - 1.0).abs() < 0.1); // below
        // Diagonal should have squared distance 2.
        assert!((grid[6] - 2.0).abs() < 0.1);
        // Corner should have squared distance 8 (4+4).
        assert!((grid[0] - 8.0).abs() < 0.1);
    }

    #[test]
    fn coverage_to_sdf_all_inside() {
        let coverage = vec![1.0; 16];
        let sdf = coverage_to_sdf(&coverage, 4, 4, 4);
        // All pixels inside → SDF values should be >= 128 (on or inside edge).
        for &v in &sdf {
            assert!(v >= 128, "inside pixel should be >= 128, got {v}");
        }
    }

    #[test]
    fn coverage_to_sdf_all_outside() {
        let coverage = vec![0.0; 16];
        let sdf = coverage_to_sdf(&coverage, 4, 4, 4);
        // All pixels outside → SDF values should be <= 128.
        for &v in &sdf {
            assert!(v <= 128, "outside pixel should be <= 128, got {v}");
        }
    }

    #[test]
    fn coverage_to_sdf_edge() {
        // A simple 10x10 grid: left half inside, right half outside.
        let mut coverage = vec![0.0; 100];
        for y in 0..10 {
            for x in 0..5 {
                coverage[y * 10 + x] = 1.0;
            }
        }

        let sdf = coverage_to_sdf(&coverage, 10, 10, 4);

        // Pixels deep inside (x=0) should be > 128.
        assert!(sdf[50] > 128);
        // Pixels at the edge (x=4 or x=5) should be near 128.
        let edge_left = sdf[5 * 10 + 4];
        let edge_right = sdf[5 * 10 + 5];
        assert!(
            edge_left > 100 && edge_left < 200,
            "edge-left should be near 128, got {edge_left}"
        );
        assert!(
            edge_right < 160,
            "edge-right should be near or below 128, got {edge_right}"
        );
        // Pixels deep outside (x=9) should be < 128.
        assert!(sdf[5 * 10 + 9] < 128);
    }

    #[test]
    fn generate_sdf_for_letter_a() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face().unwrap_or_else(|_| unreachable!());

        // Get glyph ID for 'A'.
        let glyph_id = face.glyph_index('A').unwrap_or_else(|| unreachable!());

        let params = SdfParams::default();
        let sdf = generate_glyph_sdf(&face, glyph_id, &params);
        assert!(sdf.is_some(), "letter 'A' should produce an SDF");

        let sdf = sdf.unwrap_or_else(|| unreachable!());
        assert!(sdf.width > 0);
        assert!(sdf.height > 0);
        assert_eq!(sdf.data.len(), (sdf.width * sdf.height) as usize);

        // SDF should contain both inside (>128) and outside (<128) values.
        let has_inside = sdf.data.iter().any(|&v| v > 150);
        let has_outside = sdf.data.iter().any(|&v| v < 100);
        assert!(has_inside, "SDF should have inside pixels");
        assert!(has_outside, "SDF should have outside pixels");
    }

    #[test]
    fn space_character_returns_none() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face().unwrap_or_else(|_| unreachable!());

        let glyph_id = face.glyph_index(' ').unwrap_or_else(|| unreachable!());

        let params = SdfParams::default();
        let sdf = generate_glyph_sdf(&face, glyph_id, &params);
        assert!(sdf.is_none(), "space should have no outline");
    }

    #[test]
    fn sdf_dimensions_match_params() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face().unwrap_or_else(|_| unreachable!());

        let glyph_id = face.glyph_index('M').unwrap_or_else(|| unreachable!());

        let params = SdfParams {
            render_size: 64,
            spread: 8,
        };
        let sdf = generate_glyph_sdf(&face, glyph_id, &params).unwrap_or_else(|| unreachable!());

        // The bitmap should be at most render_size in each dimension
        // (it can be smaller if the glyph is not square).
        assert!(sdf.width <= params.render_size + 2 * params.spread);
        assert!(sdf.height <= params.render_size + 2 * params.spread);
    }

    #[test]
    fn default_sdf_params() {
        let params = SdfParams::default();
        assert_eq!(params.render_size, 48);
        assert_eq!(params.spread, 6);
    }

    #[test]
    fn sdf_glyph_metrics_are_nonzero() {
        let font = FontData::default_font().unwrap_or_else(|_| unreachable!());
        let face = font.face().unwrap_or_else(|_| unreachable!());

        let glyph_id = face.glyph_index('W').unwrap_or_else(|| unreachable!());

        let sdf = generate_glyph_sdf(&face, glyph_id, &SdfParams::default())
            .unwrap_or_else(|| unreachable!());

        assert!(sdf.glyph_width_funits > 0);
        assert!(sdf.glyph_height_funits > 0);
    }
}
