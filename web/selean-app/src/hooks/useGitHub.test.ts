import { renderHook, act, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { useGitHub } from "./useGitHub";

// Mock authFetch
const mockAuthFetch = vi.fn();
vi.mock("../utils/api", () => ({
  authFetch: (...args: unknown[]) => mockAuthFetch(...args),
}));

beforeEach(() => {
  mockAuthFetch.mockReset();
});

describe("useGitHub", () => {
  it("starts in loading state", () => {
    mockAuthFetch.mockResolvedValue({
      json: () => Promise.resolve({ connected: false, login: null }),
    });
    const { result } = renderHook(() => useGitHub());
    expect(result.current.loading).toBe(true);
  });

  it("fetches status on mount", async () => {
    mockAuthFetch.mockResolvedValue({
      json: () => Promise.resolve({ connected: false, login: null }),
    });

    renderHook(() => useGitHub());

    await waitFor(() => {
      expect(mockAuthFetch).toHaveBeenCalledWith("/api/github/status");
    });
  });

  it("sets connected state from API", async () => {
    mockAuthFetch
      .mockResolvedValueOnce({
        json: () => Promise.resolve({ connected: true, login: "testuser" }),
      })
      .mockResolvedValueOnce({
        json: () =>
          Promise.resolve({
            repos: [
              {
                full_name: "testuser/repo",
                default_branch: "main",
                private: false,
              },
            ],
          }),
      });

    const { result } = renderHook(() => useGitHub());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });
    expect(result.current.status.connected).toBe(true);
    expect(result.current.status.login).toBe("testuser");
  });

  it("fetches repos when connected", async () => {
    mockAuthFetch
      .mockResolvedValueOnce({
        json: () => Promise.resolve({ connected: true, login: "testuser" }),
      })
      .mockResolvedValueOnce({
        json: () =>
          Promise.resolve({
            repos: [
              {
                full_name: "testuser/repo",
                default_branch: "main",
                private: false,
              },
            ],
          }),
      });

    const { result } = renderHook(() => useGitHub());

    await waitFor(() => {
      expect(result.current.repos).toHaveLength(1);
    });
    expect(result.current.repos[0].full_name).toBe("testuser/repo");
    expect(mockAuthFetch).toHaveBeenCalledWith("/api/github/repos");
  });

  it("disconnect clears state", async () => {
    mockAuthFetch
      .mockResolvedValueOnce({
        json: () => Promise.resolve({ connected: true, login: "testuser" }),
      })
      .mockResolvedValueOnce({
        json: () =>
          Promise.resolve({
            repos: [
              {
                full_name: "testuser/repo",
                default_branch: "main",
                private: false,
              },
            ],
          }),
      })
      .mockResolvedValueOnce({
        json: () => Promise.resolve({}),
      });

    const { result } = renderHook(() => useGitHub());

    await waitFor(() => {
      expect(result.current.status.connected).toBe(true);
    });

    await act(async () => {
      await result.current.disconnect();
    });

    expect(result.current.status.connected).toBe(false);
    expect(result.current.repos).toHaveLength(0);
  });

  it("push sends correct payload", async () => {
    const pushResponse = {
      success: true,
      commit_sha: "abc1234",
    };
    mockAuthFetch
      .mockResolvedValueOnce({
        json: () => Promise.resolve({ connected: false, login: null }),
      })
      .mockResolvedValueOnce({
        json: () => Promise.resolve(pushResponse),
      });

    const { result } = renderHook(() => useGitHub());

    await waitFor(() => {
      expect(result.current.loading).toBe(false);
    });

    const files = [{ path: "src/App.tsx", content: "export default App;" }];
    let pushResult;
    await act(async () => {
      pushResult = await result.current.push(
        "user/repo",
        "selean/export",
        files,
        "Export from Selean",
        true,
      );
    });

    expect(mockAuthFetch).toHaveBeenCalledWith("/api/github/push", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        repo: "user/repo",
        branch: "selean/export",
        files,
        message: "Export from Selean",
        create_pr: true,
      }),
    });
    expect(pushResult).toEqual(pushResponse);
  });
});
