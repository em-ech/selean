//! PPTX import: reads a .pptx file (ZIP archive) and produces a Document.

pub mod shape;
pub mod text;

use std::io::{Cursor, Read};

use selean_common::types::PageId;
use selean_engine::persistence::{Document, Page};
use selean_engine::scene::SceneGraph;

use crate::PptxError;
use shape::parse_shapes;

/// Imports a PPTX file from bytes into a Document.
///
/// Each slide becomes a page in the document. Shapes within each slide
/// become nodes in the page's scene graph.
///
/// # Errors
///
/// Returns `PptxError::Zip` if the file is not a valid ZIP archive.
/// Returns `PptxError::Xml` if slide XML cannot be parsed.
/// Returns `PptxError::InvalidFormat` for structural OOXML issues.
pub fn import_pptx(bytes: &[u8]) -> Result<Document, PptxError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    // Read slide dimensions from presentation.xml
    let (slide_width, slide_height) = read_slide_dimensions(&mut archive)?;

    // Find all slide files
    let mut slide_paths: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(file) = archive.by_index(i) {
            let name = file.name().to_string();
            if name.starts_with("ppt/slides/slide")
                && std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"))
            {
                slide_paths.push(name);
            }
        }
    }
    slide_paths.sort();

    if slide_paths.is_empty() {
        return Err(PptxError::InvalidFormat("no slides found".to_string()));
    }

    let mut pages = Vec::new();
    for (i, path) in slide_paths.iter().enumerate() {
        let mut file = archive.by_name(path)?;
        let mut xml = String::new();
        file.read_to_string(&mut xml)?;

        let nodes = parse_shapes(&xml);

        let mut scene = SceneGraph::new();
        for node in nodes {
            scene.add_root(node);
        }

        let page = Page::with_scene(
            PageId::new(),
            format!("Slide {}", i + 1),
            slide_width,
            slide_height,
            scene,
        );
        pages.push(page);
    }

    Ok(Document::from_pages(pages))
}

fn read_slide_dimensions(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<(f32, f32), PptxError> {
    let mut file = archive
        .by_name("ppt/presentation.xml")
        .map_err(|_| PptxError::MissingPart("ppt/presentation.xml".to_string()))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)?;

    Ok(parse_slide_size(&xml))
}

/// Parses slide dimensions from presentation.xml content.
fn parse_slide_size(xml: &str) -> (f32, f32) {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut width = 1920.0_f32;
    let mut height = 1080.0_f32;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let qname = e.name();
                let local_name_bytes = qname.as_ref();
                let local = match local_name_bytes.iter().rposition(|&b| b == b':') {
                    Some(pos) => &local_name_bytes[pos + 1..],
                    None => local_name_bytes,
                };
                if local == b"sldSz" {
                    for attr in e.attributes().flatten() {
                        match attr.key.as_ref() {
                            b"cx" => {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    if let Ok(emu) = v.parse::<i64>() {
                                        width = crate::coord::emu_to_px(emu);
                                    }
                                }
                            }
                            b"cy" => {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    if let Ok(emu) = v.parse::<i64>() {
                                        height = crate::coord::emu_to_px(emu);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    break;
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    (width, height)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_slide_size_from_presentation_xml() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
            <p:sldSz cx="12192000" cy="6858000"/>
        </p:presentation>"#;
        let (w, h) = parse_slide_size(xml);
        // 12192000 EMU = 1280px, 6858000 EMU = 720px
        assert!((w - 1280.0).abs() < 1.0);
        assert!((h - 720.0).abs() < 1.0);
    }

    #[test]
    fn parse_slide_size_defaults_on_missing() {
        let xml = r#"<?xml version="1.0"?><p:presentation/>"#;
        let (w, h) = parse_slide_size(xml);
        assert!((w - 1920.0).abs() < f32::EPSILON);
        assert!((h - 1080.0).abs() < f32::EPSILON);
    }

    #[test]
    fn import_rejects_invalid_zip() {
        let result = import_pptx(b"not a zip file");
        assert!(result.is_err());
    }
}
