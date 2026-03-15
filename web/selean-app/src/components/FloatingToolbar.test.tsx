import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { FloatingToolbar } from "./FloatingToolbar";
import { createMockEditorRef, makeNodeInfo } from "../test/mock-editor";

// Mock CollabContext
vi.mock("../collab/CollabContext", () => ({
  CollabContext: React.createContext(null),
  useCollab: () => null,
}));

const editorRef = createMockEditorRef();
const onSceneChanged = vi.fn();

beforeEach(() => {
  onSceneChanged.mockClear();
  (editorRef.current.execute_command as ReturnType<typeof vi.fn>).mockClear();
  (editorRef.current.execute_tool_call as ReturnType<typeof vi.fn>).mockClear();
  (editorRef.current.align_nodes as ReturnType<typeof vi.fn>).mockClear();
});

describe("FloatingToolbar", () => {
  it("renders nothing when no node selected", () => {
    const { container } = render(
      <FloatingToolbar
        node={null}
        selectedIds={[]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when dragging", () => {
    const node = makeNodeInfo();
    const { container } = render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={true}
        isEditing={false}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders nothing when editing text inline", () => {
    const node = makeNodeInfo();
    const { container } = render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={true}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  it("renders toolbar when node selected", () => {
    const node = makeNodeInfo();
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.getByTitle("Opacity")).toBeInTheDocument();
    expect(screen.getByTitle("Delete")).toBeInTheDocument();
    expect(screen.getByTitle("Fill color")).toBeInTheDocument();
  });

  it("renders font size control for Text nodes", () => {
    const node = makeNodeInfo({
      kind: "Text",
      font_size: 16,
      text_content: "Hello",
    });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.getByTitle("Font size")).toBeInTheDocument();
  });

  it("does not render font size for non-Text nodes", () => {
    const node = makeNodeInfo({ kind: "Frame" });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.queryByTitle("Font size")).not.toBeInTheDocument();
  });

  it("renders alignment buttons for multi-select", () => {
    const node = makeNodeInfo();
    render(
      <FloatingToolbar
        node={node}
        selectedIds={["n1", "n2"]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.getByTitle("Align left")).toBeInTheDocument();
    expect(screen.getByTitle("Align center")).toBeInTheDocument();
    expect(screen.getByTitle("Align right")).toBeInTheDocument();
  });

  it("does not render alignment for single select", () => {
    const node = makeNodeInfo();
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.queryByTitle("Align left")).not.toBeInTheDocument();
  });

  it("calls align_nodes when alignment button clicked", () => {
    const node = makeNodeInfo();
    render(
      <FloatingToolbar
        node={node}
        selectedIds={["n1", "n2"]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    fireEvent.click(screen.getByTitle("Align left"));
    expect(editorRef.current.align_nodes).toHaveBeenCalledWith(
      JSON.stringify(["n1", "n2"]),
      "Left",
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("deletes selected nodes on delete click", () => {
    const node = makeNodeInfo();
    render(
      <FloatingToolbar
        node={node}
        selectedIds={["n1", "n2"]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    fireEvent.click(screen.getByTitle("Delete"));
    expect(editorRef.current.execute_tool_call).toHaveBeenCalledWith(
      "delete_node",
      JSON.stringify({ node_id: "n1" }),
    );
    expect(editorRef.current.execute_tool_call).toHaveBeenCalledWith(
      "delete_node",
      JSON.stringify({ node_id: "n2" }),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("commits opacity change", () => {
    const node = makeNodeInfo({ opacity: 0.5 });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    const opacityInput = screen.getByTitle("Opacity");
    fireEvent.change(opacityInput, { target: { value: "80" } });
    expect(editorRef.current.execute_command).toHaveBeenCalledWith(
      expect.stringContaining('"SetOpacity"'),
    );
    expect(onSceneChanged).toHaveBeenCalled();
  });

  it("positions using screenBounds when provided", () => {
    const node = makeNodeInfo();
    const { container } = render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
        screenBounds={{ x: 100, y: 200, width: 300, height: 150 }}
      />,
    );
    const toolbar = container.firstChild as HTMLElement;
    expect(toolbar.style.top).toBe("148px"); // 200 - 52 = 148
  });

  it("flips below selection when near top edge", () => {
    const node = makeNodeInfo();
    const { container } = render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
        screenBounds={{ x: 100, y: 20, width: 300, height: 150 }}
      />,
    );
    const toolbar = container.firstChild as HTMLElement;
    // 20 - 52 = -32 < 8, so should flip: top = 20 + 150 + 8 = 178
    expect(toolbar.style.top).toBe("178px");
  });

  it("renders nothing when selectedIds is empty", () => {
    const node = makeNodeInfo();
    const { container } = render(
      <FloatingToolbar
        node={node}
        selectedIds={[]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(container.firstChild).toBeNull();
  });

  // --- Stroke color ---

  it("renders stroke swatch", () => {
    const node = makeNodeInfo();
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.getByTitle("Stroke color")).toBeInTheDocument();
  });

  // --- Text controls ---

  it("renders font family input for Text nodes", () => {
    const node = makeNodeInfo({
      kind: "Text",
      font_size: 16,
      font_family: "Inter",
      text_content: "Hello",
    });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.getByTitle("Font family")).toBeInTheDocument();
    expect(screen.getByDisplayValue("Inter")).toBeInTheDocument();
  });

  it("renders bold and italic toggles for Text nodes", () => {
    const node = makeNodeInfo({
      kind: "Text",
      font_size: 16,
      font_weight: 400,
      font_style: "Normal",
      text_content: "Hello",
    });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    expect(screen.getByTitle("Bold")).toBeInTheDocument();
    expect(screen.getByTitle("Italic")).toBeInTheDocument();
  });

  it("toggles bold on click", () => {
    const node = makeNodeInfo({
      kind: "Text",
      font_size: 16,
      font_weight: 400,
      text_content: "Hello",
    });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    fireEvent.click(screen.getByTitle("Bold"));
    expect(editorRef.current.execute_command).toHaveBeenCalledWith(
      expect.stringContaining('"SetFontWeight"'),
    );
  });

  it("toggles italic on click", () => {
    const node = makeNodeInfo({
      kind: "Text",
      font_size: 16,
      font_style: "Normal",
      text_content: "Hello",
    });
    render(
      <FloatingToolbar
        node={node}
        selectedIds={[node.id]}
        editorRef={editorRef}
        onSceneChanged={onSceneChanged}
        isDragging={false}
        isEditing={false}
      />,
    );
    fireEvent.click(screen.getByTitle("Italic"));
    expect(editorRef.current.execute_command).toHaveBeenCalledWith(
      expect.stringContaining('"SetFontStyle"'),
    );
  });
});
