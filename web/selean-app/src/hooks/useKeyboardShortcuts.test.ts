import { describe, it, expect, vi } from "vitest";
import { renderHook, fireEvent } from "@testing-library/react";
import { useKeyboardShortcuts } from "./useKeyboardShortcuts";
import type { ToolType } from "../types/editor";

function createMockEditor() {
  return {
    undo: vi.fn(),
    redo: vi.fn(),
    get_selected_ids: vi.fn().mockReturnValue([]),
    execute_tool_call: vi.fn().mockReturnValue("{}"),
    get_node_json: vi.fn().mockReturnValue("null"),
    clear_selection: vi.fn(),
    begin_command_group: vi.fn(),
    end_command_group: vi.fn(),
    align_nodes: vi.fn(),
    zoom_by: vi.fn(),
    zoom_to: vi.fn(),
    fit_to_all: vi.fn(),
  };
}

describe("useKeyboardShortcuts", () => {
  it("Ctrl+Z calls undo and onSceneChanged", () => {
    const mockEditor = createMockEditor();
    const onSceneChanged = vi.fn();
    const onUndoRedoTick = vi.fn();

    renderHook(() =>
      useKeyboardShortcuts({
        editorRef: { current: mockEditor } as never,
        isReady: true,
        onSceneChanged,
        activeTool: "select" as ToolType,
        setActiveTool: vi.fn(),
        handleToolChange: vi.fn(),
        clipboardRef: { current: null },
        onUndoRedoTick,
      }),
    );

    fireEvent.keyDown(window, { key: "z", ctrlKey: true });

    expect(mockEditor.undo).toHaveBeenCalled();
    expect(onSceneChanged).toHaveBeenCalled();
    expect(onUndoRedoTick).toHaveBeenCalled();
  });

  it("Delete removes selected nodes", () => {
    const mockEditor = createMockEditor();
    mockEditor.get_selected_ids.mockReturnValue(["id1"]);
    const onSceneChanged = vi.fn();

    renderHook(() =>
      useKeyboardShortcuts({
        editorRef: { current: mockEditor } as never,
        isReady: true,
        onSceneChanged,
        activeTool: "select" as ToolType,
        setActiveTool: vi.fn(),
        handleToolChange: vi.fn(),
        clipboardRef: { current: null },
        onUndoRedoTick: vi.fn(),
      }),
    );

    fireEvent.keyDown(window, { key: "Delete" });

    expect(mockEditor.execute_tool_call).toHaveBeenCalledWith(
      "delete_node",
      JSON.stringify({ node_id: "id1" }),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("Ctrl+G groups when 2+ selected", () => {
    const mockEditor = createMockEditor();
    mockEditor.get_selected_ids.mockReturnValue(["a", "b"]);
    const onSceneChanged = vi.fn();

    renderHook(() =>
      useKeyboardShortcuts({
        editorRef: { current: mockEditor } as never,
        isReady: true,
        onSceneChanged,
        activeTool: "select" as ToolType,
        setActiveTool: vi.fn(),
        handleToolChange: vi.fn(),
        clipboardRef: { current: null },
        onUndoRedoTick: vi.fn(),
      }),
    );

    fireEvent.keyDown(window, { key: "g", ctrlKey: true });

    expect(mockEditor.execute_tool_call).toHaveBeenCalledWith(
      "group_nodes",
      JSON.stringify({ node_ids: ["a", "b"] }),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("Arrow key nudge uses command group batching", () => {
    const mockEditor = createMockEditor();
    mockEditor.get_selected_ids.mockReturnValue(["id1"]);
    mockEditor.get_node_json.mockReturnValue(
      JSON.stringify({
        id: "id1",
        name: "Node",
        kind: "Rect",
        x: 100,
        y: 200,
        width: 50,
        height: 50,
      }),
    );
    const onSceneChanged = vi.fn();

    renderHook(() =>
      useKeyboardShortcuts({
        editorRef: { current: mockEditor } as never,
        isReady: true,
        onSceneChanged,
        activeTool: "select" as ToolType,
        setActiveTool: vi.fn(),
        handleToolChange: vi.fn(),
        clipboardRef: { current: null },
        onUndoRedoTick: vi.fn(),
      }),
    );

    fireEvent.keyDown(window, { key: "ArrowRight" });

    expect(mockEditor.begin_command_group).toHaveBeenCalled();
    expect(mockEditor.execute_tool_call).toHaveBeenCalledWith(
      "set_bounds",
      JSON.stringify({
        node_id: "id1",
        x: 101,
        y: 200,
        width: 50,
        height: 50,
      }),
    );
    expect(mockEditor.end_command_group).toHaveBeenCalled();
    expect(onSceneChanged).toHaveBeenCalled();
  });
});
