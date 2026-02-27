import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useSelection } from "./useSelection";
import { createMockEditor, makeNodeInfo } from "../test/mock-editor";

describe("useSelection", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns null selectedNode when not ready", () => {
    const editor = createMockEditor();
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, false));

    expect(result.current.selectedNode).toBeNull();
  });

  it("returns null when no node is selected", () => {
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue("[]"),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    // Advance past the initial poll
    act(() => {
      vi.advanceTimersByTime(200);
    });

    expect(result.current.selectedNode).toBeNull();
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
      vi.advanceTimersByTime(200);
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
      // First two calls return selected (initial poll + potential re-poll)
      return callCount <= 2 ? '["abc"]' : "[]";
    });
    const editor = createMockEditor({
      get_selected_ids: getSelectedIds,
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(node)),
    });
    const ref = { current: editor };
    const { result } = renderHook(() => useSelection(ref, true));

    // First poll: selected
    act(() => {
      vi.advanceTimersByTime(150);
    });
    expect(result.current.selectedNode).not.toBeNull();

    // Advance past the threshold so subsequent polls return empty
    act(() => {
      vi.advanceTimersByTime(300);
    });
    expect(result.current.selectedNode).toBeNull();
  });

  it("refresh forces re-fetch of node data", () => {
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

    // First poll
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(result.current.selectedNode?.name).toBe("V1");

    // Call refresh and advance
    act(() => {
      result.current.refresh();
      vi.advanceTimersByTime(200);
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
      vi.advanceTimersByTime(200);
    });
    expect(result.current.selectedNode).toBeNull();
  });

  it("does not fetch node json for same id on repeated polls", () => {
    const node = makeNodeInfo({ id: "abc" });
    const getNodeJson = vi.fn().mockReturnValue(JSON.stringify(node));
    const editor = createMockEditor({
      get_selected_ids: vi.fn().mockReturnValue('["abc"]'),
      get_node_json: getNodeJson,
    });
    const ref = { current: editor };
    renderHook(() => useSelection(ref, true));

    // First poll fetches node
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(getNodeJson).toHaveBeenCalledTimes(1);

    // Second poll with same ID skips fetch
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(getNodeJson).toHaveBeenCalledTimes(1);
  });
});
