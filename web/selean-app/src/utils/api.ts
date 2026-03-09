/**
 * Centralized API client with automatic auth header injection and token refresh.
 */

let accessToken: string | null = null;
let refreshPromise: Promise<string | null> | null = null;

/** Set the current access token (called by AuthContext on login/refresh). */
export function setAccessToken(token: string | null): void {
  accessToken = token;
}

/** Get the current access token. */
export function getAccessToken(): string | null {
  return accessToken;
}

/** Register a token refresh function (called by AuthContext on mount). */
let refreshFn: (() => Promise<string | null>) | null = null;

export function setRefreshFunction(
  fn: (() => Promise<string | null>) | null,
): void {
  refreshFn = fn;
}

/**
 * Fetch wrapper that automatically adds Authorization header and retries
 * once on 401 by refreshing the access token.
 */
export async function authFetch(
  input: RequestInfo | URL,
  init?: RequestInit,
): Promise<Response> {
  const makeRequest = (token: string | null): Promise<Response> => {
    const headers = new Headers(init?.headers);
    if (token) {
      headers.set("Authorization", `Bearer ${token}`);
    }
    return fetch(input, { ...init, headers });
  };

  let response = await makeRequest(accessToken);

  // If 401 and we have a refresh function, try refreshing once.
  if (response.status === 401 && refreshFn) {
    // Deduplicate concurrent refresh attempts.
    if (!refreshPromise) {
      refreshPromise = refreshFn().finally(() => {
        refreshPromise = null;
      });
    }
    const newToken = await refreshPromise;
    if (newToken) {
      response = await makeRequest(newToken);
    }
  }

  return response;
}

/**
 * Build a WebSocket URL with auth token as query parameter.
 */
export function authWsUrl(path: string): string {
  const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
  const base = `${protocol}//${window.location.host}${path}`;
  if (accessToken) {
    return `${base}?token=${encodeURIComponent(accessToken)}`;
  }
  return base;
}
