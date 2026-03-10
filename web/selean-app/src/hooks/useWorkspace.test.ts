import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import React from "react";

// Mock authFetch before importing the module under test.
const mockAuthFetch = vi.fn();
vi.mock("../utils/api", () => ({
  authFetch: (...args: unknown[]) => mockAuthFetch(...args),
}));

// Mock useAuth to return a fake authenticated user.
const mockUser = {
  id: "user-1",
  email: "a@b.com",
  display_name: "Alice",
  avatar_url: null,
};
vi.mock("../auth/AuthContext", () => ({
  useAuth: () => ({
    user: mockUser,
    isAuthenticated: true,
    isLoading: false,
    login: vi.fn(),
    signup: vi.fn(),
    logout: vi.fn(),
  }),
}));

import { WorkspaceProvider, useWorkspace } from "./useWorkspace";

function makeResponse(body: unknown, ok = true, status = 200) {
  return {
    ok,
    status,
    json: () => Promise.resolve(body),
  } as unknown as Response;
}

const WS_1 = {
  id: "ws-1",
  name: "My Workspace",
  owner_id: "user-1",
  billing_tier: "free",
  created_at: "2026-01-01",
  updated_at: "2026-01-01",
};

const WS_2 = {
  id: "ws-2",
  name: "Team Workspace",
  owner_id: "user-2",
  billing_tier: "pro",
  created_at: "2026-01-02",
  updated_at: "2026-01-02",
};

const MEMBER_1 = {
  workspace_id: "ws-1",
  user_id: "user-1",
  email: "a@b.com",
  display_name: "Alice",
  role: "owner",
  joined_at: "2026-01-01",
};

const MEMBER_2 = {
  workspace_id: "ws-1",
  user_id: "user-2",
  email: "b@b.com",
  display_name: "Bob",
  role: "editor",
  joined_at: "2026-01-02",
};

function wrapper({ children }: { children: React.ReactNode }) {
  return React.createElement(WorkspaceProvider, null, children);
}

describe("useWorkspace", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    localStorage.clear();
    mockAuthFetch.mockReset();
  });

  it("throws when used outside WorkspaceProvider", () => {
    // Suppress console.error from React for the expected throw.
    const spy = vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => {
      renderHook(() => useWorkspace());
    }).toThrow("useWorkspace must be used within WorkspaceProvider");
    spy.mockRestore();
  });

  it("fetches workspaces on mount", async () => {
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse([WS_1, WS_2]))
      .mockResolvedValueOnce(makeResponse([MEMBER_1, MEMBER_2]));

    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.workspaces).toHaveLength(2);
    });

    expect(result.current.activeWorkspace).toEqual(WS_1);
    expect(mockAuthFetch).toHaveBeenCalledWith("/api/workspaces");
  });

  it("switchWorkspace updates active workspace and fetches members", async () => {
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse([WS_1, WS_2]))
      .mockResolvedValueOnce(makeResponse([MEMBER_1]));

    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.workspaces).toHaveLength(2);
    });

    // Switch to ws-2
    mockAuthFetch.mockResolvedValueOnce(makeResponse([MEMBER_2]));

    await act(async () => {
      await result.current.switchWorkspace("ws-2");
    });

    expect(result.current.activeWorkspace).toEqual(WS_2);
    expect(localStorage.getItem("selean_active_workspace")).toBe("ws-2");
  });

  it("createWorkspace calls API and refreshes list", async () => {
    const newWs = {
      id: "ws-3",
      name: "New WS",
      owner_id: "user-1",
      billing_tier: "free",
      created_at: "2026-01-03",
      updated_at: "2026-01-03",
    };

    // Initial fetch
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse([WS_1]))
      .mockResolvedValueOnce(makeResponse([MEMBER_1]));

    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.workspaces).toHaveLength(1);
    });

    // Create: POST, then refresh (GET workspaces), then switch (GET members)
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse(newWs))
      .mockResolvedValueOnce(makeResponse([WS_1, newWs]))
      .mockResolvedValueOnce(makeResponse([]));

    await act(async () => {
      const created = await result.current.createWorkspace("New WS");
      expect(created.id).toBe("ws-3");
    });

    expect(mockAuthFetch).toHaveBeenCalledWith(
      "/api/workspaces",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ name: "New WS" }),
      }),
    );
  });

  it("inviteMember calls API with correct payload", async () => {
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse([WS_1]))
      .mockResolvedValueOnce(makeResponse([MEMBER_1]));

    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.activeWorkspace).toEqual(WS_1);
    });

    // Invite: POST, then refresh members
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse({}))
      .mockResolvedValueOnce(makeResponse([MEMBER_1, MEMBER_2]));

    await act(async () => {
      await result.current.inviteMember("b@b.com", "editor");
    });

    expect(mockAuthFetch).toHaveBeenCalledWith(
      "/api/workspaces/ws-1/members",
      expect.objectContaining({
        method: "POST",
        body: JSON.stringify({ email: "b@b.com", role: "editor" }),
      }),
    );
  });

  it("removeMember calls API and refreshes members", async () => {
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse([WS_1]))
      .mockResolvedValueOnce(makeResponse([MEMBER_1, MEMBER_2]));

    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.members).toHaveLength(2);
    });

    // Remove: DELETE, then refresh members
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse({}))
      .mockResolvedValueOnce(makeResponse([MEMBER_1]));

    await act(async () => {
      await result.current.removeMember("user-2");
    });

    expect(mockAuthFetch).toHaveBeenCalledWith(
      "/api/workspaces/ws-1/members/user-2",
      expect.objectContaining({ method: "DELETE" }),
    );

    await waitFor(() => {
      expect(result.current.members).toHaveLength(1);
    });
  });

  it("derives userRole from members list", async () => {
    mockAuthFetch
      .mockResolvedValueOnce(makeResponse([WS_1]))
      .mockResolvedValueOnce(makeResponse([MEMBER_1, MEMBER_2]));

    const { result } = renderHook(() => useWorkspace(), { wrapper });

    await waitFor(() => {
      expect(result.current.userRole).toBe("owner");
    });
  });
});
