import { useAuth } from "./AuthContext";
import { LoginPage } from "./LoginPage";
import { colors } from "../theme";

export function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { isAuthenticated, isGuest, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div style={loadingStyle}>
        <span>Loading...</span>
      </div>
    );
  }

  // Guests are let through: the server only reports guest mode when auth
  // is disabled, and then none of its routes require a token.
  if (!isAuthenticated && !isGuest) {
    return <LoginPage />;
  }

  return <>{children}</>;
}

const loadingStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: colors.textFaint,
};
