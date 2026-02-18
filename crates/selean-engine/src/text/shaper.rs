//! Text shaping via `rustybuzz` (pure-Rust `HarfBuzz` port).
//!
//! Converts a text string + font into a sequence of positioned glyphs with
//! correct kerning, ligatures, and OpenType feature application.

use super::font::FontData;

/// A single positioned glyph from the shaping result.
#[derive(Debug, Clone, Copy)]
pub struct ShapedGlyph {
    /// Glyph ID within the font.
    pub glyph_id: u16,
    /// Horizontal offset from the current pen position (font units).
    pub x_offset: i32,
    /// Vertical offset from the current pen position (font units).
    pub y_offset: i32,
    /// Horizontal advance to next glyph (font units).
    pub x_advance: i32,
    /// Cluster index (maps back to original string position).
    pub cluster: u32,
}

/// Result of shaping a text string with a specific font.
#[derive(Debug, Clone)]
pub struct ShapedRun {
    /// The shaped glyphs in visual order.
    pub glyphs: Vec<ShapedGlyph>,
    /// Font units per em (for scaling to pixel size).
    pub units_per_em: u16,
}

impl ShapedRun {
    /// Returns `true` if no glyphs were produced.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.glyphs.is_empty()
    }

    /// Returns the number of shaped glyphs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.glyphs.len()
    }

    /// Returns the total advance width in font units.
    #[must_use]
    pub fn total_advance(&self) -> i32 {
        self.glyphs.iter().map(|g| g.x_advance).sum()
    }
}

/// Shapes text content using `rustybuzz`.
///
/// Takes the font data and a text string, runs the `HarfBuzz` shaping algorithm,
/// and returns a `ShapedRun` containing positioned glyphs with correct kerning
/// and ligatures applied.
///
/// Returns `None` if the font cannot be parsed by `rustybuzz`.
///
/// # Arguments
/// * `font_data` — The loaded font.
/// * `content` — The text string to shape.
#[must_use]
pub fn shape_text(font_data: &FontData, content: &str) -> Option<ShapedRun> {
    if content.is_empty() {
        return Some(ShapedRun {
            glyphs: Vec::new(),
            units_per_em: font_data.units_per_em(),
        });
    }

    let face = rustybuzz::Face::from_slice(font_data.raw_bytes(), font_data.face_index())?;

    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(content);
    buffer.set_direction(rustybuzz::Direction::LeftToRight);

    let glyph_buffer = rustybuzz::shape(&face, &[], buffer);

    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let glyphs = infos
        .iter()
        .zip(positions.iter())
        .map(|(info, pos)| {
            // glyph_id from rustybuzz is u32 but guaranteed <= u16::MAX for TrueType.
            #[allow(clippy::cast_possible_truncation)]
            let glyph_id = info.glyph_id as u16;

            ShapedGlyph {
                glyph_id,
                x_offset: pos.x_offset,
                y_offset: pos.y_offset,
                x_advance: pos.x_advance,
                cluster: info.cluster,
            }
        })
        .collect();

    Some(ShapedRun {
        glyphs,
        units_per_em: font_data.units_per_em(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> FontData {
        FontData::default_font().unwrap_or_else(|_| unreachable!())
    }

    #[test]
    fn shape_hello() {
        let font = test_font();
        let run = shape_text(&font, "Hello");
        assert!(run.is_some());

        let run = run.unwrap_or_else(|| unreachable!());
        assert_eq!(run.len(), 5);
        assert!(!run.is_empty());
    }

    #[test]
    fn shape_empty_string() {
        let font = test_font();
        let run = shape_text(&font, "");
        assert!(run.is_some());

        let run = run.unwrap_or_else(|| unreachable!());
        assert!(run.is_empty());
        assert_eq!(run.len(), 0);
    }

    #[test]
    fn shaped_glyphs_have_positive_advances() {
        let font = test_font();
        let run = shape_text(&font, "Hello, world!").unwrap_or_else(|| unreachable!());

        for glyph in &run.glyphs {
            assert!(glyph.x_advance >= 0, "advance should be non-negative");
        }
    }

    #[test]
    fn total_advance_is_positive() {
        let font = test_font();
        let run = shape_text(&font, "Test").unwrap_or_else(|| unreachable!());
        assert!(run.total_advance() > 0);
    }

    #[test]
    fn units_per_em_is_set() {
        let font = test_font();
        let run = shape_text(&font, "A").unwrap_or_else(|| unreachable!());
        assert_eq!(run.units_per_em, font.units_per_em());
    }

    #[test]
    fn clusters_are_sequential() {
        let font = test_font();
        let run = shape_text(&font, "ABCD").unwrap_or_else(|| unreachable!());

        // For simple LTR text without ligatures, clusters should be in order.
        for (i, glyph) in run.glyphs.iter().enumerate() {
            #[allow(clippy::cast_possible_truncation)]
            let expected = i as u32;
            assert_eq!(glyph.cluster, expected, "cluster mismatch at glyph {i}");
        }
    }

    #[test]
    fn glyph_ids_are_nonzero_for_latin() {
        let font = test_font();
        let run = shape_text(&font, "ABC").unwrap_or_else(|| unreachable!());

        // Latin glyphs in Inter should have non-zero glyph IDs.
        for glyph in &run.glyphs {
            assert!(
                glyph.glyph_id > 0,
                "glyph ID should be non-zero for Latin text"
            );
        }
    }

    #[test]
    fn longer_text_produces_more_glyphs() {
        let font = test_font();
        let short = shape_text(&font, "Hi").unwrap_or_else(|| unreachable!());
        let long = shape_text(&font, "Hello, world!").unwrap_or_else(|| unreachable!());
        assert!(long.len() > short.len());
    }

    #[test]
    fn longer_text_has_greater_advance() {
        let font = test_font();
        let short = shape_text(&font, "Hi").unwrap_or_else(|| unreachable!());
        let long = shape_text(&font, "Hello, world!").unwrap_or_else(|| unreachable!());
        assert!(long.total_advance() > short.total_advance());
    }
}
