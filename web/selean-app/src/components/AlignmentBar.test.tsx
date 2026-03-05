import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { AlignmentBar } from "./AlignmentBar";
import { createMockEditorRef } from "../test/mock-editor";

describe("AlignmentBar", () => {
  it("renders when 2+ nodes are selected", () => {
    const ref = createMockEditorRef();
    render(
      <AlignmentBar
        selectedIds={["id-1", "id-2"]}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(screen.getByTitle("Left")).toBeInTheDocument();
    expect(screen.getByTitle("Right")).toBeInTheDocument();
    expect(screen.getByTitle("DistributeH")).toBeInTheDocument();
  });

  it("hidden when fewer than 2 nodes selected", () => {
    const ref = createMockEditorRef();
    const { container } = render(
      <AlignmentBar
        selectedIds={["id-1"]}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("calls align_nodes when a button is clicked", () => {
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <AlignmentBar
        selectedIds={["id-1", "id-2"]}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    fireEvent.click(screen.getByTitle("Left"));

    expect(ref.current.align_nodes).toHaveBeenCalledWith(
      JSON.stringify(["id-1", "id-2"]),
      "Left",
    );
    expect(onChanged).toHaveBeenCalled();
  });

  it("renders nothing for empty selectedIds", () => {
    const ref = createMockEditorRef();
    const { container } = render(
      <AlignmentBar
        selectedIds={[]}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    expect(container.innerHTML).toBe("");
  });

  it("renders all 8 alignment buttons", () => {
    const ref = createMockEditorRef();
    render(
      <AlignmentBar
        selectedIds={["a", "b"]}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    for (const kind of [
      "Left",
      "CenterH",
      "Right",
      "Top",
      "CenterV",
      "Bottom",
      "DistributeH",
      "DistributeV",
    ]) {
      expect(screen.getByTitle(kind)).toBeInTheDocument();
    }
  });

  it("does not throw when editor is null", () => {
    const ref = { current: null };
    render(
      <AlignmentBar
        selectedIds={["a", "b"]}
        editorRef={ref}
        onSceneChanged={() => {}}
      />,
    );
    // Should not throw on click
    fireEvent.click(screen.getByTitle("Left"));
  });

  it("passes correct kind for each alignment button", () => {
    const ref = createMockEditorRef();
    const onChanged = vi.fn();
    render(
      <AlignmentBar
        selectedIds={["a", "b"]}
        editorRef={ref}
        onSceneChanged={onChanged}
      />,
    );

    fireEvent.click(screen.getByTitle("DistributeV"));
    expect(ref.current.align_nodes).toHaveBeenCalledWith(
      JSON.stringify(["a", "b"]),
      "DistributeV",
    );
  });
});
