//! SVG path data parser.
//!
//! Converts SVG path data strings into `tiny_skia::Path` objects using
//! `svgtypes::SimplifyingPathParser` for tokenization and `tiny_skia::PathBuilder`
//! for construction.

use selean_common::error::EngineError;
use svgtypes::SimplePathSegment;

/// Parses an SVG path data string into a `tiny_skia::Path`.
///
/// Supports all standard SVG path commands: M, L, H, V, C, S, Q, T, A, Z
/// (both absolute and relative). The `SimplifyingPathParser` converts all
/// commands to absolute coordinates and simplifies arcs to cubic Beziers.
///
/// # Errors
///
/// Returns `EngineError::Vector` if the path data is empty or contains no
/// valid geometry.
pub fn parse_path_data(path_data: &str) -> Result<tiny_skia::Path, EngineError> {
    if path_data.is_empty() {
        return Err(EngineError::Vector {
            reason: "empty path data".to_string(),
        });
    }

    let mut builder = tiny_skia::PathBuilder::new();
    let mut has_geometry = false;

    for segment in svgtypes::SimplifyingPathParser::from(path_data) {
        let segment = segment.map_err(|e| EngineError::Vector {
            reason: format!("path parse error: {e}"),
        })?;

        #[allow(clippy::cast_possible_truncation)]
        match segment {
            SimplePathSegment::MoveTo { x, y } => {
                builder.move_to(x as f32, y as f32);
            }
            SimplePathSegment::LineTo { x, y } => {
                builder.line_to(x as f32, y as f32);
                has_geometry = true;
            }
            SimplePathSegment::CurveTo {
                x1,
                y1,
                x2,
                y2,
                x,
                y,
            } => {
                builder.cubic_to(
                    x1 as f32, y1 as f32, x2 as f32, y2 as f32, x as f32, y as f32,
                );
                has_geometry = true;
            }
            SimplePathSegment::Quadratic { x1, y1, x, y } => {
                builder.quad_to(x1 as f32, y1 as f32, x as f32, y as f32);
                has_geometry = true;
            }
            SimplePathSegment::ClosePath => {
                builder.close();
            }
        }
    }

    if !has_geometry {
        return Err(EngineError::Vector {
            reason: "path data contains no geometry".to_string(),
        });
    }

    builder.finish().ok_or_else(|| EngineError::Vector {
        reason: "failed to build path from parsed data".to_string(),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_line() {
        let path = parse_path_data("M 0 0 L 10 10").expect("should parse");
        let bounds = path.bounds();
        assert!((bounds.width() - 10.0).abs() < f32::EPSILON);
        assert!((bounds.height() - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_triangle() {
        let path = parse_path_data("M 0 0 L 50 0 L 25 50 Z").expect("should parse");
        let bounds = path.bounds();
        assert!((bounds.width() - 50.0).abs() < f32::EPSILON);
        assert!((bounds.height() - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_cubic_bezier() {
        let path = parse_path_data("M 0 0 C 10 20 30 40 50 50").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_quadratic_bezier() {
        let path = parse_path_data("M 0 0 Q 25 50 50 0").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_horizontal_vertical() {
        let path = parse_path_data("M 0 0 H 100 V 50").expect("should parse");
        let bounds = path.bounds();
        assert!((bounds.width() - 100.0).abs() < f32::EPSILON);
        assert!((bounds.height() - 50.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_relative_commands() {
        let path = parse_path_data("M 0 0 l 10 10 l 10 -10").expect("should parse");
        let bounds = path.bounds();
        assert!((bounds.width() - 20.0).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_arc() {
        let path = parse_path_data("M 0 50 A 50 50 0 0 1 50 0").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_smooth_cubic() {
        let path =
            parse_path_data("M 0 0 C 10 20 30 40 50 50 S 70 60 80 40").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_smooth_quadratic() {
        let path = parse_path_data("M 0 0 Q 25 50 50 0 T 100 0").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_empty_fails() {
        assert!(parse_path_data("").is_err());
    }

    #[test]
    fn parse_move_only_fails() {
        assert!(parse_path_data("M 0 0").is_err());
    }

    #[test]
    fn parse_complex_path() {
        let path = parse_path_data(
            "M 12 2 C 6.48 2 2 6.48 2 12 S 6.48 22 12 22 S 22 17.52 22 12 S 17.52 2 12 2 Z",
        )
        .expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_close_without_move() {
        let path = parse_path_data("M 0 0 L 10 0 L 10 10 Z").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }

    #[test]
    fn parse_multiple_subpaths() {
        let path = parse_path_data("M 0 0 L 10 10 Z M 20 20 L 30 30 Z").expect("should parse");
        assert!(path.bounds().width() > 0.0);
    }
}
