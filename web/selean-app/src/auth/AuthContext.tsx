import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { AuthUser } from "./api";
import * as authApi from "./api";
import { setAccessToken, setRefreshFunction } from "../utils/api";

interface AuthState {
  user: AuthUser | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (email: string, password: string) => Promise<void>;
  signup: (
    email: string,
    password: string,
    displayName: string,
  ) => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthState | null>(null);

const REFRESH_TOKEN_KEY = "selean_refresh_token";

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const refreshTokenRef = useRef<string | null>(null);
  const refreshTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const scheduleRefresh = useCallback((expiresIn: number) => {
    if (refreshTimerRef.current) {
      clearTimeout(refreshTimerRef.current);
    }
    // Refresh 60 seconds before expiry (or at half-life if < 120s).
    const refreshMs = Math.max((expiresIn - 60) * 1000, (expiresIn / 2) * 1000);
    refreshTimerRef.current = setTimeout(() => {
      void performRefresh();
    }, refreshMs);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const handleAuthResponse = useCallback(
    (response: authApi.AuthResponse) => {
      setAccessToken(response.access_token);
      refreshTokenRef.current = response.refresh_token;
      localStorage.setItem(REFRESH_TOKEN_KEY, response.refresh_token);
      setUser(response.user);
      scheduleRefresh(response.expires_in);
    },
    [scheduleRefresh],
  );

  const clearAuth = useCallback(() => {
    setAccessToken(null);
    refreshTokenRef.current = null;
    localStorage.removeItem(REFRESH_TOKEN_KEY);
    setUser(null);
    if (refreshTimerRef.current) {
      clearTimeout(refreshTimerRef.current);
      refreshTimerRef.current = null;
    }
  }, []);

  const performRefresh = useCallback(async (): Promise<string | null> => {
    const token = refreshTokenRef.current;
    if (!token) return null;
    try {
      const response = await authApi.refreshTokens(token);
      handleAuthResponse(response);
      return response.access_token;
    } catch {
      clearAuth();
      return null;
    }
  }, [handleAuthResponse, clearAuth]);

  // Register refresh function for authFetch.
  useEffect(() => {
    setRefreshFunction(performRefresh);
    return () => setRefreshFunction(null);
  }, [performRefresh]);

  // Try to restore session on mount.
  useEffect(() => {
    const stored = localStorage.getItem(REFRESH_TOKEN_KEY);
    if (stored) {
      refreshTokenRef.current = stored;
      performRefresh().finally(() => setIsLoading(false));
    } else {
      setIsLoading(false);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  const loginFn = useCallback(
    async (email: string, password: string) => {
      const response = await authApi.login(email, password);
      handleAuthResponse(response);
    },
    [handleAuthResponse],
  );

  const signupFn = useCallback(
    async (email: string, password: string, displayName: string) => {
      const response = await authApi.signup(email, password, displayName);
      handleAuthResponse(response);
    },
    [handleAuthResponse],
  );

  const logoutFn = useCallback(async () => {
    const token = refreshTokenRef.current;
    clearAuth();
    if (token) {
      await authApi.logout(token);
    }
  }, [clearAuth]);

  // Cleanup timer on unmount.
  useEffect(() => {
    return () => {
      if (refreshTimerRef.current) {
        clearTimeout(refreshTimerRef.current);
      }
    };
  }, []);

  const value = useMemo<AuthState>(
    () => ({
      user,
      isLoading,
      isAuthenticated: user !== null,
      login: loginFn,
      signup: signupFn,
      logout: logoutFn,
    }),
    [user, isLoading, loginFn, signupFn, logoutFn],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthState {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return ctx;
}
