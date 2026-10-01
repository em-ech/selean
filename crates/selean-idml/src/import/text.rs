//! Story XML parsing for IDML import.
//!
//! IDML stores text content in separate `Stories/Story_*.xml` files,
//! linked to text frames via a `ParentStory` attribute. Each story
//! contains `ParagraphStyleRange` > `CharacterStyleRange` > `Content`
//! elements.

use quick_xml::Reader;
use quick_xml::events::Event;

use selean_common::xml::{local_name, resolve_general_ref};
use selean_engine::scene::{Color, FontStyle, TextAlign};

use crate::coord::{parse_idml_color, parse_idml_font_size};

/// Parsed text properties from an IDML story.
#[derive(Debug, Clone)]
pub struct ParsedStory {
    /// Concatenated text content from all runs.
    pub content: String,
    /// Font size in pixels.
    pub font_size: f32,
    /// Font family name.
    pub font_family: String,
    /// Font weight (400 = normal, 700 = bold).
    pub font_weight: u16,
    /// Font style.
    pub font_style: FontStyle,
    /// Text alignment.
    pub text_align: TextAlign,
    /// Text color.
    pub text_color: Option<Color>,
}

impl Default for ParsedStory {
    fn default() -> Self {
        Self {
            content: String::new(),
            font_size: 16.0,
            font_family: "Inter".to_string(),
            font_weight: 400,
            font_style: FontStyle::Normal,
            text_align: TextAlign::Left,
            text_color: None,
        }
    }
}

/// Parses a Story XML file into a `(story_id, ParsedStory)` pair.
///
/// Extracts the story `Self` attribute as the ID, then iterates through
/// paragraph and character style ranges to extract text content and formatting.
pub fn parse_story(xml: &str) -> (String, ParsedStory) {
    let mut story_id = String::new();
    let mut result = ParsedStory::default();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();

    let mut in_content = false;
    let mut in_char_range = false;
    let mut formatting_set = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"Story" => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"Self" {
                                if let Ok(v) = std::str::from_utf8(&attr.value) {
                                    story_id = v.to_string();
                                }
                            }
                        }
                    }
                    b"ParagraphStyleRange" => {
                        parse_paragraph_attrs(e, &mut result);
                    }
                    b"CharacterStyleRange" => {
                        in_char_range = true;
                        if !formatting_set {
                            parse_character_attrs(e, &mut result);
                            formatting_set = true;
                        }
                    }
                    b"Content" => {
                        in_content = true;
                    }
                    b"Properties" | b"AppliedFont" => {}
                    _ => {
                        if in_char_range {
                            parse_inline_element(local, e, &mut result, formatting_set);
                        }
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"Content" => in_content = false,
                    b"CharacterStyleRange" => in_char_range = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) => {
                if in_content {
                    if let Ok(text) = e.decode() {
                        result.content.push_str(&text);
                    }
                }
            }
            // Entity and character references arrive as separate events.
            Ok(Event::GeneralRef(ref e)) if in_content => {
                if let Some(text) = resolve_general_ref(e) {
                    result.content.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parse error in IDML text import: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    (story_id, result)
}

fn parse_paragraph_attrs(e: &quick_xml::events::BytesStart<'_>, result: &mut ParsedStory) {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"Justification" {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                result.text_align = match val {
                    "CenterAlign" | "CenterJustified" => TextAlign::Center,
                    "RightAlign" | "RightJustified" => TextAlign::Right,
                    "FullyJustified" => TextAlign::Justify,
                    _ => TextAlign::Left,
                };
            }
        }
    }
}

fn parse_character_attrs(e: &quick_xml::events::BytesStart<'_>, result: &mut ParsedStory) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"PointSize" => {
                if let Ok(val) = std::str::from_utf8(&attr.value) {
                    result.font_size = parse_idml_font_size(val);
                }
            }
            b"FontStyle" => {
                if let Ok(val) = std::str::from_utf8(&attr.value) {
                    let lower = val.to_lowercase();
                    if lower.contains("bold") {
                        result.font_weight = 700;
                    }
                    if lower.contains("italic") {
                        result.font_style = FontStyle::Italic;
                    }
                }
            }
            b"AppliedFont" => {
                if let Ok(val) = std::str::from_utf8(&attr.value) {
                    result.font_family = val.to_string();
                }
            }
            b"FillColor" => {
                // FillColor can be "Color/C=0 M=0 Y=0 K=100" (CMYK) or direct color ref.
                // We only handle simple named swatch for now.
                if let Ok(val) = std::str::from_utf8(&attr.value) {
                    if val == "Color/Black" {
                        result.text_color = Some(Color::new(0.0, 0.0, 0.0, 1.0));
                    } else if val == "Color/White" {
                        result.text_color = Some(Color::new(1.0, 1.0, 1.0, 1.0));
                    }
                }
            }
            _ => {}
        }
    }
}

fn parse_inline_element(
    local: &[u8],
    e: &quick_xml::events::BytesStart<'_>,
    result: &mut ParsedStory,
    formatting_set: bool,
) {
    if local == b"FillColor" && !formatting_set {
        // <FillColor type="enumeration">Color/...</FillColor> pattern
        // is handled differently; skip for now.
    }
    // Handle <AppliedFont> and color references inside properties
    if local == b"ColorValue" && !formatting_set {
        // IDML uses <ColorValue type="list">r g b</ColorValue> for RGB
        // Parsed from text events; attribute-based parsing here is a no-op.
    }
    // Pick up inline color from attributes
    if local == b"sRGBColor" || local == b"RGBColor" {
        let mut r_str = String::new();
        let mut g_str = String::new();
        let mut b_str = String::new();
        let mut idx = 0;
        for attr in e.attributes().flatten() {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                match idx {
                    0 => r_str = val.to_string(),
                    1 => g_str = val.to_string(),
                    2 => b_str = val.to_string(),
                    _ => {}
                }
                idx += 1;
            }
        }
        if !r_str.is_empty() {
            if let Some(c) = parse_idml_color(&r_str, &g_str, &b_str) {
                result.text_color = Some(c);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_story() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <idPkg:Story xmlns:idPkg="http://ns.adobe.com/AdobeInDesign/idml/1.0/packaging">
            <Story Self="story_1">
                <ParagraphStyleRange>
                    <CharacterStyleRange>
                        <Content>Hello World</Content>
                    </CharacterStyleRange>
                </ParagraphStyleRange>
            </Story>
        </idPkg:Story>"#;
        let (id, story) = parse_story(xml);
        assert_eq!(id, "story_1");
        assert_eq!(story.content, "Hello World");
    }

    #[test]
    fn parse_story_resolves_entities_and_char_refs() {
        let xml = r#"<Story Self="s_ent">
            <ParagraphStyleRange>
                <CharacterStyleRange>
                    <Content>R&amp;D &lt;caf&#233;&gt; &#x41;</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.content, "R&D <caf\u{e9}> A");
    }

    #[test]
    fn parse_story_with_font_size() {
        let xml = r#"<Story Self="s2">
            <ParagraphStyleRange>
                <CharacterStyleRange PointSize="24">
                    <Content>Big text</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.content, "Big text");
        assert!((story.font_size - 32.0).abs() < 0.01);
    }

    #[test]
    fn parse_story_bold() {
        let xml = r#"<Story Self="s3">
            <ParagraphStyleRange>
                <CharacterStyleRange FontStyle="Bold">
                    <Content>Bold</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.font_weight, 700);
    }

    #[test]
    fn parse_story_italic() {
        let xml = r#"<Story Self="s4">
            <ParagraphStyleRange>
                <CharacterStyleRange FontStyle="Italic">
                    <Content>Italic</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.font_style, FontStyle::Italic);
    }

    #[test]
    fn parse_story_bold_italic() {
        let xml = r#"<Story Self="s5">
            <ParagraphStyleRange>
                <CharacterStyleRange FontStyle="Bold Italic">
                    <Content>Both</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.font_weight, 700);
        assert_eq!(story.font_style, FontStyle::Italic);
    }

    #[test]
    fn parse_story_center_aligned() {
        let xml = r#"<Story Self="s6">
            <ParagraphStyleRange Justification="CenterAlign">
                <CharacterStyleRange>
                    <Content>Center</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.text_align, TextAlign::Center);
    }

    #[test]
    fn parse_story_right_aligned() {
        let xml = r#"<Story Self="s7">
            <ParagraphStyleRange Justification="RightAlign">
                <CharacterStyleRange>
                    <Content>Right</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.text_align, TextAlign::Right);
    }

    #[test]
    fn parse_story_justified() {
        let xml = r#"<Story Self="s8">
            <ParagraphStyleRange Justification="FullyJustified">
                <CharacterStyleRange>
                    <Content>Justified</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.text_align, TextAlign::Justify);
    }

    #[test]
    fn parse_story_font_family() {
        let xml = r#"<Story Self="s9">
            <ParagraphStyleRange>
                <CharacterStyleRange AppliedFont="Helvetica">
                    <Content>Text</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.font_family, "Helvetica");
    }

    #[test]
    fn parse_story_multiple_content_elements() {
        let xml = r#"<Story Self="s10">
            <ParagraphStyleRange>
                <CharacterStyleRange>
                    <Content>Hello </Content>
                </CharacterStyleRange>
                <CharacterStyleRange>
                    <Content>World</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.content, "Hello World");
    }

    #[test]
    fn default_parsed_story() {
        let result = ParsedStory::default();
        assert!(result.content.is_empty());
        assert_eq!(result.font_size, 16.0);
        assert_eq!(result.font_family, "Inter");
        assert_eq!(result.font_weight, 400);
        assert_eq!(result.font_style, FontStyle::Normal);
        assert_eq!(result.text_align, TextAlign::Left);
        assert!(result.text_color.is_none());
    }

    // ---- Error path tests ----

    #[test]
    fn parse_story_missing_self_id_returns_empty_id() {
        let xml = r"<Story>
            <ParagraphStyleRange>
                <CharacterStyleRange>
                    <Content>Text</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>";
        let (id, story) = parse_story(xml);
        assert!(id.is_empty());
        assert_eq!(story.content, "Text");
    }

    #[test]
    fn parse_story_empty_xml_returns_defaults() {
        let xml = "";
        let (id, story) = parse_story(xml);
        assert!(id.is_empty());
        assert!(story.content.is_empty());
        assert_eq!(story.font_size, 16.0);
    }

    #[test]
    fn parse_story_malformed_xml_does_not_panic() {
        let xml = r"<Story Self='s1'><broken<<>>";
        let (id, _story) = parse_story(xml);
        // Should not panic; may extract partial data.
        let _ = id;
    }

    #[test]
    fn parse_story_unknown_justification_defaults_left() {
        let xml = r#"<Story Self="s1">
            <ParagraphStyleRange Justification="SomeUnknownValue">
                <CharacterStyleRange>
                    <Content>Text</Content>
                </CharacterStyleRange>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert_eq!(story.text_align, TextAlign::Left);
    }

    #[test]
    fn parse_story_no_content_elements_returns_empty() {
        let xml = r#"<Story Self="s1">
            <ParagraphStyleRange>
                <CharacterStyleRange PointSize="24"/>
            </ParagraphStyleRange>
        </Story>"#;
        let (_, story) = parse_story(xml);
        assert!(story.content.is_empty());
        assert!((story.font_size - 32.0).abs() < 0.01);
    }
}
