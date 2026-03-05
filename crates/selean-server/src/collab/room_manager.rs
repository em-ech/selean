//! Server-side room management for collaborative editing.
//!
//! Each room holds a canonical document, an operation log, per-user undo/redo
//! stacks, and active WebSocket sessions. All mutations are sequenced through
//! the room to establish a global total order.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use selean_collab::inverse::compute_inverse;
use selean_collab::op_log::OpLog;
use selean_collab::protocol::{ClientMessage, PageOpType, ServerMessage};
use selean_collab::types::{CursorPosition, Operation, Participant, RoomId, SeqNum, SessionId};
use selean_common::types::{NodeId, PageId, UserId};
use selean_engine::command::CommandDescriptor;
use selean_engine::persistence::{Document, save_document};
use tokio::sync::mpsc;

/// Maximum number of undo entries per user per page.
const MAX_UNDO_SIZE: usize = 200;

/// Manages all active collaborative editing rooms.
///
/// Each room is independently locked via `Arc<Mutex<Room>>` so operations
/// on different rooms never contend.
pub struct RoomManager {
    rooms: HashMap<RoomId, Arc<Mutex<Room>>>,
}

/// State for a single active session (one WebSocket connection).
pub struct SessionState {
    /// User identity.
    pub user_id: UserId,
    /// Display name for presence.
    pub display_name: String,
    /// Channel to send messages back to this session's WebSocket writer.
    pub tx: mpsc::Sender<ServerMessage>,
}

/// A collaborative editing room holding the canonical document and all session state.
pub struct Room {
    /// Room identifier.
    pub id: RoomId,
    /// Canonical document (source of truth).
    document: Document,
    /// Ordered log of all operations.
    op_log: OpLog,
    /// Next sequence number to assign.
    next_seq: SeqNum,
    /// Active sessions.
    sessions: HashMap<SessionId, SessionState>,
    /// Per-user, per-page undo stacks. Each entry is `(SeqNum, inverse_descriptor)`.
    /// The inverse is captured at submission time (before the op is applied).
    /// Capped at [`MAX_UNDO_SIZE`]; oldest entries are evicted when full.
    user_undo_stacks: HashMap<(UserId, PageId), VecDeque<(SeqNum, CommandDescriptor)>>,
    /// Per-user, per-page redo stacks (descriptors to re-apply).
    /// Capped at [`MAX_UNDO_SIZE`]; oldest entries are evicted when full.
    user_redo_stacks: HashMap<(UserId, PageId), VecDeque<CommandDescriptor>>,
}

/// Errors from room operations.
#[derive(Debug, thiserror::Error)]
pub enum RoomError {
    /// Room not found.
    #[error("room not found: {0}")]
    NotFound(RoomId),
    /// Session not found.
    #[error("session not found")]
    SessionNotFound,
    /// Document serialization failed.
    #[error("serialization error: {0}")]
    Serialization(String),
}

impl RoomManager {
    /// Creates a new empty room manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            rooms: HashMap::new(),
        }
    }

    /// Returns the per-room lock for the given room, creating it if absent.
    pub fn get_or_create_room(&mut self, room_id: RoomId) -> Arc<Mutex<Room>> {
        Arc::clone(
            self.rooms
                .entry(room_id)
                .or_insert_with(|| Arc::new(Mutex::new(Room::new(room_id)))),
        )
    }

    /// Returns the per-room lock for the given room, creating it with an
    /// existing document if absent.
    pub fn get_or_create_room_with_document(
        &mut self,
        room_id: RoomId,
        document: Document,
    ) -> Arc<Mutex<Room>> {
        Arc::clone(
            self.rooms
                .entry(room_id)
                .or_insert_with(|| Arc::new(Mutex::new(Room::with_document(room_id, document)))),
        )
    }

    /// Returns the per-room lock if the room exists.
    #[must_use]
    pub fn get_room(&self, room_id: &RoomId) -> Option<Arc<Mutex<Room>>> {
        self.rooms.get(room_id).cloned()
    }

    /// Removes a room if it exists.
    pub fn remove_room(&mut self, room_id: &RoomId) -> bool {
        self.rooms.remove(room_id).is_some()
    }

    /// Returns the number of active rooms.
    #[must_use]
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }

    /// Returns an iterator over `(RoomId, Arc<Mutex<Room>>)` pairs.
    pub fn rooms(&self) -> impl Iterator<Item = (RoomId, Arc<Mutex<Room>>)> + '_ {
        self.rooms.iter().map(|(&id, arc)| (id, Arc::clone(arc)))
    }
}

impl Default for RoomManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Room {
    /// Creates a new room with an empty default document.
    #[must_use]
    pub fn new(id: RoomId) -> Self {
        Self {
            id,
            document: Document::new(),
            op_log: OpLog::new(),
            next_seq: 1,
            sessions: HashMap::new(),
            user_undo_stacks: HashMap::new(),
            user_redo_stacks: HashMap::new(),
        }
    }

    /// Creates a new room with a pre-existing document.
    #[must_use]
    pub fn with_document(id: RoomId, document: Document) -> Self {
        Self {
            id,
            document,
            op_log: OpLog::new(),
            next_seq: 1,
            sessions: HashMap::new(),
            user_undo_stacks: HashMap::new(),
            user_redo_stacks: HashMap::new(),
        }
    }

    /// Returns a reference to the canonical document.
    #[must_use]
    pub fn document(&self) -> &Document {
        &self.document
    }

    /// Returns the number of active sessions.
    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Returns the latest sequence number (0 if no ops yet).
    #[must_use]
    pub fn latest_seq(&self) -> SeqNum {
        self.next_seq.saturating_sub(1)
    }

    /// Returns the list of current participants.
    #[must_use]
    pub fn participants(&self) -> Vec<Participant> {
        self.sessions
            .iter()
            .map(|(session_id, state)| Participant {
                session_id: *session_id,
                user_id: state.user_id,
                display_name: state.display_name.clone(),
            })
            .collect()
    }

    /// Adds a session to the room. Returns the `RoomJoined` message to send to the joiner.
    ///
    /// # Errors
    ///
    /// Returns `RoomError::Serialization` if the document cannot be serialized.
    pub fn join(
        &mut self,
        session_id: SessionId,
        user_id: UserId,
        display_name: String,
        tx: mpsc::Sender<ServerMessage>,
    ) -> Result<ServerMessage, RoomError> {
        let document_json =
            save_document(&self.document).map_err(|e| RoomError::Serialization(e.to_string()))?;

        let participants = self.participants();
        let latest_seq = self.latest_seq();

        // Broadcast ParticipantJoined to existing sessions.
        let join_msg = ServerMessage::ParticipantJoined {
            participant: Participant {
                session_id,
                user_id,
                display_name: display_name.clone(),
            },
        };
        self.broadcast_except(session_id, &join_msg);

        self.sessions.insert(
            session_id,
            SessionState {
                user_id,
                display_name,
                tx,
            },
        );

        Ok(ServerMessage::RoomJoined {
            room_id: self.id,
            session_id,
            participants,
            document_json,
            latest_seq,
        })
    }

    /// Removes a session from the room. Returns `true` if the room is now empty.
    pub fn leave(&mut self, session_id: SessionId) -> bool {
        self.sessions.remove(&session_id);
        let left_msg = ServerMessage::ParticipantLeft { session_id };
        self.broadcast_all(&left_msg);
        self.sessions.is_empty()
    }

    /// Submits a single operation for sequencing and broadcast.
    ///
    /// Returns the assigned sequence number.
    pub fn submit_op(&mut self, mut op: Operation) -> SeqNum {
        let seq = self.assign_seq();
        op.seq = Some(seq);
        op.timestamp = Some(current_timestamp_millis());

        let page_id = op.page_id;
        let user_id = op.user_id;
        let session_id = op.session_id;
        let client_seq = op.client_seq;

        // Compute inverse BEFORE applying (captures pre-mutation state).
        let inverse = self.compute_inverse_for_page(&op.descriptor, page_id);

        // Apply to canonical document.
        self.apply_descriptor(&op.descriptor, page_id);

        // Record in op log.
        self.op_log.append(op.clone());

        // Push to user's undo stack (with pre-computed inverse), clear redo.
        if let Some(inv) = inverse {
            self.push_undo(user_id, page_id, (seq, inv));
        }
        self.user_redo_stacks.remove(&(user_id, page_id));

        // Send OpAck to submitter, RemoteOp to everyone else.
        let ack = ServerMessage::OpAck {
            client_seq,
            server_seq: seq,
        };
        self.send_to(session_id, &ack);

        let remote = ServerMessage::RemoteOp { op: Box::new(op) };
        self.broadcast_except(session_id, &remote);

        seq
    }

    /// Submits a group of operations atomically.
    ///
    /// Returns the assigned sequence numbers.
    pub fn submit_op_group(&mut self, mut ops: Vec<Operation>, group_label: String) -> Vec<SeqNum> {
        if ops.is_empty() {
            return vec![];
        }

        let session_id = ops[0].session_id;
        let user_id = ops[0].user_id;
        let page_id = ops[0].page_id;
        let mut seqs = Vec::with_capacity(ops.len());
        let mut client_seqs = Vec::with_capacity(ops.len());

        for op in &mut ops {
            let seq = self.assign_seq();
            op.seq = Some(seq);
            op.timestamp = Some(current_timestamp_millis());
            seqs.push(seq);
            client_seqs.push(op.client_seq);

            let inverse = self.compute_inverse_for_page(&op.descriptor, op.page_id);
            self.apply_descriptor(&op.descriptor, op.page_id);
            self.op_log.append(op.clone());

            if let Some(inv) = inverse {
                self.push_undo(user_id, page_id, (seq, inv));
            }
        }
        self.user_redo_stacks.remove(&(user_id, page_id));

        let ack = ServerMessage::OpGroupAck {
            client_seqs,
            server_seqs: seqs.clone(),
        };
        self.send_to(session_id, &ack);

        let remote = ServerMessage::RemoteOpGroup { ops, group_label };
        self.broadcast_except(session_id, &remote);

        seqs
    }

    /// Handles an undo request. Pops the user's last operation's pre-computed
    /// inverse, applies it, and broadcasts.
    pub fn handle_undo(
        &mut self,
        session_id: SessionId,
        user_id: UserId,
        page_id: PageId,
    ) -> Option<SeqNum> {
        let stack = self.user_undo_stacks.get_mut(&(user_id, page_id))?;
        let (undone_seq, inverse) = stack.pop_back()?;

        let original_op = self.op_log.get_by_seq(undone_seq)?;
        let original_desc = original_op.descriptor.clone();

        // Apply the pre-computed inverse as a new sequenced operation.
        let inv_seq = self.assign_seq();
        let inv_op = Operation {
            seq: Some(inv_seq),
            client_seq: 0,
            user_id,
            session_id,
            page_id,
            descriptor: inverse,
            timestamp: Some(current_timestamp_millis()),
        };

        self.apply_descriptor(&inv_op.descriptor, page_id);
        self.op_log.append(inv_op.clone());

        // Push original descriptor to redo stack.
        self.push_redo(user_id, page_id, original_desc);

        // Send UndoResult to requester, RemoteOp to others.
        let result = ServerMessage::UndoResult {
            page_id,
            inverse_ops: vec![inv_op.clone()],
        };
        self.send_to(session_id, &result);

        let remote = ServerMessage::RemoteOp {
            op: Box::new(inv_op),
        };
        self.broadcast_except(session_id, &remote);

        Some(inv_seq)
    }

    /// Handles a redo request. Pops the user's last undone descriptor,
    /// applies it as a new operation, and broadcasts.
    pub fn handle_redo(
        &mut self,
        session_id: SessionId,
        user_id: UserId,
        page_id: PageId,
    ) -> Option<SeqNum> {
        let stack = self.user_redo_stacks.get_mut(&(user_id, page_id))?;
        let desc = stack.pop_back()?;

        let seq = self.assign_seq();
        let op = Operation {
            seq: Some(seq),
            client_seq: 0,
            user_id,
            session_id,
            page_id,
            descriptor: desc,
            timestamp: Some(current_timestamp_millis()),
        };

        // Compute inverse BEFORE applying so undo can reverse this redo.
        let inverse = self.compute_inverse_for_page(&op.descriptor, page_id);
        self.apply_descriptor(&op.descriptor, page_id);
        self.op_log.append(op.clone());

        // Push to undo stack with pre-computed inverse.
        if let Some(inv) = inverse {
            self.push_undo(user_id, page_id, (seq, inv));
        }

        let result = ServerMessage::RedoResult {
            page_id,
            inverse_ops: vec![op.clone()],
        };
        self.send_to(session_id, &result);

        let remote = ServerMessage::RemoteOp { op: Box::new(op) };
        self.broadcast_except(session_id, &remote);

        Some(seq)
    }

    /// Handles a page-level operation (add, remove, rename).
    ///
    /// Applies the operation to the canonical document and broadcasts a
    /// `RemotePageOp` to all sessions (including the submitter).
    pub fn handle_page_op(
        &mut self,
        op_type: PageOpType,
        page_id: PageId,
        name: Option<String>,
        width: Option<f32>,
        height: Option<f32>,
    ) -> SeqNum {
        let seq = self.assign_seq();

        match op_type {
            PageOpType::Add => {
                let page_name = name.as_deref().unwrap_or("New Page");
                let w = width.unwrap_or(1920.0);
                let h = height.unwrap_or(1080.0);
                self.document.add_page_with_id(page_id, page_name, w, h);
            }
            PageOpType::Remove => {
                self.document.remove_page(page_id);
            }
            PageOpType::Rename => {
                if let Some(new_name) = &name {
                    if let Some(page) = self.document.page_mut(page_id) {
                        page.name.clone_from(new_name);
                    }
                }
            }
        }

        let msg = ServerMessage::RemotePageOp {
            op_type,
            page_id,
            name,
            width,
            height,
            server_seq: seq,
        };
        self.broadcast_all(&msg);

        seq
    }

    /// Handles a presence update from a session.
    pub fn handle_presence(
        &self,
        session_id: SessionId,
        page_id: PageId,
        cursor: Option<CursorPosition>,
        selected_node_ids: Vec<NodeId>,
    ) {
        let Some(session) = self.sessions.get(&session_id) else {
            return;
        };
        let msg = ServerMessage::PresenceBroadcast {
            session_id,
            user_id: session.user_id,
            display_name: session.display_name.clone(),
            page_id,
            cursor,
            selected_node_ids,
        };
        self.broadcast_except(session_id, &msg);
    }

    /// Dispatches a parsed client message to the appropriate handler.
    ///
    /// Returns `true` if the room should be removed (all sessions left).
    pub fn handle_message(&mut self, session_id: SessionId, msg: ClientMessage) -> bool {
        match msg {
            ClientMessage::LeaveRoom => self.leave(session_id),
            ClientMessage::SubmitOp { op } => {
                self.submit_op(*op);
                false
            }
            ClientMessage::SubmitOpGroup { ops, group_label } => {
                self.submit_op_group(ops, group_label);
                false
            }
            ClientMessage::Undo { page_id } => {
                let user_id = self.user_id_for_session(session_id);
                if let Some(uid) = user_id {
                    self.handle_undo(session_id, uid, page_id);
                }
                false
            }
            ClientMessage::Redo { page_id } => {
                let user_id = self.user_id_for_session(session_id);
                if let Some(uid) = user_id {
                    self.handle_redo(session_id, uid, page_id);
                }
                false
            }
            ClientMessage::SubmitPageOp {
                op_type,
                page_id,
                name,
                width,
                height,
            } => {
                self.handle_page_op(op_type, page_id, name, width, height);
                false
            }
            ClientMessage::PresenceUpdate {
                page_id,
                cursor,
                selected_node_ids,
            } => {
                self.handle_presence(session_id, page_id, cursor, selected_node_ids);
                false
            }
            ClientMessage::JoinRoom { .. } => {
                // Already handled at the ws_handler level.
                false
            }
        }
    }

    // -- Internal helpers --

    fn assign_seq(&mut self) -> SeqNum {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }

    fn apply_descriptor(&mut self, descriptor: &CommandDescriptor, page_id: PageId) {
        if let Some(page) = self.document.page_mut(page_id) {
            let mut cmd = descriptor.clone().into_command();
            cmd.execute(&mut page.scene);
        }
    }

    fn compute_inverse_for_page(
        &self,
        descriptor: &CommandDescriptor,
        page_id: PageId,
    ) -> Option<CommandDescriptor> {
        let page = self.document.page(page_id)?;
        compute_inverse(descriptor, &page.scene)
    }

    fn push_undo(&mut self, user_id: UserId, page_id: PageId, entry: (SeqNum, CommandDescriptor)) {
        let stack = self.user_undo_stacks.entry((user_id, page_id)).or_default();
        if stack.len() >= MAX_UNDO_SIZE {
            stack.pop_front();
        }
        stack.push_back(entry);
    }

    fn push_redo(&mut self, user_id: UserId, page_id: PageId, desc: CommandDescriptor) {
        let stack = self.user_redo_stacks.entry((user_id, page_id)).or_default();
        if stack.len() >= MAX_UNDO_SIZE {
            stack.pop_front();
        }
        stack.push_back(desc);
    }

    fn user_id_for_session(&self, session_id: SessionId) -> Option<UserId> {
        self.sessions.get(&session_id).map(|s| s.user_id)
    }

    fn send_to(&self, session_id: SessionId, msg: &ServerMessage) {
        if let Some(session) = self.sessions.get(&session_id) {
            let _ = session.tx.try_send(msg.clone());
        }
    }

    fn broadcast_all(&self, msg: &ServerMessage) {
        for session in self.sessions.values() {
            let _ = session.tx.try_send(msg.clone());
        }
    }

    fn broadcast_except(&self, exclude: SessionId, msg: &ServerMessage) {
        for (sid, session) in &self.sessions {
            if *sid != exclude {
                let _ = session.tx.try_send(msg.clone());
            }
        }
    }
}

fn current_timestamp_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| {
            // Millis won't exceed u64 for astronomical amounts of time.
            #[allow(clippy::cast_possible_truncation)]
            let ms = d.as_millis() as u64;
            ms
        })
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::command::descriptor::create_frame_node;
    use selean_engine::scene::BoundingBox;
    use tokio::sync::mpsc;

    fn make_session() -> (SessionId, UserId, String, mpsc::Sender<ServerMessage>) {
        let (tx, _rx) = mpsc::channel(32);
        (SessionId::new(), UserId::new(), "User".to_string(), tx)
    }

    fn make_session_with_rx() -> (
        SessionId,
        UserId,
        String,
        mpsc::Sender<ServerMessage>,
        mpsc::Receiver<ServerMessage>,
    ) {
        let (tx, rx) = mpsc::channel(32);
        (SessionId::new(), UserId::new(), "User".to_string(), tx, rx)
    }

    fn make_op(
        client_seq: selean_collab::ClientSeqNum,
        user_id: UserId,
        session_id: SessionId,
        page_id: PageId,
        descriptor: CommandDescriptor,
    ) -> Operation {
        Operation {
            seq: None,
            client_seq,
            user_id,
            session_id,
            page_id,
            descriptor,
            timestamp: None,
        }
    }

    // -- RoomManager tests --

    #[test]
    fn create_and_get_room() {
        let mut mgr = RoomManager::new();
        let room_id = RoomId::new();
        mgr.get_or_create_room(room_id);
        assert_eq!(mgr.room_count(), 1);
        assert!(mgr.get_room(&room_id).is_some());
    }

    #[test]
    fn remove_room() {
        let mut mgr = RoomManager::new();
        let room_id = RoomId::new();
        mgr.get_or_create_room(room_id);
        assert!(mgr.remove_room(&room_id));
        assert_eq!(mgr.room_count(), 0);
    }

    #[test]
    fn remove_nonexistent_room() {
        let mut mgr = RoomManager::new();
        assert!(!mgr.remove_room(&RoomId::new()));
    }

    // -- Room join/leave tests --

    #[test]
    fn join_room_returns_room_joined() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        let msg = room.join(sid, uid, name, tx).unwrap();
        match msg {
            ServerMessage::RoomJoined {
                session_id,
                participants,
                ..
            } => {
                assert_eq!(session_id, sid);
                assert!(participants.is_empty()); // no one else yet
            }
            _ => panic!("expected RoomJoined"),
        }
        assert_eq!(room.session_count(), 1);
    }

    #[test]
    fn leave_room_returns_empty_status() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();
        assert!(room.leave(sid)); // room is empty
    }

    #[test]
    fn leave_room_not_empty() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, name1, tx1) = make_session();
        let (sid2, uid2, name2, tx2) = make_session();
        room.join(sid1, uid1, name1, tx1).unwrap();
        room.join(sid2, uid2, name2, tx2).unwrap();
        assert!(!room.leave(sid1)); // still has sid2
        assert_eq!(room.session_count(), 1);
    }

    // -- Op submission tests --

    #[test]
    fn submit_op_assigns_seq() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let op = make_op(1, uid, sid, page_id, CommandDescriptor::AddRoot { node });
        let seq = room.submit_op(op);
        assert_eq!(seq, 1);
        assert_eq!(room.latest_seq(), 1);
    }

    #[test]
    fn submit_op_mutates_document() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;
        let op = make_op(1, uid, sid, page_id, CommandDescriptor::AddRoot { node });
        room.submit_op(op);

        assert!(room.document().active_page().scene.get(node_id).is_some());
    }

    #[test]
    fn submit_op_sends_ack_to_submitter() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx, mut rx) = make_session_with_rx();
        room.join(sid, uid, name, tx).unwrap();
        // Drain the RoomJoined that is NOT sent to submitter (it's returned)

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let op = make_op(42, uid, sid, page_id, CommandDescriptor::AddRoot { node });
        room.submit_op(op);

        let msg = rx.try_recv().unwrap();
        match msg {
            ServerMessage::OpAck {
                client_seq,
                server_seq,
            } => {
                assert_eq!(client_seq, 42);
                assert_eq!(server_seq, 1);
            }
            _ => panic!("expected OpAck"),
        }
    }

    #[test]
    fn submit_op_broadcasts_to_others() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, name1, tx1) = make_session();
        let (sid2, uid2, name2, tx2, mut rx2) = make_session_with_rx();
        room.join(sid1, uid1, name1, tx1).unwrap();
        room.join(sid2, uid2, name2, tx2).unwrap();
        // Drain ParticipantJoined for sid2
        let _ = rx2.try_recv();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let op = make_op(1, uid1, sid1, page_id, CommandDescriptor::AddRoot { node });
        room.submit_op(op);

        let msg = rx2.try_recv().unwrap();
        assert!(matches!(msg, ServerMessage::RemoteOp { .. }));
    }

    // -- Op group tests --

    #[test]
    fn submit_op_group() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx, mut rx) = make_session_with_rx();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let n1 = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let n2 = create_frame_node("B", 60.0, 0.0, 50.0, 50.0, None, [0.0; 4]);

        let ops = vec![
            make_op(
                1,
                uid,
                sid,
                page_id,
                CommandDescriptor::AddRoot { node: n1 },
            ),
            make_op(
                2,
                uid,
                sid,
                page_id,
                CommandDescriptor::AddRoot { node: n2 },
            ),
        ];

        let seqs = room.submit_op_group(ops, "Batch".to_string());
        assert_eq!(seqs, vec![1, 2]);

        let msg = rx.try_recv().unwrap();
        match msg {
            ServerMessage::OpGroupAck {
                client_seqs,
                server_seqs,
            } => {
                assert_eq!(client_seqs, vec![1, 2]);
                assert_eq!(server_seqs, vec![1, 2]);
            }
            _ => panic!("expected OpGroupAck"),
        }
    }

    #[test]
    fn submit_op_group_partial_failure_keeps_valid_ops() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx, _rx) = make_session_with_rx();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;

        let fake_id = NodeId::new();
        let ops = vec![
            make_op(
                1,
                uid,
                sid,
                page_id,
                CommandDescriptor::AddRoot { node },
            ),
            make_op(
                2,
                uid,
                sid,
                page_id,
                CommandDescriptor::SetBounds {
                    node_id: fake_id,
                    bounds: BoundingBox::new(0.0, 0.0, 10.0, 10.0),
                },
            ),
        ];

        let seqs = room.submit_op_group(ops, "Partial".to_string());
        assert_eq!(seqs.len(), 2);

        // The valid AddRoot op should have been applied.
        let scene = &room.document().active_page().scene;
        assert!(scene.get(node_id).is_some());

        // The invalid SetBounds on a nonexistent node should have been a no-op.
        assert!(scene.get(fake_id).is_none());
    }

    #[test]
    fn submit_op_group_empty_returns_empty() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let seqs = room.submit_op_group(vec![], "Empty".to_string());
        assert!(seqs.is_empty());
    }

    #[test]
    fn submit_op_group_undo_reverts_all() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx, mut rx) = make_session_with_rx();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;

        // First add the node.
        room.submit_op(make_op(
            1,
            uid,
            sid,
            page_id,
            CommandDescriptor::AddRoot { node },
        ));
        let _ = rx.try_recv(); // drain ack

        // Then submit a group that modifies it.
        let ops = vec![
            make_op(
                2,
                uid,
                sid,
                page_id,
                CommandDescriptor::SetOpacity {
                    node_id,
                    opacity: 0.5,
                },
            ),
            make_op(
                3,
                uid,
                sid,
                page_id,
                CommandDescriptor::SetName {
                    node_id,
                    name: "Changed".to_string(),
                },
            ),
        ];
        room.submit_op_group(ops, "Group".to_string());
        let _ = rx.try_recv(); // drain group ack

        // Verify both applied.
        let scene = &room.document().active_page().scene;
        assert!((scene.get(node_id).unwrap().opacity - 0.5).abs() < f32::EPSILON);
        assert_eq!(scene.get(node_id).unwrap().name, "Changed");

        // Undo should revert the group ops (each op individually).
        room.handle_undo(sid, uid, page_id);
        let _ = rx.try_recv();
        room.handle_undo(sid, uid, page_id);
        let _ = rx.try_recv();

        let scene = &room.document().active_page().scene;
        assert!((scene.get(node_id).unwrap().opacity - 1.0).abs() < f32::EPSILON);
        assert_eq!(scene.get(node_id).unwrap().name, "A");
    }

    // -- Undo/redo tests --

    #[test]
    fn undo_reverts_property() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;
        room.submit_op(make_op(
            1,
            uid,
            sid,
            page_id,
            CommandDescriptor::AddRoot { node },
        ));

        // Set opacity
        room.submit_op(make_op(
            2,
            uid,
            sid,
            page_id,
            CommandDescriptor::SetOpacity {
                node_id,
                opacity: 0.5,
            },
        ));

        assert_eq!(
            room.document()
                .active_page()
                .scene
                .get(node_id)
                .unwrap()
                .opacity,
            0.5
        );

        // Undo opacity change
        let inv_seq = room.handle_undo(sid, uid, page_id);
        assert!(inv_seq.is_some());

        // Opacity should be back to 1.0
        assert_eq!(
            room.document()
                .active_page()
                .scene
                .get(node_id)
                .unwrap()
                .opacity,
            1.0
        );
    }

    #[test]
    fn redo_after_undo() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;
        room.submit_op(make_op(
            1,
            uid,
            sid,
            page_id,
            CommandDescriptor::AddRoot { node },
        ));

        room.submit_op(make_op(
            2,
            uid,
            sid,
            page_id,
            CommandDescriptor::SetOpacity {
                node_id,
                opacity: 0.5,
            },
        ));

        room.handle_undo(sid, uid, page_id);
        assert_eq!(
            room.document()
                .active_page()
                .scene
                .get(node_id)
                .unwrap()
                .opacity,
            1.0
        );

        room.handle_redo(sid, uid, page_id);
        assert_eq!(
            room.document()
                .active_page()
                .scene
                .get(node_id)
                .unwrap()
                .opacity,
            0.5
        );
    }

    #[test]
    fn undo_empty_stack_returns_none() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();
        let page_id = room.document().active_page().id;
        assert!(room.handle_undo(sid, uid, page_id).is_none());
    }

    #[test]
    fn redo_empty_stack_returns_none() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();
        let page_id = room.document().active_page().id;
        assert!(room.handle_redo(sid, uid, page_id).is_none());
    }

    #[test]
    fn new_op_clears_redo_stack() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;
        room.submit_op(make_op(
            1,
            uid,
            sid,
            page_id,
            CommandDescriptor::AddRoot { node },
        ));
        room.submit_op(make_op(
            2,
            uid,
            sid,
            page_id,
            CommandDescriptor::SetOpacity {
                node_id,
                opacity: 0.5,
            },
        ));

        room.handle_undo(sid, uid, page_id);

        // Submit a new op, which should clear the redo stack.
        room.submit_op(make_op(
            3,
            uid,
            sid,
            page_id,
            CommandDescriptor::SetName {
                node_id,
                name: "Renamed".to_string(),
            },
        ));

        assert!(room.handle_redo(sid, uid, page_id).is_none());
    }

    // -- Multi-user undo isolation --

    #[test]
    fn undo_only_affects_own_ops() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, name1, tx1) = make_session();
        let (sid2, uid2, name2, tx2) = make_session();
        room.join(sid1, uid1, name1, tx1).unwrap();
        room.join(sid2, uid2, name2, tx2).unwrap();

        let page_id = room.document().active_page().id;

        // User 1 adds a node.
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;
        room.submit_op(make_op(
            1,
            uid1,
            sid1,
            page_id,
            CommandDescriptor::AddRoot { node },
        ));

        // User 2 sets opacity.
        room.submit_op(make_op(
            1,
            uid2,
            sid2,
            page_id,
            CommandDescriptor::SetOpacity {
                node_id,
                opacity: 0.3,
            },
        ));

        // User 2 undoes their opacity change.
        room.handle_undo(sid2, uid2, page_id);
        assert_eq!(
            room.document()
                .active_page()
                .scene
                .get(node_id)
                .unwrap()
                .opacity,
            1.0
        );

        // Node still exists (user 1's add was not undone).
        assert!(room.document().active_page().scene.get(node_id).is_some());
    }

    // -- Failed op on deleted node --

    #[test]
    fn op_on_deleted_node_still_sequenced() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let bogus_id = NodeId::new();

        // Submit an op targeting a non-existent node.
        let seq = room.submit_op(make_op(
            1,
            uid,
            sid,
            page_id,
            CommandDescriptor::SetOpacity {
                node_id: bogus_id,
                opacity: 0.5,
            },
        ));

        // Still gets a sequence number.
        assert_eq!(seq, 1);
        assert_eq!(room.latest_seq(), 1);
    }

    // -- Presence --

    #[test]
    fn presence_broadcasts_to_others() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, name1, tx1) = make_session();
        let (sid2, uid2, name2, tx2, mut rx2) = make_session_with_rx();
        room.join(sid1, uid1, name1, tx1).unwrap();
        room.join(sid2, uid2, name2, tx2).unwrap();
        let _ = rx2.try_recv(); // drain ParticipantJoined

        let page_id = room.document().active_page().id;
        room.handle_presence(
            sid1,
            page_id,
            Some(CursorPosition { x: 100.0, y: 200.0 }),
            vec![],
        );

        let msg = rx2.try_recv().unwrap();
        match msg {
            ServerMessage::PresenceBroadcast {
                session_id, cursor, ..
            } => {
                assert_eq!(session_id, sid1);
                assert!(cursor.is_some());
            }
            _ => panic!("expected PresenceBroadcast"),
        }
    }

    // -- Participants list --

    #[test]
    fn participants_list() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, _, tx1) = make_session();
        let (sid2, uid2, _, tx2) = make_session();
        room.join(sid1, uid1, "Alice".to_string(), tx1).unwrap();
        room.join(sid2, uid2, "Bob".to_string(), tx2).unwrap();

        let parts = room.participants();
        assert_eq!(parts.len(), 2);
    }

    // -- Sequential seq assignment --

    #[test]
    fn sequential_seq_numbers() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        for i in 1..=5 {
            let node = create_frame_node(&format!("N{i}"), 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
            let seq = room.submit_op(make_op(
                i,
                uid,
                sid,
                page_id,
                CommandDescriptor::AddRoot { node },
            ));
            assert_eq!(seq, i);
        }
        assert_eq!(room.latest_seq(), 5);
    }

    // -- Concurrent property conflict (LWW) --

    #[test]
    fn concurrent_property_last_writer_wins() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, name1, tx1) = make_session();
        let (sid2, uid2, name2, tx2) = make_session();
        room.join(sid1, uid1, name1, tx1).unwrap();
        room.join(sid2, uid2, name2, tx2).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("A", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let node_id = node.id;
        room.submit_op(make_op(
            1,
            uid1,
            sid1,
            page_id,
            CommandDescriptor::AddRoot { node },
        ));

        // Both users set bounds concurrently; server orders them.
        room.submit_op(make_op(
            2,
            uid1,
            sid1,
            page_id,
            CommandDescriptor::SetBounds {
                node_id,
                bounds: BoundingBox::new(10.0, 10.0, 100.0, 100.0),
            },
        ));
        room.submit_op(make_op(
            1,
            uid2,
            sid2,
            page_id,
            CommandDescriptor::SetBounds {
                node_id,
                bounds: BoundingBox::new(20.0, 20.0, 200.0, 200.0),
            },
        ));

        // Last writer wins: user 2's bounds.
        let bounds = room
            .document()
            .active_page()
            .scene
            .get(node_id)
            .unwrap()
            .bounds;
        assert_eq!(bounds.x, 20.0);
        assert_eq!(bounds.width, 200.0);
    }

    // -- Room cleanup on last leave --

    #[test]
    fn room_cleanup_on_last_leave() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();
        assert!(room.leave(sid));
        assert_eq!(room.session_count(), 0);
    }

    // -- handle_message dispatch --

    #[test]
    fn handle_message_leave() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();
        let empty = room.handle_message(sid, ClientMessage::LeaveRoom);
        assert!(empty);
    }

    #[test]
    fn handle_message_submit_op() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        let node = create_frame_node("X", 0.0, 0.0, 50.0, 50.0, None, [0.0; 4]);
        let op = make_op(1, uid, sid, page_id, CommandDescriptor::AddRoot { node });
        let empty = room.handle_message(sid, ClientMessage::SubmitOp { op: Box::new(op) });
        assert!(!empty);
        assert_eq!(room.latest_seq(), 1);
    }

    // -- Page operation tests --

    #[test]
    fn page_op_add() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx, mut rx) = make_session_with_rx();
        room.join(sid, uid, name, tx).unwrap();
        assert_eq!(room.document().page_count(), 1);

        let new_page_id = PageId::new();
        let seq = room.handle_page_op(
            selean_collab::protocol::PageOpType::Add,
            new_page_id,
            Some("Slide 2".to_string()),
            Some(800.0),
            Some(600.0),
        );

        assert_eq!(seq, 1);
        assert_eq!(room.document().page_count(), 2);
        let page = room.document().page(new_page_id).unwrap();
        assert_eq!(page.name, "Slide 2");
        assert_eq!(page.width, 800.0);
        assert_eq!(page.height, 600.0);

        // Should broadcast RemotePageOp.
        let msg = rx.try_recv().unwrap();
        assert!(matches!(msg, ServerMessage::RemotePageOp { .. }));
    }

    #[test]
    fn page_op_remove() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let new_page_id = PageId::new();
        room.handle_page_op(
            selean_collab::protocol::PageOpType::Add,
            new_page_id,
            Some("To Remove".to_string()),
            None,
            None,
        );
        assert_eq!(room.document().page_count(), 2);

        room.handle_page_op(
            selean_collab::protocol::PageOpType::Remove,
            new_page_id,
            None,
            None,
            None,
        );
        assert_eq!(room.document().page_count(), 1);
        assert!(room.document().page(new_page_id).is_none());
    }

    #[test]
    fn page_op_rename() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        room.handle_page_op(
            selean_collab::protocol::PageOpType::Rename,
            page_id,
            Some("Renamed Page".to_string()),
            None,
            None,
        );
        assert_eq!(room.document().active_page().name, "Renamed Page");
    }

    #[test]
    fn page_op_remove_last_page_no_op() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let page_id = room.document().active_page().id;
        room.handle_page_op(
            selean_collab::protocol::PageOpType::Remove,
            page_id,
            None,
            None,
            None,
        );
        // Cannot remove the last page.
        assert_eq!(room.document().page_count(), 1);
    }

    #[test]
    fn page_op_add_broadcasts_to_all() {
        let mut room = Room::new(RoomId::new());
        let (sid1, uid1, name1, tx1, mut rx1) = make_session_with_rx();
        let (sid2, uid2, name2, tx2, mut rx2) = make_session_with_rx();
        room.join(sid1, uid1, name1, tx1).unwrap();
        room.join(sid2, uid2, name2, tx2).unwrap();
        let _ = rx1.try_recv(); // drain ParticipantJoined for sid2
        let _ = rx2.try_recv(); // drain ParticipantJoined for sid2

        room.handle_page_op(
            selean_collab::protocol::PageOpType::Add,
            PageId::new(),
            Some("Shared Page".to_string()),
            None,
            None,
        );

        // Both sessions should receive the broadcast.
        let msg1 = rx1.try_recv().unwrap();
        let msg2 = rx2.try_recv().unwrap();
        assert!(matches!(msg1, ServerMessage::RemotePageOp { .. }));
        assert!(matches!(msg2, ServerMessage::RemotePageOp { .. }));
    }

    #[test]
    fn handle_message_submit_page_op() {
        let mut room = Room::new(RoomId::new());
        let (sid, uid, name, tx) = make_session();
        room.join(sid, uid, name, tx).unwrap();

        let empty = room.handle_message(
            sid,
            ClientMessage::SubmitPageOp {
                op_type: selean_collab::protocol::PageOpType::Add,
                page_id: PageId::new(),
                name: Some("Via Message".to_string()),
                width: Some(1280.0),
                height: Some(720.0),
            },
        );
        assert!(!empty);
        assert_eq!(room.document().page_count(), 2);
    }
}
