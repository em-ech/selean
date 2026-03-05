//! IDML import: reads an .idml file (ZIP archive) and produces a Document.
//!
//! IDML is a ZIP containing XML files organized as:
//! - `designmap.xml`: manifest listing spreads, stories, and resources
//! - `Spreads/Spread_*.xml`: page layout with page items
//! - `Stories/Story_*.xml`: text content linked from text frames

pub mod shape;
pub mod text;

use std::collections::HashMap;
use std::io::{Cursor, Read};

use selean_common::types::PageId;
use selean_common::xml::local_name;
use selean_engine::persistence::{Document, Page};
use selean_engine::scene::SceneGraph;

use crate::IdmlError;
use shape::parse_page_items;
use text::{ParsedStory, parse_story};

/// US Letter page dimensions in points.
const DEFAULT_PAGE_WIDTH_PT: f32 = 612.0;
const DEFAULT_PAGE_HEIGHT_PT: f32 = 792.0;

/// Imports an IDML file from bytes into a Document.
///
/// Two-pass approach:
/// 1. Parse `designmap.xml` to discover spread and story file paths
/// 2. Parse all stories, then parse spreads referencing the story map
///
/// Each spread becomes a page. Page items become scene nodes.
///
/// # Errors
///
/// Returns `IdmlError::Zip` if the file is not a valid ZIP archive.
/// Returns `IdmlError::Xml` if XML cannot be parsed.
/// Returns `IdmlError::InvalidFormat` for structural issues.
pub fn import_idml(bytes: &[u8]) -> Result<Document, IdmlError> {
    let cursor = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    // Parse designmap.xml for file references
    let (spread_paths, story_paths) = parse_designmap(&mut archive)?;

    if spread_paths.is_empty() {
        return Err(IdmlError::InvalidFormat(
            "no spreads found in designmap.xml".to_string(),
        ));
    }

    // Pass 1: Parse all stories
    let mut stories: HashMap<String, ParsedStory> = HashMap::new();
    for path in &story_paths {
        let xml = read_zip_entry(&mut archive, path)?;
        let (story_id, parsed) = parse_story(&xml);
        if !story_id.is_empty() {
            stories.insert(story_id, parsed);
        }
    }

    // Pass 2: Parse spreads into pages
    let half_w = DEFAULT_PAGE_WIDTH_PT / 2.0;
    let half_h = DEFAULT_PAGE_HEIGHT_PT / 2.0;
    let page_width_px = crate::coord::pt_to_px(DEFAULT_PAGE_WIDTH_PT);
    let page_height_px = crate::coord::pt_to_px(DEFAULT_PAGE_HEIGHT_PT);

    let mut pages = Vec::new();
    for (i, path) in spread_paths.iter().enumerate() {
        let xml = read_zip_entry(&mut archive, path)?;
        let nodes = parse_page_items(&xml, &stories, half_w, half_h);

        let mut scene = SceneGraph::new();
        for node in nodes {
            scene.add_root(node);
        }

        let page = Page::with_scene(
            PageId::new(),
            format!("Page {}", i + 1),
            page_width_px,
            page_height_px,
            scene,
        );
        pages.push(page);
    }

    if pages.is_empty() {
        return Err(IdmlError::InvalidFormat(
            "no pages produced from spreads".to_string(),
        ));
    }

    Ok(Document::from_pages(pages))
}

/// Parses designmap.xml to extract spread and story file paths.
fn parse_designmap(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
) -> Result<(Vec<String>, Vec<String>), IdmlError> {
    let xml = read_zip_entry(archive, "designmap.xml")?;
    Ok(parse_designmap_xml(&xml))
}

/// Extracts spread and story paths from designmap.xml content.
fn parse_designmap_xml(xml: &str) -> (Vec<String>, Vec<String>) {
    use quick_xml::Reader;
    use quick_xml::events::Event;

    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut spread_paths = Vec::new();
    let mut story_paths = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                if local == b"Spread" || local == b"Story" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"src" {
                            if let Ok(src) = std::str::from_utf8(&attr.value) {
                                if local == b"Spread" {
                                    spread_paths.push(src.to_string());
                                } else {
                                    story_paths.push(src.to_string());
                                }
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parse error in IDML spread import: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    spread_paths.sort();
    story_paths.sort();
    (spread_paths, story_paths)
}

fn read_zip_entry(
    archive: &mut zip::ZipArchive<Cursor<&[u8]>>,
    path: &str,
) -> Result<String, IdmlError> {
    let mut file = archive
        .by_name(path)
        .map_err(|_| IdmlError::MissingPart(path.to_string()))?;
    let mut xml = String::new();
    file.read_to_string(&mut xml)?;
    Ok(xml)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_designmap_extracts_paths() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <Document>
            <idPkg:Spread src="Spreads/Spread_u1.xml"/>
            <idPkg:Story src="Stories/Story_u2.xml"/>
            <idPkg:Story src="Stories/Story_u3.xml"/>
        </Document>"#;
        let (spreads, stories) = parse_designmap_xml(xml);
        assert_eq!(spreads.len(), 1);
        assert_eq!(stories.len(), 2);
        assert_eq!(spreads[0], "Spreads/Spread_u1.xml");
    }

    #[test]
    fn parse_designmap_empty() {
        let xml = r"<Document></Document>";
        let (spreads, stories) = parse_designmap_xml(xml);
        assert!(spreads.is_empty());
        assert!(stories.is_empty());
    }

    #[test]
    fn import_rejects_invalid_zip() {
        let result = import_idml(b"not a zip file");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IdmlError::Zip(_)));
    }

    #[test]
    fn parse_designmap_multiple_spreads_sorted() {
        let xml = r#"<Document>
            <idPkg:Spread src="Spreads/Spread_z.xml"/>
            <idPkg:Spread src="Spreads/Spread_a.xml"/>
        </Document>"#;
        let (spreads, _) = parse_designmap_xml(xml);
        assert_eq!(spreads.len(), 2);
        assert_eq!(spreads[0], "Spreads/Spread_a.xml");
        assert_eq!(spreads[1], "Spreads/Spread_z.xml");
    }

    #[test]
    fn parse_designmap_namespaced_elements() {
        let xml = r#"<Document xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging">
            <idPkg:Spread src="Spreads/Spread_u1.xml"/>
            <idPkg:Story src="Stories/Story_u2.xml"/>
        </Document>"#;
        let (spreads, stories) = parse_designmap_xml(xml);
        assert_eq!(spreads.len(), 1);
        assert_eq!(stories.len(), 1);
    }
}
