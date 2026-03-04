//! Wire protocol messages for client-server communication.
//!
//! All messages use `#[serde(tag = "type")]` for discriminated union JSON
//! encoding. The WebSocket transport sends these as JSON text frames.

use selean_common::types::{NodeId, PageId, UserId};
use serde::{Deserialize, Serialize};

use crate::types::{
    ClientSeqNum, CursorPosition, Operation, Participant, RoomId, SeqNum, SessionId,
};

/// Types of page-level operations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageOpType {
    /// Add a new page.
    Add,
    /// Remove an existing page.
    Remove,
    /// Rename an existing page.
    Rename,
}

/// Messages sent from client to server over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// Request to join a collaborative room.
    JoinRoom {
        /// Room to join.
        room_id: RoomId,
        /// Joining user.
        user_id: UserId,
        /// Display name shown to other participants.
        display_name: String,
    },
    /// Leave the current room.
    LeaveRoom,
    /// Submit a single operation for sequencing.
    SubmitOp {
        /// The operation to apply.
        op: Box<Operation>,
    },
    /// Submit a group of operations as an atomic batch.
    SubmitOpGroup {
        /// Operations in the group.
        ops: Vec<Operation>,
        /// Label for the group (e.g. "Move", "Resize").
        group_label: String,
    },
    /// Request undo of this user's last operation on a page.
    Undo {
        /// Page to undo on.
        page_id: PageId,
    },
    /// Request redo of this user's last undone operation on a page.
    Redo {
        /// Page to redo on.
        page_id: PageId,
    },
    /// Submit a page-level operation (add, remove, rename).
    SubmitPageOp {
        /// The type of page operation.
        op_type: PageOpType,
        /// Target page (for remove/rename) or new page (for add).
        page_id: PageId,
        /// Page name (for add/rename).
        name: Option<String>,
        /// Page dimensions (for add).
        width: Option<f32>,
        /// Page dimensions (for add).
        height: Option<f32>,
    },
    /// Update cursor and selection presence for other participants.
    PresenceUpdate {
        /// Current page.
        page_id: PageId,
        /// Cursor position, or null if cursor left canvas.
        cursor: Option<CursorPosition>,
        /// Currently selected node IDs.
        selected_node_ids: Vec<NodeId>,
    },
}

/// Messages sent from server to client over WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// Confirmation of successful room join with initial state.
    RoomJoined {
        /// The room that was joined.
        room_id: RoomId,
        /// Assigned session ID.
        session_id: SessionId,
        /// Current participants.
        participants: Vec<Participant>,
        /// Serialized document JSON (canonical state).
        document_json: String,
        /// Latest server sequence number.
        latest_seq: SeqNum,
    },
    /// Acknowledgement of a single submitted operation.
    OpAck {
        /// Client-local sequence that was acknowledged.
        client_seq: ClientSeqNum,
        /// Server-assigned sequence number.
        server_seq: SeqNum,
    },
    /// Acknowledgement of a submitted operation group.
    OpGroupAck {
        /// Client-local sequences that were acknowledged.
        client_seqs: Vec<ClientSeqNum>,
        /// Server-assigned sequence numbers.
        server_seqs: Vec<SeqNum>,
    },
    /// A remote operation to apply locally.
    RemoteOp {
        /// The confirmed operation.
        op: Box<Operation>,
    },
    /// A remote operation group to apply locally.
    RemoteOpGroup {
        /// The confirmed operations.
        ops: Vec<Operation>,
        /// Group label.
        group_label: String,
    },
    /// Result of an undo request.
    UndoResult {
        /// Page the undo was applied to.
        page_id: PageId,
        /// Inverse operations to apply (may be empty if nothing to undo).
        inverse_ops: Vec<Operation>,
    },
    /// Result of a redo request.
    RedoResult {
        /// Page the redo was applied to.
        page_id: PageId,
        /// Inverse operations to apply.
        inverse_ops: Vec<Operation>,
    },
    /// A remote page operation to apply locally.
    RemotePageOp {
        /// The type of page operation.
        op_type: PageOpType,
        /// Target page ID.
        page_id: PageId,
        /// Page name (for add/rename).
        name: Option<String>,
        /// Page dimensions (for add).
        width: Option<f32>,
        /// Page dimensions (for add).
        height: Option<f32>,
        /// Server sequence number for this operation.
        server_seq: SeqNum,
    },
    /// Presence update broadcast from another participant.
    PresenceBroadcast {
        /// Session that sent the update.
        session_id: SessionId,
        /// User that sent the update.
        user_id: UserId,
        /// Display name.
        display_name: String,
        /// Current page.
        page_id: PageId,
        /// Cursor position, or null.
        cursor: Option<CursorPosition>,
        /// Selected node IDs.
        selected_node_ids: Vec<NodeId>,
    },
    /// A new participant joined the room.
    ParticipantJoined {
        /// The new participant.
        participant: Participant,
    },
    /// A participant left the room.
    ParticipantLeft {
        /// Session that left.
        session_id: SessionId,
    },
    /// An error occurred processing a client message.
    Error {
        /// Human-readable error description.
        message: String,
    },
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::command::CommandDescriptor;
    use selean_engine::scene::BoundingBox;

    fn sample_op() -> Operation {
        Operation {
            seq: Some(1),
            client_seq: 1,
            user_id: UserId::new(),
            session_id: SessionId::new(),
            page_id: PageId::new(),
            descriptor: CommandDescriptor::SetBounds {
                node_id: NodeId::new(),
                bounds: BoundingBox::new(0.0, 0.0, 100.0, 50.0),
            },
            timestamp: Some(1_700_000_000),
        }
    }

    #[test]
    fn join_room_roundtrip() {
        let msg = ClientMessage::JoinRoom {
            room_id: RoomId::new(),
            user_id: UserId::new(),
            display_name: "Alice".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"JoinRoom\""));
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::JoinRoom { .. }));
    }

    #[test]
    fn leave_room_roundtrip() {
        let msg = ClientMessage::LeaveRoom;
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::LeaveRoom));
    }

    #[test]
    fn submit_op_roundtrip() {
        let msg = ClientMessage::SubmitOp {
            op: Box::new(sample_op()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"SubmitOp\""));
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::SubmitOp { .. }));
    }

    #[test]
    fn submit_op_group_roundtrip() {
        let msg = ClientMessage::SubmitOpGroup {
            ops: vec![sample_op(), sample_op()],
            group_label: "Move".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"SubmitOpGroup\""));
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::SubmitOpGroup { .. }));
    }

    #[test]
    fn undo_roundtrip() {
        let msg = ClientMessage::Undo {
            page_id: PageId::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::Undo { .. }));
    }

    #[test]
    fn redo_roundtrip() {
        let msg = ClientMessage::Redo {
            page_id: PageId::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::Redo { .. }));
    }

    #[test]
    fn presence_update_roundtrip() {
        let msg = ClientMessage::PresenceUpdate {
            page_id: PageId::new(),
            cursor: Some(CursorPosition { x: 10.0, y: 20.0 }),
            selected_node_ids: vec![NodeId::new()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::PresenceUpdate { .. }));
    }

    #[test]
    fn room_joined_roundtrip() {
        let msg = ServerMessage::RoomJoined {
            room_id: RoomId::new(),
            session_id: SessionId::new(),
            participants: vec![],
            document_json: "{}".to_string(),
            latest_seq: 0,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"RoomJoined\""));
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::RoomJoined { .. }));
    }

    #[test]
    fn op_ack_roundtrip() {
        let msg = ServerMessage::OpAck {
            client_seq: 3,
            server_seq: 42,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::OpAck { .. }));
    }

    #[test]
    fn op_group_ack_roundtrip() {
        let msg = ServerMessage::OpGroupAck {
            client_seqs: vec![1, 2, 3],
            server_seqs: vec![10, 11, 12],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::OpGroupAck { .. }));
    }

    #[test]
    fn remote_op_roundtrip() {
        let msg = ServerMessage::RemoteOp {
            op: Box::new(sample_op()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::RemoteOp { .. }));
    }

    #[test]
    fn remote_op_group_roundtrip() {
        let msg = ServerMessage::RemoteOpGroup {
            ops: vec![sample_op()],
            group_label: "Resize".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::RemoteOpGroup { .. }));
    }

    #[test]
    fn undo_result_roundtrip() {
        let msg = ServerMessage::UndoResult {
            page_id: PageId::new(),
            inverse_ops: vec![sample_op()],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::UndoResult { .. }));
    }

    #[test]
    fn redo_result_roundtrip() {
        let msg = ServerMessage::RedoResult {
            page_id: PageId::new(),
            inverse_ops: vec![],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::RedoResult { .. }));
    }

    #[test]
    fn presence_broadcast_roundtrip() {
        let msg = ServerMessage::PresenceBroadcast {
            session_id: SessionId::new(),
            user_id: UserId::new(),
            display_name: "Bob".to_string(),
            page_id: PageId::new(),
            cursor: None,
            selected_node_ids: vec![],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::PresenceBroadcast { .. }));
    }

    #[test]
    fn participant_joined_roundtrip() {
        let msg = ServerMessage::ParticipantJoined {
            participant: Participant {
                session_id: SessionId::new(),
                user_id: UserId::new(),
                display_name: "Charlie".to_string(),
            },
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::ParticipantJoined { .. }));
    }

    #[test]
    fn participant_left_roundtrip() {
        let msg = ServerMessage::ParticipantLeft {
            session_id: SessionId::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::ParticipantLeft { .. }));
    }

    #[test]
    fn error_roundtrip() {
        let msg = ServerMessage::Error {
            message: "something went wrong".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::Error { .. }));
    }

    #[test]
    fn submit_page_op_add_roundtrip() {
        let msg = ClientMessage::SubmitPageOp {
            op_type: PageOpType::Add,
            page_id: PageId::new(),
            name: Some("New Page".to_string()),
            width: Some(1920.0),
            height: Some(1080.0),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"SubmitPageOp\""));
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::SubmitPageOp { .. }));
    }

    #[test]
    fn submit_page_op_remove_roundtrip() {
        let msg = ClientMessage::SubmitPageOp {
            op_type: PageOpType::Remove,
            page_id: PageId::new(),
            name: None,
            width: None,
            height: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::SubmitPageOp { .. }));
    }

    #[test]
    fn submit_page_op_rename_roundtrip() {
        let msg = ClientMessage::SubmitPageOp {
            op_type: PageOpType::Rename,
            page_id: PageId::new(),
            name: Some("Renamed".to_string()),
            width: None,
            height: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let back: ClientMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ClientMessage::SubmitPageOp { .. }));
    }

    #[test]
    fn remote_page_op_roundtrip() {
        let msg = ServerMessage::RemotePageOp {
            op_type: PageOpType::Add,
            page_id: PageId::new(),
            name: Some("Slide 2".to_string()),
            width: Some(800.0),
            height: Some(600.0),
            server_seq: 42,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"RemotePageOp\""));
        let back: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(back, ServerMessage::RemotePageOp { .. }));
    }

    #[test]
    fn page_op_type_serde() {
        let add = serde_json::to_string(&PageOpType::Add).unwrap();
        assert_eq!(add, "\"add\"");
        let remove = serde_json::to_string(&PageOpType::Remove).unwrap();
        assert_eq!(remove, "\"remove\"");
        let rename = serde_json::to_string(&PageOpType::Rename).unwrap();
        assert_eq!(rename, "\"rename\"");

        let back: PageOpType = serde_json::from_str("\"add\"").unwrap();
        assert_eq!(back, PageOpType::Add);
    }
}
