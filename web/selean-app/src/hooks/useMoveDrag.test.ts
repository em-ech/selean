import { describe, it, expect, vi } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useMoveDrag, applyMoveDelta } from "./useMoveDrag";
import { createMockEditor, makeNodeInfo } from "../test/mock-editor";
import type { SeleanEditor } from "../wasm/types";

function setup(
  editorOverrides: Partial<Record<keyof SeleanEditor, unknown>> = {},
) {
  const editor = createMockEditor(editorOverrides);
  const editorRef = { current: editor };
  const onSceneChanged = vi.fn();
  const { result } = renderHook(() =>
    useMoveDrag({ editorRef, onSceneChanged }),
  );
  return { editor, editorRef, onSceneChanged, result };
}

describe("applyMoveDelta", () => {
  it("offsets position by delta without changing size", () => {
    const bounds = { x: 100, y: 200, width: 300, height: 150 };
    const result = applyMoveDelta(bounds, 10, -20);
    expect(result).toEqual({ x: 110, y: 180, width: 300, height: 150 });
  });

  it("handles zero deltas as no-op", () => {
    const bounds = { x: 50, y: 50, width: 100, height: 100 };
    const result = applyMoveDelta(bounds, 0, 0);
    expect(result).toEqual(bounds);
  });

  it("handles negative positions", () => {
    const bounds = { x: -10, y: -20, width: 50, height: 50 };
    const result = applyMoveDelta(bounds, -5, -5);
    expect(result).toEqual({ x: -15, y: -25, width: 50, height: 50 });
  });
});

describe("useMoveDrag", () => {
  const node1 = makeNodeInfo({
    id: "node-1",
    x: 100,
    y: 200,
    width: 300,
    height: 150,
  });
  const node2 = makeNodeInfo({
    id: "node-2",
    x: 400,
    y: 100,
    width: 200,
    height: 200,
  });

  it("begins a group on DragStarted with selected nodes", () => {
    const { editor, result } = setup({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    expect(editor.begin_group).toHaveBeenCalledWith("Move");
  });

  it("does not begin group when no nodes selected", () => {
    const { editor, result } = setup({
      get_selected_ids: vi.fn().mockReturnValue([]),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    expect(editor.begin_group).not.toHaveBeenCalled();
  });

  it("moves a single node on DragMoved", () => {
    const { editor, result, onSceneChanged } = setup({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 10,
        delta_y: 5,
      });
    });

    expect(editor.execute_command).toHaveBeenCalledWith(
      expect.stringContaining('"x":110'),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("moves multiple selected nodes on DragMoved", () => {
    const { editor, result } = setup({
      get_selected_ids: vi.fn().mockReturnValue(["node-1", "node-2"]),
      get_node_json: vi.fn().mockImplementation((id: string) => {
        if (id === "node-1") return JSON.stringify(node1);
        if (id === "node-2") return JSON.stringify(node2);
        return "null";
      }),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 5,
        delta_y: 10,
      });
    });

    expect(editor.execute_command).toHaveBeenCalledTimes(2);
  });

  it("skips zero-delta DragMoved", () => {
    const { editor, result, onSceneChanged } = setup({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 0,
        delta_y: 0,
      });
    });

    expect(editor.execute_command).not.toHaveBeenCalled();
    expect(onSceneChanged).not.toHaveBeenCalled();
  });

  it("ends group on DragEnded", () => {
    const { editor, result, onSceneChanged } = setup({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragEnded" });
    });

    expect(editor.end_group).toHaveBeenCalled();
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("ignores DragEnded without prior DragStarted", () => {
    const { editor, result } = setup();

    act(() => {
      result.current.handleDragEvent({ type: "DragEnded" });
    });

    expect(editor.end_group).not.toHaveBeenCalled();
  });

  it("ignores DragMoved without prior DragStarted", () => {
    const { editor, result } = setup({
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });

    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 10,
        delta_y: 10,
      });
    });

    expect(editor.execute_command).not.toHaveBeenCalled();
  });

  it("handles missing node gracefully during DragMoved", () => {
    const { editor, result } = setup({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue("null"),
    });

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 10,
        delta_y: 10,
      });
    });

    // Should not crash; no tool call for missing node
    expect(editor.execute_command).not.toHaveBeenCalled();
  });

  it("handles no editor ref gracefully", () => {
    const editorRef = { current: null };
    const onSceneChanged = vi.fn();
    const { result } = renderHook(() =>
      useMoveDrag({ editorRef, onSceneChanged }),
    );

    // Should not throw
    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });
    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 10,
        delta_y: 10,
      });
    });
    act(() => {
      result.current.handleDragEvent({ type: "DragEnded" });
    });

    expect(onSceneChanged).not.toHaveBeenCalled();
  });

  it("isDragging is true during drag and false after", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });
    const editorRef = { current: editor };
    const onSceneChanged = vi.fn();
    const { result } = renderHook(() =>
      useMoveDrag({ editorRef, onSceneChanged }),
    );

    expect(result.current.isDragging).toBe(false);

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });
    expect(result.current.isDragging).toBe(true);

    act(() => {
      result.current.handleDragEvent({ type: "DragEnded" });
    });
    expect(result.current.isDragging).toBe(false);
  });

  it("shift axis-lock constrains to dominant axis", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue(["node-1"]),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
    });
    const editorRef = { current: editor };
    const onSceneChanged = vi.fn();
    const shiftKeyRef = { current: true };
    const { result } = renderHook(() =>
      useMoveDrag({ editorRef, onSceneChanged, shiftKeyRef }),
    );

    act(() => {
      result.current.handleDragEvent({ type: "DragStarted" });
    });

    // Move mostly horizontal (dx=20, dy=5) with shift held
    act(() => {
      result.current.handleDragEvent({
        type: "DragMoved",
        delta_x: 20,
        delta_y: 5,
      });
    });

    // With shift held and accDx > accDy, dy should be zeroed.
    // So x should change by 20, y should stay at 200 (original).
    const calls = (editor.execute_command as ReturnType<typeof vi.fn>).mock
      .calls;
    expect(calls.length).toBe(1);
    const cmd = JSON.parse(calls[0][0]);
    expect(cmd.bounds.x).toBe(120); // 100 + 20
    expect(cmd.bounds.y).toBe(200); // unchanged (dy zeroed)
  });
});
