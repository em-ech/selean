import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemberManager } from "./MemberManager";

const mockInviteMember = vi.fn().mockResolvedValue(undefined);
const mockUpdateMemberRole = vi.fn().mockResolvedValue(undefined);
const mockRemoveMember = vi.fn().mockResolvedValue(undefined);

let mockUserRole = "admin";

const mockMembers = [
  {
    workspace_id: "ws-1",
    user_id: "user-1",
    email: "owner@test.com",
    display_name: "Owner",
    role: "owner",
    joined_at: "2026-01-01",
  },
  {
    workspace_id: "ws-1",
    user_id: "user-2",
    email: "editor@test.com",
    display_name: "Editor",
    role: "editor",
    joined_at: "2026-01-02",
  },
  {
    workspace_id: "ws-1",
    user_id: "user-3",
    email: "viewer@test.com",
    display_name: "Viewer",
    role: "viewer",
    joined_at: "2026-01-03",
  },
];

vi.mock("../hooks/useWorkspace", () => ({
  useWorkspace: () => ({
    activeWorkspace: {
      id: "ws-1",
      name: "Test Workspace",
      owner_id: "user-1",
      billing_tier: "free",
      created_at: "2026-01-01",
      updated_at: "2026-01-01",
    },
    members: mockMembers,
    userRole: mockUserRole,
    workspaces: [],
    isLoading: false,
    switchWorkspace: vi.fn(),
    createWorkspace: vi.fn(),
    renameWorkspace: vi.fn(),
    deleteWorkspace: vi.fn(),
    inviteMember: mockInviteMember,
    updateMemberRole: mockUpdateMemberRole,
    removeMember: mockRemoveMember,
    refreshWorkspaces: vi.fn(),
  }),
}));

describe("MemberManager", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    mockInviteMember.mockResolvedValue(undefined);
    mockUpdateMemberRole.mockResolvedValue(undefined);
    mockRemoveMember.mockResolvedValue(undefined);
    mockUserRole = "admin";
  });

  it("renders member list", () => {
    render(<MemberManager onClose={vi.fn()} />);
    // "Owner" appears as both display_name and badge, so use getAllByText.
    expect(screen.getAllByText("Owner").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Editor")).toBeInTheDocument();
    expect(screen.getByText("Viewer")).toBeInTheDocument();
    // Verify emails are shown too.
    expect(screen.getByText("owner@test.com")).toBeInTheDocument();
    expect(screen.getByText("editor@test.com")).toBeInTheDocument();
    expect(screen.getByText("viewer@test.com")).toBeInTheDocument();
  });

  it("shows invite form for admin users", () => {
    mockUserRole = "admin";
    render(<MemberManager onClose={vi.fn()} />);
    expect(screen.getByPlaceholderText("Email address")).toBeInTheDocument();
    expect(screen.getByText("Invite")).toBeInTheDocument();
  });

  it("hides invite form for non-admin users", () => {
    mockUserRole = "editor";
    render(<MemberManager onClose={vi.fn()} />);
    expect(
      screen.queryByPlaceholderText("Email address"),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("Invite")).not.toBeInTheDocument();
  });

  it("calls onClose when close button clicked", () => {
    const onClose = vi.fn();
    render(<MemberManager onClose={onClose} />);
    fireEvent.click(screen.getByLabelText("Close"));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("calls onClose when overlay backdrop clicked", () => {
    const onClose = vi.fn();
    const { container } = render(<MemberManager onClose={onClose} />);
    // The overlay is the outermost div.
    const overlay = container.firstChild as HTMLElement;
    fireEvent.click(overlay);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("shows owner badge for owner member", () => {
    render(<MemberManager onClose={vi.fn()} />);
    // Owner row has a distinct "Owner" badge text.
    const ownerBadges = screen.getAllByText("Owner");
    // One is the display_name, one is the badge.
    expect(ownerBadges.length).toBeGreaterThanOrEqual(2);
  });

  it("shows remove buttons for non-owner members when admin", () => {
    mockUserRole = "admin";
    render(<MemberManager onClose={vi.fn()} />);
    const removeButtons = screen.getAllByText("Remove");
    // Editor and Viewer should have remove buttons, but not Owner.
    expect(removeButtons).toHaveLength(2);
  });

  it("shows invite form for owner role too", () => {
    mockUserRole = "owner";
    render(<MemberManager onClose={vi.fn()} />);
    expect(screen.getByPlaceholderText("Email address")).toBeInTheDocument();
  });
});
