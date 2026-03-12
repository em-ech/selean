import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRotationDrag, computeRotation } from "./useRotationDrag";
import {
  createMockEditor,
  DEFAULT_CAMERA,
  makeNodeInfo,
} from "../test/mock-editor";
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

function makePointerEvent(
  x: number,
  y: number,
  opts: { shiftKey?: boolean; pointerId?: number } = {},
): React.PointerEvent {
  const target = document.createElement("div");
  return {
    clientX: x,
    clientY: y,
    pointerId: opts.pointerId ?? 1,
    shiftKey: opts.shiftKey ?? false,
    target,
    preventDefault: vi.fn(),
    stopPropagation: vi.fn(),
  } as unknown as React.PointerEvent;
}

describe("computeRotation", () => {
  it("returns 0 for pointer directly to the right of center", () => {
    const angle = computeRotation(100, 100, 200, 100);
    expect(angle).toBeCloseTo(0, 5);
  });

  it("returns PI/2 for pointer directly below center", () => {
    const angle = computeRotation(100, 100, 100, 200);
    expect(angle).toBeCloseTo(Math.PI / 2, 5);
  });

  it("returns -PI/2 for pointer directly above center", () => {
    const angle = computeRotation(100, 100, 100, 0);
    expect(angle).toBeCloseTo(-Math.PI / 2, 5);
  });

  it("returns PI for pointer directly to the left", () => {
    const angle = computeRotation(100, 100, 0, 100);
    expect(Math.abs(angle)).toBeCloseTo(Math.PI, 5);
  });
});

describe("useRotationDrag", () => {
  function setup(overrides: Record<string, unknown> = {}) {
    const node = makeNodeInfo({
      id: "node-abc",
      transform: [1, 0, 0, 1, 0, 0], // no rotation
    });
    const editor = createMockEditor({
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node)),
      ...overrides,
    });
    const editorRef = { current: editor };
    const onSceneChanged = vi.fn();
    const opts = {
      editorRef,
      bounds: [makeBounds()],
      camera: { ...DEFAULT_CAMERA } as CameraInfo,
      onSceneChanged,
    };
    const { result } = renderHook(() => useRotationDrag(opts));
    return { editor, result, onSceneChanged };
  }

  it("isRotating starts as false", () => {
    const { result } = setup();
    expect(result.current.isRotating).toBe(false);
  });

  it("begins rotation on pointer down", () => {
    const { editor, result } = setup();

    act(() => {
      result.current.rotationHandleProps.onPointerDown(
        makePointerEvent(500, 100),
      );
    });

    expect(result.current.isRotating).toBe(true);
    expect(editor.begin_group).toHaveBeenCalledWith("Rotate");
  });

  it("calls execute_tool_call on pointer move", () => {
    const { editor, result, onSceneChanged } = setup();

    act(() => {
      result.current.rotationHandleProps.onPointerDown(
        makePointerEvent(500, 100),
      );
    });

    act(() => {
      result.current.rotationHandleProps.onPointerMove(
        makePointerEvent(500, 200),
      );
    });

    expect(editor.execute_tool_call).toHaveBeenCalledWith(
      "set_rotation",
      expect.any(String),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("ends rotation on pointer up", () => {
    const { editor, result } = setup();

    act(() => {
      result.current.rotationHandleProps.onPointerDown(
        makePointerEvent(500, 100),
      );
    });

    act(() => {
      result.current.rotationHandleProps.onPointerUp(
        makePointerEvent(500, 200),
      );
    });

    expect(result.current.isRotating).toBe(false);
    expect(editor.end_group).toHaveBeenCalledTimes(1);
  });

  it("does nothing when bounds is empty", () => {
    const editor = createMockEditor();
    const editorRef = { current: editor };
    const opts = {
      editorRef,
      bounds: [] as SelectionBounds[],
      camera: { ...DEFAULT_CAMERA } as CameraInfo,
      onSceneChanged: vi.fn(),
    };
    const { result } = renderHook(() => useRotationDrag(opts));

    act(() => {
      result.current.rotationHandleProps.onPointerDown(
        makePointerEvent(100, 100),
      );
    });

    expect(result.current.isRotating).toBe(false);
    expect(editor.begin_group).not.toHaveBeenCalled();
  });

  it("does nothing when camera is null", () => {
    const editor = createMockEditor();
    const editorRef = { current: editor };
    const opts = {
      editorRef,
      bounds: [makeBounds()],
      camera: null,
      onSceneChanged: vi.fn(),
    };
    const { result } = renderHook(() => useRotationDrag(opts));

    act(() => {
      result.current.rotationHandleProps.onPointerDown(
        makePointerEvent(100, 100),
      );
    });

    expect(result.current.isRotating).toBe(false);
  });

  it("snaps to 15-degree increments when shift is held", () => {
    const { editor, result } = setup();

    act(() => {
      result.current.rotationHandleProps.onPointerDown(
        makePointerEvent(500, 100),
      );
    });

    act(() => {
      result.current.rotationHandleProps.onPointerMove(
        makePointerEvent(510, 120, { shiftKey: true }),
      );
    });

    const call = (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock
      .calls[0];
    const args = JSON.parse(call[1]);
    // Angle should be a multiple of 15
    expect(args.angle_degrees % 15).toBe(0);
  });
});
