import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { WorkspaceSelector } from "./WorkspaceSelector";
import type { WorkspaceState } from "../hooks/useWorkspace";

const mockSwitchWorkspace = vi.fn().mockResolvedValue(undefined);
const mockCreateWorkspace = vi
  .fn()
  .mockResolvedValue({ id: "ws-new", name: "New" });

const mockWorkspaceState: WorkspaceState = {
  workspaces: [
    {
      id: "ws-1",
      name: "My Workspace",
      owner_id: "user-1",
      billing_tier: "free",
      created_at: "2026-01-01",
      updated_at: "2026-01-01",
    },
    {
      id: "ws-2",
      name: "Team Workspace",
      owner_id: "user-2",
      billing_tier: "pro",
      created_at: "2026-01-02",
      updated_at: "2026-01-02",
    },
  ],
  activeWorkspace: {
    id: "ws-1",
    name: "My Workspace",
    owner_id: "user-1",
    billing_tier: "free",
    created_at: "2026-01-01",
    updated_at: "2026-01-01",
  },
  members: [],
  userRole: "owner",
  isLoading: false,
  switchWorkspace: mockSwitchWorkspace,
  createWorkspace: mockCreateWorkspace,
  renameWorkspace: vi.fn(),
  deleteWorkspace: vi.fn(),
  inviteMember: vi.fn(),
  updateMemberRole: vi.fn(),
  removeMember: vi.fn(),
  refreshWorkspaces: vi.fn(),
};

vi.mock("../hooks/useWorkspace", () => ({
  useWorkspace: () => mockWorkspaceState,
}));

describe("WorkspaceSelector", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    mockSwitchWorkspace.mockResolvedValue(undefined);
    mockCreateWorkspace.mockResolvedValue({ id: "ws-new", name: "New" });
  });

  it("renders current workspace name", () => {
    render(<WorkspaceSelector />);
    expect(screen.getByText("My Workspace")).toBeInTheDocument();
  });

  it("shows dropdown on click", () => {
    render(<WorkspaceSelector />);
    fireEvent.click(screen.getByLabelText("Switch workspace"));
    expect(screen.getByRole("listbox")).toBeInTheDocument();
  });

  it("lists all workspaces in dropdown", () => {
    render(<WorkspaceSelector />);
    fireEvent.click(screen.getByLabelText("Switch workspace"));
    // "My Workspace" appears in both trigger and dropdown.
    expect(screen.getAllByText("My Workspace")).toHaveLength(2);
    expect(screen.getByText("Team Workspace")).toBeInTheDocument();
  });

  it("calls switchWorkspace on selection", async () => {
    render(<WorkspaceSelector />);
    fireEvent.click(screen.getByLabelText("Switch workspace"));
    fireEvent.click(screen.getByText("Team Workspace"));
    expect(mockSwitchWorkspace).toHaveBeenCalledWith("ws-2");
  });

  it("shows new workspace form when clicking + New Workspace", () => {
    render(<WorkspaceSelector />);
    fireEvent.click(screen.getByLabelText("Switch workspace"));
    fireEvent.click(screen.getByText("+ New Workspace"));
    expect(screen.getByPlaceholderText("Workspace name")).toBeInTheDocument();
  });

  it("returns null when no workspaces and no active workspace", () => {
    mockWorkspaceState.workspaces = [];
    mockWorkspaceState.activeWorkspace = null;
    const { container } = render(<WorkspaceSelector />);
    expect(container.innerHTML).toBe("");
    // Restore for other tests.
    mockWorkspaceState.workspaces = [
      {
        id: "ws-1",
        name: "My Workspace",
        owner_id: "user-1",
        billing_tier: "free",
        created_at: "2026-01-01",
        updated_at: "2026-01-01",
      },
      {
        id: "ws-2",
        name: "Team Workspace",
        owner_id: "user-2",
        billing_tier: "pro",
        created_at: "2026-01-02",
        updated_at: "2026-01-02",
      },
    ];
    mockWorkspaceState.activeWorkspace = mockWorkspaceState.workspaces[0];
  });
});
