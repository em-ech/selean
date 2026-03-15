/** Shared design tokens for the Selean editor UI. */

/** Color palette type shared between light and dark themes. */
export type ColorPalette = {
  bg: string;
  surface: string;
  surfaceAlt: string;
  border: string;
  borderHover: string;
  accent: string;
  accentHover: string;
  accentLight: string;
  text: string;
  textMuted: string;
  textDim: string;
  textFaint: string;
  bgLight: string;
  white: string;
  danger: string;
  dangerLight: string;
  success: string;
  successLight: string;
  userBubble: string;
  userBubbleText: string;
  assistantBubble: string;
  assistantBubbleText: string;
  canvasBg: string;
};

export const lightColors: ColorPalette = {
  bg: "#ffffff",
  surface: "#f7f7f8",
  surfaceAlt: "#f0f0f2",
  border: "#e4e4e7",
  borderHover: "#d1d1d6",
  accent: "#6c5ce7",
  accentHover: "#5a4bd1",
  accentLight: "#ede9fc",
  text: "#1a1a2e",
  textMuted: "#555",
  textDim: "#777",
  textFaint: "#999",
  bgLight: "#fafafa",
  white: "#ffffff",
  danger: "#e74c3c",
  dangerLight: "#fdecea",
  success: "#27ae60",
  successLight: "#e8f8ef",
  userBubble: "#6c5ce7",
  userBubbleText: "#ffffff",
  assistantBubble: "#f0f0f2",
  assistantBubbleText: "#1a1a2e",
  canvasBg: "#e8e8ec",
};

export const darkColors: ColorPalette = {
  bg: "#0e0e1a",
  surface: "#1a1a2e",
  surfaceAlt: "#1e1e3a",
  border: "#2a2a4a",
  borderHover: "#3a3a5a",
  accent: "#6c5ce7",
  accentHover: "#7d6ff0",
  accentLight: "#2a2a4a",
  text: "#e0e0e0",
  textMuted: "#aaa",
  textDim: "#888",
  textFaint: "#666",
  bgLight: "#2a2a3e",
  white: "#ffffff",
  danger: "#e74c3c",
  dangerLight: "#3a1a1a",
  success: "#27ae60",
  successLight: "#1a3a2a",
  userBubble: "#6c5ce7",
  userBubbleText: "#ffffff",
  assistantBubble: "#1e1e3a",
  assistantBubbleText: "#e0e0e0",
  canvasBg: "#12121e",
};

/**
 * Mutable active color palette. Components import this and use it directly
 * in inline styles. `applyTheme()` swaps the values at runtime.
 */
export const colors: ColorPalette = { ...lightColors };

/** Swap the active color palette. Call this from useTheme on toggle. */
export function applyTheme(mode: "light" | "dark"): void {
  const source = mode === "dark" ? darkColors : lightColors;
  Object.assign(colors, source);
  // Update the root HTML element to match.
  if (typeof document !== "undefined") {
    document.documentElement.style.background = source.bg;
    document.documentElement.style.color = source.text;
    const body = document.body;
    if (body) {
      body.style.background = source.bg;
      body.style.color = source.text;
    }
  }
}

export const fontSizes = {
  xs: 11,
  sm: 12,
  base: 13,
  md: 14,
  lg: 15,
  xl: 17,
  xxl: 20,
  heading: 24,
} as const;

export const spacing = {
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 24,
  xxl: 32,
} as const;

export const radii = {
  sm: 4,
  md: 8,
  lg: 12,
  xl: 16,
  full: 9999,
} as const;

export const shadows = {
  sm: "0 1px 2px rgba(0,0,0,0.06)",
  md: "0 2px 8px rgba(0,0,0,0.08)",
  lg: "0 4px 16px rgba(0,0,0,0.12)",
  toolbar: "0 2px 12px rgba(0,0,0,0.15)",
} as const;
