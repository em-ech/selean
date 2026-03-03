//! Real-time collaborative editing protocol for Selean.
//!
//! This crate provides:
//! - Wire protocol types for client-server WebSocket communication
//! - Operation envelope and sequencing types
//! - Inverse computation for server-side undo
//! - Append-only operation log with per-user indexing

pub mod inverse;
pub mod op_log;
pub mod protocol;
pub mod types;

pub use inverse::compute_inverse;
pub use op_log::OpLog;
pub use protocol::{ClientMessage, ServerMessage};
pub use types::{ClientSeqNum, CursorPosition, Operation, Participant, RoomId, SeqNum, SessionId};
