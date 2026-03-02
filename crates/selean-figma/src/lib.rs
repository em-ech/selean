//! Figma REST API import for the Selean design platform.
//!
//! Fetches a Figma file via `GET /v1/files/:key?geometry=paths` and converts
//! the node tree into a Selean `Document`. Each Figma canvas (page) maps to a
//! `Page`, and nodes map to scene nodes based on their type.
//!
//! Import only in v1. Figma does not expose a simple write API, so export
//! is not supported.

pub mod api;
pub mod client;
pub mod color;
pub mod convert;

use selean_engine::persistence::Document;

/// Errors that can occur during Figma import.
#[derive(Debug, thiserror::Error)]
pub enum FigmaError {
    /// HTTP request to the Figma API failed.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// The Figma API returned an error response.
    #[error("figma api error ({status}): {body}")]
    Api {
        /// HTTP status code from the Figma API.
        status: u16,
        /// Response body text.
        body: String,
    },

    /// Failed to parse the Figma API response JSON.
    #[error("parse error: {0}")]
    Parse(#[from] serde_json::Error),

    /// The file structure is invalid (e.g. no canvases).
    #[error("invalid file: {0}")]
    InvalidFile(String),
}

/// Imports a Figma file into a Selean Document.
///
/// Fetches the file from the Figma REST API and converts nodes into
/// the engine scene graph representation.
///
/// # Errors
///
/// Returns `FigmaError` on HTTP, API, parse, or conversion errors.
pub async fn import_figma(
    http_client: &reqwest::Client,
    access_token: &str,
    file_key: &str,
) -> Result<Document, FigmaError> {
    let response = client::fetch_file(http_client, access_token, file_key).await?;
    convert::file_to_document(&response)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats() {
        let err = FigmaError::Api {
            status: 403,
            body: "forbidden".to_string(),
        };
        assert_eq!(err.to_string(), "figma api error (403): forbidden");

        let err = FigmaError::InvalidFile("no canvases".to_string());
        assert_eq!(err.to_string(), "invalid file: no canvases");
    }

    #[test]
    fn error_display_api_with_status() {
        let err = FigmaError::Api {
            status: 404,
            body: "not found".to_string(),
        };
        assert!(err.to_string().contains("404"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn integration_file_to_document() {
        let response = api::FigmaFileResponse {
            name: "Test File".to_string(),
            document: api::FigmaNode {
                id: "0:0".to_string(),
                name: "Document".to_string(),
                node_type: api::FigmaNodeType::Document,
                children: vec![api::FigmaNode {
                    id: "0:1".to_string(),
                    name: "Page 1".to_string(),
                    node_type: api::FigmaNodeType::Canvas,
                    children: vec![api::FigmaNode {
                        id: "1:1".to_string(),
                        name: "Rect".to_string(),
                        node_type: api::FigmaNodeType::Rectangle,
                        absolute_bounding_box: Some(api::FigmaRect {
                            x: 0.0,
                            y: 0.0,
                            width: 200.0,
                            height: 100.0,
                        }),
                        fills: vec![api::FigmaPaint {
                            paint_type: "SOLID".to_string(),
                            color: Some(api::FigmaColor {
                                r: 1.0,
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                        corner_radius: 8.0,
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
        };

        let doc = convert::file_to_document(&response).unwrap();
        assert_eq!(doc.page_count(), 1);
        assert_eq!(doc.active_page().name, "Page 1");
        assert_eq!(doc.active_page().scene.len(), 1);
    }

    #[test]
    fn integration_multi_canvas() {
        let response = api::FigmaFileResponse {
            name: "Multi".to_string(),
            document: api::FigmaNode {
                node_type: api::FigmaNodeType::Document,
                children: vec![make_canvas("Page A", vec![]), make_canvas("Page B", vec![])],
                ..Default::default()
            },
        };

        let doc = convert::file_to_document(&response).unwrap();
        assert_eq!(doc.page_count(), 2);
        assert_eq!(doc.pages()[0].name, "Page A");
        assert_eq!(doc.pages()[1].name, "Page B");
    }

    fn make_canvas(name: &str, children: Vec<api::FigmaNode>) -> api::FigmaNode {
        api::FigmaNode {
            id: "0:1".to_string(),
            name: name.to_string(),
            node_type: api::FigmaNodeType::Canvas,
            children,
            ..Default::default()
        }
    }
}
