import { renderHook } from "@testing-library/react";
import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { useResizablePanel } from "./useResizablePanel";

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  localStorage.clear();
});

describe("useResizablePanel", () => {
  it("initializes with default width", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        edge: "right",
      }),
    );
    expect(result.current.width).toBe(300);
    expect(result.current.isResizing).toBe(false);
  });

  it("restores width from localStorage", () => {
    localStorage.setItem("test-width", "350");
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        storageKey: "test-width",
        edge: "right",
      }),
    );
    expect(result.current.width).toBe(350);
  });

  it("ignores invalid localStorage values", () => {
    localStorage.setItem("test-width", "not-a-number");
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        storageKey: "test-width",
        edge: "right",
      }),
    );
    expect(result.current.width).toBe(300);
  });

  it("ignores out-of-range localStorage values", () => {
    localStorage.setItem("test-width", "9999");
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        storageKey: "test-width",
        edge: "right",
      }),
    );
    expect(result.current.width).toBe(300);
  });

  it("persists width to localStorage", () => {
    renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        storageKey: "test-persist",
        edge: "right",
      }),
    );
    expect(localStorage.getItem("test-persist")).toBe("300");
  });

  it("returns handle props with correct style for right edge", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        edge: "right",
      }),
    );
    expect(result.current.handleProps.style.cursor).toBe("col-resize");
    expect(result.current.handleProps.style.right).toBe(-2);
  });

  it("returns handle props with correct style for left edge", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 500,
        edge: "left",
      }),
    );
    expect(result.current.handleProps.style.left).toBe(-2);
  });
});
