//! HTTP client for the Figma REST API.
//!
//! Handles authentication and request construction for `GET /v1/files/:key`.
//! Response parsing is separated into [`parse_response`] to allow testing
//! without network calls.

use std::time::Duration;

use crate::FigmaError;
use crate::api::FigmaFileResponse;

/// Base URL for the Figma REST API.
const FIGMA_API_BASE: &str = "https://api.figma.com";

/// Request timeout for Figma API calls.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Fetches a Figma file by key.
///
/// Calls `GET /v1/files/{file_key}?geometry=paths` with the provided
/// access token in the `X-Figma-Token` header. Applies a 60-second timeout.
///
/// # Errors
///
/// Returns `FigmaError::Http` on network errors or timeout,
/// `FigmaError::Api` if the Figma API returns a non-2xx status,
/// or `FigmaError::Parse` if the response body cannot be deserialized.
pub async fn fetch_file(
    client: &reqwest::Client,
    access_token: &str,
    file_key: &str,
) -> Result<FigmaFileResponse, FigmaError> {
    let url = format!("{FIGMA_API_BASE}/v1/files/{file_key}?geometry=paths");

    let response = client
        .get(&url)
        .header("X-Figma-Token", access_token)
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await?;

    let status = response.status().as_u16();
    let body = response.text().await?;
    parse_response(status, &body)
}

/// Parses a Figma API response body given its HTTP status code.
///
/// Separated from the HTTP layer to allow testing without network calls.
///
/// # Errors
///
/// Returns `FigmaError::Api` for non-2xx status codes, or
/// `FigmaError::Parse` if the body cannot be deserialized.
pub fn parse_response(status: u16, body: &str) -> Result<FigmaFileResponse, FigmaError> {
    if !(200..300).contains(&status) {
        return Err(FigmaError::Api {
            status,
            body: body.to_string(),
        });
    }
    let file_response: FigmaFileResponse = serde_json::from_str(body)?;
    Ok(file_response)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::api::FigmaNodeType;

    #[test]
    fn figma_api_base_url_is_https() {
        assert!(FIGMA_API_BASE.starts_with("https://"));
    }

    #[test]
    fn request_timeout_is_60_seconds() {
        assert_eq!(REQUEST_TIMEOUT, Duration::from_secs(60));
    }

    #[test]
    fn parse_response_success() {
        let body = r#"{"name":"Test","document":{"id":"0:0","name":"Doc","type":"DOCUMENT"}}"#;
        let result = parse_response(200, body).unwrap();
        assert_eq!(result.name, "Test");
        assert_eq!(result.document.node_type, FigmaNodeType::Document);
    }

    #[test]
    fn parse_response_403_error() {
        let result = parse_response(403, "forbidden");
        let err = result.unwrap_err();
        match err {
            FigmaError::Api { status, body } => {
                assert_eq!(status, 403);
                assert_eq!(body, "forbidden");
            }
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn parse_response_404_error() {
        let result = parse_response(404, "not found");
        let err = result.unwrap_err();
        match err {
            FigmaError::Api { status, .. } => assert_eq!(status, 404),
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn parse_response_500_error() {
        let result = parse_response(500, "internal error");
        let err = result.unwrap_err();
        match err {
            FigmaError::Api { status, .. } => assert_eq!(status, 500),
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn parse_response_invalid_json() {
        let result = parse_response(200, "not json");
        assert!(matches!(result, Err(FigmaError::Parse(_))));
    }

    // ---- Error path tests ----

    #[test]
    fn parse_response_empty_body_200_is_parse_error() {
        let result = parse_response(200, "");
        assert!(matches!(result, Err(FigmaError::Parse(_))));
    }

    #[test]
    fn parse_response_null_json_is_parse_error() {
        let result = parse_response(200, "null");
        assert!(matches!(result, Err(FigmaError::Parse(_))));
    }

    #[test]
    fn parse_response_empty_object_uses_defaults() {
        // An empty JSON object should deserialize with all defaults.
        let result = parse_response(200, "{}").unwrap();
        assert!(result.name.is_empty());
        assert_eq!(result.document.node_type, FigmaNodeType::Unknown);
    }

    #[test]
    fn parse_response_missing_document_key_uses_default() {
        let result = parse_response(200, r#"{"name":"Test"}"#).unwrap();
        assert_eq!(result.name, "Test");
        assert_eq!(result.document.node_type, FigmaNodeType::Unknown);
    }

    #[test]
    fn parse_response_401_unauthorized() {
        let result = parse_response(401, r#"{"err": "Invalid token"}"#);
        let err = result.unwrap_err();
        match err {
            FigmaError::Api { status, body } => {
                assert_eq!(status, 401);
                assert!(body.contains("Invalid token"));
            }
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn parse_response_429_rate_limited() {
        let result = parse_response(429, "Rate limit exceeded");
        let err = result.unwrap_err();
        match err {
            FigmaError::Api { status, .. } => assert_eq!(status, 429),
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn parse_response_malformed_json_with_200() {
        // Valid-ish JSON structure but wrong types.
        let result = parse_response(200, r#"{"name": 123, "document": "not_an_object"}"#);
        assert!(matches!(result, Err(FigmaError::Parse(_))));
    }

    #[test]
    fn parse_response_boundary_status_codes() {
        // 199 is not 2xx; should be an error.
        let result = parse_response(199, "body");
        assert!(matches!(result, Err(FigmaError::Api { status: 199, .. })));

        // 300 is not 2xx; should be an error.
        let result = parse_response(300, "redirect");
        assert!(matches!(result, Err(FigmaError::Api { status: 300, .. })));

        // 200 and 299 are 2xx; should attempt parse.
        let result = parse_response(200, r#"{"name":"A","document":{"type":"DOCUMENT"}}"#);
        assert!(result.is_ok());

        let result = parse_response(299, r#"{"name":"B","document":{"type":"DOCUMENT"}}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn parse_response_partial_document_uses_defaults() {
        // Document node with only a type; everything else should default.
        let body = r#"{"name":"Partial","document":{"id":"0:0","type":"DOCUMENT"}}"#;
        let result = parse_response(200, body).unwrap();
        assert_eq!(result.document.node_type, FigmaNodeType::Document);
        assert!(result.document.children.is_empty());
        assert!(result.document.fills.is_empty());
        assert!(result.document.absolute_bounding_box.is_none());
    }
}
