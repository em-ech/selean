import {
  render,
  screen,
  fireEvent,
  waitFor,
  act,
} from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AuthProvider, useAuth } from "./AuthContext";
import * as authApi from "./api";
import * as apiUtils from "../utils/api";

// Mock the auth API module.
vi.mock("./api", () => ({
  login: vi.fn(),
  signup: vi.fn(),
  refreshTokens: vi.fn(),
  logout: vi.fn(),
}));

// Helper component to expose auth state in tests.
function AuthConsumer({
  onAuth,
}: {
  onAuth: (auth: ReturnType<typeof useAuth>) => void;
}) {
  const auth = useAuth();
  onAuth(auth);
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
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("starts unauthenticated with no stored token", async () => {
    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => {
      expect(authState?.isLoading).toBe(false);
    });
    expect(authState?.isAuthenticated).toBe(false);
    expect(authState?.user).toBeNull();
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

    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => expect(authState?.isLoading).toBe(false));

    await act(async () => {
      await authState?.login("a@b.com", "password123");
    });

    expect(authState?.isAuthenticated).toBe(true);
    expect(authState?.user?.display_name).toBe("Alice");
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

    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => expect(authState?.isLoading).toBe(false));

    await act(async () => {
      await authState?.signup("b@c.com", "password123", "Bob");
    });

    expect(authState?.isAuthenticated).toBe(true);
    expect(authState?.user?.display_name).toBe("Bob");
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

    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => expect(authState?.isLoading).toBe(false));

    await act(async () => {
      await authState?.login("a@b.com", "pass");
    });
    expect(authState?.isAuthenticated).toBe(true);

    await act(async () => {
      await authState?.logout();
    });
    expect(authState?.isAuthenticated).toBe(false);
    expect(authState?.user).toBeNull();
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

    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => {
      expect(authState?.isLoading).toBe(false);
      expect(authState?.isAuthenticated).toBe(true);
    });
    expect(authState?.user?.display_name).toBe("Alice");
    expect(authApi.refreshTokens).toHaveBeenCalledWith("stored-refresh");
  });

  it("clears auth when stored refresh token is invalid", async () => {
    storedItems["selean_refresh_token"] = "expired-refresh";
    vi.mocked(authApi.refreshTokens).mockRejectedValue(new Error("expired"));

    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => {
      expect(authState?.isLoading).toBe(false);
    });
    expect(authState?.isAuthenticated).toBe(false);
    expect(authState?.user).toBeNull();
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

    let authState: ReturnType<typeof useAuth> | null = null;
    render(
      <AuthProvider>
        <AuthConsumer
          onAuth={(a) => {
            authState = a;
          }}
        />
      </AuthProvider>,
    );

    await waitFor(() => expect(authState?.isLoading).toBe(false));
    await act(async () => {
      await authState?.login("a@b.com", "pass");
    });

    expect(spy).toHaveBeenCalledWith("access-xyz");
  });
});
