import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { useAuth } from "../auth/AuthContext";
import { authFetch } from "../utils/api";

export interface Workspace {
  id: string;
  name: string;
  owner_id: string;
  billing_tier: string;
  created_at: string;
  updated_at: string;
}

export interface WorkspaceMember {
  workspace_id: string;
  user_id: string;
  email: string;
  display_name: string;
  role: string;
  joined_at: string;
}

export interface WorkspaceState {
  workspaces: Workspace[];
  activeWorkspace: Workspace | null;
  members: WorkspaceMember[];
  userRole: string | null;
  isLoading: boolean;
  switchWorkspace: (id: string) => Promise<void>;
  createWorkspace: (name: string) => Promise<Workspace>;
  renameWorkspace: (id: string, name: string) => Promise<void>;
  deleteWorkspace: (id: string) => Promise<void>;
  inviteMember: (email: string, role: string) => Promise<void>;
  updateMemberRole: (userId: string, role: string) => Promise<void>;
  removeMember: (userId: string) => Promise<void>;
  refreshWorkspaces: () => Promise<Workspace[]>;
}

const STORAGE_KEY = "selean_active_workspace";

const WorkspaceContext = createContext<WorkspaceState | null>(null);

async function fetchWorkspaces(): Promise<Workspace[]> {
  const res = await authFetch("/api/workspaces");
  if (!res.ok) {
    throw new Error(`Failed to fetch workspaces (${res.status})`);
  }
  return res.json();
}

async function fetchMembers(workspaceId: string): Promise<WorkspaceMember[]> {
  const res = await authFetch(`/api/workspaces/${workspaceId}/members`);
  if (!res.ok) {
    throw new Error(`Failed to fetch members (${res.status})`);
  }
  return res.json();
}

export function WorkspaceProvider({ children }: { children: React.ReactNode }) {
  const { user, isAuthenticated } = useAuth();
  const [workspaces, setWorkspaces] = useState<Workspace[]>([]);
  const [activeWorkspaceId, setActiveWorkspaceId] = useState<string | null>(
    () => localStorage.getItem(STORAGE_KEY),
  );
  const [members, setMembers] = useState<WorkspaceMember[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const activeWorkspace = useMemo(
    () => workspaces.find((w) => w.id === activeWorkspaceId) ?? null,
    [workspaces, activeWorkspaceId],
  );

  const userRole = useMemo(() => {
    if (!user || !activeWorkspace) return null;
    const member = members.find((m) => m.user_id === user.id);
    return member?.role ?? null;
  }, [user, activeWorkspace, members]);

  const loadMembers = useCallback(async (workspaceId: string) => {
    try {
      const result = await fetchMembers(workspaceId);
      setMembers(result);
    } catch (err) {
      console.warn("Failed to load workspace members", err);
      setMembers([]);
    }
  }, []);

  const refreshWorkspaces = useCallback(async () => {
    setIsLoading(true);
    try {
      const result = await fetchWorkspaces();
      setWorkspaces(result);
      return result;
    } catch (err) {
      console.warn("Failed to load workspaces", err);
      setWorkspaces([]);
      return [];
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Fetch workspaces on mount when authenticated.
  useEffect(() => {
    if (!isAuthenticated) {
      setWorkspaces([]);
      setMembers([]);
      return;
    }
    void refreshWorkspaces().then((result) => {
      if (result.length === 0) return;
      const stored = localStorage.getItem(STORAGE_KEY);
      const match = result.find((w) => w.id === stored);
      const selected = match ?? result[0];
      setActiveWorkspaceId(selected.id);
      localStorage.setItem(STORAGE_KEY, selected.id);
      void loadMembers(selected.id);
    });
  }, [isAuthenticated]); // eslint-disable-line react-hooks/exhaustive-deps

  const switchWorkspace = useCallback(
    async (id: string) => {
      setActiveWorkspaceId(id);
      localStorage.setItem(STORAGE_KEY, id);
      await loadMembers(id);
    },
    [loadMembers],
  );

  const createWorkspace = useCallback(
    async (name: string): Promise<Workspace> => {
      const res = await authFetch("/api/workspaces", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      if (!res.ok) {
        throw new Error(`Failed to create workspace (${res.status})`);
      }
      const created: Workspace = await res.json();
      await refreshWorkspaces();
      await switchWorkspace(created.id);
      return created;
    },
    [refreshWorkspaces, switchWorkspace],
  );

  const renameWorkspace = useCallback(
    async (id: string, name: string) => {
      const res = await authFetch(`/api/workspaces/${id}`, {
        method: "PATCH",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name }),
      });
      if (!res.ok) {
        throw new Error(`Failed to rename workspace (${res.status})`);
      }
      await refreshWorkspaces();
    },
    [refreshWorkspaces],
  );

  const deleteWorkspace = useCallback(
    async (id: string) => {
      const res = await authFetch(`/api/workspaces/${id}`, {
        method: "DELETE",
      });
      if (!res.ok) {
        throw new Error(`Failed to delete workspace (${res.status})`);
      }
      const updated = await refreshWorkspaces();
      if (id === activeWorkspaceId && updated.length > 0) {
        await switchWorkspace(updated[0].id);
      }
    },
    [refreshWorkspaces, activeWorkspaceId, switchWorkspace],
  );

  const inviteMember = useCallback(
    async (email: string, role: string) => {
      if (!activeWorkspaceId) {
        throw new Error("No active workspace");
      }
      const res = await authFetch(
        `/api/workspaces/${activeWorkspaceId}/members`,
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ email, role }),
        },
      );
      if (!res.ok) {
        throw new Error(`Failed to invite member (${res.status})`);
      }
      await loadMembers(activeWorkspaceId);
    },
    [activeWorkspaceId, loadMembers],
  );

  const updateMemberRole = useCallback(
    async (userId: string, role: string) => {
      if (!activeWorkspaceId) {
        throw new Error("No active workspace");
      }
      const res = await authFetch(
        `/api/workspaces/${activeWorkspaceId}/members/${userId}`,
        {
          method: "PATCH",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ role }),
        },
      );
      if (!res.ok) {
        throw new Error(`Failed to update member role (${res.status})`);
      }
      await loadMembers(activeWorkspaceId);
    },
    [activeWorkspaceId, loadMembers],
  );

  const removeMember = useCallback(
    async (userId: string) => {
      if (!activeWorkspaceId) {
        throw new Error("No active workspace");
      }
      const res = await authFetch(
        `/api/workspaces/${activeWorkspaceId}/members/${userId}`,
        {
          method: "DELETE",
        },
      );
      if (!res.ok) {
        throw new Error(`Failed to remove member (${res.status})`);
      }
      await loadMembers(activeWorkspaceId);
    },
    [activeWorkspaceId, loadMembers],
  );

  const value = useMemo<WorkspaceState>(
    () => ({
      workspaces,
      activeWorkspace,
      members,
      userRole,
      isLoading,
      switchWorkspace,
      createWorkspace,
      renameWorkspace,
      deleteWorkspace,
      inviteMember,
      updateMemberRole,
      removeMember,
      refreshWorkspaces,
    }),
    [
      workspaces,
      activeWorkspace,
      members,
      userRole,
      isLoading,
      switchWorkspace,
      createWorkspace,
      renameWorkspace,
      deleteWorkspace,
      inviteMember,
      updateMemberRole,
      removeMember,
      refreshWorkspaces,
    ],
  );

  return React.createElement(WorkspaceContext.Provider, { value }, children);
}

export function useWorkspace(): WorkspaceState {
  const ctx = useContext(WorkspaceContext);
  if (!ctx) {
    throw new Error("useWorkspace must be used within WorkspaceProvider");
  }
  return ctx;
}
