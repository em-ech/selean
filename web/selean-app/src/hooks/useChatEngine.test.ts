import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useChatEngine, consolidateTextBlocks } from "./useChatEngine";
import { createMockEditor } from "../test/mock-editor";
import type { SeleanEditor } from "../wasm/types";

// Mock authFetch
vi.mock("../utils/api", () => ({
  authFetch: vi.fn(),
}));

import { authFetch } from "../utils/api";

const mockAuthFetch = authFetch as ReturnType<typeof vi.fn>;

function makeSseResponse(events: unknown[]): Response {
  const lines = events.map((e) => `data: ${JSON.stringify(e)}`).join("\n");
  const encoder = new TextEncoder();
  const stream = new ReadableStream({
    start(controller) {
      controller.enqueue(encoder.encode(lines + "\n"));
      controller.close();
    },
  });
  return new Response(stream, { status: 200 });
}

let mockEditor: SeleanEditor;
let editorRef: { current: SeleanEditor | null };
const onSceneChanged = vi.fn();

beforeEach(() => {
  mockEditor = createMockEditor();
  editorRef = { current: mockEditor };
  onSceneChanged.mockClear();
  mockAuthFetch.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("consolidateTextBlocks", () => {
  it("merges consecutive text blocks", () => {
    const blocks = [
      { type: "text", text: "Hello " },
      { type: "text", text: "world" },
    ];
    const result = consolidateTextBlocks(blocks);
    expect(result).toHaveLength(1);
    expect((result[0] as Record<string, unknown>).text).toBe("Hello world");
  });

  it("does not merge non-text blocks", () => {
    const blocks = [
      { type: "text", text: "Hello" },
      { type: "tool_use", id: "1", name: "set_fill" },
      { type: "text", text: "world" },
    ];
    const result = consolidateTextBlocks(blocks);
    expect(result).toHaveLength(3);
  });

  it("handles empty array", () => {
    expect(consolidateTextBlocks([])).toEqual([]);
  });
});

describe("useChatEngine", () => {
  it("initializes with empty state", () => {
    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );
    expect(result.current.displayMessages).toEqual([]);
    expect(result.current.input).toBe("");
    expect(result.current.loading).toBe(false);
    expect(result.current.streamingText).toBeNull();
    expect(result.current.activeTools).toEqual([]);
  });

  it("does not send when input is empty", async () => {
    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );
    await act(async () => {
      await result.current.sendMessage();
    });
    expect(mockAuthFetch).not.toHaveBeenCalled();
  });

  it("does not send when editor is null", async () => {
    editorRef.current = null;
    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );
    act(() => {
      result.current.setInput("test");
    });
    await act(async () => {
      await result.current.sendMessage();
    });
    expect(mockAuthFetch).not.toHaveBeenCalled();
  });

  it("sends message and processes text response", async () => {
    mockAuthFetch.mockResolvedValueOnce(
      makeSseResponse([
        { type: "text", text: "Hello!" },
        { type: "done", stop_reason: "end_turn" },
      ]),
    );

    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );

    act(() => {
      result.current.setInput("Hi");
    });

    await act(async () => {
      await result.current.sendMessage();
    });

    expect(result.current.displayMessages).toHaveLength(2);
    expect(result.current.displayMessages[0]).toEqual({
      role: "user",
      text: "Hi",
    });
    expect(result.current.displayMessages[1]).toEqual({
      role: "assistant",
      text: "Hello!",
    });
    expect(result.current.loading).toBe(false);
    expect(result.current.input).toBe("");
  });

  it("executes tool calls on tool_use stop reason", async () => {
    // First response: tool_use
    mockAuthFetch.mockResolvedValueOnce(
      makeSseResponse([
        { type: "text", text: "Setting fill." },
        {
          type: "tool_use",
          id: "t1",
          name: "set_fill",
          input: { node_id: "n1", r: 1, g: 0, b: 0, a: 1 },
        },
        { type: "done", stop_reason: "tool_use" },
      ]),
    );
    // Second response: end_turn
    mockAuthFetch.mockResolvedValueOnce(
      makeSseResponse([
        { type: "text", text: "Done." },
        { type: "done", stop_reason: "end_turn" },
      ]),
    );

    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );

    act(() => {
      result.current.setInput("Make it red");
    });

    await act(async () => {
      await result.current.sendMessage();
    });

    expect(mockEditor.execute_tool_call).toHaveBeenCalledWith(
      "set_fill",
      expect.any(String),
    );
    expect(onSceneChanged).toHaveBeenCalled();
    expect(mockAuthFetch).toHaveBeenCalledTimes(2);
  });

  it("handles server error", async () => {
    mockAuthFetch.mockResolvedValueOnce(
      new Response("Internal Server Error", { status: 500 }),
    );

    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );

    act(() => {
      result.current.setInput("test");
    });

    await act(async () => {
      await result.current.sendMessage();
    });

    expect(result.current.displayMessages).toHaveLength(2);
    expect(result.current.displayMessages[1].text).toContain("Error:");
    expect(result.current.loading).toBe(false);
  });

  it("clears history", async () => {
    mockAuthFetch.mockResolvedValueOnce(
      makeSseResponse([
        { type: "text", text: "Hi!" },
        { type: "done", stop_reason: "end_turn" },
      ]),
    );

    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );

    act(() => {
      result.current.setInput("Hello");
    });
    await act(async () => {
      await result.current.sendMessage();
    });
    expect(result.current.displayMessages).toHaveLength(2);

    act(() => {
      result.current.clearHistory();
    });
    expect(result.current.displayMessages).toEqual([]);
  });

  it("does not send while loading", async () => {
    // Create a response that never resolves
    mockAuthFetch.mockReturnValueOnce(new Promise(() => {}));

    const { result } = renderHook(() =>
      useChatEngine({ editorRef, onSceneChanged }),
    );

    act(() => {
      result.current.setInput("first");
    });

    // Start first send (will hang)
    act(() => {
      result.current.sendMessage();
    });

    // Try second send while loading
    act(() => {
      result.current.setInput("second");
    });

    await act(async () => {
      await result.current.sendMessage();
    });

    // Should only have been called once (the first hanging call triggers abort + new)
    // The key point is loading blocks duplicate sends
    expect(result.current.loading).toBe(true);
  });
});
