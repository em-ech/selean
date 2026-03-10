//! Storage backend abstraction for asset file storage.
//!
//! Supports local filesystem (development) and S3-compatible backends (production).

pub mod local;
pub mod s3;

use std::fmt;

/// Errors from storage operations.
#[derive(Debug)]
pub enum StorageError {
    /// The requested object was not found.
    NotFound(String),
    /// An I/O or network error occurred.
    Io(String),
    /// The storage backend is not configured or unreachable.
    NotConfigured(String),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(key) => write!(f, "storage object not found: {key}"),
            Self::Io(msg) => write!(f, "storage I/O error: {msg}"),
            Self::NotConfigured(msg) => write!(f, "storage not configured: {msg}"),
        }
    }
}

impl std::error::Error for StorageError {}

/// A boxed future returned by `StorageBackend` methods.
pub type StorageFuture<'a, T> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, StorageError>> + Send + 'a>>;

/// Trait for storage backends. Object-safe, async via boxed futures.
pub trait StorageBackend: Send + Sync {
    /// Stores bytes under the given key.
    fn put(&self, key: &str, data: Vec<u8>, content_type: &str) -> StorageFuture<'_, ()>;

    /// Retrieves bytes for the given key. Returns `(bytes, content_type)`.
    fn get(&self, key: &str) -> StorageFuture<'_, (Vec<u8>, String)>;

    /// Deletes the object at the given key.
    fn delete(&self, key: &str) -> StorageFuture<'_, ()>;

    /// Returns the backend name (for logging).
    fn name(&self) -> &'static str;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn storage_error_display_not_found() {
        let err = StorageError::NotFound("some/key.png".to_string());
        assert_eq!(err.to_string(), "storage object not found: some/key.png");
    }

    #[test]
    fn storage_error_display_io() {
        let err = StorageError::Io("connection reset".to_string());
        assert_eq!(err.to_string(), "storage I/O error: connection reset");
    }

    #[test]
    fn storage_error_display_not_configured() {
        let err = StorageError::NotConfigured("missing bucket".to_string());
        assert_eq!(err.to_string(), "storage not configured: missing bucket");
    }

    #[test]
    fn storage_error_is_error_trait() {
        let err: Box<dyn std::error::Error> = Box::new(StorageError::Io("test".to_string()));
        assert!(err.to_string().contains("test"));
    }
}
