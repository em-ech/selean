//! Workspace and membership models.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A workspace that owns documents and has members.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Workspace {
    /// Unique workspace ID.
    pub id: Uuid,
    /// Workspace name.
    pub name: String,
    /// User who created the workspace.
    pub owner_id: Uuid,
    /// When the workspace was created.
    pub created_at: DateTime<Utc>,
    /// When the workspace was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A user's membership in a workspace.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceMember {
    /// The workspace this membership belongs to.
    pub workspace_id: Uuid,
    /// The member user.
    pub user_id: Uuid,
    /// Role within the workspace.
    pub role: String,
    /// When the user joined.
    pub joined_at: DateTime<Utc>,
}

/// Role levels for workspace access control.
///
/// Higher numeric level = more permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WorkspaceRole {
    /// Read-only access to documents.
    Viewer = 0,
    /// Can create and edit documents.
    Editor = 1,
    /// Can manage members (except owner).
    Admin = 2,
    /// Full control including workspace deletion.
    Owner = 3,
}

impl WorkspaceRole {
    /// Parses a role string from the database.
    #[must_use]
    pub fn from_str_role(s: &str) -> Option<Self> {
        match s {
            "viewer" => Some(Self::Viewer),
            "editor" => Some(Self::Editor),
            "admin" => Some(Self::Admin),
            "owner" => Some(Self::Owner),
            _ => None,
        }
    }

    /// Returns the database string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Viewer => "viewer",
            Self::Editor => "editor",
            Self::Admin => "admin",
            Self::Owner => "owner",
        }
    }

    /// Returns true if this role has at least the given permission level.
    #[must_use]
    pub fn has_at_least(&self, required: Self) -> bool {
        (*self as u8) >= (required as u8)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn role_ordering() {
        assert!(WorkspaceRole::Owner > WorkspaceRole::Admin);
        assert!(WorkspaceRole::Admin > WorkspaceRole::Editor);
        assert!(WorkspaceRole::Editor > WorkspaceRole::Viewer);
    }

    #[test]
    fn role_roundtrip() {
        for role in [
            WorkspaceRole::Viewer,
            WorkspaceRole::Editor,
            WorkspaceRole::Admin,
            WorkspaceRole::Owner,
        ] {
            let s = role.as_str();
            let parsed = WorkspaceRole::from_str_role(s).expect("valid role string");
            assert_eq!(parsed, role);
        }
    }

    #[test]
    fn role_has_at_least() {
        assert!(WorkspaceRole::Owner.has_at_least(WorkspaceRole::Viewer));
        assert!(WorkspaceRole::Owner.has_at_least(WorkspaceRole::Owner));
        assert!(WorkspaceRole::Editor.has_at_least(WorkspaceRole::Editor));
        assert!(!WorkspaceRole::Viewer.has_at_least(WorkspaceRole::Editor));
        assert!(!WorkspaceRole::Editor.has_at_least(WorkspaceRole::Admin));
    }

    #[test]
    fn unknown_role_returns_none() {
        assert!(WorkspaceRole::from_str_role("superadmin").is_none());
        assert!(WorkspaceRole::from_str_role("").is_none());
    }
}
