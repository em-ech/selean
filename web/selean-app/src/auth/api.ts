/**
 * Auth API client for signup, login, refresh, logout, and me endpoints.
 */

export interface AuthUser {
  id: string;
  email: string;
  display_name: string;
  avatar_url: string | null;
}

export interface AuthResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: AuthUser;
}

export interface AuthError {
  error: string;
}

async function parseErrorMessage(response: Response): Promise<string> {
  try {
    const body: AuthError = await response.json();
    return body.error || `Request failed (${response.status})`;
  } catch {
    return `Request failed (${response.status})`;
  }
}

export async function signup(
  email: string,
  password: string,
  displayName: string,
): Promise<AuthResponse> {
  const response = await fetch("/api/auth/signup", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      email,
      password,
      display_name: displayName,
    }),
  });
  if (!response.ok) {
    throw new Error(await parseErrorMessage(response));
  }
  return response.json();
}

export async function login(
  email: string,
  password: string,
): Promise<AuthResponse> {
  const response = await fetch("/api/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ email, password }),
  });
  if (!response.ok) {
    throw new Error(await parseErrorMessage(response));
  }
  return response.json();
}

export async function refreshTokens(
  refreshToken: string,
): Promise<AuthResponse> {
  const response = await fetch("/api/auth/refresh", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
  if (!response.ok) {
    throw new Error(await parseErrorMessage(response));
  }
  return response.json();
}

export async function logout(refreshToken: string): Promise<void> {
  await fetch("/api/auth/logout", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ refresh_token: refreshToken }),
  });
}

export async function getMe(accessToken: string): Promise<AuthUser> {
  const response = await fetch("/api/auth/me", {
    headers: { Authorization: `Bearer ${accessToken}` },
  });
  if (!response.ok) {
    throw new Error(await parseErrorMessage(response));
  }
  return response.json();
}

export async function githubAuthorizeUrl(): Promise<string> {
  const resp = await fetch("/api/github/authorize");
  const data = await resp.json();
  return data.url;
}
