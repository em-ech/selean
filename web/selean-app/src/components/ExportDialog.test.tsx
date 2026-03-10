import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ExportDialog } from "./ExportDialog";
import { createMockEditorRef } from "../test/mock-editor";

// Mock useGitHub hook
let mockGitHubState = {
  status: { connected: false, login: null as string | null },
  repos: [] as Array<{
    full_name: string;
    default_branch: string;
    private: boolean;
  }>,
  loading: false,
  connect: vi.fn(),
  disconnect: vi.fn(),
  refreshRepos: vi.fn(),
  createRepo: vi.fn(),
  push: vi.fn(),
};

vi.mock("../hooks/useGitHub", () => ({
  useGitHub: () => mockGitHubState,
}));

// Mock JSZip
vi.mock("jszip", () => {
  return {
    default: class MockJSZip {
      file = vi.fn();
      generateAsync = vi.fn().mockResolvedValue(new Blob(["zip"]));
    },
  };
});

// Mock URL.createObjectURL / revokeObjectURL
beforeEach(() => {
  vi.restoreAllMocks();
  URL.createObjectURL = vi.fn().mockReturnValue("blob:test");
  URL.revokeObjectURL = vi.fn();
  mockGitHubState = {
    status: { connected: false, login: null },
    repos: [],
    loading: false,
    connect: vi.fn(),
    disconnect: vi.fn(),
    refreshRepos: vi.fn(),
    createRepo: vi.fn(),
    push: vi.fn(),
  };
});

describe("ExportDialog", () => {
  it("renders three tabs", () => {
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);
    expect(screen.getByText("GitHub")).toBeInTheDocument();
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("Download ZIP")).toBeInTheDocument();
  });

  it("default tab is GitHub", () => {
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);
    // GitHub tab content should be visible (connect button since not connected)
    expect(screen.getByText("Connect GitHub")).toBeInTheDocument();
  });

  it("shows connect button when not connected", () => {
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);
    const btn = screen.getByText("Connect GitHub");
    expect(btn).toBeInTheDocument();
  });

  it("shows repo list when connected", () => {
    mockGitHubState.status = { connected: true, login: "testuser" };
    mockGitHubState.repos = [
      { full_name: "testuser/my-repo", default_branch: "main", private: false },
      {
        full_name: "testuser/private-repo",
        default_branch: "main",
        private: true,
      },
    ];
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);
    expect(screen.getByText("testuser")).toBeInTheDocument();
    expect(screen.getByTestId("repo-select")).toBeInTheDocument();
  });

  it("download zip triggers download", async () => {
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);

    fireEvent.click(screen.getByText("Download ZIP"));
    const downloadBtn = screen.getByTestId("zip-download-button");
    fireEvent.click(downloadBtn);

    await waitFor(() => {
      expect(URL.createObjectURL).toHaveBeenCalled();
    });
  });

  it("claude code tab has download button", () => {
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);

    fireEvent.click(screen.getByText("Claude Code"));
    expect(screen.getByTestId("claude-download-button")).toBeInTheDocument();
    expect(screen.getByTestId("claude-download-button")).toHaveTextContent(
      "Download ZIP",
    );
  });

  it("close button calls onClose", () => {
    const onClose = vi.fn();
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={onClose} />);

    fireEvent.click(screen.getByTestId("close-button"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("push button disabled without repo selection", () => {
    mockGitHubState.status = { connected: true, login: "testuser" };
    mockGitHubState.repos = [
      { full_name: "testuser/repo", default_branch: "main", private: false },
    ];
    const ref = createMockEditorRef();
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);

    const pushBtn = screen.getByTestId("push-button");
    expect(pushBtn).toBeDisabled();
  });

  it("handles null editor gracefully", () => {
    const ref = { current: null };
    render(<ExportDialog editorRef={ref} onClose={vi.fn()} />);
    // Should render without crashing
    expect(screen.getByText("GitHub")).toBeInTheDocument();
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("Download ZIP")).toBeInTheDocument();
  });
});
