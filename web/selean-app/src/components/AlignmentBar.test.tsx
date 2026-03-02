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
});
