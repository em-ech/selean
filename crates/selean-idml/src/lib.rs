//! IDML import and export for the Selean design platform.
//!
//! Provides bidirectional conversion between IDML (.idml) files and
//! the Selean `Document` model. Each spread maps to a `Page`, and page
//! items map to scene nodes.
//!
//! IDML is a ZIP archive of XML files with separate story-based text
//! storage, spread-center coordinate system, and `PathGeometry`-based
//! shape definitions.

pub mod coord;
pub mod export;
pub mod import;

use selean_engine::persistence::Document;

/// Errors that can occur during IDML import or export.
#[derive(Debug, thiserror::Error)]
pub enum IdmlError {
    /// ZIP archive error (invalid or corrupted file).
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// XML parsing error.
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// I/O error during reading/writing.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The IDML structure is invalid or unsupported.
    #[error("invalid format: {0}")]
    InvalidFormat(String),

    /// A required IDML part is missing from the archive.
    #[error("missing part: {0}")]
    MissingPart(String),
}

/// Imports an IDML file from bytes into a Document.
///
/// Each spread becomes a page. Page items become scene nodes.
/// Text frames are resolved against their linked stories.
///
/// # Errors
///
/// Returns `IdmlError` on ZIP, XML, or format errors.
pub fn import_idml(bytes: &[u8]) -> Result<Document, IdmlError> {
    import::import_idml(bytes)
}

/// Exports a Document to IDML format as bytes.
///
/// Creates a minimal IDML-compliant .idml file with spreads, stories,
/// and required resource stubs.
///
/// # Errors
///
/// Returns `IdmlError` on ZIP writing errors.
pub fn export_idml(document: &Document) -> Result<Vec<u8>, IdmlError> {
    export::export_idml(document)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::scene::{BoundingBox, Color, SceneNode, SceneNodeKind};

    fn make_test_document() -> Document {
        let mut doc = Document::new();
        let mut node = SceneNode::new(
            NodeId::new(),
            "Rect".to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(100.0, 100.0, 200.0, 150.0),
        );
        node.fill = Some(Color::new(0.0, 0.5, 1.0, 1.0));
        doc.active_page_mut().scene.add_root(node);
        doc
    }

    #[test]
    fn export_then_import_roundtrip() {
        let doc = make_test_document();
        let bytes = export_idml(&doc).unwrap();
        let imported = import_idml(&bytes).unwrap();

        assert_eq!(imported.page_count(), 1);
        assert_eq!(imported.active_page().scene.len(), 1);
    }

    #[test]
    fn error_display_formats() {
        let err = IdmlError::InvalidFormat("test".to_string());
        assert_eq!(err.to_string(), "invalid format: test");

        let err = IdmlError::MissingPart("designmap.xml".to_string());
        assert_eq!(err.to_string(), "missing part: designmap.xml");
    }

    #[test]
    fn import_invalid_bytes_returns_error() {
        let result = import_idml(b"not valid zip");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), IdmlError::Zip(_)));
    }
}
