import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useCollabSession } from "./useCollabSession";
import { createMockEditorRef } from "../test/mock-editor";
import type { ServerMessage } from "../collab/types";

// Capture callbacks from WsClient constructor.
let capturedOnMessage: ((msg: ServerMessage) => void) | undefined;
let capturedOnReconnect: (() => void) | undefined;
let capturedOnStatusChange: ((s: string) => void) | undefined;
let mockSend: ReturnType<typeof vi.fn>;
let mockConnect: ReturnType<typeof vi.fn>;
let mockDisconnect: ReturnType<typeof vi.fn>;
let mockGetStatus: ReturnType<typeof vi.fn>;

vi.mock("../collab/ws-client", () => ({
  WsClient: class MockWsClient {
    constructor(options: Record<string, unknown>) {
      capturedOnMessage = options.onMessage as typeof capturedOnMessage;
      capturedOnReconnect = options.onReconnect as typeof capturedOnReconnect;
      capturedOnStatusChange =
        options.onStatusChange as typeof capturedOnStatusChange;
      this.send = mockSend;
      this.connect = mockConnect;
      this.disconnect = mockDisconnect;
      this.getStatus = mockGetStatus;
    }
    send = mockSend;
    connect = mockConnect;
    disconnect = mockDisconnect;
    getStatus = mockGetStatus;
  },
}));

beforeEach(() => {
  mockSend = vi.fn().mockReturnValue(true);
  mockConnect = vi.fn();
  mockDisconnect = vi.fn();
  mockGetStatus = vi.fn().mockReturnValue("disconnected");
  capturedOnMessage = undefined;
  capturedOnReconnect = undefined;
  capturedOnStatusChange = undefined;
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

/**
 * Helper: renders the hook and connects so the WsClient mock is
 * instantiated and all captured callbacks are available.
 */
function setupConnected(options: { onRemoteChange?: () => void } = {}) {
  const editorRef = createMockEditorRef();
  const onRemoteChange = options.onRemoteChange ?? vi.fn();
  const { result } = renderHook(() =>
    useCollabSession({ editorRef, onRemoteChange }),
  );

  // Trigger connect to instantiate the mock WsClient.
  act(() => {
    result.current.connect("room-1", "user-1", "Alice");
  });

  // Simulate the WS becoming connected and advance past the join interval.
  mockGetStatus.mockReturnValue("connected");
  act(() => {
    vi.advanceTimersByTime(100);
  });

  // Clear the JoinRoom send call so subsequent assertions are clean.
  mockSend.mockClear();

  return { editorRef, result, onRemoteChange };
}

describe("useCollabSession", () => {
  it("starts disconnected with empty participants", () => {
    const editorRef = createMockEditorRef();
    const { result } = renderHook(() => useCollabSession({ editorRef }));
    expect(result.current.status).toBe("disconnected");
    expect(result.current.participants).toEqual([]);
    expect(result.current.hasPendingOps).toBe(false);
  });

  it("connect sends JoinRoom when ws becomes connected", () => {
    const editorRef = createMockEditorRef();
    const { result } = renderHook(() => useCollabSession({ editorRef }));

    act(() => {
      result.current.connect("room-1", "user-1", "Alice");
    });

    expect(mockConnect).toHaveBeenCalled();

    // Simulate connected status.
    mockGetStatus.mockReturnValue("connected");
    act(() => {
      vi.advanceTimersByTime(100);
    });

    expect(mockSend).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "JoinRoom",
        room_id: "room-1",
        user_id: "user-1",
        display_name: "Alice",
      }),
    );
  });

  it("handles RoomJoined message", () => {
    const onRemoteChange = vi.fn();
    const { editorRef } = setupConnected({ onRemoteChange });

    act(() => {
      capturedOnMessage!({
        type: "RoomJoined",
        room_id: "room-1",
        session_id: "sess-1",
        participants: [
          { session_id: "sess-2", user_id: "user-2", display_name: "Bob" },
        ],
        document_json: "{}",
        latest_seq: 5,
      });
    });

    expect(editorRef.current!.import_document).toHaveBeenCalledWith("{}");
    expect(onRemoteChange).toHaveBeenCalled();
  });

  it("handles RemoteOp message", () => {
    const onRemoteChange = vi.fn();
    const { editorRef } = setupConnected({ onRemoteChange });

    act(() => {
      capturedOnMessage!({
        type: "RemoteOp",
        op: {
          seq: 1,
          client_seq: 1,
          user_id: "user-2",
          session_id: "sess-2",
          page_id: "page-1",
          descriptor: { type: "SetOpacity", node_id: "n1", opacity: 0.5 },
          timestamp: null,
        },
      });
    });

    expect(editorRef.current!.apply_remote_op).toHaveBeenCalledWith(
      "page-1",
      JSON.stringify({ type: "SetOpacity", node_id: "n1", opacity: 0.5 }),
    );
    expect(onRemoteChange).toHaveBeenCalled();
  });

  it("handles RemotePageOp message", () => {
    const onRemoteChange = vi.fn();
    const { editorRef } = setupConnected({ onRemoteChange });

    act(() => {
      capturedOnMessage!({
        type: "RemotePageOp",
        op_type: "add",
        page_id: "page-new",
        name: "New Page",
        width: 800,
        height: 600,
        server_seq: 3,
      });
    });

    expect(editorRef.current!.apply_remote_page_op).toHaveBeenCalledWith(
      "add",
      "page-new",
      "New Page",
      800,
      600,
    );
    expect(onRemoteChange).toHaveBeenCalled();
  });

  it("handles ParticipantJoined message", () => {
    const { result } = setupConnected();

    act(() => {
      capturedOnMessage!({
        type: "ParticipantJoined",
        participant: {
          session_id: "sess-2",
          user_id: "user-2",
          display_name: "Bob",
        },
      });
    });

    expect(result.current.participants).toEqual([
      { session_id: "sess-2", user_id: "user-2", display_name: "Bob" },
    ]);
  });

  it("handles ParticipantLeft message", () => {
    const { result } = setupConnected();

    act(() => {
      capturedOnMessage!({
        type: "ParticipantJoined",
        participant: {
          session_id: "sess-2",
          user_id: "user-2",
          display_name: "Bob",
        },
      });
    });
    expect(result.current.participants.length).toBe(1);

    act(() => {
      capturedOnMessage!({
        type: "ParticipantLeft",
        session_id: "sess-2",
      });
    });

    expect(result.current.participants).toEqual([]);
  });

  it("handles OpAck by clearing pending ops", () => {
    const { result } = setupConnected();

    // Submit an op to set hasPendingOps.
    act(() => {
      result.current.submitOp({ type: "SetOpacity" }, "page-1");
    });
    expect(result.current.hasPendingOps).toBe(true);

    act(() => {
      capturedOnMessage!({
        type: "OpAck",
        client_seq: 1,
        server_seq: 10,
      });
    });

    expect(result.current.hasPendingOps).toBe(false);
  });

  it("submitOp sends SubmitOp message", () => {
    const { result } = setupConnected();

    act(() => {
      result.current.submitOp(
        { type: "SetOpacity", node_id: "n1", opacity: 0.5 },
        "page-1",
      );
    });

    expect(mockSend).toHaveBeenCalledWith(
      expect.objectContaining({ type: "SubmitOp" }),
    );
  });

  it("submitPageOp sends SubmitPageOp message", () => {
    const { result } = setupConnected();

    act(() => {
      result.current.submitPageOp("add", "page-new", "Slide 2", 800, 600);
    });

    expect(mockSend).toHaveBeenCalledWith({
      type: "SubmitPageOp",
      op_type: "add",
      page_id: "page-new",
      name: "Slide 2",
      width: 800,
      height: 600,
    });
  });

  it("disconnect clears state", () => {
    const { result } = setupConnected();

    act(() => {
      capturedOnMessage!({
        type: "ParticipantJoined",
        participant: {
          session_id: "sess-2",
          user_id: "user-2",
          display_name: "Bob",
        },
      });
    });
    expect(result.current.participants.length).toBe(1);

    act(() => {
      result.current.disconnect();
    });

    expect(result.current.participants).toEqual([]);
    expect(result.current.remotePresences).toEqual([]);
    expect(result.current.hasPendingOps).toBe(false);
    expect(mockDisconnect).toHaveBeenCalled();
  });

  it("onReconnect re-sends JoinRoom", () => {
    setupConnected();
    expect(capturedOnReconnect).toBeDefined();

    // Simulate reconnection.
    act(() => {
      capturedOnReconnect!();
    });

    // Should send JoinRoom again.
    expect(mockSend).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "JoinRoom",
        room_id: "room-1",
        user_id: "user-1",
        display_name: "Alice",
      }),
    );
  });

  it("handles PresenceBroadcast message", () => {
    const { result } = setupConnected();

    act(() => {
      capturedOnMessage!({
        type: "PresenceBroadcast",
        session_id: "sess-2",
        user_id: "user-2",
        display_name: "Bob",
        page_id: "page-1",
        cursor: { x: 100, y: 200 },
        selected_node_ids: ["n1"],
      });
    });

    expect(result.current.remotePresences.length).toBe(1);
    expect(result.current.remotePresences[0]).toMatchObject({
      sessionId: "sess-2",
      displayName: "Bob",
      cursor: { x: 100, y: 200 },
    });
  });

  it("undo sends Undo message", () => {
    const { result } = setupConnected();

    act(() => {
      result.current.undo("page-1");
    });

    expect(mockSend).toHaveBeenCalledWith({
      type: "Undo",
      page_id: "page-1",
    });
  });

  it("redo sends Redo message", () => {
    const { result } = setupConnected();

    act(() => {
      result.current.redo("page-1");
    });

    expect(mockSend).toHaveBeenCalledWith({
      type: "Redo",
      page_id: "page-1",
    });
  });

  it("submitOpGroup sends SubmitOpGroup message", () => {
    const { result } = setupConnected();

    act(() => {
      result.current.submitOpGroup(
        [{ type: "SetBounds" }, { type: "SetBounds" }],
        "page-1",
        "Move",
      );
    });

    expect(mockSend).toHaveBeenCalledWith(
      expect.objectContaining({
        type: "SubmitOpGroup",
        group_label: "Move",
      }),
    );
  });

  it("updatePresence sends PresenceUpdate message", () => {
    const { result } = setupConnected();

    act(() => {
      result.current.updatePresence("page-1", { x: 50, y: 75 }, ["n1"]);
    });

    expect(mockSend).toHaveBeenCalledWith({
      type: "PresenceUpdate",
      page_id: "page-1",
      cursor: { x: 50, y: 75 },
      selected_node_ids: ["n1"],
    });
  });
});
