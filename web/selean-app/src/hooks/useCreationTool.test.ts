import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { useCreationTool } from "./useCreationTool";
import type { ToolType } from "../types/editor";
import { createMockEditor, DEFAULT_CAMERA } from "../test/mock-editor";

function makeOptions(overrides: Record<string, unknown> = {}) {
  const editor = createMockEditor({
    get_camera: vi.fn().mockReturnValue(DEFAULT_CAMERA),
    ...overrides,
  });
  return {
    editorRef: { current: editor },
    activeTool: "frame" as ToolType,
    onToolReset: vi.fn(),
    onSceneChanged: vi.fn(),
  };
}

describe("useCreationTool", () => {
  it("returns null creationHandlers when tool is select", () => {
    const opts = makeOptions();
    opts.activeTool = "select";
    const { result } = renderHook(() => useCreationTool(opts));

    expect(result.current.creationHandlers).toBeNull();
  });

  it("returns handlers when tool is frame", () => {
    const opts = makeOptions();
    opts.activeTool = "frame";
    const { result } = renderHook(() => useCreationTool(opts));

    expect(result.current.creationHandlers).not.toBeNull();
    expect(result.current.creationHandlers?.onPointerDown).toBeTypeOf(
      "function",
    );
    expect(result.current.creationHandlers?.onPointerMove).toBeTypeOf(
      "function",
    );
    expect(result.current.creationHandlers?.onPointerUp).toBeTypeOf("function");
  });

  it("returns handlers when tool is text", () => {
    const opts = makeOptions();
    opts.activeTool = "text";
    const { result } = renderHook(() => useCreationTool(opts));

    expect(result.current.creationHandlers).not.toBeNull();
  });

  it("isDragging starts as false", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useCreationTool(opts));

    expect(result.current.isDragging).toBe(false);
  });

  it("isDragging becomes true on pointerDown and false on pointerUp", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useCreationTool(opts));

    const target = document.createElement("div");
    const rect = { left: 0, top: 0, width: 100, height: 100 };
    target.getBoundingClientRect = vi.fn().mockReturnValue(rect);

    const downEvent = {
      clientX: 50,
      clientY: 50,
      pointerId: 1,
      target,
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
    };

    act(() => {
      result.current.creationHandlers?.onPointerDown(downEvent as never);
    });
    expect(result.current.isDragging).toBe(true);

    const upEvent = {
      clientX: 150,
      clientY: 150,
      pointerId: 1,
      target,
      preventDefault: vi.fn(),
      stopPropagation: vi.fn(),
    };

    act(() => {
      result.current.creationHandlers?.onPointerUp(upEvent as never);
    });
    expect(result.current.isDragging).toBe(false);
  });

  it("calls execute_tool_call with create_node for frame tool", () => {
    const opts = makeOptions();
    opts.activeTool = "frame";
    const { result } = renderHook(() => useCreationTool(opts));

    const target = document.createElement("div");
    const rect = { left: 0, top: 0, width: 1920, height: 1080 };
    target.getBoundingClientRect = vi.fn().mockReturnValue(rect);

    act(() => {
      result.current.creationHandlers?.onPointerDown({
        clientX: 100,
        clientY: 100,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    act(() => {
      result.current.creationHandlers?.onPointerUp({
        clientX: 300,
        clientY: 250,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    const editor = opts.editorRef.current;
    expect(editor.execute_tool_call).toHaveBeenCalledWith(
      "create_node",
      expect.any(String),
    );

    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.kind).toBe("Frame");
    expect(args.name).toBe("Frame");
    expect(args.width).toBeGreaterThanOrEqual(10);
    expect(args.height).toBeGreaterThanOrEqual(10);
  });

  it("calls execute_tool_call with create_node for text tool", () => {
    const opts = makeOptions();
    opts.activeTool = "text";
    const { result } = renderHook(() => useCreationTool(opts));

    const target = document.createElement("div");
    const rect = { left: 0, top: 0, width: 1920, height: 1080 };
    target.getBoundingClientRect = vi.fn().mockReturnValue(rect);

    act(() => {
      result.current.creationHandlers?.onPointerDown({
        clientX: 100,
        clientY: 100,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    act(() => {
      result.current.creationHandlers?.onPointerUp({
        clientX: 110,
        clientY: 110,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    const editor = opts.editorRef.current;
    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.kind).toBe("Text");
    expect(args.text_content).toBe("Text");
    // Text has minimum width 100, height 30
    expect(args.width).toBeGreaterThanOrEqual(100);
    expect(args.height).toBeGreaterThanOrEqual(30);
  });

  it("calls onSceneChanged and onToolReset after creation", () => {
    const opts = makeOptions();
    const { result } = renderHook(() => useCreationTool(opts));

    const target = document.createElement("div");
    const rect = { left: 0, top: 0, width: 1920, height: 1080 };
    target.getBoundingClientRect = vi.fn().mockReturnValue(rect);

    act(() => {
      result.current.creationHandlers?.onPointerDown({
        clientX: 50,
        clientY: 50,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    act(() => {
      result.current.creationHandlers?.onPointerUp({
        clientX: 200,
        clientY: 200,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    expect(opts.onSceneChanged).toHaveBeenCalled();
    expect(opts.onToolReset).toHaveBeenCalled();
  });

  it("enforces minimum size of 10x10 for frame creation", () => {
    const opts = makeOptions();
    opts.activeTool = "frame";
    const { result } = renderHook(() => useCreationTool(opts));

    const target = document.createElement("div");
    const rect = { left: 0, top: 0, width: 1920, height: 1080 };
    target.getBoundingClientRect = vi.fn().mockReturnValue(rect);

    // Click without drag (same position)
    act(() => {
      result.current.creationHandlers?.onPointerDown({
        clientX: 500,
        clientY: 500,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    act(() => {
      result.current.creationHandlers?.onPointerUp({
        clientX: 500,
        clientY: 500,
        pointerId: 1,
        target,
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as never);
    });

    const editor = opts.editorRef.current;
    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.width).toBeGreaterThanOrEqual(10);
    expect(args.height).toBeGreaterThanOrEqual(10);
  });
});
