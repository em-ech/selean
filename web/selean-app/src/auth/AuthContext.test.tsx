import { act, render, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AuthProvider, useAuth, type AuthState } from "./AuthContext";
import * as authApi from "./api";
import * as apiUtils from "../utils/api";

// Mock the auth API module.
vi.mock("./api", () => ({
  login: vi.fn(),
  signup: vi.fn(),
  refreshTokens: vi.fn(),
  logout: vi.fn(),
  fetchAuthEnabled: vi.fn(),
}));

// Mutable ref container so TypeScript control-flow analysis doesn't narrow to `null`.
interface AuthStateRef {
  current: AuthState | null;
}

// Helper component to expose auth state in tests.
function AuthConsumer({ stateRef }: { stateRef: AuthStateRef }) {
  const auth = useAuth();
  stateRef.current = auth;
  return (
    <div>
      {auth.isLoading && <span>loading</span>}
      {auth.isAuthenticated && <span>authenticated</span>}
      {auth.user && (
        <span data-testid="user-name">{auth.user.display_name}</span>
      )}
    </div>
  );
}

function renderWithAuth(ref: AuthStateRef) {
  return render(
    <AuthProvider>
      <AuthConsumer stateRef={ref} />
    </AuthProvider>,
  );
}

describe("AuthContext", () => {
  let storedItems: Record<string, string>;

  beforeEach(() => {
    storedItems = {};
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(
      (key: string) => storedItems[key] ?? null,
    );
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(
      (key: string, value: string) => {
        storedItems[key] = value;
      },
    );
    vi.spyOn(Storage.prototype, "removeItem").mockImplementation(
      (key: string) => {
        delete storedItems[key];
      },
    );
    vi.clearAllMocks();
    // Auth is enabled unless a test says otherwise.
    vi.mocked(authApi.fetchAuthEnabled).mockResolvedValue(true);
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("starts unauthenticated with no stored token", async () => {
    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => {
      expect(ref.current?.isLoading).toBe(false);
    });
    expect(ref.current?.isAuthenticated).toBe(false);
    expect(ref.current?.isGuest).toBe(false);
    expect(ref.current?.user).toBeNull();
  });

  it("runs as a guest when the server has auth disabled", async () => {
    vi.mocked(authApi.fetchAuthEnabled).mockResolvedValue(false);

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => {
      expect(ref.current?.isLoading).toBe(false);
    });
    expect(ref.current?.isGuest).toBe(true);
    expect(ref.current?.isAuthenticated).toBe(false);
    expect(ref.current?.user).toBeNull();
  });

  it("does not try to restore a session in guest mode", async () => {
    storedItems["selean_refresh_token"] = "stored-refresh";
    vi.mocked(authApi.fetchAuthEnabled).mockResolvedValue(false);

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => {
      expect(ref.current?.isLoading).toBe(false);
    });
    expect(ref.current?.isGuest).toBe(true);
    expect(authApi.refreshTokens).not.toHaveBeenCalled();
  });

  it("logs in and stores tokens", async () => {
    const mockResponse = {
      access_token: "access-123",
      refresh_token: "refresh-456",
      expires_in: 900,
      user: {
        id: "u1",
        email: "a@b.com",
        display_name: "Alice",
        avatar_url: null,
      },
    };
    vi.mocked(authApi.login).mockResolvedValue(mockResponse);

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => expect(ref.current?.isLoading).toBe(false));

    await act(async () => {
      await ref.current?.login("a@b.com", "password123");
    });

    expect(ref.current?.isAuthenticated).toBe(true);
    expect(ref.current?.user?.display_name).toBe("Alice");
    expect(storedItems["selean_refresh_token"]).toBe("refresh-456");
  });

  it("signs up and stores tokens", async () => {
    const mockResponse = {
      access_token: "access-789",
      refresh_token: "refresh-012",
      expires_in: 900,
      user: {
        id: "u2",
        email: "b@c.com",
        display_name: "Bob",
        avatar_url: null,
      },
    };
    vi.mocked(authApi.signup).mockResolvedValue(mockResponse);

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => expect(ref.current?.isLoading).toBe(false));

    await act(async () => {
      await ref.current?.signup("b@c.com", "password123", "Bob");
    });

    expect(ref.current?.isAuthenticated).toBe(true);
    expect(ref.current?.user?.display_name).toBe("Bob");
  });

  it("logs out and clears state", async () => {
    const mockResponse = {
      access_token: "access-123",
      refresh_token: "refresh-456",
      expires_in: 900,
      user: {
        id: "u1",
        email: "a@b.com",
        display_name: "Alice",
        avatar_url: null,
      },
    };
    vi.mocked(authApi.login).mockResolvedValue(mockResponse);
    vi.mocked(authApi.logout).mockResolvedValue(undefined);

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => expect(ref.current?.isLoading).toBe(false));

    await act(async () => {
      await ref.current?.login("a@b.com", "pass");
    });
    expect(ref.current?.isAuthenticated).toBe(true);

    await act(async () => {
      await ref.current?.logout();
    });
    expect(ref.current?.isAuthenticated).toBe(false);
    expect(ref.current?.user).toBeNull();
    expect(storedItems["selean_refresh_token"]).toBeUndefined();
  });

  it("restores session from stored refresh token", async () => {
    storedItems["selean_refresh_token"] = "stored-refresh";
    vi.mocked(authApi.refreshTokens).mockResolvedValue({
      access_token: "new-access",
      refresh_token: "new-refresh",
      expires_in: 900,
      user: {
        id: "u1",
        email: "a@b.com",
        display_name: "Alice",
        avatar_url: null,
      },
    });

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => {
      expect(ref.current?.isLoading).toBe(false);
      expect(ref.current?.isAuthenticated).toBe(true);
    });
    expect(ref.current?.user?.display_name).toBe("Alice");
    expect(authApi.refreshTokens).toHaveBeenCalledWith("stored-refresh");
  });

  it("clears auth when stored refresh token is invalid", async () => {
    storedItems["selean_refresh_token"] = "expired-refresh";
    vi.mocked(authApi.refreshTokens).mockRejectedValue(new Error("expired"));

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => {
      expect(ref.current?.isLoading).toBe(false);
    });
    expect(ref.current?.isAuthenticated).toBe(false);
    expect(ref.current?.user).toBeNull();
  });

  it("sets access token via setAccessToken utility", async () => {
    const spy = vi.spyOn(apiUtils, "setAccessToken");
    const mockResponse = {
      access_token: "access-xyz",
      refresh_token: "refresh-xyz",
      expires_in: 900,
      user: {
        id: "u1",
        email: "a@b.com",
        display_name: "Alice",
        avatar_url: null,
      },
    };
    vi.mocked(authApi.login).mockResolvedValue(mockResponse);

    const ref: AuthStateRef = { current: null };
    renderWithAuth(ref);

    await waitFor(() => expect(ref.current?.isLoading).toBe(false));
    await act(async () => {
      await ref.current?.login("a@b.com", "pass");
    });

    expect(spy).toHaveBeenCalledWith("access-xyz");
  });
});
