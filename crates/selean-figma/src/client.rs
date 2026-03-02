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
}
