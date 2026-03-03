//! Real-time collaborative editing server module.
//!
//! Manages rooms, WebSocket connections, and operation sequencing.

pub mod room_manager;
pub mod ws_handler;

pub use room_manager::{Room, RoomManager};
pub use ws_handler::ws_handler;
