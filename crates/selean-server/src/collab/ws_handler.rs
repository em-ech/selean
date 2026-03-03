//! WebSocket handler for collaborative editing sessions.
//!
//! Upgrades HTTP connections to WebSocket, splits into reader/writer tasks,
//! and dispatches parsed client messages to the room manager.

use std::sync::Arc;

use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::{Mutex, mpsc};

use selean_collab::protocol::{ClientMessage, ServerMessage};
use selean_collab::types::SessionId;

use super::room_manager::RoomManager;

/// Shared state for collaborative editing.
#[derive(Clone)]
pub struct CollabState {
    /// Room manager protected by a mutex for serialized access.
    pub room_manager: Arc<Mutex<RoomManager>>,
}

impl CollabState {
    /// Creates a new collab state with an empty room manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            room_manager: Arc::new(Mutex::new(RoomManager::new())),
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

async fn handle_socket(socket: WebSocket, state: CollabState) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    let (msg_tx, mut msg_rx) = mpsc::channel::<ServerMessage>(64);

    // Writer task: forwards ServerMessages to the WebSocket.
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

    // Reader loop: parse incoming messages and dispatch.
    while let Some(Ok(raw_msg)) = ws_rx.next().await {
        let text = match &raw_msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

        let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) else {
            let err = ServerMessage::Error {
                message: "invalid message format".to_string(),
            };
            let _ = msg_tx.try_send(err);
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

                let mut mgr = state.room_manager.lock().await;
                let room = mgr.create_room(rid);
                match room.join(sid, user_id, display_name, msg_tx.clone()) {
                    Ok(joined_msg) => {
                        let _ = msg_tx.try_send(joined_msg);
                    }
                    Err(e) => {
                        let _ = msg_tx.try_send(ServerMessage::Error {
                            message: e.to_string(),
                        });
                    }
                }
            }
            other => {
                if let (Some(sid), Some(rid)) = (session_id, room_id) {
                    let mut mgr = state.room_manager.lock().await;
                    if let Some(room) = mgr.room_mut(&rid) {
                        let should_remove = room.handle_message(sid, other);
                        if should_remove {
                            mgr.remove_room(&rid);
                            break;
                        }
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
    if let (Some(sid), Some(rid)) = (session_id, room_id) {
        let mut mgr = state.room_manager.lock().await;
        if let Some(room) = mgr.room_mut(&rid) {
            let empty = room.leave(sid);
            if empty {
                mgr.remove_room(&rid);
            }
        }
    }

    writer_handle.abort();
}
