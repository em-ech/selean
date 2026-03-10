import { useCallback, useState } from "react";
import { useAuth } from "./AuthContext";
import { githubAuthorizeUrl } from "./api";
import { colors, fontSizes } from "../theme";

type Mode = "login" | "signup";

export function LoginPage() {
  const { login, signup } = useAuth();
  const [mode, setMode] = useState<Mode>("login");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [displayName, setDisplayName] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault();
      setError("");
      setLoading(true);
      try {
        if (mode === "login") {
          await login(email, password);
        } else {
          await signup(email, password, displayName);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : "An error occurred");
      } finally {
        setLoading(false);
      }
    },
    [mode, email, password, displayName, login, signup],
  );

  const toggleMode = useCallback(() => {
    setMode((m) => (m === "login" ? "signup" : "login"));
    setError("");
  }, []);

  const handleGitHubLogin = useCallback(async () => {
    try {
      const url = await githubAuthorizeUrl();
      window.location.href = url;
    } catch (e) {
      console.warn("GitHub login failed", e);
    }
  }, []);

  return (
    <div style={pageStyle}>
      <div style={cardStyle}>
        <h1 style={titleStyle}>Selean</h1>
        <p style={subtitleStyle}>
          {mode === "login" ? "Sign in to your account" : "Create an account"}
        </p>

        <form onSubmit={handleSubmit} style={formStyle}>
          {mode === "signup" && (
            <input
              type="text"
              placeholder="Display name"
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              required
              style={inputStyle}
              autoComplete="name"
            />
          )}
          <input
            type="email"
            placeholder="Email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            required
            style={inputStyle}
            autoComplete="email"
          />
          <input
            type="password"
            placeholder="Password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            minLength={8}
            style={inputStyle}
            autoComplete={
              mode === "login" ? "current-password" : "new-password"
            }
          />

          {error && <p style={errorStyle}>{error}</p>}

          <button type="submit" disabled={loading} style={submitStyle}>
            {loading ? "..." : mode === "login" ? "Sign in" : "Create account"}
          </button>
        </form>

        <div style={dividerStyle}>
          <span style={dividerLineStyle} />
          <span style={dividerTextStyle}>or</span>
          <span style={dividerLineStyle} />
        </div>

        <button
          onClick={() => void handleGitHubLogin()}
          style={githubBtnStyle}
          type="button"
        >
          Sign in with GitHub
        </button>

        <button onClick={toggleMode} style={toggleStyle}>
          {mode === "login"
            ? "Need an account? Sign up"
            : "Already have an account? Sign in"}
        </button>
      </div>
    </div>
  );
}

const pageStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: colors.bg,
};

const cardStyle: React.CSSProperties = {
  width: 360,
  padding: 32,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: 8,
  display: "flex",
  flexDirection: "column",
  gap: 16,
};

const titleStyle: React.CSSProperties = {
  fontSize: 24,
  fontWeight: 700,
  color: colors.text,
  textAlign: "center",
  margin: 0,
};

const subtitleStyle: React.CSSProperties = {
  fontSize: fontSizes.base,
  color: colors.textDim,
  textAlign: "center",
  margin: 0,
};

const formStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 12,
};

const inputStyle: React.CSSProperties = {
  padding: "10px 12px",
  border: `1px solid ${colors.border}`,
  borderRadius: 4,
  background: colors.bg,
  color: colors.text,
  fontSize: fontSizes.base,
  outline: "none",
};

const errorStyle: React.CSSProperties = {
  color: "#e74c3c",
  fontSize: fontSizes.sm,
  margin: 0,
};

const submitStyle: React.CSSProperties = {
  padding: "10px 16px",
  background: "#3b82f6",
  color: "#fff",
  border: "none",
  borderRadius: 4,
  fontSize: fontSizes.base,
  fontWeight: 600,
  cursor: "pointer",
};

const dividerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 12,
};

const dividerLineStyle: React.CSSProperties = {
  flex: 1,
  height: 1,
  background: colors.border,
};

const dividerTextStyle: React.CSSProperties = {
  color: colors.textDim,
  fontSize: fontSizes.sm,
};

const githubBtnStyle: React.CSSProperties = {
  padding: "10px 16px",
  background: "#24292e",
  color: "#fff",
  border: `1px solid ${colors.border}`,
  borderRadius: 4,
  fontSize: fontSizes.base,
  fontWeight: 600,
  cursor: "pointer",
  textAlign: "center",
};

const toggleStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: "#3b82f6",
  cursor: "pointer",
  fontSize: fontSizes.sm,
  textAlign: "center",
  padding: 0,
};
