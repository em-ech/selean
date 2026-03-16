import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ChatPanel } from "./ChatPanel";
import { createMockEditorRef } from "../test/mock-editor";

// Mock useChatEngine to control chat state.
const mockSendMessage = vi.fn();
const mockAbort = vi.fn();
const mockSetInput = vi.fn();
let mockChatState = {
  displayMessages: [] as { role: "user" | "assistant"; text: string }[],
  input: "",
  setInput: mockSetInput,
  loading: false,
  streamingText: null as string | null,
  activeTools: [] as { name: string; status: "running" | "done" | "error" }[],
  sendMessage: mockSendMessage,
  abort: mockAbort,
  clearHistory: vi.fn(),
};

vi.mock("../hooks/useChatEngine", () => ({
  useChatEngine: () => mockChatState,
}));

// Mock CollabContext
vi.mock("../collab/CollabContext", () => ({
  CollabContext: React.createContext(null),
  useCollab: () => null,
}));

const editorRef = createMockEditorRef();
const onSceneChanged = vi.fn();
const onToggle = vi.fn();

beforeEach(() => {
  mockChatState = {
    displayMessages: [],
    input: "",
    setInput: mockSetInput,
    loading: false,
    streamingText: null,
    activeTools: [],
    sendMessage: mockSendMessage,
    abort: mockAbort,
    clearHistory: vi.fn(),
  };
  mockSendMessage.mockClear();
  mockAbort.mockClear();
  mockSetInput.mockClear();
  onToggle.mockClear();
});

describe("ChatPanel", () => {
  it("renders collapsed button when not open", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={false}
        onToggle={onToggle}
      />,
    );
    const btn = screen.getByTitle("Open chat (Cmd+J)");
    expect(btn).toBeInTheDocument();
  });

  it("calls onToggle when collapsed button clicked", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={false}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByTitle("Open chat (Cmd+J)"));
    expect(onToggle).toHaveBeenCalled();
  });

  it("renders empty state with suggestion chips when open", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Design with AI")).toBeInTheDocument();
    expect(screen.getByText("Add a title and subtitle")).toBeInTheDocument();
    expect(screen.getByText("Create a 3-column layout")).toBeInTheDocument();
  });

  it("auto-sends when suggestion chip clicked", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByText("Add a title and subtitle"));
    expect(mockSendMessage).toHaveBeenCalledWith("Add a title and subtitle");
  });

  it("renders user and assistant messages", () => {
    mockChatState.displayMessages = [
      { role: "user", text: "Make it blue" },
      { role: "assistant", text: "Done!" },
    ];
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Make it blue")).toBeInTheDocument();
    expect(screen.getByText("Done!")).toBeInTheDocument();
  });

  it("renders thinking indicator when loading with no stream", () => {
    mockChatState.loading = true;
    mockChatState.streamingText = null;
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Thinking...")).toBeInTheDocument();
  });

  it("renders streaming text", () => {
    mockChatState.loading = true;
    mockChatState.streamingText = "I am streaming...";
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("I am streaming...")).toBeInTheDocument();
  });

  it("renders tool execution badges", () => {
    mockChatState.activeTools = [
      { name: "set_fill", status: "running" },
      { name: "create_node", status: "done" },
    ];
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("set fill")).toBeInTheDocument();
    expect(screen.getByText("create node")).toBeInTheDocument();
  });

  it("shows stop button when loading", () => {
    mockChatState.loading = true;
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    const stopBtn = screen.getByTitle("Stop");
    expect(stopBtn).toBeInTheDocument();
    fireEvent.click(stopBtn);
    expect(mockAbort).toHaveBeenCalled();
  });

  it("shows send button when not loading", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByTitle("Send")).toBeInTheDocument();
  });

  it("calls onToggle when close button clicked", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    fireEvent.click(screen.getByTitle("Close chat"));
    expect(onToggle).toHaveBeenCalled();
  });

  it("renders input area with placeholder", () => {
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByPlaceholderText("Ask Selean...")).toBeInTheDocument();
  });

  it("disables input when loading", () => {
    mockChatState.loading = true;
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByPlaceholderText("Ask Selean...")).toBeDisabled();
  });

  it("does not render empty state when messages exist", () => {
    mockChatState.displayMessages = [{ role: "user", text: "Hi" }];
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.queryByText("Design with AI")).not.toBeInTheDocument();
  });

  it("displays error message on send failure", () => {
    mockChatState.displayMessages = [
      { role: "user", text: "do something" },
      { role: "assistant", text: "Error: Server error: 500" },
    ];
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    expect(screen.getByText("Error: Server error: 500")).toBeInTheDocument();
  });

  it("Enter key in input triggers send", () => {
    mockChatState.input = "hello";
    render(
      <ChatPanel
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isOpen={true}
        onToggle={onToggle}
      />,
    );
    const input = screen.getByPlaceholderText("Ask Selean...");
    fireEvent.keyDown(input, { key: "Enter", shiftKey: false });
    expect(mockSendMessage).toHaveBeenCalled();
  });
});
