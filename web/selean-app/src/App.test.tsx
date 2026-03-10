import React from "react";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, type Mock } from "vitest";
import { App } from "./App";
import { createMockEditor } from "./test/mock-editor";
import type { SeleanEditor } from "./wasm/types";

// Mock useAuth to bypass authentication.
vi.mock("./auth/AuthContext", () => ({
  useAuth: () => ({
    user: {
      id: "test-user",
      email: "test@test.com",
      display_name: "Test User",
      avatar_url: null,
    },
    isLoading: false,
    isAuthenticated: true,
    login: vi.fn(),
    signup: vi.fn(),
    logout: vi.fn(),
  }),
}));

// Mock useSeleanEditor to inject a controlled editor ref.
let mockEditor: SeleanEditor;
let mockStatus: "loading" | "ready" | "error" | "unsupported";
let mockError: string | null;

vi.mock("./hooks/useSeleanEditor", () => ({
  useSeleanEditor: () => {
    const ref = { current: mockStatus === "ready" ? mockEditor : null };
    return { editorRef: ref, status: mockStatus, error: mockError };
  },
}));

// Mock useCollabSession to avoid WebSocket setup.
vi.mock("./hooks/useCollabSession", () => ({
  useCollabSession: () => ({
    status: "disconnected" as const,
    participants: [],
    remotePresences: [],
    hasPendingOps: false,
    connect: vi.fn(),
    disconnect: vi.fn(),
    submitOp: vi.fn(),
    submitOpGroup: vi.fn(),
    submitPageOp: vi.fn(),
    undo: vi.fn(),
    redo: vi.fn(),
    updatePresence: vi.fn(),
  }),
}));

// Mock useAutoSave to avoid localStorage interaction.
vi.mock("./hooks/useAutoSave", () => ({
  useAutoSave: () => ({
    markDirty: vi.fn(),
    loadSavedDocument: vi.fn().mockReturnValue(null),
    clearSavedDocument: vi.fn(),
  }),
}));

// Mock useWorkspace (used by AppContent inside WorkspaceProvider).
vi.mock("./hooks/useWorkspace", () => ({
  WorkspaceProvider: ({ children }: { children: React.ReactNode }) => children,
  useWorkspace: () => ({
    workspaces: [],
    activeWorkspace: null,
    members: [],
    userRole: null,
    isLoading: false,
    switchWorkspace: vi.fn(),
    createWorkspace: vi.fn(),
    renameWorkspace: vi.fn(),
    deleteWorkspace: vi.fn(),
    inviteMember: vi.fn(),
    updateMemberRole: vi.fn(),
    removeMember: vi.fn(),
    refreshWorkspaces: vi.fn(),
  }),
}));

beforeEach(() => {
  mockEditor = createMockEditor();
  mockStatus = "ready";
  mockError = null;
});

describe("App", () => {
  // --- Rendering states ---

  it("renders loading state", () => {
    mockStatus = "loading";
    render(<App />);
    expect(screen.getByText("Initializing...")).toBeInTheDocument();
  });

  it("renders error state with message", () => {
    mockStatus = "error";
    mockError = "WASM init failed";
    render(<App />);
    expect(screen.getByText("Error: WASM init failed")).toBeInTheDocument();
  });

  it("renders unsupported state", () => {
    mockStatus = "unsupported";
    render(<App />);
    expect(screen.getByText("WebGPU not supported")).toBeInTheDocument();
  });

  it("renders ready state with header and status", () => {
    render(<App />);
    expect(screen.getByText("Selean")).toBeInTheDocument();
    expect(screen.getByText("Ready")).toBeInTheDocument();
  });

  // --- Undo/redo buttons ---

  it("renders undo and redo buttons when ready", () => {
    render(<App />);
    expect(screen.getByText("Undo")).toBeInTheDocument();
    expect(screen.getByText("Redo")).toBeInTheDocument();
  });

  it("disables undo button when can_undo returns false", () => {
    (mockEditor.can_undo as Mock).mockReturnValue(false);
    (mockEditor.can_redo as Mock).mockReturnValue(false);
    render(<App />);
    expect(screen.getByText("Undo")).toBeDisabled();
    expect(screen.getByText("Redo")).toBeDisabled();
  });

  it("enables undo button when can_undo returns true", () => {
    (mockEditor.can_undo as Mock).mockReturnValue(true);
    render(<App />);
    expect(screen.getByText("Undo")).not.toBeDisabled();
  });

  it("calls editor.undo when undo button clicked", () => {
    (mockEditor.can_undo as Mock).mockReturnValue(true);
    render(<App />);
    fireEvent.click(screen.getByText("Undo"));
    expect(mockEditor.undo).toHaveBeenCalled();
  });

  it("calls editor.redo when redo button clicked", () => {
    (mockEditor.can_redo as Mock).mockReturnValue(true);
    render(<App />);
    fireEvent.click(screen.getByText("Redo"));
    expect(mockEditor.redo).toHaveBeenCalled();
  });

  // --- Loading state hides interactive elements ---

  it("hides toolbar and panels when loading", () => {
    mockStatus = "loading";
    render(<App />);
    expect(screen.queryByText("Undo")).not.toBeInTheDocument();
    expect(screen.queryByText("Redo")).not.toBeInTheDocument();
  });

  // --- Context menu ---

  it("does not render context menu by default", () => {
    render(<App />);
    expect(screen.queryByTestId("context-menu")).not.toBeInTheDocument();
  });

  // --- Hidden file input for image upload ---

  it("renders hidden file input for image upload", () => {
    render(<App />);
    const input = document.querySelector(
      'input[type="file"][accept="image/png,image/jpeg,image/webp"]',
    );
    expect(input).not.toBeNull();
    expect((input as HTMLInputElement).style.display).toBe("none");
  });

  // --- active_page_id derivation ---

  it("calls active_page_id when ready", () => {
    render(<App />);
    expect(mockEditor.active_page_id).toHaveBeenCalled();
  });

  // --- Error state does not show panels ---

  it("does not render panels in error state", () => {
    mockStatus = "error";
    mockError = "fail";
    render(<App />);
    expect(screen.queryByText("Undo")).not.toBeInTheDocument();
  });
});
