//! Asset model for uploaded files.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An uploaded file (image, font, etc.) stored externally.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Asset {
    /// Unique asset ID.
    pub id: Uuid,
    /// Workspace that owns this asset.
    pub workspace_id: Uuid,
    /// Original filename.
    pub filename: String,
    /// MIME content type.
    pub content_type: String,
    /// File size in bytes.
    pub size_bytes: i64,
    /// Storage backend key (e.g. S3 object key).
    pub storage_key: String,
    /// User who uploaded the asset.
    pub uploaded_by: Uuid,
    /// When the asset was uploaded.
    pub created_at: DateTime<Utc>,
}
