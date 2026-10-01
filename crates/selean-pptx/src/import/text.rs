//! Text run parsing from OOXML shape XML.

use quick_xml::Reader;
use quick_xml::events::Event;

use selean_common::xml::{local_name, resolve_general_ref};
use selean_engine::scene::{Color, FontStyle, TextAlign};

use crate::coord::{ooxml_font_size_to_px, parse_ooxml_color};

/// Parsed text properties from a PPTX shape's text body.
#[derive(Debug, Clone)]
pub struct ParsedText {
    /// Concatenated text content from all runs.
    pub content: String,
    /// Font size in pixels (from the first run with a size, or default 16.0).
    pub font_size: f32,
    /// Font family name.
    pub font_family: String,
    /// Font weight (400 = normal, 700 = bold).
    pub font_weight: u16,
    /// Font style.
    pub font_style: FontStyle,
    /// Text alignment.
    pub text_align: TextAlign,
    /// Text color from run properties.
    pub text_color: Option<Color>,
}

impl Default for ParsedText {
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

/// Parses text body XML fragment into a `ParsedText`.
///
/// Expects XML containing `<a:txBody>` elements with `<a:p>` paragraphs
/// and `<a:r>` runs.
pub fn parse_text_body(xml: &str) -> ParsedText {
    let mut result = ParsedText::default();
    let mut reader = Reader::from_str(xml);
    let mut buf = Vec::new();
    let mut in_run = false;
    let mut in_text = false;
    let mut font_size_set = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"r" => in_run = true,
                    b"t" => in_text = true,
                    b"rPr" if in_run => {
                        parse_run_properties(e, &mut result, &mut font_size_set);
                    }
                    b"pPr" => {
                        parse_paragraph_properties(e, &mut result);
                    }
                    b"latin" | b"cs" | b"ea" if in_run => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"typeface" {
                                if let Ok(face) = std::str::from_utf8(&attr.value) {
                                    result.font_family = face.to_string();
                                }
                            }
                        }
                    }
                    b"srgbClr" if in_run => {
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"val" {
                                if let Ok(hex) = std::str::from_utf8(&attr.value) {
                                    if let Some(c) = parse_ooxml_color(hex) {
                                        result.text_color = Some(c);
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let qname = e.name();
                let local = local_name(qname.as_ref());
                match local {
                    b"r" => in_run = false,
                    b"t" => in_text = false,
                    _ => {}
                }
            }
            Ok(Event::Text(ref e)) if in_text => {
                if let Ok(text) = e.decode() {
                    result.content.push_str(&text);
                }
            }
            // Entity and character references arrive as separate events.
            Ok(Event::GeneralRef(ref e)) if in_text => {
                if let Some(text) = resolve_general_ref(e) {
                    result.content.push_str(&text);
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                tracing::warn!("XML parse error in PPTX text import: {e}");
                break;
            }
            _ => {}
        }
        buf.clear();
    }

    result
}

fn parse_run_properties(
    e: &quick_xml::events::BytesStart<'_>,
    result: &mut ParsedText,
    font_size_set: &mut bool,
) {
    for attr in e.attributes().flatten() {
        match attr.key.as_ref() {
            b"sz" => {
                if !*font_size_set {
                    if let Ok(val) = std::str::from_utf8(&attr.value) {
                        if let Ok(hundredths) = val.parse::<i32>() {
                            result.font_size = ooxml_font_size_to_px(hundredths);
                            *font_size_set = true;
                        }
                    }
                }
            }
            b"b" => {
                if let Ok(val) = std::str::from_utf8(&attr.value) {
                    if val == "1" || val == "true" {
                        result.font_weight = 700;
                    }
                }
            }
            b"i" => {
                if let Ok(val) = std::str::from_utf8(&attr.value) {
                    if val == "1" || val == "true" {
                        result.font_style = FontStyle::Italic;
                    }
                }
            }
            _ => {}
        }
    }
}

fn parse_paragraph_properties(e: &quick_xml::events::BytesStart<'_>, result: &mut ParsedText) {
    for attr in e.attributes().flatten() {
        if attr.key.as_ref() == b"algn" {
            if let Ok(val) = std::str::from_utf8(&attr.value) {
                result.text_align = match val {
                    "ctr" => TextAlign::Center,
                    "r" => TextAlign::Right,
                    "just" => TextAlign::Justify,
                    _ => TextAlign::Left,
                };
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_text() {
        let xml = r"<a:txBody><a:p><a:r><a:t>Hello World</a:t></a:r></a:p></a:txBody>";
        let result = parse_text_body(xml);
        assert_eq!(result.content, "Hello World");
    }

    #[test]
    fn parse_text_resolves_entities_and_char_refs() {
        let xml = r"<a:txBody><a:p><a:r><a:t>R&amp;D &lt;caf&#233;&gt; &#x41;</a:t></a:r></a:p></a:txBody>";
        let result = parse_text_body(xml);
        assert_eq!(result.content, "R&D <caf\u{e9}> A");
    }

    #[test]
    fn parse_text_ignores_references_outside_text_runs() {
        let xml = r"<a:txBody><a:p>&amp;<a:r><a:t>Hi</a:t></a:r></a:p></a:txBody>";
        let result = parse_text_body(xml);
        assert_eq!(result.content, "Hi");
    }

    #[test]
    fn parse_text_with_font_size() {
        let xml = r#"<a:txBody><a:p><a:r><a:rPr sz="2400"/><a:t>Big</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.content, "Big");
        assert!((result.font_size - 32.0).abs() < 0.1);
    }

    #[test]
    fn parse_bold_text() {
        let xml = r#"<a:txBody><a:p><a:r><a:rPr b="1"/><a:t>Bold</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.font_weight, 700);
    }

    #[test]
    fn parse_italic_text() {
        let xml = r#"<a:txBody><a:p><a:r><a:rPr i="1"/><a:t>Italic</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.font_style, FontStyle::Italic);
    }

    #[test]
    fn parse_centered_text() {
        let xml =
            r#"<a:txBody><a:p><a:pPr algn="ctr"/><a:r><a:t>Center</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.text_align, TextAlign::Center);
    }

    #[test]
    fn parse_right_aligned_text() {
        let xml = r#"<a:txBody><a:p><a:pPr algn="r"/><a:r><a:t>Right</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.text_align, TextAlign::Right);
    }

    #[test]
    fn parse_justified_text() {
        let xml =
            r#"<a:txBody><a:p><a:pPr algn="just"/><a:r><a:t>Just</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.text_align, TextAlign::Justify);
    }

    #[test]
    fn parse_text_color() {
        let xml = r#"<a:txBody><a:p><a:r><a:rPr><a:solidFill><a:srgbClr val="FF0000"/></a:solidFill></a:rPr><a:t>Red</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert!(result.text_color.is_some());
        let c = result.text_color.unwrap();
        assert!((c.r - 1.0).abs() < 0.01);
    }

    #[test]
    fn parse_font_family() {
        let xml = r#"<a:txBody><a:p><a:r><a:rPr><a:latin typeface="Arial"/></a:rPr><a:t>Text</a:t></a:r></a:p></a:txBody>"#;
        let result = parse_text_body(xml);
        assert_eq!(result.font_family, "Arial");
    }

    #[test]
    fn empty_text_body() {
        let xml = r"<a:txBody><a:p/></a:txBody>";
        let result = parse_text_body(xml);
        assert!(result.content.is_empty());
        assert_eq!(result.font_size, 16.0);
    }

    #[test]
    fn default_parsed_text() {
        let result = ParsedText::default();
        assert!(result.content.is_empty());
        assert_eq!(result.font_size, 16.0);
        assert_eq!(result.font_family, "Inter");
        assert_eq!(result.font_weight, 400);
        assert_eq!(result.font_style, FontStyle::Normal);
        assert_eq!(result.text_align, TextAlign::Left);
        assert!(result.text_color.is_none());
    }

    #[test]
    fn parse_multiple_runs() {
        let xml = r"<a:txBody><a:p><a:r><a:t>Hello </a:t></a:r><a:r><a:t>World</a:t></a:r></a:p></a:txBody>";
        let result = parse_text_body(xml);
        assert_eq!(result.content, "Hello World");
    }
}
