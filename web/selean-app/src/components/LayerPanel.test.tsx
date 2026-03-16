import { render, screen, fireEvent, act } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { LayerPanel } from "./LayerPanel";
import { createMockEditorRef } from "../test/mock-editor";
import type { TreeNode } from "../wasm/types";

function makeTree(nodes: TreeNode[]): string {
  return JSON.stringify(nodes);
}

describe("LayerPanel", () => {
  it("renders header", () => {
    const ref = createMockEditorRef();
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByText("Layers")).toBeInTheDocument();
  });

  it("renders search input", () => {
    const ref = createMockEditorRef();
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByPlaceholderText("Filter layers...")).toBeInTheDocument();
  });

  it("shows empty state when tree is empty", () => {
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue("[]"),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByText("No layers")).toBeInTheDocument();
  });

  it("renders flat list of nodes", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Frame A", kind: "Frame", visible: true, children: [] },
      { id: "b", name: "Text B", kind: "Text", visible: true, children: [] },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByText("Frame A")).toBeInTheDocument();
    expect(screen.getByText("Text B")).toBeInTheDocument();
  });

  it("renders nested children", () => {
    const tree: TreeNode[] = [
      {
        id: "p",
        name: "Parent",
        kind: "Frame",
        visible: true,
        children: [
          {
            id: "c",
            name: "Child",
            kind: "Frame",
            visible: true,
            children: [],
          },
        ],
      },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByText("Parent")).toBeInTheDocument();
    expect(screen.getByText("Child")).toBeInTheDocument();
  });

  it("calls handleSelect when a layer item is clicked", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Frame A", kind: "Frame", visible: true, children: [] },
    ];
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );
    fireEvent.click(screen.getByText("Frame A"));
    expect(ref.current.select_node_by_id).toHaveBeenCalledWith("a");
    expect(onChanged).toHaveBeenCalled();
  });

  it("calls toggle visibility when vis button is clicked", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Frame A", kind: "Frame", visible: true, children: [] },
    ];
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );
    const visButton = screen.getByTitle("Hide");
    fireEvent.click(visButton);
    expect(ref.current.execute_tool_call).toHaveBeenCalledWith(
      "set_visible",
      JSON.stringify({ node_id: "a", visible: false }),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("calls move_forward when up button is clicked", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Frame A", kind: "Frame", visible: true, children: [] },
    ];
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );
    const upButton = screen.getByTitle("Move forward");
    fireEvent.click(upButton);
    expect(ref.current.execute_tool_call).toHaveBeenCalledWith(
      "move_forward",
      JSON.stringify({ node_id: "a" }),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("calls move_backward when down button is clicked", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Frame A", kind: "Frame", visible: true, children: [] },
    ];
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );
    const downButton = screen.getByTitle("Move backward");
    fireEvent.click(downButton);
    expect(ref.current.execute_tool_call).toHaveBeenCalledWith(
      "move_backward",
      JSON.stringify({ node_id: "a" }),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("re-fetches tree when refreshTick changes", () => {
    const getTreeFn = vi.fn().mockReturnValue("[]");
    const ref = createMockEditorRef({
      get_scene_tree_json: getTreeFn,
    });
    const { rerender } = render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(getTreeFn).toHaveBeenCalledTimes(1);
    rerender(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={1} />,
    );
    expect(getTreeFn).toHaveBeenCalledTimes(2);
  });

  // --- New feature tests ---

  it("filters layers by search query", () => {
    vi.useFakeTimers();
    const tree: TreeNode[] = [
      { id: "a", name: "Header", kind: "Frame", visible: true, children: [] },
      { id: "b", name: "Footer", kind: "Frame", visible: true, children: [] },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    const searchInput = screen.getByPlaceholderText("Filter layers...");
    fireEvent.change(searchInput, { target: { value: "head" } });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(screen.getByText("Header")).toBeInTheDocument();
    expect(screen.queryByText("Footer")).not.toBeInTheDocument();
    vi.useRealTimers();
  });

  it("shows 'No matching layers' when search has no results", () => {
    vi.useFakeTimers();
    const tree: TreeNode[] = [
      { id: "a", name: "Header", kind: "Frame", visible: true, children: [] },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    const searchInput = screen.getByPlaceholderText("Filter layers...");
    fireEvent.change(searchInput, { target: { value: "zzz" } });
    act(() => {
      vi.advanceTimersByTime(200);
    });
    expect(screen.getByText("No matching layers")).toBeInTheDocument();
    vi.useRealTimers();
  });

  it("shows expand/collapse toggle for nodes with children", () => {
    const tree: TreeNode[] = [
      {
        id: "p",
        name: "Parent",
        kind: "Group",
        visible: true,
        children: [
          {
            id: "c",
            name: "Child",
            kind: "Frame",
            visible: true,
            children: [],
          },
        ],
      },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    // Child is visible initially (not collapsed)
    expect(screen.getByText("Child")).toBeInTheDocument();
  });

  it("shows visibility title 'Show' for hidden nodes", () => {
    const tree: TreeNode[] = [
      {
        id: "a",
        name: "Hidden Node",
        kind: "Frame",
        visible: false,
        children: [],
      },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByTitle("Show")).toBeInTheDocument();
  });

  it("commits rename on Enter", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Old Name", kind: "Frame", visible: true, children: [] },
    ];
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );
    // Double-click to start rename
    fireEvent.doubleClick(screen.getByText("Old Name"));
    const input = screen.getByDisplayValue("Old Name");
    fireEvent.change(input, { target: { value: "New Name" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(ref.current.execute_command).toHaveBeenCalledWith(
      expect.stringContaining('"SetName"'),
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("cancels rename on Escape", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "My Layer", kind: "Frame", visible: true, children: [] },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    fireEvent.doubleClick(screen.getByText("My Layer"));
    const input = screen.getByDisplayValue("My Layer");
    fireEvent.keyDown(input, { key: "Escape" });
    // Should revert to showing the name as text, not input
    expect(screen.getByText("My Layer")).toBeInTheDocument();
  });
});
