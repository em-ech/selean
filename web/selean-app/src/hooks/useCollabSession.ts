/**
 * React hook for managing a real-time collaboration session.
 *
 * Provides connection management, operation submission, undo/redo routing,
 * and presence updates over the WebSocket collab protocol.
 */

import { useCallback, useEffect, useRef, useState } from "react";
import { OperationBuffer } from "../collab/op-buffer";
import type {
  ClientSeqNum,
  CursorPosition,
  Operation,
  PageOpType,
  Participant,
  ServerMessage,
} from "../collab/types";
import { WsClient, type ConnectionStatus } from "../collab/ws-client";
import type { SeleanEditor } from "../wasm/types";

/** Presence data for a remote participant. */
export interface RemotePresenceData {
  sessionId: string;
  userId: string;
  displayName: string;
  pageId: string;
  cursor: CursorPosition | null;
  selectedNodeIds: string[];
}

export interface CollabSession {
  status: ConnectionStatus;
  participants: Participant[];
  remotePresences: RemotePresenceData[];
  hasPendingOps: boolean;
  connect(roomId: string, userId: string, displayName: string): void;
  disconnect(): void;
  submitOp(descriptor: Record<string, unknown>, pageId: string): void;
  submitOpGroup(
    descriptors: Record<string, unknown>[],
    pageId: string,
    label: string,
  ): void;
  submitPageOp(
    opType: PageOpType,
    pageId: string,
    name?: string,
    width?: number,
    height?: number,
  ): void;
  undo(pageId: string): void;
  redo(pageId: string): void;
  updatePresence(
    pageId: string,
    cursor: CursorPosition | null,
    selectedIds: string[],
  ): void;
}

export interface UseCollabSessionOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  onRemoteChange?: () => void;
}

export function useCollabSession(
  options: UseCollabSessionOptions,
): CollabSession {
  const { editorRef, onRemoteChange } = options;

  const [status, setStatus] = useState<ConnectionStatus>("disconnected");
  const [participants, setParticipants] = useState<Participant[]>([]);
  const [remotePresences, setRemotePresences] = useState<RemotePresenceData[]>(
    [],
  );
  const [hasPendingOps, setHasPendingOps] = useState(false);

  const wsRef = useRef<WsClient | null>(null);
  const bufferRef = useRef(new OperationBuffer());
  const sessionIdRef = useRef<string>("");
  const userIdRef = useRef<string>("");
  const roomIdRef = useRef<string>("");
  const displayNameRef = useRef<string>("");

  const handleMessage = useCallback(
    (msg: ServerMessage) => {
      const editor = editorRef.current;

      switch (msg.type) {
        case "RoomJoined": {
          sessionIdRef.current = msg.session_id;
          setParticipants(msg.participants);
          if (editor && msg.document_json) {
            editor.import_document(msg.document_json);
            onRemoteChange?.();
          }
          break;
        }

        case "OpAck": {
          bufferRef.current.acknowledge(msg.client_seq);
          setHasPendingOps(bufferRef.current.hasPending());
          break;
        }

        case "OpGroupAck": {
          bufferRef.current.acknowledgeAll(msg.client_seqs);
          setHasPendingOps(bufferRef.current.hasPending());
          break;
        }

        case "RemoteOp": {
          if (editor) {
            editor.apply_remote_op(
              msg.op.page_id,
              JSON.stringify(msg.op.descriptor),
            );
            onRemoteChange?.();
          }
          break;
        }

        case "RemoteOpGroup": {
          if (editor) {
            const descs = msg.ops.map((op) => op.descriptor);
            const pageId = msg.ops[0]?.page_id;
            if (pageId) {
              editor.apply_remote_op_group(pageId, JSON.stringify(descs));
              onRemoteChange?.();
            }
          }
          break;
        }

        case "UndoResult":
        case "RedoResult": {
          if (editor) {
            for (const op of msg.inverse_ops) {
              editor.apply_remote_op(op.page_id, JSON.stringify(op.descriptor));
            }
            onRemoteChange?.();
          }
          break;
        }

        case "RemotePageOp": {
          if (editor) {
            editor.apply_remote_page_op(
              msg.op_type,
              msg.page_id,
              msg.name,
              msg.width,
              msg.height,
            );
            onRemoteChange?.();
          }
          break;
        }

        case "PresenceBroadcast": {
          setParticipants((prev) => {
            const existing = prev.find((p) => p.session_id === msg.session_id);
            if (!existing) {
              return [
                ...prev,
                {
                  session_id: msg.session_id,
                  user_id: msg.user_id,
                  display_name: msg.display_name,
                },
              ];
            }
            return prev;
          });
          setRemotePresences((prev) => {
            const updated: RemotePresenceData = {
              sessionId: msg.session_id,
              userId: msg.user_id,
              displayName: msg.display_name,
              pageId: msg.page_id,
              cursor: msg.cursor,
              selectedNodeIds: msg.selected_node_ids,
            };
            const idx = prev.findIndex((p) => p.sessionId === msg.session_id);
            if (idx >= 0) {
              const next = [...prev];
              next[idx] = updated;
              return next;
            }
            return [...prev, updated];
          });
          break;
        }

        case "ParticipantJoined": {
          setParticipants((prev) => [...prev, msg.participant]);
          break;
        }

        case "ParticipantLeft": {
          setParticipants((prev) =>
            prev.filter((p) => p.session_id !== msg.session_id),
          );
          setRemotePresences((prev) =>
            prev.filter((p) => p.sessionId !== msg.session_id),
          );
          break;
        }

        case "Error": {
          console.error("[collab] server error:", msg.message);
          break;
        }
      }
    },
    [editorRef, onRemoteChange],
  );

  // Cleanup on unmount.
  useEffect(() => {
    return () => {
      wsRef.current?.disconnect();
    };
  }, []);

  const connect = useCallback(
    (roomId: string, userId: string, displayName: string) => {
      userIdRef.current = userId;
      roomIdRef.current = roomId;
      displayNameRef.current = displayName;
      bufferRef.current.clear();
      setHasPendingOps(false);

      const ws = new WsClient({
        onStatusChange: setStatus,
        onMessage: handleMessage,
        onReconnect: () => {
          // Re-join room on reconnection. The server will respond with
          // a fresh RoomJoined containing the current document snapshot.
          bufferRef.current.clear();
          setHasPendingOps(false);
          ws.send({
            type: "JoinRoom",
            room_id: roomIdRef.current,
            user_id: userIdRef.current,
            display_name: displayNameRef.current,
          });
        },
      });
      wsRef.current = ws;

      const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
      const wsUrl = `${protocol}//${window.location.host}/api/ws`;
      ws.connect(wsUrl);

      // Send JoinRoom once connected. Poll until open.
      const joinInterval = setInterval(() => {
        if (ws.getStatus() === "connected") {
          clearInterval(joinInterval);
          ws.send({
            type: "JoinRoom",
            room_id: roomId,
            user_id: userId,
            display_name: displayName,
          });
        }
      }, 50);

      // Clean up the interval if we disconnect before joining.
      setTimeout(() => clearInterval(joinInterval), 10_000);
    },
    [handleMessage],
  );

  const disconnect = useCallback(() => {
    wsRef.current?.send({ type: "LeaveRoom" });
    wsRef.current?.disconnect();
    wsRef.current = null;
    setParticipants([]);
    setRemotePresences([]);
    bufferRef.current.clear();
    setHasPendingOps(false);
  }, []);

  const makeOperation = useCallback(
    (
      descriptor: Record<string, unknown>,
      pageId: string,
      clientSeq: ClientSeqNum,
    ): Operation => ({
      seq: null,
      client_seq: clientSeq,
      user_id: userIdRef.current,
      session_id: sessionIdRef.current,
      page_id: pageId,
      descriptor,
      timestamp: null,
    }),
    [],
  );

  const submitOp = useCallback(
    (descriptor: Record<string, unknown>, pageId: string) => {
      const clientSeq = bufferRef.current.enqueue(descriptor, pageId);
      setHasPendingOps(true);
      wsRef.current?.send({
        type: "SubmitOp",
        op: makeOperation(descriptor, pageId, clientSeq),
      });
    },
    [makeOperation],
  );

  const submitOpGroup = useCallback(
    (descriptors: Record<string, unknown>[], pageId: string, label: string) => {
      const ops = descriptors.map((desc) => {
        const clientSeq = bufferRef.current.enqueue(desc, pageId);
        return makeOperation(desc, pageId, clientSeq);
      });
      setHasPendingOps(true);
      wsRef.current?.send({
        type: "SubmitOpGroup",
        ops,
        group_label: label,
      });
    },
    [makeOperation],
  );

  const submitPageOp = useCallback(
    (
      opType: PageOpType,
      pageId: string,
      name?: string,
      width?: number,
      height?: number,
    ) => {
      wsRef.current?.send({
        type: "SubmitPageOp",
        op_type: opType,
        page_id: pageId,
        name: name ?? null,
        width: width ?? null,
        height: height ?? null,
      });
    },
    [],
  );

  const undo = useCallback((pageId: string) => {
    wsRef.current?.send({ type: "Undo", page_id: pageId });
  }, []);

  const redo = useCallback((pageId: string) => {
    wsRef.current?.send({ type: "Redo", page_id: pageId });
  }, []);

  const updatePresence = useCallback(
    (pageId: string, cursor: CursorPosition | null, selectedIds: string[]) => {
      wsRef.current?.send({
        type: "PresenceUpdate",
        page_id: pageId,
        cursor,
        selected_node_ids: selectedIds,
      });
    },
    [],
  );

  return {
    status,
    participants,
    remotePresences,
    hasPendingOps,
    connect,
    disconnect,
    submitOp,
    submitOpGroup,
    submitPageOp,
    undo,
    redo,
    updatePresence,
  };
}
