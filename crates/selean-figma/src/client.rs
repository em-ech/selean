//! HTTP client for the Figma REST API.
//!
//! Handles authentication and request construction for `GET /v1/files/:key`.

use crate::FigmaError;
use crate::api::FigmaFileResponse;

/// Base URL for the Figma REST API.
const FIGMA_API_BASE: &str = "https://api.figma.com";

/// Fetches a Figma file by key.
///
/// Calls `GET /v1/files/{file_key}?geometry=paths` with the provided
/// access token in the `X-Figma-Token` header.
///
/// # Errors
///
/// Returns `FigmaError::Http` on network errors, `FigmaError::Api` if
/// the Figma API returns a non-200 status, or `FigmaError::Parse` if
/// the response body cannot be deserialized.
pub async fn fetch_file(
    client: &reqwest::Client,
    access_token: &str,
    file_key: &str,
) -> Result<FigmaFileResponse, FigmaError> {
    let url = format!("{FIGMA_API_BASE}/v1/files/{file_key}?geometry=paths");

    let response = client
        .get(&url)
        .header("X-Figma-Token", access_token)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(FigmaError::Api(format!("{status}: {body}")));
    }

    let file_response: FigmaFileResponse = response.json().await?;
    Ok(file_response)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figma_api_base_url_is_https() {
        assert!(FIGMA_API_BASE.starts_with("https://"));
    }
}
