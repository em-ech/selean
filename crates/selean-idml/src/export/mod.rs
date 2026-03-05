//! IDML export: creates a minimal .idml ZIP archive from a Document.
//!
//! An IDML archive requires these entries:
//! - `mimetype` (uncompressed, first entry)
//! - `META-INF/container.xml`
//! - `designmap.xml`
//! - `Resources/Fonts.xml`, `Resources/Graphic.xml`
//! - `Styles/RootParagraphStyleGroup.xml`, `Styles/RootCharacterStyleGroup.xml`
//! - Per-page `Spreads/Spread_*.xml`
//! - Per-text-node `Stories/Story_*.xml`

pub mod shape;

use std::fmt::Write as FmtWrite;
use std::io::{Cursor, Write};

use zip::write::{SimpleFileOptions, ZipWriter};

use selean_common::xml::xml_escape;
use selean_engine::persistence::{Document, Page};
use selean_engine::scene::SceneNodeKind;

use crate::IdmlError;
use crate::coord::px_to_pt;
use shape::{build_story_xml, node_to_idml_element};

/// Exports a Document to IDML format as bytes.
///
/// Creates a minimal IDML-compliant ZIP archive with one spread per page.
///
/// # Errors
///
/// Returns `IdmlError::Zip` if ZIP writing fails.
pub fn export_idml(document: &Document) -> Result<Vec<u8>, IdmlError> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);

    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // mimetype must be first entry, uncompressed
    zip.start_file("mimetype", stored)?;
    zip.write_all(b"application/vnd.adobe.indesign-idml-package")?;

    // META-INF/container.xml
    zip.start_file("META-INF/container.xml", deflated)?;
    zip.write_all(CONTAINER_XML.as_bytes())?;

    // Collect spread and story file references for designmap
    let mut spread_refs = Vec::new();
    let mut story_refs = Vec::new();
    let mut story_entries: Vec<(String, String)> = Vec::new();

    // Generate spread and story files
    for (i, page) in document.pages().iter().enumerate() {
        let spread_num = i + 1;
        let spread_path = format!("Spreads/Spread_u{spread_num}.xml");
        spread_refs.push(spread_path.clone());

        let (spread_xml, page_stories) = build_spread_xml(page, spread_num);

        zip.start_file(&spread_path, deflated)?;
        zip.write_all(spread_xml.as_bytes())?;

        for (story_path, story_xml) in page_stories {
            story_refs.push(story_path.clone());
            story_entries.push((story_path, story_xml));
        }
    }

    // Write story files
    for (path, xml) in &story_entries {
        zip.start_file(path, deflated)?;
        zip.write_all(xml.as_bytes())?;
    }

    // designmap.xml
    let designmap = build_designmap(&spread_refs, &story_refs);
    zip.start_file("designmap.xml", deflated)?;
    zip.write_all(designmap.as_bytes())?;

    // Resource files (minimal stubs)
    zip.start_file("Resources/Fonts.xml", deflated)?;
    zip.write_all(FONTS_XML.as_bytes())?;

    zip.start_file("Resources/Graphic.xml", deflated)?;
    zip.write_all(GRAPHIC_XML.as_bytes())?;

    // Style files (minimal stubs)
    zip.start_file("Styles/RootParagraphStyleGroup.xml", deflated)?;
    zip.write_all(PARAGRAPH_STYLE_XML.as_bytes())?;

    zip.start_file("Styles/RootCharacterStyleGroup.xml", deflated)?;
    zip.write_all(CHARACTER_STYLE_XML.as_bytes())?;

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn build_spread_xml(page: &Page, spread_num: usize) -> (String, Vec<(String, String)>) {
    let half_w = px_to_pt(page.width) / 2.0;
    let half_h = px_to_pt(page.height) / 2.0;

    let mut elements = String::new();
    let mut stories = Vec::new();
    let mut element_idx = 1_u32;

    for &root_id in page.scene.roots() {
        if let Some(node) = page.scene.get(root_id) {
            let element_id = format!("item_u{spread_num}_{element_idx}");
            let story_id = if matches!(node.kind, SceneNodeKind::Text { .. }) {
                let sid = format!("story_u{spread_num}_{element_idx}");
                let story_xml = build_story_xml(node, &sid);
                let story_path = format!("Stories/Story_u{spread_num}_{element_idx}.xml");
                stories.push((story_path, story_xml));
                Some(sid)
            } else {
                None
            };

            elements.push_str(&node_to_idml_element(
                node,
                &element_id,
                story_id.as_deref(),
                half_w,
                half_h,
            ));
            elements.push('\n');
            element_idx += 1;
        }
    }

    let page_width = px_to_pt(page.width);
    let page_height = px_to_pt(page.height);

    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<idPkg:Spread xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">
    <Spread Self="spread_u{spread_num}" PageCount="1" BindingLocation="0" AllowPageShuffle="true" FlattenerOverride="Default" ItemTransform="1 0 0 1 0 0">
        <Page Self="page_u{spread_num}" Name="{page_name}" AppliedMaster="n" GeometricBounds="0 0 {page_height} {page_width}" ItemTransform="1 0 0 1 -{half_w} -{half_h}"/>
{elements}    </Spread>
</idPkg:Spread>"#,
        page_name = xml_escape(&page.name),
    );

    (xml, stories)
}

fn build_designmap(spread_refs: &[String], story_refs: &[String]) -> String {
    let mut refs = String::new();
    for path in spread_refs {
        let _ = write!(refs, "\n    <idPkg:Spread src=\"{path}\"/>");
    }
    for path in story_refs {
        let _ = write!(refs, "\n    <idPkg:Story src=\"{path}\"/>");
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Document xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">{refs}
    <idPkg:Graphic src="Resources/Graphic.xml"/>
    <idPkg:Fonts src="Resources/Fonts.xml"/>
    <idPkg:Styles src="Styles/RootParagraphStyleGroup.xml"/>
    <idPkg:Styles src="Styles/RootCharacterStyleGroup.xml"/>
</Document>"#
    )
}

const CONTAINER_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<container>
    <rootfiles>
        <rootfile full-path="designmap.xml"/>
    </rootfiles>
</container>"#;

const FONTS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<idPkg:Fonts xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">
</idPkg:Fonts>"#;

const GRAPHIC_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<idPkg:Graphic xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">
    <Color Self="Color/Black" Model="Process" Space="RGB" ColorValue="0 0 0"/>
    <Color Self="Color/White" Model="Process" Space="RGB" ColorValue="255 255 255"/>
</idPkg:Graphic>"#;

const PARAGRAPH_STYLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<idPkg:Styles xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">
    <RootParagraphStyleGroup Self="RootParagraphStyleGroup">
        <ParagraphStyle Self="ParagraphStyle/$ID/NormalParagraphStyle" Name="$ID/NormalParagraphStyle"/>
    </RootParagraphStyleGroup>
</idPkg:Styles>"#;

const CHARACTER_STYLE_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<idPkg:Styles xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging" DOMVersion="7.0">
    <RootCharacterStyleGroup Self="RootCharacterStyleGroup">
        <CharacterStyle Self="CharacterStyle/$ID/[No character style]" Name="$ID/[No character style]"/>
    </RootCharacterStyleGroup>
</idPkg:Styles>"#;

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
        let bytes = export_idml(&doc).unwrap();
        assert!(!bytes.is_empty());

        let cursor = Cursor::new(bytes);
        let archive = zip::ZipArchive::new(cursor).unwrap();
        assert!(!archive.is_empty());
    }

    #[test]
    fn export_contains_expected_parts() {
        let doc = Document::new();
        let bytes = export_idml(&doc).unwrap();

        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        assert!(archive.by_name("mimetype").is_ok());
        assert!(archive.by_name("META-INF/container.xml").is_ok());
        assert!(archive.by_name("designmap.xml").is_ok());
        assert!(archive.by_name("Resources/Fonts.xml").is_ok());
        assert!(archive.by_name("Resources/Graphic.xml").is_ok());
        assert!(
            archive
                .by_name("Styles/RootParagraphStyleGroup.xml")
                .is_ok()
        );
        assert!(
            archive
                .by_name("Styles/RootCharacterStyleGroup.xml")
                .is_ok()
        );
        assert!(archive.by_name("Spreads/Spread_u1.xml").is_ok());
    }

    #[test]
    fn export_mimetype_is_first_entry() {
        let doc = Document::new();
        let bytes = export_idml(&doc).unwrap();

        let cursor = Cursor::new(bytes);
        let archive = zip::ZipArchive::new(cursor).unwrap();
        assert_eq!(archive.name_for_index(0).unwrap(), "mimetype");
    }

    #[test]
    fn export_multiple_pages_creates_multiple_spreads() {
        let mut doc = Document::new();
        doc.add_page("Page 2", 1920.0, 1080.0);
        doc.add_page("Page 3", 1920.0, 1080.0);

        let bytes = export_idml(&doc).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        assert!(archive.by_name("Spreads/Spread_u1.xml").is_ok());
        assert!(archive.by_name("Spreads/Spread_u2.xml").is_ok());
        assert!(archive.by_name("Spreads/Spread_u3.xml").is_ok());
    }

    #[test]
    fn export_spread_contains_elements() {
        let mut doc = Document::new();
        let mut node = make_frame("TestBox", 10.0, 20.0, 100.0, 50.0);
        node.fill = Some(Color::new(1.0, 0.0, 0.0, 1.0));
        doc.active_page_mut().scene.add_root(node);

        let bytes = export_idml(&doc).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        let mut spread = archive.by_name("Spreads/Spread_u1.xml").unwrap();
        let mut xml = String::new();
        std::io::Read::read_to_string(&mut spread, &mut xml).unwrap();

        assert!(xml.contains("TestBox"));
        assert!(xml.contains("Rectangle"));
    }

    #[test]
    fn export_text_node_creates_story_file() {
        let mut doc = Document::new();
        let text_node = SceneNode::new(
            NodeId::new(),
            "Heading".to_string(),
            SceneNodeKind::Text {
                content: "Test Content".to_string(),
                font_size: 24.0,
                font_family: "Inter".to_string(),
                font_weight: 400,
                font_style: selean_engine::scene::FontStyle::Normal,
                text_align: selean_engine::scene::TextAlign::Left,
                line_height: 1.2,
                text_color: None,
            },
            BoundingBox::new(10.0, 10.0, 200.0, 50.0),
        );
        doc.active_page_mut().scene.add_root(text_node);

        let bytes = export_idml(&doc).unwrap();
        let cursor = Cursor::new(bytes);
        let mut archive = zip::ZipArchive::new(cursor).unwrap();

        // Should have a story file
        let mut found_story = false;
        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                if file.name().starts_with("Stories/") {
                    found_story = true;
                    break;
                }
            }
        }
        assert!(found_story, "expected a Story file in the archive");

        // Designmap should reference the story
        let mut dm = archive.by_name("designmap.xml").unwrap();
        let mut dm_xml = String::new();
        std::io::Read::read_to_string(&mut dm, &mut dm_xml).unwrap();
        assert!(dm_xml.contains("idPkg:Story"));
    }

    #[test]
    fn build_designmap_includes_all_refs() {
        let spreads = vec!["Spreads/Spread_u1.xml".to_string()];
        let stories = vec!["Stories/Story_u1_1.xml".to_string()];
        let xml = build_designmap(&spreads, &stories);
        assert!(xml.contains("Spread_u1.xml"));
        assert!(xml.contains("Story_u1_1.xml"));
        assert!(xml.contains("Resources/Graphic.xml"));
        assert!(xml.contains("Resources/Fonts.xml"));
    }

    #[test]
    fn build_spread_xml_structure() {
        let page = Page::new("Test Page", 816.0, 1056.0);
        let (xml, stories) = build_spread_xml(&page, 1);
        assert!(xml.contains("idPkg:Spread"));
        assert!(xml.contains("Page Self"));
        assert!(stories.is_empty());
    }
}
