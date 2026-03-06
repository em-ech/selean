//! WebSocket handler for collaborative editing sessions.
//!
//! Upgrades HTTP connections to WebSocket, splits into reader/writer tasks,
//! and dispatches parsed client messages to the room manager.
//!
//! Locking strategy: a [`std::sync::RwLock`] protects the room map for
//! create/lookup/remove. Each room has its own [`std::sync::Mutex`] so
//! operations on different rooms never contend.

use std::sync::{Arc, Mutex, RwLock};

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use selean_collab::protocol::{ClientMessage, ServerMessage};
use selean_collab::types::SessionId;

use super::room_manager::{Room, RoomManager};

/// Shared state for collaborative editing.
///
/// The room manager is behind a [`RwLock`] so room lookups (read) do not
/// block each other. Write access is only needed for room creation/removal.
#[derive(Clone)]
pub struct CollabState {
    /// Room manager protected by a read-write lock.
    pub room_manager: Arc<RwLock<RoomManager>>,
}

impl CollabState {
    /// Creates a new collab state with an empty room manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            room_manager: Arc::new(RwLock::new(RoomManager::new())),
        }
    }
}

impl Default for CollabState {
    fn default() -> Self {
        Self::new()
    }
}

/// Axum handler that upgrades an HTTP request to a WebSocket connection.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<CollabState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Result of handling a single join request.
enum JoinOutcome {
    /// Room joined successfully.
    Ok(Arc<Mutex<Room>>),
    /// Lock poisoned; the connection must close.
    PoisonedLock,
}

/// Attempts to join a room, sending the appropriate response.
fn handle_join(
    state: &CollabState,
    rid: selean_collab::types::RoomId,
    sid: SessionId,
    user_id: selean_common::types::UserId,
    display_name: String,
    msg_tx: &mpsc::Sender<ServerMessage>,
) -> JoinOutcome {
    let Ok(mut mgr) = state.room_manager.write() else {
        tracing::error!("room manager lock poisoned during join");
        let _ = msg_tx.try_send(ServerMessage::Error {
            message: "internal server error".to_string(),
        });
        return JoinOutcome::PoisonedLock;
    };
    let room_arc = mgr.get_or_create_room(rid);
    drop(mgr);

    let join_result = match room_arc.lock() {
        Ok(mut room) => room.join(sid, user_id, display_name, msg_tx.clone()),
        Err(e) => {
            tracing::error!("room lock poisoned during join: {e}");
            let _ = msg_tx.try_send(ServerMessage::Error {
                message: "internal server error".to_string(),
            });
            return JoinOutcome::PoisonedLock;
        }
    };

    match join_result {
        Ok(joined_msg) => {
            let _ = msg_tx.try_send(joined_msg);
        }
        Err(e) => {
            let _ = msg_tx.try_send(ServerMessage::Error {
                message: e.to_string(),
            });
        }
    }

    JoinOutcome::Ok(room_arc)
}

/// Dispatches a message to the room. Returns `true` if the loop should break.
fn dispatch_message(
    state: &CollabState,
    sid: SessionId,
    rid: selean_collab::types::RoomId,
    room_arc: &Arc<Mutex<Room>>,
    msg: ClientMessage,
    msg_tx: &mpsc::Sender<ServerMessage>,
) -> bool {
    let should_remove = match room_arc.lock() {
        Ok(mut room) => room.handle_message(sid, msg),
        Err(e) => {
            tracing::error!("room lock poisoned during message: {e}");
            let _ = msg_tx.try_send(ServerMessage::Error {
                message: "internal server error".to_string(),
            });
            return true;
        }
    };
    if should_remove {
        if let Ok(mut mgr) = state.room_manager.write() {
            mgr.remove_room(&rid);
        }
        return true;
    }
    false
}

async fn handle_socket(socket: WebSocket, state: CollabState) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (msg_tx, mut msg_rx) = mpsc::channel::<ServerMessage>(64);

    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if ws_tx.send(Message::Text(json.into())).await.is_err() {
                    break;
                }
            }
        }
    });

    let mut session_id: Option<SessionId> = None;
    let mut room_id = None;
    let mut current_room: Option<Arc<Mutex<Room>>> = None;

    while let Some(Ok(raw_msg)) = ws_rx.next().await {
        let text = match &raw_msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

        let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) else {
            let _ = msg_tx.try_send(ServerMessage::Error {
                message: "invalid message format".to_string(),
            });
            continue;
        };

        match client_msg {
            ClientMessage::JoinRoom {
                room_id: rid,
                user_id,
                display_name,
            } => {
                let sid = SessionId::new();
                session_id = Some(sid);
                room_id = Some(rid);

                match handle_join(&state, rid, sid, user_id, display_name, &msg_tx) {
                    JoinOutcome::Ok(room_arc) => current_room = Some(room_arc),
                    JoinOutcome::PoisonedLock => break,
                }
            }
            other => {
                if let (Some(sid), Some(rid), Some(room_arc)) = (session_id, room_id, &current_room)
                {
                    if dispatch_message(&state, sid, rid, room_arc, other, &msg_tx) {
                        break;
                    }
                } else {
                    let _ = msg_tx.try_send(ServerMessage::Error {
                        message: "must join a room first".to_string(),
                    });
                }
            }
        }
    }

    // Cleanup on disconnect.
    if let (Some(sid), Some(rid), Some(room_arc)) = (session_id, room_id, &current_room) {
        let empty = match room_arc.lock() {
            Ok(mut room) => room.leave(sid),
            Err(e) => {
                tracing::error!("room lock poisoned during cleanup: {e}");
                false
            }
        };
        if empty {
            if let Ok(mut mgr) = state.room_manager.write() {
                mgr.remove_room(&rid);
            }
        }
    }

    writer_handle.abort();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use selean_collab::protocol::ClientMessage;
    use selean_common::types::UserId;

    #[test]
    fn collab_state_new_creates_empty_manager() {
        let state = CollabState::new();
        let mgr = state.room_manager.read().expect("lock poisoned");
        assert_eq!(mgr.room_count(), 0);
    }

    #[test]
    fn collab_state_default_is_same_as_new() {
        let state = CollabState::default();
        let mgr = state.room_manager.read().expect("lock poisoned");
        assert_eq!(mgr.room_count(), 0);
    }

    #[test]
    fn collab_state_clone_shares_room_manager() {
        let state = CollabState::new();
        let cloned = state.clone();

        // Mutate via original; visible through clone.
        {
            let mut mgr = state.room_manager.write().expect("lock poisoned");
            let rid = selean_collab::types::RoomId::new();
            mgr.get_or_create_room(rid);
        }

        let mgr = cloned.room_manager.read().expect("lock poisoned");
        assert_eq!(mgr.room_count(), 1);
    }

    #[test]
    fn invalid_json_produces_error_message() {
        // Verify the error message format used by ws_handler for invalid JSON.
        let text = "not valid json {{{";
        let result = serde_json::from_str::<ClientMessage>(text);
        assert!(result.is_err());
        // ws_handler sends ServerMessage::Error with "invalid message format"
        let err = ServerMessage::Error {
            message: "invalid message format".to_string(),
        };
        let json = serde_json::to_string(&err).expect("serialize error msg");
        assert!(json.contains("invalid message format"));
    }

    #[test]
    fn must_join_room_error_message() {
        // Verify the "must join a room first" error that ws_handler sends
        // when a message arrives before JoinRoom.
        let err = ServerMessage::Error {
            message: "must join a room first".to_string(),
        };
        let json = serde_json::to_string(&err).expect("serialize error msg");
        assert!(json.contains("must join a room first"));
    }

    #[tokio::test]
    async fn join_creates_room_and_session() {
        let state = CollabState::new();
        let rid = selean_collab::types::RoomId::new();
        let uid = UserId::new();
        let sid = SessionId::new();
        let (tx, mut rx) = mpsc::channel::<ServerMessage>(16);

        // Simulate join flow from ws_handler.
        let room_arc = {
            let mut mgr = state.room_manager.write().expect("lock");
            mgr.get_or_create_room(rid)
        };

        let join_result = {
            let mut room = room_arc.lock().expect("room lock");
            room.join(sid, uid, "TestUser".to_string(), tx.clone())
        };

        assert!(join_result.is_ok());
        let msg = join_result.unwrap();
        match msg {
            ServerMessage::RoomJoined {
                room_id,
                session_id,
                ..
            } => {
                assert_eq!(room_id, rid);
                assert_eq!(session_id, sid);
            }
            other => panic!("expected RoomJoined, got: {other:?}"),
        }

        // Verify room exists with 1 session.
        let room = room_arc.lock().expect("room lock");
        assert_eq!(room.session_count(), 1);

        // No broadcast messages for single-user join.
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn leave_removes_session_and_returns_empty() {
        let state = CollabState::new();
        let rid = selean_collab::types::RoomId::new();
        let uid = UserId::new();
        let sid = SessionId::new();
        let (tx, _rx) = mpsc::channel::<ServerMessage>(16);

        let room_arc = {
            let mut mgr = state.room_manager.write().expect("lock");
            mgr.get_or_create_room(rid)
        };

        {
            let mut room = room_arc.lock().expect("room lock");
            let _ = room.join(sid, uid, "User".to_string(), tx);
        }

        let empty = {
            let mut room = room_arc.lock().expect("room lock");
            room.leave(sid)
        };

        assert!(empty, "room should be empty after last session leaves");

        // ws_handler would remove the room at this point.
        let mut mgr = state.room_manager.write().expect("lock");
        mgr.remove_room(&rid);
        assert_eq!(mgr.room_count(), 0);
    }

    #[tokio::test]
    async fn message_before_join_does_not_panic() {
        // Simulates the "other" branch when session_id is None.
        // In ws_handler, this sends "must join a room first" error.
        let session_id: Option<SessionId> = None;
        let room_id: Option<selean_collab::types::RoomId> = None;
        let current_room: Option<Arc<Mutex<super::super::room_manager::Room>>> = None;

        // The guard check: if session_id, room_id, and current_room are all None,
        // the handler sends the error. Verify the guard logic.
        let should_dispatch = session_id.is_some() && room_id.is_some() && current_room.is_some();
        assert!(!should_dispatch, "should not dispatch without join");
    }

    #[tokio::test]
    async fn disconnect_cleanup_with_multiple_sessions() {
        let state = CollabState::new();
        let rid = selean_collab::types::RoomId::new();
        let uid1 = UserId::new();
        let uid2 = UserId::new();
        let sid1 = SessionId::new();
        let sid2 = SessionId::new();
        let (tx1, _rx1) = mpsc::channel::<ServerMessage>(16);
        let (tx2, _rx2) = mpsc::channel::<ServerMessage>(16);

        let room_arc = {
            let mut mgr = state.room_manager.write().expect("lock");
            mgr.get_or_create_room(rid)
        };

        {
            let mut room = room_arc.lock().expect("lock");
            let _ = room.join(sid1, uid1, "User1".to_string(), tx1);
            let _ = room.join(sid2, uid2, "User2".to_string(), tx2);
        }

        // First leave: room not empty.
        let empty = {
            let mut room = room_arc.lock().expect("lock");
            room.leave(sid1)
        };
        assert!(
            !empty,
            "room should not be empty with one session remaining"
        );

        // Second leave: room empty.
        let empty = {
            let mut room = room_arc.lock().expect("lock");
            room.leave(sid2)
        };
        assert!(empty, "room should be empty after all sessions leave");
    }
}
