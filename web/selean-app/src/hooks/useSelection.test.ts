import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { useSelection } from "./useSelection";
import { createMockEditor, makeNodeInfo } from "../test/mock-editor";

describe("useSelection", () => {
  it("returns null selectedNode when not ready", () => {
    const editor = createMockEditor();
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, false));

    expect(result.current.selectedNode).toBeNull();
    expect(result.current.selectedIds).toEqual([]);
  });

  it("returns null when no node is selected after refresh", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue("[]"),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    act(() => {
      result.current.refresh();
    });

    expect(result.current.selectedNode).toBeNull();
    expect(result.current.selectedIds).toEqual([]);
  });

  it("returns NodeInfo when a node is selected", () => {
    const node = makeNodeInfo({ id: "abc", name: "Selected" });
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue('["abc"]'),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node)),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    act(() => {
      result.current.refresh();
    });

    expect(result.current.selectedNode).not.toBeNull();
    expect(result.current.selectedNode?.id).toBe("abc");
    expect(result.current.selectedNode?.name).toBe("Selected");
  });

  it("clears selectedNode when selection becomes empty", () => {
    const node = makeNodeInfo({ id: "abc" });
    let callCount = 0;
    const getSelectedIds = vi.fn().mockImplementation(() => {
      callCount++;
      return callCount <= 1 ? '["abc"]' : "[]";
    });
    const editor = createMockEditor({
      get_selected_ids: getSelectedIds,
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node)),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    // First refresh: selected
    act(() => {
      result.current.refresh();
    });
    expect(result.current.selectedNode).not.toBeNull();

    // Second refresh: deselected
    act(() => {
      result.current.refresh();
    });
    expect(result.current.selectedNode).toBeNull();
  });

  it("refresh re-reads node properties even for same id", () => {
    const node1 = makeNodeInfo({ id: "abc", name: "V1" });
    const node2 = makeNodeInfo({ id: "abc", name: "V2" });
    const getNodeJson = vi
      .fn()
      .mockReturnValueOnce(JSON.stringify(node1))
      .mockReturnValue(JSON.stringify(node2));
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue('["abc"]'),
      get_node_json: getNodeJson,
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    act(() => {
      result.current.refresh();
    });
    expect(result.current.selectedNode?.name).toBe("V1");

    act(() => {
      result.current.refresh();
    });
    expect(result.current.selectedNode?.name).toBe("V2");
  });

  it("handles WASM call failure gracefully", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockImplementation(() => {
        throw new Error("WASM not ready");
      }),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    // Should not throw
    act(() => {
      result.current.refresh();
    });
    expect(result.current.selectedNode).toBeNull();
  });

  it("refresh is a no-op when not ready", () => {
    const editor = createMockEditor();
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, false));

    act(() => {
      result.current.refresh();
    });

    expect(editor.get_selected_ids).not.toHaveBeenCalled();
    expect(result.current.selectedNode).toBeNull();
  });

  it("refresh is a no-op when editor is null", () => {
    const ref = { current: null };
    const { result } = renderHook(() => useSelection(ref, true));

    act(() => {
      result.current.refresh();
    });

    expect(result.current.selectedNode).toBeNull();
  });

  it("returns selectedIds array", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue('["a","b","c"]'),
      get_node_json: vi
        .fn()
        .mockReturnValue(JSON.stringify(makeNodeInfo({ id: "a" }))),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    act(() => {
      result.current.refresh();
    });

    expect(result.current.selectedIds).toEqual(["a", "b", "c"]);
  });

  it("handles get_node_json returning 'null'", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue('["deleted-node"]'),
      get_node_json: vi.fn().mockReturnValue("null"),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    act(() => {
      result.current.refresh();
    });

    expect(result.current.selectedNode).toBeNull();
    expect(result.current.selectedIds).toEqual(["deleted-node"]);
  });
});
