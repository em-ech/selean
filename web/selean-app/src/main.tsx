import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { AuthProvider } from "./auth/AuthContext";
import { ProtectedRoute } from "./auth/ProtectedRoute";
import { applyTheme } from "./theme";

// Apply saved theme before first paint to avoid flash.
const savedTheme = localStorage.getItem("selean-theme-mode");
applyTheme(savedTheme === "dark" ? "dark" : "light");

const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <AuthProvider>
        <ProtectedRoute>
          <App />
        </ProtectedRoute>
      </AuthProvider>
    </StrictMode>,
  );
}
