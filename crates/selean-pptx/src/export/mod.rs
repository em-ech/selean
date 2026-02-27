//! PPTX export: creates a minimal .pptx ZIP archive from a Document.

pub mod shape;

use std::fmt::Write as FmtWrite;
use std::io::{Cursor, Write};

use zip::write::{SimpleFileOptions, ZipWriter};

use selean_engine::persistence::{Document, Page};

use crate::PptxError;
use crate::coord::px_to_emu;
use shape::node_to_shape_xml;

/// Exports a Document to PPTX format as bytes.
///
/// Creates a minimal OOXML-compliant ZIP archive with one slide per page.
///
/// # Errors
///
/// Returns `PptxError::Zip` if ZIP writing fails.
pub fn export_pptx(document: &Document) -> Result<Vec<u8>, PptxError> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // [Content_Types].xml
    let content_types = build_content_types(document.page_count());
    zip.start_file("[Content_Types].xml", options)?;
    zip.write_all(content_types.as_bytes())?;

    // _rels/.rels
    let rels = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/></Relationships>"#;
    zip.start_file("_rels/.rels", options)?;
    zip.write_all(rels.as_bytes())?;

    // Slide dimensions from first page
    let first_page = &document.pages()[0];
    let cx = px_to_emu(first_page.width);
    let cy = px_to_emu(first_page.height);

    // ppt/presentation.xml
    let slide_refs = (1..=document.page_count()).fold(String::new(), |mut acc, i| {
        let _ = write!(acc, r#"<p:sldId id="{}" r:id="rId{i}"/>"#, 255 + i);
        acc
    });
    let presentation = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldIdLst>{slide_refs}</p:sldIdLst><p:sldSz cx="{cx}" cy="{cy}"/></p:presentation>"#,
    );
    zip.start_file("ppt/presentation.xml", options)?;
    zip.write_all(presentation.as_bytes())?;

    // ppt/_rels/presentation.xml.rels
    let pres_rels = (1..=document.page_count()).fold(String::new(), |mut acc, i| {
        let _ = write!(
            acc,
            r#"<Relationship Id="rId{i}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{i}.xml"/>"#
        );
        acc
    });
    let pres_rels_xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{pres_rels}</Relationships>"#
    );
    zip.start_file("ppt/_rels/presentation.xml.rels", options)?;
    zip.write_all(pres_rels_xml.as_bytes())?;

    // Slides
    for (i, page) in document.pages().iter().enumerate() {
        let slide_num = i + 1;
        let slide_xml = build_slide_xml(page);
        zip.start_file(format!("ppt/slides/slide{slide_num}.xml"), options)?;
        zip.write_all(slide_xml.as_bytes())?;
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn build_content_types(slide_count: usize) -> String {
    let slide_overrides = (1..=slide_count).fold(String::new(), |mut acc, i| {
        let _ = write!(
            acc,
            r#"<Override PartName="/ppt/slides/slide{i}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        );
        acc
    });

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/>{slide_overrides}</Types>"#
    )
}

fn build_slide_xml(page: &Page) -> String {
    let mut shapes = String::new();
    let mut shape_id = 2_u32; // 1 is reserved for the spTree

    for &root_id in page.scene.roots() {
        if let Some(node) = page.scene.get(root_id) {
            shapes.push_str(&node_to_shape_xml(node, shape_id));
            shape_id += 1;
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>{shapes}</p:spTree></p:cSld></p:sld>"#
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, Color, SceneNode, SceneNodeKind};

    fn make_frame(name: &str, x: f32, y: f32, w: f32, h: f32) -> SceneNode {
        SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(x, y, w, h),
        )
    }

    #[test]
    fn export_empty_document() {
        let doc = Document::new();
        let bytes = export_pptx(&doc).unwrap();
        assert!(!bytes.is_empty());

        // Verify it's a valid ZIP
        let cursor = Cursor::new(bytes);
        let archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(!archive.is_empty());
    }

    #[test]
    fn export_contains_expected_parts() {
        let doc = Document::new();
        let bytes = export_pptx(&doc).unwrap();

        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        assert!(archive.by_name("[Content_Types].xml").is_ok());
        assert!(archive.by_name("_rels/.rels").is_ok());
        assert!(archive.by_name("ppt/presentation.xml").is_ok());
        assert!(archive.by_name("ppt/_rels/presentation.xml.rels").is_ok());
        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
    }

    #[test]
    fn export_multiple_pages_creates_multiple_slides() {
        let mut doc = Document::new();
        doc.add_page("Page 2", 1920.0, 1080.0);
        doc.add_page("Page 3", 1920.0, 1080.0);

        let bytes = export_pptx(&doc).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        assert!(archive.by_name("ppt/slides/slide1.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide2.xml").is_ok());
        assert!(archive.by_name("ppt/slides/slide3.xml").is_ok());
    }

    #[test]
    fn export_slide_contains_shapes() {
        let mut doc = Document::new();
        let mut node = make_frame("TestBox", 10.0, 20.0, 100.0, 50.0);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        doc.active_page_mut().scene.add_root(node);

        let bytes = export_pptx(&doc).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        let mut slide = archive.by_name("ppt/slides/slide1.xml").unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut slide, &mut xml).unwrap();

        assert!(xml.contains("TestBox"));
        assert!(xml.contains("FF0000"));
    }

    #[test]
    fn export_presentation_contains_slide_size() {
        let mut doc = Document::new();
        // Default page is 1920x1080
        doc.active_page_mut().width = 1280.0;
        doc.active_page_mut().height = 720.0;

        let bytes = export_pptx(&doc).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        let mut pres = archive.by_name("ppt/presentation.xml").unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut pres, &mut xml).unwrap();

        assert!(xml.contains("sldSz"));
        // 1280px = 12192000 EMU
        assert!(xml.contains("12192000"));
    }

    #[test]
    fn build_content_types_includes_slides() {
        let ct = build_content_types(3);
        assert!(ct.contains("slide1.xml"));
        assert!(ct.contains("slide2.xml"));
        assert!(ct.contains("slide3.xml"));
    }

    #[test]
    fn build_slide_xml_has_valid_structure() {
        let page = Page::new("Test", 1920.0, 1080.0);
        let xml = build_slide_xml(&page);
        assert!(xml.contains("p:sld"));
        assert!(xml.contains("p:spTree"));
        assert!(xml.contains("p:nvGrpSpPr"));
    }
}
