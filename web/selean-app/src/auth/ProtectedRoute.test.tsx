import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { ProtectedRoute } from "./ProtectedRoute";
import type { AuthState } from "./AuthContext";

let authState: AuthState;

vi.mock("./AuthContext", () => ({
  useAuth: () => authState,
}));

vi.mock("./LoginPage", () => ({
  LoginPage: () => <div>login page</div>,
}));

function makeAuthState(overrides: Partial<AuthState>): AuthState {
  return {
    user: null,
    isLoading: false,
    isAuthenticated: false,
    isGuest: false,
    login: vi.fn(),
    signup: vi.fn(),
    logout: vi.fn(),
    ...overrides,
  };
}

function renderRoute() {
  return render(
    <ProtectedRoute>
      <div>editor</div>
    </ProtectedRoute>,
  );
}

describe("ProtectedRoute", () => {
  beforeEach(() => {
    authState = makeAuthState({});
  });

  it("shows a loading state while auth is resolving", () => {
    authState = makeAuthState({ isLoading: true });
    renderRoute();
    expect(screen.getByText("Loading...")).toBeInTheDocument();
    expect(screen.queryByText("editor")).not.toBeInTheDocument();
    expect(screen.queryByText("login page")).not.toBeInTheDocument();
  });

  it("shows the login page when auth is on and nobody is signed in", () => {
    renderRoute();
    expect(screen.getByText("login page")).toBeInTheDocument();
    expect(screen.queryByText("editor")).not.toBeInTheDocument();
  });

  it("renders children for a signed-in user", () => {
    authState = makeAuthState({
      isAuthenticated: true,
      user: {
        id: "u1",
        email: "a@b.com",
        display_name: "Alice",
        avatar_url: null,
      },
    });
    renderRoute();
    expect(screen.getByText("editor")).toBeInTheDocument();
    expect(screen.queryByText("login page")).not.toBeInTheDocument();
  });

  it("renders children for a guest when the server has auth disabled", () => {
    authState = makeAuthState({ isGuest: true });
    renderRoute();
    expect(screen.getByText("editor")).toBeInTheDocument();
    expect(screen.queryByText("login page")).not.toBeInTheDocument();
  });
});
