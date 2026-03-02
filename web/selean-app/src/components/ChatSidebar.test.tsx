import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ChatSidebar, consolidateTextBlocks } from "./ChatSidebar";
import { createMockEditorRef } from "../test/mock-editor";

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
