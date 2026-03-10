//! Local filesystem storage backend for development.

use std::path::{Path, PathBuf};

use super::{StorageBackend, StorageError, StorageFuture};

/// Stores assets on the local filesystem under a base directory.
///
/// Each object is stored as a file at `{base_dir}/{key}`, with a sidecar
/// `.meta` file containing the content type string.
pub struct LocalStorage {
    base_dir: PathBuf,
}

impl LocalStorage {
    /// Creates a new `LocalStorage` backend at the given directory.
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Converts a storage key to a filesystem path, sanitizing `..` segments
    /// to prevent path traversal attacks.
    fn key_to_path(&self, key: &str) -> PathBuf {
        let safe_key = key.replace("..", "_");
        self.base_dir.join(safe_key)
    }

    /// Returns the sidecar metadata path for a given object path.
    fn meta_path(path: &Path) -> PathBuf {
        PathBuf::from(format!("{}.meta", path.display()))
    }
}

impl StorageBackend for LocalStorage {
    fn put(&self, key: &str, data: Vec<u8>, content_type: &str) -> StorageFuture<'_, ()> {
        let path = self.key_to_path(key);
        let content_type = content_type.to_string();
        Box::pin(async move {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| StorageError::Io(e.to_string()))?;
            }
            let meta = Self::meta_path(&path);
            tokio::fs::write(&meta, &content_type)
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
            tokio::fs::write(&path, &data)
                .await
                .map_err(|e| StorageError::Io(e.to_string()))?;
            Ok(())
        })
    }

    fn get(&self, key: &str) -> StorageFuture<'_, (Vec<u8>, String)> {
        let path = self.key_to_path(key);
        let key_owned = key.to_string();
        Box::pin(async move {
            let data = tokio::fs::read(&path).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::NotFound(key_owned.clone())
                } else {
                    StorageError::Io(e.to_string())
                }
            })?;
            let meta = Self::meta_path(&path);
            let content_type = tokio::fs::read_to_string(&meta)
                .await
                .unwrap_or_else(|_| "application/octet-stream".to_string());
            Ok((data, content_type))
        })
    }

    fn delete(&self, key: &str) -> StorageFuture<'_, ()> {
        let path = self.key_to_path(key);
        let key_owned = key.to_string();
        Box::pin(async move {
            tokio::fs::remove_file(&path).await.map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    StorageError::NotFound(key_owned)
                } else {
                    StorageError::Io(e.to_string())
                }
            })?;
            // Clean up sidecar meta file.
            let meta = Self::meta_path(&path);
            let _ = tokio::fs::remove_file(&meta).await;
            Ok(())
        })
    }

    fn name(&self) -> &'static str {
        "local"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn put_and_get_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LocalStorage::new(dir.path());

        storage
            .put("test.png", b"hello".to_vec(), "image/png")
            .await
            .expect("put");

        let (data, ct) = storage.get("test.png").await.expect("get");
        assert_eq!(data, b"hello");
        assert_eq!(ct, "image/png");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LocalStorage::new(dir.path());

        let result = storage.get("no-such-file.txt").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, StorageError::NotFound(_)),
            "expected NotFound, got: {err}"
        );
    }

    #[tokio::test]
    async fn put_delete_get_returns_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LocalStorage::new(dir.path());

        storage
            .put("del.txt", b"data".to_vec(), "text/plain")
            .await
            .expect("put");
        storage.delete("del.txt").await.expect("delete");

        let result = storage.get("del.txt").await;
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn put_creates_nested_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LocalStorage::new(dir.path());

        storage
            .put(
                "a/b/c/file.bin",
                b"nested".to_vec(),
                "application/octet-stream",
            )
            .await
            .expect("put nested");

        let (data, _) = storage.get("a/b/c/file.bin").await.expect("get nested");
        assert_eq!(data, b"nested");
    }

    #[tokio::test]
    async fn path_traversal_is_sanitized() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LocalStorage::new(dir.path());

        // ".." segments should be replaced with "_"
        storage
            .put("../escape.txt", b"bad".to_vec(), "text/plain")
            .await
            .expect("put with traversal");

        // The file should be stored under the base dir, not escaped.
        let path = storage.key_to_path("../escape.txt");
        assert!(
            path.starts_with(dir.path()),
            "path {path:?} should be under {dir:?}"
        );

        // Should be retrievable with the same key.
        let (data, _) = storage.get("../escape.txt").await.expect("get");
        assert_eq!(data, b"bad");
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = LocalStorage::new(dir.path());

        let result = storage.delete("nope.bin").await;
        assert!(matches!(result, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn name_returns_local() {
        let storage = LocalStorage::new("/tmp");
        assert_eq!(storage.name(), "local");
    }
}
