import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, afterEach } from "vitest";
import { ChatSidebar, consolidateTextBlocks } from "./ChatSidebar";
import { createMockEditorRef } from "../test/mock-editor";

/**
 * Creates a mock fetch Response whose body reader yields the given SSE lines.
 * Each string in `chunks` is encoded and delivered as a separate read() call.
 * The final read() returns done: true.
 */
function mockSSEResponse(chunks: string[]) {
  let readIndex = 0;
  return {
    ok: true,
    body: {
      getReader: () => ({
        read: () => {
          if (readIndex < chunks.length) {
            const value = new TextEncoder().encode(chunks[readIndex]);
            readIndex++;
            return Promise.resolve({ done: false, value });
          }
          return Promise.resolve({ done: true, value: undefined });
        },
      }),
    },
  };
}

/** Builds a single SSE data line from a ChatEvent-shaped object. */
function sseData(obj: Record<string, unknown>): string {
  return `data: ${JSON.stringify(obj)}\n`;
}

/** Helper: type text into the input and click Send. */
function sendUserMessage(text: string) {
  const input = screen.getByPlaceholderText("Describe a change...");
  fireEvent.change(input, { target: { value: text } });
  fireEvent.click(screen.getByText("Send"));
}

describe("ChatSidebar", () => {
  it("renders header", () => {
    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);
    expect(screen.getByText("Chat")).toBeInTheDocument();
  });

  it("shows empty state prompt when no messages", () => {
    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);
    expect(screen.getByText(/Ask me to modify/)).toBeInTheDocument();
  });

  it("renders input field and send button", () => {
    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    expect(input).toBeInTheDocument();
    expect(screen.getByText("Send")).toBeInTheDocument();
  });

  it("send button is disabled when input is empty", () => {
    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const sendBtn = screen.getByText("Send");
    expect(sendBtn).toBeDisabled();
  });

  it("send button is enabled when input has text", () => {
    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Hello" } });

    const sendBtn = screen.getByText("Send");
    expect(sendBtn).not.toBeDisabled();
  });

  it("clears input after send button is clicked", async () => {
    // Mock fetch to return a simple SSE response
    const sseResponse =
      'data: {"type":"text","text":"OK"}\ndata: {"type":"done","stop_reason":"end_turn"}\n';
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        body: {
          getReader: () => {
            let done = false;
            return {
              read: () => {
                if (!done) {
                  done = true;
                  return Promise.resolve({
                    done: false,
                    value: new TextEncoder().encode(sseResponse),
                  });
                }
                return Promise.resolve({ done: true, value: undefined });
              },
            };
          },
        },
      }),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Make it blue" } });
    fireEvent.click(screen.getByText("Send"));

    // Input should be cleared immediately
    expect(input).toHaveValue("");

    vi.unstubAllGlobals();
  });

  it("displays user message in the list after sending", () => {
    // Mock fetch that never resolves (we're only testing the immediate UI update)
    vi.stubGlobal("fetch", vi.fn().mockReturnValue(new Promise(() => {})));

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Test message" } });
    fireEvent.click(screen.getByText("Send"));

    expect(screen.getByText("Test message")).toBeInTheDocument();
    expect(screen.getByText("You")).toBeInTheDocument();

    vi.unstubAllGlobals();
  });

  it("shows Thinking... while loading", () => {
    vi.stubGlobal("fetch", vi.fn().mockReturnValue(new Promise(() => {})));

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Test" } });
    fireEvent.click(screen.getByText("Send"));

    expect(screen.getByText("Thinking...")).toBeInTheDocument();

    vi.unstubAllGlobals();
  });

  it("disables input while loading", () => {
    vi.stubGlobal("fetch", vi.fn().mockReturnValue(new Promise(() => {})));

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Test" } });
    fireEvent.click(screen.getByText("Send"));

    expect(input).toBeDisabled();

    vi.unstubAllGlobals();
  });

  it("streaming text replaces Thinking... indicator", async () => {
    // Simulate a stream that delivers text in two chunks
    const chunk1 = 'data: {"type":"text","text":"Hello"}\n';
    const chunk2 =
      'data: {"type":"text","text":" world"}\ndata: {"type":"done","stop_reason":"end_turn"}\n';

    let readCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        body: {
          getReader: () => ({
            read: () => {
              readCount++;
              if (readCount === 1) {
                return Promise.resolve({
                  done: false,
                  value: new TextEncoder().encode(chunk1),
                });
              }
              if (readCount === 2) {
                return Promise.resolve({
                  done: false,
                  value: new TextEncoder().encode(chunk2),
                });
              }
              return Promise.resolve({ done: true, value: undefined });
            },
          }),
        },
      }),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Hi" } });
    fireEvent.click(screen.getByText("Send"));

    // After streaming completes, the full text should appear as a display message
    await waitFor(() => {
      expect(screen.getByText("Hello world")).toBeInTheDocument();
    });
    // "Thinking..." should no longer be visible
    expect(screen.queryByText("Thinking...")).not.toBeInTheDocument();

    vi.unstubAllGlobals();
  });

  it("accumulates text across multiple reader.read() calls", async () => {
    const chunks = ["Hel", "lo ", "wor", "ld"];
    let readIndex = 0;

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        body: {
          getReader: () => ({
            read: () => {
              if (readIndex < chunks.length) {
                const text = chunks[readIndex];
                readIndex++;
                const sse = `data: {"type":"text","text":"${text}"}\n`;
                return Promise.resolve({
                  done: false,
                  value: new TextEncoder().encode(sse),
                });
              }
              if (readIndex === chunks.length) {
                readIndex++;
                const sse = 'data: {"type":"done","stop_reason":"end_turn"}\n';
                return Promise.resolve({
                  done: false,
                  value: new TextEncoder().encode(sse),
                });
              }
              return Promise.resolve({ done: true, value: undefined });
            },
          }),
        },
      }),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Test" } });
    fireEvent.click(screen.getByText("Send"));

    await waitFor(() => {
      expect(screen.getByText("Hello world")).toBeInTheDocument();
    });

    vi.unstubAllGlobals();
  });

  it("shows error message when streaming fails", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: false,
        status: 500,
      }),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Fail please" } });
    fireEvent.click(screen.getByText("Send"));

    await waitFor(() => {
      expect(screen.getByText(/Error: Server error: 500/)).toBeInTheDocument();
    });

    vi.unstubAllGlobals();
  });
});

describe("ChatSidebar tool loop integration", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("calls execute_tool_call when stop_reason is tool_use", async () => {
    const toolUseChunk =
      sseData({ type: "text", text: "Let me change the fill." }) +
      sseData({
        type: "tool_use",
        id: "call_1",
        name: "set_fill",
        input: { node_id: "n1", r: 0, g: 0, b: 1, a: 1 },
      }) +
      sseData({ type: "done", stop_reason: "tool_use" });

    const finalChunk =
      sseData({ type: "text", text: "Done!" }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    let fetchCallCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation(() => {
        fetchCallCount++;
        if (fetchCallCount === 1) {
          return Promise.resolve(mockSSEResponse([toolUseChunk]));
        }
        return Promise.resolve(mockSSEResponse([finalChunk]));
      }),
    );

    const ref = createMockEditorRef();
    const onSceneChanged = vi.fn();
    render(<ChatSidebar editorRef={ref} onSceneChanged={onSceneChanged} />);

    sendUserMessage("Make it blue");

    await waitFor(() => {
      expect(ref.current!.execute_tool_call).toHaveBeenCalledWith(
        "set_fill",
        JSON.stringify({ node_id: "n1", r: 0, g: 0, b: 1, a: 1 }),
      );
    });

    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("sends tool_result back in a follow-up request", async () => {
    const toolUseChunk =
      sseData({ type: "text", text: "Changing name." }) +
      sseData({
        type: "tool_use",
        id: "call_abc",
        name: "set_name",
        input: { node_id: "n1", name: "Header" },
      }) +
      sseData({ type: "done", stop_reason: "tool_use" });

    const finalChunk =
      sseData({ type: "text", text: "Renamed." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    const fetchMock = vi.fn();
    let fetchCallCount = 0;
    fetchMock.mockImplementation(() => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        return Promise.resolve(mockSSEResponse([toolUseChunk]));
      }
      return Promise.resolve(mockSSEResponse([finalChunk]));
    });
    vi.stubGlobal("fetch", fetchMock);

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Rename it");

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });

    // The second fetch call should include tool_result in messages
    const secondCallBody = JSON.parse(fetchMock.mock.calls[1][1].body);
    const lastMsg = secondCallBody.messages[secondCallBody.messages.length - 1];
    expect(lastMsg.role).toBe("user");
    expect(Array.isArray(lastMsg.content)).toBe(true);

    const toolResult = lastMsg.content.find(
      (b: Record<string, unknown>) => b.type === "tool_result",
    );
    expect(toolResult).toBeDefined();
    expect(toolResult.tool_use_id).toBe("call_abc");
  });

  it("executes multiple tools in a single iteration", async () => {
    const toolUseChunk =
      sseData({
        type: "tool_use",
        id: "c1",
        name: "set_fill",
        input: { node_id: "n1", r: 1, g: 0, b: 0, a: 1 },
      }) +
      sseData({
        type: "tool_use",
        id: "c2",
        name: "set_name",
        input: { node_id: "n1", name: "Red Box" },
      }) +
      sseData({ type: "done", stop_reason: "tool_use" });

    const finalChunk =
      sseData({ type: "text", text: "Applied both." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    let fetchCallCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation(() => {
        fetchCallCount++;
        if (fetchCallCount === 1) {
          return Promise.resolve(mockSSEResponse([toolUseChunk]));
        }
        return Promise.resolve(mockSSEResponse([finalChunk]));
      }),
    );

    const ref = createMockEditorRef();
    const onSceneChanged = vi.fn();
    render(<ChatSidebar editorRef={ref} onSceneChanged={onSceneChanged} />);

    sendUserMessage("Make it red and rename");

    await waitFor(() => {
      expect(ref.current!.execute_tool_call).toHaveBeenCalledTimes(2);
    });

    expect(ref.current!.execute_tool_call).toHaveBeenCalledWith(
      "set_fill",
      JSON.stringify({ node_id: "n1", r: 1, g: 0, b: 0, a: 1 }),
    );
    expect(ref.current!.execute_tool_call).toHaveBeenCalledWith(
      "set_name",
      JSON.stringify({ node_id: "n1", name: "Red Box" }),
    );
    // onSceneChanged called once per tool
    expect(onSceneChanged).toHaveBeenCalledTimes(2);
  });

  it("loops multiple iterations: tool_use -> result -> tool_use -> result -> end", async () => {
    const iteration1 =
      sseData({ type: "text", text: "Step 1." }) +
      sseData({
        type: "tool_use",
        id: "c1",
        name: "set_fill",
        input: { node_id: "n1", r: 1, g: 0, b: 0, a: 1 },
      }) +
      sseData({ type: "done", stop_reason: "tool_use" });

    const iteration2 =
      sseData({ type: "text", text: "Step 2." }) +
      sseData({
        type: "tool_use",
        id: "c2",
        name: "set_opacity",
        input: { node_id: "n1", opacity: 0.5 },
      }) +
      sseData({ type: "done", stop_reason: "tool_use" });

    const iteration3 =
      sseData({ type: "text", text: "All done!" }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    let fetchCallCount = 0;
    vi.stubGlobal(
      "fetch",
      vi.fn().mockImplementation(() => {
        fetchCallCount++;
        if (fetchCallCount === 1) {
          return Promise.resolve(mockSSEResponse([iteration1]));
        }
        if (fetchCallCount === 2) {
          return Promise.resolve(mockSSEResponse([iteration2]));
        }
        return Promise.resolve(mockSSEResponse([iteration3]));
      }),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Do two things");

    await waitFor(() => {
      expect(fetchCallCount).toBe(3);
    });

    // Both tools should have been called
    expect(ref.current!.execute_tool_call).toHaveBeenCalledWith(
      "set_fill",
      JSON.stringify({ node_id: "n1", r: 1, g: 0, b: 0, a: 1 }),
    );
    expect(ref.current!.execute_tool_call).toHaveBeenCalledWith(
      "set_opacity",
      JSON.stringify({ node_id: "n1", opacity: 0.5 }),
    );

    // Final text should be displayed
    await waitFor(() => {
      expect(
        screen.getByText(/All done!/, { exact: false }),
      ).toBeInTheDocument();
    });
  });

  it("stops looping after MAX_TOOL_ITERATIONS (10)", async () => {
    // Every response is a tool_use, forcing the loop to hit the guard
    const alwaysToolUse =
      sseData({
        type: "tool_use",
        id: "c",
        name: "get_scene_summary",
        input: {},
      }) + sseData({ type: "done", stop_reason: "tool_use" });

    const fetchMock = vi.fn().mockImplementation(() => {
      return Promise.resolve(mockSSEResponse([alwaysToolUse]));
    });
    vi.stubGlobal("fetch", fetchMock);

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Loop forever");

    // Wait for the loop to exhaust. 10 iterations = 10 fetch calls.
    await waitFor(
      () => {
        expect(fetchMock).toHaveBeenCalledTimes(10);
      },
      { timeout: 5000 },
    );

    // Should NOT make an 11th call
    // Give a brief window to confirm no additional calls are queued
    await new Promise((r) => setTimeout(r, 50));
    expect(fetchMock).toHaveBeenCalledTimes(10);
  });

  it("displays error when fetch rejects (network failure)", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockRejectedValue(new TypeError("Failed to fetch")),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Crash test");

    await waitFor(() => {
      expect(screen.getByText(/Error: Failed to fetch/)).toBeInTheDocument();
    });
  });

  it("displays error when response body is missing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        body: null,
      }),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("No body");

    await waitFor(() => {
      expect(screen.getByText(/Error: No response body/)).toBeInTheDocument();
    });
  });

  it("sends error content in tool_result when execute_tool_call throws", async () => {
    const toolUseChunk =
      sseData({
        type: "tool_use",
        id: "call_err",
        name: "set_fill",
        input: { node_id: "bad", r: 0, g: 0, b: 0, a: 1 },
      }) + sseData({ type: "done", stop_reason: "tool_use" });

    const finalChunk =
      sseData({ type: "text", text: "Noted the error." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    const fetchMock = vi.fn();
    let fetchCallCount = 0;
    fetchMock.mockImplementation(() => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        return Promise.resolve(mockSSEResponse([toolUseChunk]));
      }
      return Promise.resolve(mockSSEResponse([finalChunk]));
    });
    vi.stubGlobal("fetch", fetchMock);

    // execute_tool_call returns non-JSON string (error case)
    const ref = createMockEditorRef({
      execute_tool_call: vi.fn().mockReturnValue("ERROR: node not found"),
    });
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Try bad node");

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });

    // The tool result should contain the raw error string since JSON.parse fails
    const secondCallBody = JSON.parse(fetchMock.mock.calls[1][1].body);
    const lastMsg = secondCallBody.messages[secondCallBody.messages.length - 1];
    const toolResult = lastMsg.content.find(
      (b: Record<string, unknown>) => b.type === "tool_result",
    );
    expect(toolResult).toBeDefined();
    expect(toolResult.tool_use_id).toBe("call_err");
    expect(toolResult.content).toBe("ERROR: node not found");
  });

  it("refreshes scene_summary between loop iterations", async () => {
    const toolUseChunk =
      sseData({
        type: "tool_use",
        id: "c1",
        name: "set_fill",
        input: { node_id: "n1", r: 1, g: 0, b: 0, a: 1 },
      }) + sseData({ type: "done", stop_reason: "tool_use" });

    const finalChunk =
      sseData({ type: "text", text: "Done." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    const fetchMock = vi.fn();
    let fetchCallCount = 0;
    fetchMock.mockImplementation(() => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        return Promise.resolve(mockSSEResponse([toolUseChunk]));
      }
      return Promise.resolve(mockSSEResponse([finalChunk]));
    });
    vi.stubGlobal("fetch", fetchMock);

    // get_scene_json returns different values on successive calls
    const getSceneJsonMock = vi
      .fn()
      .mockReturnValueOnce('{"nodes":[],"roots":[],"node_count":0}')
      .mockReturnValueOnce(
        '{"nodes":[{"id":"n1"}],"roots":["n1"],"node_count":1}',
      );

    const ref = createMockEditorRef({
      get_scene_json: getSceneJsonMock,
    });
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Update scene");

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });

    // get_scene_json should be called at least twice:
    // once before the first request, once after tool execution
    expect(getSceneJsonMock).toHaveBeenCalledTimes(2);

    // The second fetch should receive the updated scene summary
    const secondCallBody = JSON.parse(fetchMock.mock.calls[1][1].body);
    expect(secondCallBody.scene_summary).toBe(
      '{"nodes":[{"id":"n1"}],"roots":["n1"],"node_count":1}',
    );
  });

  it("re-enables input and send button after tool loop completes", async () => {
    const sseChunk =
      sseData({ type: "text", text: "Response." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockSSEResponse([sseChunk])),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Quick test");

    const input = screen.getByPlaceholderText("Describe a change...");

    // While loading, input is disabled
    expect(input).toBeDisabled();

    // After completion, input should be re-enabled
    await waitFor(() => {
      expect(input).not.toBeDisabled();
    });
  });

  it("sends Enter key to submit message (not just click)", async () => {
    const sseChunk =
      sseData({ type: "text", text: "Hi." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(mockSSEResponse([sseChunk])),
    );

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    const input = screen.getByPlaceholderText("Describe a change...");
    fireEvent.change(input, { target: { value: "Enter test" } });
    fireEvent.keyDown(input, { key: "Enter" });

    // User message should appear
    expect(screen.getByText("Enter test")).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText("Hi.")).toBeInTheDocument();
    });
  });

  it("includes assistant tool_use blocks in conversation history for follow-up", async () => {
    const toolUseChunk =
      sseData({ type: "text", text: "Applying change." }) +
      sseData({
        type: "tool_use",
        id: "tc_1",
        name: "set_opacity",
        input: { node_id: "n1", opacity: 0.8 },
      }) +
      sseData({ type: "done", stop_reason: "tool_use" });

    const finalChunk =
      sseData({ type: "text", text: "Complete." }) +
      sseData({ type: "done", stop_reason: "end_turn" });

    const fetchMock = vi.fn();
    let fetchCallCount = 0;
    fetchMock.mockImplementation(() => {
      fetchCallCount++;
      if (fetchCallCount === 1) {
        return Promise.resolve(mockSSEResponse([toolUseChunk]));
      }
      return Promise.resolve(mockSSEResponse([finalChunk]));
    });
    vi.stubGlobal("fetch", fetchMock);

    const ref = createMockEditorRef();
    render(<ChatSidebar editorRef={ref} onSceneChanged={() => {}} />);

    sendUserMessage("Change opacity");

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledTimes(2);
    });

    // Verify the second request includes the full conversation:
    // [user msg, assistant (text + tool_use), user (tool_result)]
    const secondCallBody = JSON.parse(fetchMock.mock.calls[1][1].body);
    const messages = secondCallBody.messages;

    expect(messages).toHaveLength(3);
    expect(messages[0].role).toBe("user");
    expect(messages[0].content).toBe("Change opacity");

    // Assistant message should have consolidated text + tool_use blocks
    expect(messages[1].role).toBe("assistant");
    const assistantContent = messages[1].content as Record<string, unknown>[];
    const toolUseBlock = assistantContent.find((b) => b.type === "tool_use");
    expect(toolUseBlock).toBeDefined();
    expect((toolUseBlock as Record<string, unknown>).name).toBe("set_opacity");

    // Tool result message
    expect(messages[2].role).toBe("user");
  });
});

describe("consolidateTextBlocks", () => {
  it("merges consecutive text blocks into one", () => {
    const blocks = [
      { type: "text", text: "Hello" },
      { type: "text", text: " " },
      { type: "text", text: "world" },
    ];
    const result = consolidateTextBlocks(blocks) as Array<
      Record<string, unknown>
    >;
    expect(result).toHaveLength(1);
    expect(result[0].type).toBe("text");
    expect(result[0].text).toBe("Hello world");
  });

  it("preserves non-text blocks between text blocks", () => {
    const blocks = [
      { type: "text", text: "A" },
      { type: "tool_use", id: "t1", name: "set_fill", input: {} },
      { type: "text", text: "B" },
    ];
    const result = consolidateTextBlocks(blocks) as Array<
      Record<string, unknown>
    >;
    expect(result).toHaveLength(3);
    expect(result[0].text).toBe("A");
    expect(result[1].type).toBe("tool_use");
    expect(result[2].text).toBe("B");
  });

  it("returns empty array for empty input", () => {
    expect(consolidateTextBlocks([])).toHaveLength(0);
  });
});
