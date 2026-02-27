//! PPTX import and export for the Selean design platform.
//!
//! Provides bidirectional conversion between OOXML (.pptx) files and
//! the Selean `Document` model. Each slide maps to a `Page`, and shapes
//! map to scene nodes.

pub mod coord;
pub mod export;
pub mod import;

use selean_engine::persistence::Document;

/// Errors that can occur during PPTX import or export.
#[derive(Debug, thiserror::Error)]
pub enum PptxError {
    /// ZIP archive error (invalid or corrupted file).
    #[error("zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// XML parsing error.
    #[error("xml error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// I/O error during reading/writing.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The PPTX structure is invalid or unsupported.
    #[error("invalid format: {0}")]
    InvalidFormat(String),

    /// A required OOXML part is missing from the archive.
    #[error("missing part: {0}")]
    MissingPart(String),
}

/// Imports a PPTX file from bytes into a Document.
///
/// Each slide becomes a page. Shapes become scene nodes.
///
/// # Errors
///
/// Returns `PptxError` on ZIP, XML, or format errors.
pub fn import_pptx(bytes: &[u8]) -> Result<Document, PptxError> {
    import::import_pptx(bytes)
}

/// Exports a Document to PPTX format as bytes.
///
/// Creates a minimal OOXML-compliant .pptx file.
///
/// # Errors
///
/// Returns `PptxError` on ZIP writing errors.
pub fn export_pptx(document: &Document) -> Result<Vec<u8>, PptxError> {
    export::export_pptx(document)
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
        let bytes = export_pptx(&doc).unwrap();
        let imported = import_pptx(&bytes).unwrap();

        assert_eq!(imported.page_count(), 1);
        assert_eq!(imported.active_page().scene.len(), 1);
    }

    #[test]
    fn error_display_formats() {
        let err = PptxError::InvalidFormat("test".to_string());
        assert_eq!(err.to_string(), "invalid format: test");

        let err = PptxError::MissingPart("ppt/x.xml".to_string());
        assert_eq!(err.to_string(), "missing part: ppt/x.xml");
    }

    #[test]
    fn import_invalid_bytes_returns_error() {
        let result = import_pptx(b"not valid zip");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), PptxError::Zip(_)));
    }
}
