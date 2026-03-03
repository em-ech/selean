/**
 * TypeScript types for the real-time collaboration protocol.
 *
 * These mirror the Rust types in `crates/selean-collab/src/protocol.rs`
 * and `crates/selean-collab/src/types.rs`.
 */

/** Server-assigned monotonic sequence number. */
export type SeqNum = number;

/** Client-local sequence counter. */
export type ClientSeqNum = number;

/** Cursor position in world coordinates. */
export interface CursorPosition {
  x: number;
  y: number;
}

/** A remote participant in the collaboration session. */
export interface Participant {
  session_id: string;
  user_id: string;
  display_name: string;
}

/** An operation envelope sent over the wire. */
export interface Operation {
  seq: SeqNum | null;
  client_seq: ClientSeqNum;
  user_id: string;
  session_id: string;
  page_id: string;
  descriptor: Record<string, unknown>;
  timestamp: number | null;
}

// --- Client-to-Server Messages ---

/** Page operation type. Matches Rust `PageOpType`. */
export type PageOpType = "add" | "remove" | "rename";

export type ClientMessage =
  | {
      type: "JoinRoom";
      room_id: string;
      user_id: string;
      display_name: string;
    }
  | { type: "LeaveRoom" }
  | { type: "SubmitOp"; op: Operation }
  | { type: "SubmitOpGroup"; ops: Operation[]; group_label: string }
  | { type: "Undo"; page_id: string }
  | { type: "Redo"; page_id: string }
  | {
      type: "SubmitPageOp";
      op_type: PageOpType;
      page_id: string;
      name: string | null;
      width: number | null;
      height: number | null;
    }
  | {
      type: "PresenceUpdate";
      page_id: string;
      cursor: CursorPosition | null;
      selected_node_ids: string[];
    };

// --- Server-to-Client Messages ---

export type ServerMessage =
  | {
      type: "RoomJoined";
      room_id: string;
      session_id: string;
      participants: Participant[];
      document_json: string;
      latest_seq: SeqNum;
    }
  | { type: "OpAck"; client_seq: ClientSeqNum; server_seq: SeqNum }
  | {
      type: "OpGroupAck";
      client_seqs: ClientSeqNum[];
      server_seqs: SeqNum[];
    }
  | { type: "RemoteOp"; op: Operation }
  | { type: "RemoteOpGroup"; ops: Operation[]; group_label: string }
  | {
      type: "UndoResult";
      page_id: string;
      inverse_ops: Operation[];
    }
  | {
      type: "RedoResult";
      page_id: string;
      inverse_ops: Operation[];
    }
  | {
      type: "PresenceBroadcast";
      session_id: string;
      user_id: string;
      display_name: string;
      page_id: string;
      cursor: CursorPosition | null;
      selected_node_ids: string[];
    }
  | {
      type: "RemotePageOp";
      op_type: PageOpType;
      page_id: string;
      name: string | null;
      width: number | null;
      height: number | null;
      server_seq: SeqNum;
    }
  | { type: "ParticipantJoined"; participant: Participant }
  | { type: "ParticipantLeft"; session_id: string }
  | { type: "Error"; message: string };
