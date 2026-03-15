import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { useTheme } from "./useTheme";
import { colors, lightColors, darkColors } from "../theme";

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  localStorage.clear();
});

describe("useTheme", () => {
  it("defaults to light mode", () => {
    const { result } = renderHook(() => useTheme());
    expect(result.current.mode).toBe("light");
  });

  it("restores dark mode from localStorage", () => {
    localStorage.setItem("selean-theme-mode", "dark");
    const { result } = renderHook(() => useTheme());
    expect(result.current.mode).toBe("dark");
  });

  it("toggles from light to dark", () => {
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.toggle();
    });
    expect(result.current.mode).toBe("dark");
  });

  it("toggles from dark to light", () => {
    localStorage.setItem("selean-theme-mode", "dark");
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.toggle();
    });
    expect(result.current.mode).toBe("light");
  });

  it("persists mode to localStorage", () => {
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.toggle();
    });
    expect(localStorage.getItem("selean-theme-mode")).toBe("dark");
  });

  it("applies dark colors when toggled to dark", () => {
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.toggle();
    });
    expect(colors.bg).toBe(darkColors.bg);
    expect(colors.text).toBe(darkColors.text);
  });

  it("applies light colors when toggled back to light", () => {
    localStorage.setItem("selean-theme-mode", "dark");
    const { result } = renderHook(() => useTheme());
    act(() => {
      result.current.toggle();
    });
    expect(colors.bg).toBe(lightColors.bg);
    expect(colors.text).toBe(lightColors.text);
  });
});
