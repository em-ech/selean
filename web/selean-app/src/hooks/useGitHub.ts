import { useState, useEffect, useCallback } from "react";
import { authFetch } from "../utils/api";

export interface GitHubRepo {
  full_name: string;
  default_branch: string;
  private: boolean;
}

export interface GitHubStatus {
  connected: boolean;
  login: string | null;
}

export interface PushResult {
  success: boolean;
  commit_sha?: string;
  pr_url?: string;
  error?: string;
}

export function useGitHub() {
  const [status, setStatus] = useState<GitHubStatus>({
    connected: false,
    login: null,
  });
  const [repos, setRepos] = useState<GitHubRepo[]>([]);
  const [loading, setLoading] = useState(true);

  // Check connection status on mount
  useEffect(() => {
    authFetch("/api/github/status")
      .then((r) => r.json())
      .then((data) => {
        setStatus(data);
        if (data.connected) {
          return authFetch("/api/github/repos").then((r) => r.json());
        }
        return { repos: [] };
      })
      .then((data) => setRepos(data.repos || []))
      .catch(() => setStatus({ connected: false, login: null }))
      .finally(() => setLoading(false));
  }, []);

  const connect = useCallback(async () => {
    const resp = await authFetch("/api/github/authorize");
    const { url } = await resp.json();
    // Open GitHub OAuth in same window
    window.location.href = url;
  }, []);

  const disconnect = useCallback(async () => {
    await authFetch("/api/github/disconnect", { method: "POST" });
    setStatus({ connected: false, login: null });
    setRepos([]);
  }, []);

  const refreshRepos = useCallback(async () => {
    const resp = await authFetch("/api/github/repos");
    const data = await resp.json();
    setRepos(data.repos || []);
  }, []);

  const createRepo = useCallback(
    async (name: string, isPrivate: boolean): Promise<GitHubRepo | null> => {
      const resp = await authFetch("/api/github/repos", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ name, private: isPrivate }),
      });
      if (!resp.ok) return null;
      const repo = await resp.json();
      await refreshRepos();
      return repo;
    },
    [refreshRepos],
  );

  const push = useCallback(
    async (
      repo: string,
      branch: string,
      files: Array<{ path: string; content: string }>,
      message: string,
      createPr: boolean,
    ): Promise<PushResult> => {
      const resp = await authFetch("/api/github/push", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          repo,
          branch,
          files,
          message,
          create_pr: createPr,
        }),
      });
      return resp.json();
    },
    [],
  );

  return {
    status,
    repos,
    loading,
    connect,
    disconnect,
    refreshRepos,
    createRepo,
    push,
  };
}
