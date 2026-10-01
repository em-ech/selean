//! Shared XML helper functions for parsing and serializing XML elements.
//!
//! These utilities are used across multiple import crates (selean-pptx, selean-idml)
//! to avoid duplicating namespace-stripping and tag-building logic.

use quick_xml::escape::resolve_xml_entity;
use quick_xml::events::{BytesEnd, BytesRef, BytesStart};

/// Extracts the local name from a potentially namespaced XML tag.
///
/// Given `b"a:srgbClr"`, returns `b"srgbClr"`. If there is no namespace
/// prefix, returns the input unchanged.
pub fn local_name(name: &[u8]) -> &[u8] {
    match name.iter().rposition(|&b| b == b':') {
        Some(pos) => &name[pos + 1..],
        None => name,
    }
}

/// Appends an entity or character reference (`&amp;`, `&#233;`) to the buffer
/// exactly as it appeared in the source.
///
/// The reader reports references as their own events, separate from the
/// surrounding text, so code that re-serializes a fragment must write them
/// back or the referenced characters are lost.
pub fn append_general_ref(buf: &mut String, e: &BytesRef<'_>) {
    buf.push('&');
    buf.push_str(&String::from_utf8_lossy(e.as_ref()));
    buf.push(';');
}

/// Resolves an entity or character reference to the text it stands for.
///
/// Handles numeric references (`&#233;`, `&#xE9;`) and the five predefined
/// XML entities. Returns `None` for anything else (custom or malformed
/// references).
pub fn resolve_general_ref(e: &BytesRef<'_>) -> Option<String> {
    if e.is_char_ref() {
        return e.resolve_char_ref().ok().flatten().map(String::from);
    }
    let name = e.decode().ok()?;
    resolve_xml_entity(&name).map(str::to_owned)
}

/// Appends an opening XML tag with attributes to the buffer.
///
/// Produces `<tagName attr1="val1" attr2="val2">`.
pub fn append_start_tag(s: &mut String, e: &BytesStart<'_>) {
    s.push('<');
    s.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        s.push(' ');
        s.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        s.push_str("=\"");
        s.push_str(&String::from_utf8_lossy(&attr.value));
        s.push('"');
    }
    s.push('>');
}

/// Appends a closing XML tag to the buffer.
///
/// Produces `</tagName>`.
pub fn append_end_tag(s: &mut String, e: &BytesEnd<'_>) {
    s.push_str("</");
    s.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    s.push('>');
}

/// Escapes the five XML special characters in a string for safe embedding
/// in XML text content or attribute values.
///
/// Replaces `&`, `<`, `>`, `"`, and `'` with their corresponding XML entities.
#[must_use]
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Appends a self-closing XML tag with attributes to the buffer.
///
/// Produces `<tagName attr1="val1" attr2="val2"/>`.
pub fn append_empty_tag(s: &mut String, e: &BytesStart<'_>) {
    s.push('<');
    s.push_str(&String::from_utf8_lossy(e.name().as_ref()));
    for attr in e.attributes().flatten() {
        s.push(' ');
        s.push_str(&String::from_utf8_lossy(attr.key.as_ref()));
        s.push_str("=\"");
        s.push_str(&String::from_utf8_lossy(&attr.value));
        s.push('"');
    }
    s.push_str("/>");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_name_strips_namespace() {
        assert_eq!(local_name(b"a:srgbClr"), b"srgbClr");
    }

    #[test]
    fn local_name_no_namespace() {
        assert_eq!(local_name(b"Rectangle"), b"Rectangle");
    }

    #[test]
    fn local_name_empty() {
        assert_eq!(local_name(b""), b"");
    }

    #[test]
    fn local_name_multiple_colons() {
        assert_eq!(local_name(b"ns1:ns2:tag"), b"tag");
    }

    #[test]
    fn local_name_colon_at_end() {
        assert_eq!(local_name(b"ns:"), b"");
    }

    #[test]
    fn append_start_tag_no_attrs() {
        let mut buf = String::new();
        let e = BytesStart::new("div");
        append_start_tag(&mut buf, &e);
        assert_eq!(buf, "<div>");
    }

    #[test]
    fn append_start_tag_with_attrs() {
        let mut buf = String::new();
        let mut e = BytesStart::new("span");
        e.push_attribute(("class", "bold"));
        e.push_attribute(("id", "x"));
        append_start_tag(&mut buf, &e);
        assert_eq!(buf, r#"<span class="bold" id="x">"#);
    }

    #[test]
    fn append_end_tag_basic() {
        let mut buf = String::new();
        let e = BytesEnd::new("div");
        append_end_tag(&mut buf, &e);
        assert_eq!(buf, "</div>");
    }

    #[test]
    fn append_empty_tag_no_attrs() {
        let mut buf = String::new();
        let e = BytesStart::new("br");
        append_empty_tag(&mut buf, &e);
        assert_eq!(buf, "<br/>");
    }

    #[test]
    fn append_empty_tag_with_attrs() {
        let mut buf = String::new();
        let mut e = BytesStart::new("img");
        e.push_attribute(("src", "pic.png"));
        append_empty_tag(&mut buf, &e);
        assert_eq!(buf, r#"<img src="pic.png"/>"#);
    }

    #[test]
    fn append_general_ref_writes_reference_back() {
        let mut buf = String::from("A ");
        append_general_ref(&mut buf, &BytesRef::new("amp"));
        append_general_ref(&mut buf, &BytesRef::new("#233"));
        assert_eq!(buf, "A &amp;&#233;");
    }

    #[test]
    fn resolve_general_ref_predefined_entities() {
        let cases = [
            ("amp", "&"),
            ("lt", "<"),
            ("gt", ">"),
            ("quot", "\""),
            ("apos", "'"),
        ];
        for (name, expected) in cases {
            assert_eq!(
                resolve_general_ref(&BytesRef::new(name)).as_deref(),
                Some(expected),
                "entity {name}"
            );
        }
    }

    #[test]
    fn resolve_general_ref_numeric_references() {
        assert_eq!(
            resolve_general_ref(&BytesRef::new("#233")).as_deref(),
            Some("\u{e9}")
        );
        assert_eq!(
            resolve_general_ref(&BytesRef::new("#xE9")).as_deref(),
            Some("\u{e9}")
        );
    }

    #[test]
    fn resolve_general_ref_unknown_or_malformed_is_none() {
        assert_eq!(resolve_general_ref(&BytesRef::new("nbsp")), None);
        assert_eq!(resolve_general_ref(&BytesRef::new("#zz")), None);
        assert_eq!(resolve_general_ref(&BytesRef::new("#0")), None);
    }

    #[test]
    fn xml_escape_all_special_chars() {
        assert_eq!(
            xml_escape(r#"A & B < C > D " E ' F"#),
            "A &amp; B &lt; C &gt; D &quot; E &apos; F"
        );
    }

    #[test]
    fn xml_escape_no_special_chars() {
        assert_eq!(xml_escape("hello world"), "hello world");
    }

    #[test]
    fn xml_escape_empty_string() {
        assert_eq!(xml_escape(""), "");
    }

    #[test]
    fn xml_escape_only_ampersands() {
        assert_eq!(xml_escape("&&"), "&amp;&amp;");
    }
}
