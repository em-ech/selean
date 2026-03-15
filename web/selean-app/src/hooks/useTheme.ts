import { useCallback, useEffect, useState } from "react";
import { applyTheme } from "../theme";

export type ThemeMode = "light" | "dark";

const STORAGE_KEY = "selean-theme-mode";

/**
 * Hook for toggling between light and dark themes.
 * Persists choice to localStorage.
 * Calls `applyTheme()` to swap the mutable `colors` object,
 * then triggers a re-render so all components pick up the new values.
 */
export function useTheme() {
  const [mode, setMode] = useState<ThemeMode>(() => {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "dark" || saved === "light") return saved;
    return "light";
  });

  useEffect(() => {
    applyTheme(mode);
    localStorage.setItem(STORAGE_KEY, mode);
  }, [mode]);

  const toggle = useCallback(() => {
    setMode((m) => (m === "light" ? "dark" : "light"));
  }, []);

  return { mode, toggle };
}
