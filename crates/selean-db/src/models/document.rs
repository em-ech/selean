//! Document and document version models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A design document owned by a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Document {
    /// Unique document ID.
    pub id: Uuid,
    /// Workspace that owns this document.
    pub workspace_id: Uuid,
    /// Document name.
    pub name: String,
    /// User who created the document.
    pub created_by: Uuid,
    /// When the document was created.
    pub created_at: DateTime<Utc>,
    /// When the document was last updated.
    pub updated_at: DateTime<Utc>,
    /// Soft delete timestamp. `None` means active.
    pub deleted_at: Option<DateTime<Utc>>,
}

/// An immutable snapshot of document state at a point in time.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DocumentVersion {
    /// Unique version ID.
    pub id: Uuid,
    /// Document this version belongs to.
    pub document_id: Uuid,
    /// Sequential version number (1, 2, 3...).
    pub version: i32,
    /// Full document data as JSON.
    pub data: serde_json::Value,
    /// User who created this version.
    pub created_by: Uuid,
    /// When this version was created.
    pub created_at: DateTime<Utc>,
}
