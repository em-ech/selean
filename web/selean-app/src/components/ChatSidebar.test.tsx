import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ChatSidebar } from "./ChatSidebar";
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
});
