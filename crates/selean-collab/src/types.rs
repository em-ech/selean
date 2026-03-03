//! Core identifiers and the operation envelope for collaborative editing.

use selean_common::types::{PageId, UserId};
use selean_engine::command::CommandDescriptor;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Server-assigned monotonic sequence number. Defines the global total order.
pub type SeqNum = u64;

/// Client-local sequence counter, used to correlate acknowledgements.
pub type ClientSeqNum = u64;

/// Identifies a WebSocket session (one browser tab).
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(Uuid);

impl SessionId {
    /// Creates a new random session ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Identifies a collaborative editing room (one shared document).
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RoomId(Uuid);

impl RoomId {
    /// Creates a new random room ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Wraps an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl Default for RoomId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RoomId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A single mutation operation, the unit of collaborative exchange.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Server-assigned sequence number. `None` until the server confirms.
    pub seq: Option<SeqNum>,
    /// Client-local sequence counter for correlation.
    pub client_seq: ClientSeqNum,
    /// The user who submitted this operation.
    pub user_id: UserId,
    /// The session (browser tab) that submitted this operation.
    pub session_id: SessionId,
    /// The page this operation targets.
    pub page_id: PageId,
    /// The scene graph mutation to apply.
    pub descriptor: CommandDescriptor,
    /// Server-assigned timestamp (millis since epoch). `None` until confirmed.
    pub timestamp: Option<u64>,
}

/// A remote participant's cursor position in world coordinates.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct CursorPosition {
    /// X coordinate in world space.
    pub x: f32,
    /// Y coordinate in world space.
    pub y: f32,
}

/// Information about a connected participant.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Participant {
    /// Session identifier.
    pub session_id: SessionId,
    /// User identifier.
    pub user_id: UserId,
    /// Display name shown to other participants.
    pub display_name: String,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::scene::BoundingBox;

    #[test]
    fn session_id_unique() {
        let a = SessionId::new();
        let b = SessionId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn room_id_unique() {
        let a = RoomId::new();
        let b = RoomId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn session_id_serde_roundtrip() {
        let id = SessionId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: SessionId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn room_id_serde_roundtrip() {
        let id = RoomId::new();
        let json = serde_json::to_string(&id).unwrap();
        let back: RoomId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn operation_serde_roundtrip() {
        let op = Operation {
            seq: Some(42),
            client_seq: 1,
            user_id: UserId::new(),
            session_id: SessionId::new(),
            page_id: PageId::new(),
            descriptor: CommandDescriptor::SetBounds {
                node_id: NodeId::new(),
                bounds: BoundingBox::new(0.0, 0.0, 100.0, 100.0),
            },
            timestamp: Some(1_700_000_000),
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: Operation = serde_json::from_str(&json).unwrap();
        assert_eq!(back.seq, Some(42));
        assert_eq!(back.client_seq, 1);
    }

    #[test]
    fn operation_without_seq() {
        let op = Operation {
            seq: None,
            client_seq: 5,
            user_id: UserId::new(),
            session_id: SessionId::new(),
            page_id: PageId::new(),
            descriptor: CommandDescriptor::RemoveNode {
                node_id: NodeId::new(),
            },
            timestamp: None,
        };
        let json = serde_json::to_string(&op).unwrap();
        let back: Operation = serde_json::from_str(&json).unwrap();
        assert!(back.seq.is_none());
        assert!(back.timestamp.is_none());
    }

    #[test]
    fn cursor_position_serde_roundtrip() {
        let pos = CursorPosition { x: 10.5, y: 20.3 };
        let json = serde_json::to_string(&pos).unwrap();
        let back: CursorPosition = serde_json::from_str(&json).unwrap();
        assert_eq!(pos, back);
    }

    #[test]
    fn participant_serde_roundtrip() {
        let p = Participant {
            session_id: SessionId::new(),
            user_id: UserId::new(),
            display_name: "Alice".to_string(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: Participant = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }
}
