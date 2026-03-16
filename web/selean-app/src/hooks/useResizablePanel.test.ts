import { renderHook, act } from "@testing-library/react";
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

  it("pointerdown + pointermove updates width", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 100,
        maxWidth: 600,
        edge: "right",
      }),
    );

    act(() => {
      result.current.handleProps.onPointerDown({
        clientX: 300,
        preventDefault: () => {},
        stopPropagation: () => {},
      } as unknown as React.PointerEvent);
    });

    expect(result.current.isResizing).toBe(true);

    act(() => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 350 }));
    });

    // edge "right": newWidth = startWidth + (clientX - startX) = 300 + 50 = 350
    expect(result.current.width).toBe(350);
  });

  it("drag below min clamps to min", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 200,
        maxWidth: 600,
        edge: "right",
      }),
    );

    act(() => {
      result.current.handleProps.onPointerDown({
        clientX: 300,
        preventDefault: () => {},
        stopPropagation: () => {},
      } as unknown as React.PointerEvent);
    });

    act(() => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 50 }));
    });

    // 300 + (50 - 300) = 50, clamped to min 200
    expect(result.current.width).toBe(200);
  });

  it("drag above max clamps to max", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 100,
        maxWidth: 400,
        edge: "right",
      }),
    );

    act(() => {
      result.current.handleProps.onPointerDown({
        clientX: 300,
        preventDefault: () => {},
        stopPropagation: () => {},
      } as unknown as React.PointerEvent);
    });

    act(() => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 600 }));
    });

    // 300 + (600 - 300) = 600, clamped to max 400
    expect(result.current.width).toBe(400);
  });

  it("pointerup stops resize and persists width", () => {
    const { result } = renderHook(() =>
      useResizablePanel({
        initialWidth: 300,
        minWidth: 100,
        maxWidth: 600,
        storageKey: "test-pointerup",
        edge: "right",
      }),
    );

    act(() => {
      result.current.handleProps.onPointerDown({
        clientX: 300,
        preventDefault: () => {},
        stopPropagation: () => {},
      } as unknown as React.PointerEvent);
    });

    act(() => {
      window.dispatchEvent(new PointerEvent("pointermove", { clientX: 380 }));
    });

    expect(result.current.width).toBe(380);

    act(() => {
      window.dispatchEvent(new PointerEvent("pointerup"));
    });

    expect(result.current.isResizing).toBe(false);
    expect(localStorage.getItem("test-pointerup")).toBe("380");
  });
});
