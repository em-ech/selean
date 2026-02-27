import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { LayerPanel } from "./LayerPanel";
import {
  createMockEditorRef,
  makeNodeInfo,
  DEFAULT_CAMERA,
} from "../test/mock-editor";
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

  it("shows kind badge with first letter", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Rect", kind: "Frame", visible: true, children: [] },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    expect(screen.getByText("F")).toBeInTheDocument(); // First letter of "Frame"
  });

  it("shows V for visible nodes and H for hidden nodes", () => {
    const tree: TreeNode[] = [
      { id: "a", name: "Visible", kind: "Frame", visible: true, children: [] },
      { id: "b", name: "Hidden", kind: "Frame", visible: false, children: [] },
    ];
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={() => {}} refreshTick={0} />,
    );
    const buttons = screen.getAllByRole("button");
    // First button: "V" for visible, second: "H" for hidden
    expect(buttons[0]).toHaveTextContent("V");
    expect(buttons[1]).toHaveTextContent("H");
  });

  it("calls handleSelect when a layer item is clicked", () => {
    const nodeInfo = makeNodeInfo({
      id: "a",
      x: 100,
      y: 100,
      width: 50,
      height: 50,
    });
    const tree: TreeNode[] = [
      { id: "a", name: "Frame A", kind: "Frame", visible: true, children: [] },
    ];
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_scene_tree_json: vi.fn().mockReturnValue(makeTree(tree)),
      get_node_json: vi.fn().mockReturnValue(JSON.stringify(nodeInfo)),
      get_camera_json: vi.fn().mockReturnValue(JSON.stringify(DEFAULT_CAMERA)),
    });
    render(
      <LayerPanel editorRef={ref} onSceneChanged={onChanged} refreshTick={0} />,
    );

    fireEvent.click(screen.getByText("Frame A"));
    expect(ref.current.on_pointer_down).toHaveBeenCalled();
    expect(ref.current.on_pointer_up).toHaveBeenCalled();
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

    const visButton = screen.getByText("V");
    fireEvent.click(visButton);
    expect(ref.current.execute_tool_call).toHaveBeenCalledWith(
      "set_visible",
      JSON.stringify({ node_id: "a", visible: false }),
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
});
