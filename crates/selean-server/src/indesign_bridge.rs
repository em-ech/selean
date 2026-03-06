//! `InDesign` Server bridge for `.indd` to IDML conversion.
//!
//! When `INDESIGN_SERVER_URL` is configured, this module handles
//! sending `.indd` files to `InDesign` Server for conversion to IDML format.

use selean_engine::persistence::Document;

/// Errors from `InDesign` Server communication.
#[derive(Debug, thiserror::Error)]
pub enum IndesignBridgeError {
    /// The `INDESIGN_SERVER_URL` environment variable is not set.
    #[error(
        "InDesign Server not configured. Export your file as IDML (.idml) \
         from InDesign and use Import IDML instead."
    )]
    NotConfigured,

    /// Network or HTTP error communicating with `InDesign` Server.
    #[error("InDesign Server request failed: {0}")]
    Http(#[from] reqwest::Error),

    /// `InDesign` Server returned a non-success status.
    #[error("InDesign Server conversion failed: {message}")]
    ConversionFailed {
        /// The error detail from the server response.
        message: String,
    },

    /// The IDML bytes returned by `InDesign` Server failed to parse.
    #[error("IDML import failed after conversion: {0}")]
    IdmlImport(#[from] selean_idml::IdmlError),
}

/// Request timeout for `InDesign` Server (conversion can be slow).
const CONVERSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Converts an `.indd` file to a Selean `Document` via `InDesign` Server.
///
/// Sends the `.indd` bytes to `InDesign` Server's REST endpoint for
/// conversion to IDML, then parses the resulting IDML through
/// `selean_idml::import_idml`.
///
/// # Errors
///
/// Returns `IndesignBridgeError::NotConfigured` if `server_url` is `None`.
/// Returns `IndesignBridgeError::Http` on network errors.
/// Returns `IndesignBridgeError::ConversionFailed` if the server returns an error.
/// Returns `IndesignBridgeError::IdmlImport` if the IDML parsing fails.
pub async fn convert_indd_to_document(
    client: &reqwest::Client,
    server_url: Option<&str>,
    indd_bytes: &[u8],
) -> Result<Document, IndesignBridgeError> {
    let url = server_url.ok_or(IndesignBridgeError::NotConfigured)?;

    let endpoint = format!("{}/convert/idml", url.trim_end_matches('/'));

    let response = client
        .post(&endpoint)
        .header("Content-Type", "application/octet-stream")
        .body(indd_bytes.to_vec())
        .timeout(CONVERSION_TIMEOUT)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(IndesignBridgeError::ConversionFailed {
            message: format!("status {status}: {body}"),
        });
    }

    let idml_bytes = response.bytes().await?;
    let doc = selean_idml::import_idml(&idml_bytes)?;
    Ok(doc)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn not_configured_returns_error() {
        let client = reqwest::Client::new();
        let result = convert_indd_to_document(&client, None, b"fake-indd").await;
        assert!(result.is_err());
        assert!(
            matches!(result.unwrap_err(), IndesignBridgeError::NotConfigured),
            "expected NotConfigured variant"
        );
    }

    #[tokio::test]
    async fn empty_url_none_returns_error() {
        let client = reqwest::Client::new();
        // Passing None (not empty string) ensures NotConfigured.
        let result = convert_indd_to_document(&client, None, b"").await;
        let err = result.unwrap_err();
        assert!(
            matches!(err, IndesignBridgeError::NotConfigured),
            "expected NotConfigured, got: {err}"
        );
    }

    #[test]
    fn error_display_messages() {
        let not_configured = IndesignBridgeError::NotConfigured;
        assert!(
            not_configured.to_string().contains("IDML"),
            "NotConfigured message should mention IDML as alternative"
        );

        let conversion = IndesignBridgeError::ConversionFailed {
            message: "status 500: internal error".to_string(),
        };
        assert!(
            conversion.to_string().contains("status 500"),
            "ConversionFailed should include the status"
        );
    }

    #[test]
    fn conversion_timeout_is_reasonable() {
        assert_eq!(
            CONVERSION_TIMEOUT,
            std::time::Duration::from_secs(120),
            "timeout should be 120 seconds for large .indd files"
        );
    }
}
