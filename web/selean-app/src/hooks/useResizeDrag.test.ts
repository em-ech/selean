import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { useResizeDrag, computeResizedBounds } from "./useResizeDrag";
import { createMockEditor, DEFAULT_CAMERA } from "../test/mock-editor";
import type { SelectionBounds, CameraInfo } from "../wasm/types";

function makeBounds(overrides: Partial<SelectionBounds> = {}): SelectionBounds {
  return {
    node_id: "node-abc",
    x: 100,
    y: 200,
    width: 300,
    height: 150,
    ...overrides,
  };
}

function makeOptions(overrides: Record<string, unknown> = {}) {
  const editor = createMockEditor(overrides);
  return {
    editorRef: { current: editor },
    bounds: [makeBounds()],
    camera: { ...DEFAULT_CAMERA } as CameraInfo,
    onSceneChanged: vi.fn(),
  };
}

function makePointerEvent(
  x: number,
  y: number,
  pointerId = 1,
): React.PointerEvent {
  const target = document.createElement("div");
  return {
    clientX: x,
    clientY: y,
    pointerId,
    target,
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as React.PointerEvent;
}

describe("useResizeDrag", () => {
  it("isDragging starts as false", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useResizeDrag(opts));
    expect(result.current.isDragging).toBe(false);
  });

  it("returns getHandleProps function", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(0);
    expect(props.onPointerDown).toBeTypeOf("function");
    expect(props.onPointerMove).toBeTypeOf("function");
    expect(props.onPointerUp).toBeTypeOf("function");
  });

  it("drag bottom-right handle increases width and height", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(4); // bottom-right

    act(() => {
      props.onPointerDown(makePointerEvent(500, 500));
    });
    expect(result.current.isDragging).toBe(true);
    expect(opts.editorRef.current.begin_group).toHaveBeenCalledWith("Resize");

    act(() => {
      props.onPointerMove(makePointerEvent(600, 550));
    });

    // execute_command should have been called with new bounds
    const calls = (
      opts.editorRef.current.execute_command as ReturnType<typeof vi.fn>
    ).mock.calls;
    expect(calls.length).toBeGreaterThanOrEqual(1);
    const lastArgs = JSON.parse(calls[calls.length - 1][0]);
    expect(lastArgs.type).toBe("SetBounds");
    expect(lastArgs.node_id).toBe("node-abc");
    // Bottom-right: x,y stay same, width/height increase.
    // DPR=1, zoom=1, so delta world = delta client * dpr / zoom = delta client.
    // delta = (100, 50), original = (300, 150)
    expect(lastArgs.bounds.x).toBe(100);
    expect(lastArgs.bounds.y).toBe(200);
    expect(lastArgs.bounds.width).toBe(400);
    expect(lastArgs.bounds.height).toBe(200);

    act(() => {
      props.onPointerUp(makePointerEvent(600, 550));
    });
    expect(result.current.isDragging).toBe(false);
    expect(opts.editorRef.current.end_group).toHaveBeenCalledTimes(1);
  });

  it("drag top-left handle moves origin and decreases size", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(0); // top-left

    act(() => {
      props.onPointerDown(makePointerEvent(200, 200));
    });

    act(() => {
      // Move 50 right, 30 down => origin shifts right+down, size shrinks
      props.onPointerMove(makePointerEvent(250, 230));
    });

    const calls = (
      opts.editorRef.current.execute_command as ReturnType<typeof vi.fn>
    ).mock.calls;
    const lastArgs = JSON.parse(calls[calls.length - 1][0]);
    expect(lastArgs.bounds.x).toBe(150); // 100 + 50
    expect(lastArgs.bounds.y).toBe(230); // 200 + 30
    expect(lastArgs.bounds.width).toBe(250); // 300 - 50
    expect(lastArgs.bounds.height).toBe(120); // 150 - 30

    act(() => {
      props.onPointerUp(makePointerEvent(250, 230));
    });
  });

  it("clamps minimum size to 10x10", () => {
    const opts = makeOptions();
    opts.bounds = [makeBounds({ width: 50, height: 50 })];
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(0); // top-left

    act(() => {
      props.onPointerDown(makePointerEvent(100, 100));
    });

    // Move 200 right (more than width) and 200 down (more than height)
    act(() => {
      props.onPointerMove(makePointerEvent(300, 300));
    });

    const calls = (
      opts.editorRef.current.execute_command as ReturnType<typeof vi.fn>
    ).mock.calls;
    const lastArgs = JSON.parse(calls[calls.length - 1][0]);
    // Width clamped: can only shift left edge by (50 - 10) = 40
    expect(lastArgs.bounds.width).toBeGreaterThanOrEqual(10);
    expect(lastArgs.bounds.height).toBeGreaterThanOrEqual(10);
  });

  it("pointer up without move commits group cleanly", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(4);

    act(() => {
      props.onPointerDown(makePointerEvent(500, 500));
    });
    expect(opts.editorRef.current.begin_group).toHaveBeenCalledTimes(1);

    // Pointer up at same position (no move)
    act(() => {
      props.onPointerUp(makePointerEvent(500, 500));
    });
    expect(opts.editorRef.current.end_group).toHaveBeenCalledTimes(1);
    expect(result.current.isDragging).toBe(false);
  });

  it("does nothing when bounds is empty", () => {
    const opts = makeOptions();
    opts.bounds = [];
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(0);

    act(() => {
      props.onPointerDown(makePointerEvent(100, 100));
    });
    expect(result.current.isDragging).toBe(false);
    expect(opts.editorRef.current.begin_group).not.toHaveBeenCalled();
  });

  it("does nothing when camera is null", () => {
    const opts = makeOptions();
    opts.camera = null;
    const { result } = renderHook(() => useResizeDrag(opts));
    const props = result.current.getHandleProps(0);

    act(() => {
      props.onPointerDown(makePointerEvent(100, 100));
    });
    expect(result.current.isDragging).toBe(false);
  });
});

describe("computeResizedBounds", () => {
  const origX = 100;
  const origY = 200;
  const origW = 300;
  const origH = 150;

  it("handle 0 (top-left): moves x+y, shrinks w+h", () => {
    const r = computeResizedBounds(0, origX, origY, origW, origH, 20, 10);
    expect(r.x).toBe(120);
    expect(r.y).toBe(210);
    expect(r.width).toBe(280);
    expect(r.height).toBe(140);
  });

  it("handle 1 (top-center): moves y, shrinks h", () => {
    const r = computeResizedBounds(1, origX, origY, origW, origH, 0, 30);
    expect(r.x).toBe(100);
    expect(r.y).toBe(230);
    expect(r.width).toBe(300);
    expect(r.height).toBe(120);
  });

  it("handle 2 (top-right): moves y, grows w, shrinks h", () => {
    const r = computeResizedBounds(2, origX, origY, origW, origH, 40, 20);
    expect(r.x).toBe(100);
    expect(r.y).toBe(220);
    expect(r.width).toBe(340);
    expect(r.height).toBe(130);
  });

  it("handle 3 (middle-right): grows w only", () => {
    const r = computeResizedBounds(3, origX, origY, origW, origH, 50, 99);
    expect(r.x).toBe(100);
    expect(r.y).toBe(200);
    expect(r.width).toBe(350);
    expect(r.height).toBe(150);
  });

  it("handle 4 (bottom-right): grows w+h", () => {
    const r = computeResizedBounds(4, origX, origY, origW, origH, 60, 40);
    expect(r.x).toBe(100);
    expect(r.y).toBe(200);
    expect(r.width).toBe(360);
    expect(r.height).toBe(190);
  });

  it("handle 5 (bottom-center): grows h only", () => {
    const r = computeResizedBounds(5, origX, origY, origW, origH, 99, 25);
    expect(r.x).toBe(100);
    expect(r.y).toBe(200);
    expect(r.width).toBe(300);
    expect(r.height).toBe(175);
  });

  it("handle 6 (bottom-left): moves x, shrinks w, grows h", () => {
    const r = computeResizedBounds(6, origX, origY, origW, origH, 30, 20);
    expect(r.x).toBe(130);
    expect(r.y).toBe(200);
    expect(r.width).toBe(270);
    expect(r.height).toBe(170);
  });

  it("handle 7 (middle-left): moves x, shrinks w only", () => {
    const r = computeResizedBounds(7, origX, origY, origW, origH, 15, 99);
    expect(r.x).toBe(115);
    expect(r.y).toBe(200);
    expect(r.width).toBe(285);
    expect(r.height).toBe(150);
  });

  it("clamps left-moving handles to MIN_SIZE", () => {
    // Try to shift left edge 400 units on a 300-wide node
    const r = computeResizedBounds(7, origX, origY, origW, origH, 400, 0);
    expect(r.width).toBe(10);
    expect(r.x).toBe(origX + (origW - 10));
  });

  it("clamps top-moving handles to MIN_SIZE", () => {
    const r = computeResizedBounds(1, origX, origY, origW, origH, 0, 400);
    expect(r.height).toBe(10);
    expect(r.y).toBe(origY + (origH - 10));
  });

  it("clamps right-growing handles to MIN_SIZE", () => {
    const r = computeResizedBounds(3, origX, origY, origW, origH, -500, 0);
    expect(r.width).toBe(10);
    expect(r.x).toBe(100);
  });

  it("clamps bottom-growing handles to MIN_SIZE", () => {
    const r = computeResizedBounds(5, origX, origY, origW, origH, 0, -500);
    expect(r.height).toBe(10);
    expect(r.y).toBe(200);
  });

  // --- Shift (aspect-ratio lock) tests ---

  it("shift + corner handle 4 (bottom-right): maintains aspect ratio", () => {
    // Aspect ratio = 300/150 = 2:1
    const r = computeResizedBounds(4, origX, origY, origW, origH, 60, 10, true);
    // Both dimensions should change proportionally
    const newRatio = r.width / r.height;
    const origRatio = origW / origH;
    expect(newRatio).toBeCloseTo(origRatio, 1);
  });

  it("shift + corner handle 0 (top-left): maintains aspect ratio", () => {
    const r = computeResizedBounds(0, origX, origY, origW, origH, -40, -10, true);
    const newRatio = r.width / r.height;
    const origRatio = origW / origH;
    expect(newRatio).toBeCloseTo(origRatio, 1);
  });

  it("shift + edge handle 3 (middle-right): scales both dimensions", () => {
    const r = computeResizedBounds(3, origX, origY, origW, origH, 60, 0, true);
    // Width should grow, and height should grow proportionally
    expect(r.width).toBeGreaterThan(origW);
    expect(r.height).toBeGreaterThan(origH);
  });

  it("shift + edge handle 5 (bottom-center): scales both dimensions", () => {
    const r = computeResizedBounds(5, origX, origY, origW, origH, 0, 30, true);
    expect(r.height).toBeGreaterThan(origH);
    expect(r.width).toBeGreaterThan(origW);
  });

  it("shift resize still enforces MIN_SIZE", () => {
    const r = computeResizedBounds(0, origX, origY, origW, origH, 500, 500, true);
    expect(r.width).toBeGreaterThanOrEqual(10);
    expect(r.height).toBeGreaterThanOrEqual(10);
  });
});
