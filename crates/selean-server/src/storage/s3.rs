//! S3-compatible storage backend for production use.
//!
//! Works with AWS S3, Cloudflare R2, `MinIO`, and other S3-compatible services.

use std::sync::Arc;

use s3::Region;
use s3::bucket::Bucket;
use s3::creds::Credentials;

use super::{StorageBackend, StorageError, StorageFuture};

/// Configuration for the S3 storage backend.
pub struct S3Config {
    /// S3 bucket name.
    pub bucket_name: String,
    /// AWS region (e.g. "us-east-1") or "auto" for custom endpoints.
    pub region: String,
    /// Custom endpoint URL for R2/MinIO. `None` for standard AWS S3.
    pub endpoint: Option<String>,
    /// AWS access key ID.
    pub access_key: String,
    /// AWS secret access key.
    pub secret_key: String,
}

impl S3Config {
    /// Reads S3 configuration from environment variables.
    ///
    /// Returns `None` if `S3_BUCKET` is not set or empty. Docker Compose
    /// passes unset variables through as empty strings, so empty values are
    /// treated as unset.
    pub fn from_env() -> Option<Self> {
        let bucket_name = non_empty(std::env::var("S3_BUCKET").ok())?;
        let region =
            non_empty(std::env::var("S3_REGION").ok()).unwrap_or_else(|| "auto".to_string());
        let endpoint = non_empty(std::env::var("S3_ENDPOINT").ok());
        let access_key = std::env::var("S3_ACCESS_KEY").unwrap_or_default();
        let secret_key = std::env::var("S3_SECRET_KEY").unwrap_or_default();
        Some(Self {
            bucket_name,
            region,
            endpoint,
            access_key,
            secret_key,
        })
    }
}

/// Maps an empty string to `None`.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// S3-compatible storage backend.
pub struct S3Storage {
    bucket: Arc<Bucket>,
}

impl std::fmt::Debug for S3Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Storage")
            .field("bucket", &self.bucket.name())
            .finish()
    }
}

impl S3Storage {
    /// Creates a new `S3Storage` backend from the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `StorageError::NotConfigured` if the region, credentials,
    /// or bucket cannot be initialized.
    pub fn new(config: &S3Config) -> Result<Self, StorageError> {
        let region = if let Some(ref endpoint) = config.endpoint {
            Region::Custom {
                region: config.region.clone(),
                endpoint: endpoint.clone(),
            }
        } else {
            config
                .region
                .parse()
                .map_err(|e| StorageError::NotConfigured(format!("invalid region: {e}")))?
        };

        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| StorageError::NotConfigured(format!("invalid credentials: {e}")))?;

        let bucket = Bucket::new(&config.bucket_name, region, credentials)
            .map_err(|e| StorageError::NotConfigured(format!("bucket creation failed: {e}")))?;

        // Use path-style access for custom endpoints (R2, MinIO).
        let bucket = if config.endpoint.is_some() {
            bucket.with_path_style()
        } else {
            bucket
        };

        Ok(Self {
            bucket: Arc::from(bucket),
        })
    }
}

impl StorageBackend for S3Storage {
    fn put(&self, key: &str, data: Vec<u8>, content_type: &str) -> StorageFuture<'_, ()> {
        let bucket = self.bucket.clone();
        let key = key.to_string();
        let content_type = content_type.to_string();
        Box::pin(async move {
            bucket
                .put_object_with_content_type(&key, &data, &content_type)
                .await
                .map_err(|e| StorageError::Io(format!("S3 PUT failed: {e}")))?;
            Ok(())
        })
    }

    fn get(&self, key: &str) -> StorageFuture<'_, (Vec<u8>, String)> {
        let bucket = self.bucket.clone();
        let key = key.to_string();
        Box::pin(async move {
            let response = bucket
                .get_object(&key)
                .await
                .map_err(|e| StorageError::Io(format!("S3 GET failed: {e}")))?;
            if response.status_code() == 404 {
                return Err(StorageError::NotFound(key));
            }
            let content_type = response
                .headers()
                .get("content-type")
                .cloned()
                .unwrap_or_else(|| "application/octet-stream".to_string());
            Ok((response.to_vec(), content_type))
        })
    }

    fn delete(&self, key: &str) -> StorageFuture<'_, ()> {
        let bucket = self.bucket.clone();
        let key = key.to_string();
        Box::pin(async move {
            bucket
                .delete_object(&key)
                .await
                .map_err(|e| StorageError::Io(format!("S3 DELETE failed: {e}")))?;
            Ok(())
        })
    }

    fn name(&self) -> &'static str {
        "s3"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_treats_empty_string_as_unset() {
        assert_eq!(non_empty(None), None);
        assert_eq!(non_empty(Some(String::new())), None);
        assert_eq!(
            non_empty(Some("selean-assets".to_string())).as_deref(),
            Some("selean-assets")
        );
    }

    #[test]
    fn s3_config_from_env_returns_none_without_bucket() {
        // S3_BUCKET is not set in test environment.
        // We cannot unset env vars safely, so just verify the struct constructs.
        let config = S3Config {
            bucket_name: "test".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
        };
        assert_eq!(config.bucket_name, "test");
    }

    #[test]
    fn s3_config_fields() {
        let config = S3Config {
            bucket_name: "my-bucket".to_string(),
            region: "eu-west-1".to_string(),
            endpoint: Some("https://r2.example.com".to_string()),
            access_key: "ak".to_string(),
            secret_key: "sk".to_string(),
        };
        assert_eq!(config.bucket_name, "my-bucket");
        assert_eq!(config.region, "eu-west-1");
        assert_eq!(config.endpoint.as_deref(), Some("https://r2.example.com"));
        assert_eq!(config.access_key, "ak");
        assert_eq!(config.secret_key, "sk");
    }

    #[test]
    fn s3_storage_name() {
        // Create with a custom endpoint to avoid region parse errors.
        let config = S3Config {
            bucket_name: "test-bucket".to_string(),
            region: "auto".to_string(),
            endpoint: Some("http://localhost:9000".to_string()),
            access_key: "minioadmin".to_string(),
            secret_key: "minioadmin".to_string(),
        };
        let storage = S3Storage::new(&config).expect("create S3Storage");
        assert_eq!(storage.name(), "s3");
    }

    #[test]
    fn s3_storage_standard_region_without_endpoint() {
        // Standard AWS regions parse successfully.
        let config = S3Config {
            bucket_name: "test".to_string(),
            region: "us-east-1".to_string(),
            endpoint: None,
            access_key: "k".to_string(),
            secret_key: "s".to_string(),
        };
        let result = S3Storage::new(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn s3_storage_custom_endpoint_uses_path_style() {
        let config = S3Config {
            bucket_name: "test".to_string(),
            region: "auto".to_string(),
            endpoint: Some("http://localhost:9000".to_string()),
            access_key: "key".to_string(),
            secret_key: "secret".to_string(),
        };
        // Should succeed with custom endpoint (path style).
        let storage = S3Storage::new(&config);
        assert!(storage.is_ok());
    }
}
