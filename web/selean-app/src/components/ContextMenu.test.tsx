import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { ContextMenu } from "./ContextMenu";
import { createMockEditorRef } from "../test/mock-editor";
import type { NodeInfo } from "../wasm/types";

function makeClipboardRef(): React.MutableRefObject<NodeInfo | null> {
  return { current: null };
}

describe("ContextMenu", () => {
  it("renders all menu items", () => {
    const ref = createMockEditorRef();
    render(
      <ContextMenu
        x={100}
        y={200}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={() => {}}
        clipboardRef={makeClipboardRef()}
      />,
    );
    expect(screen.getByText("Copy")).toBeInTheDocument();
    expect(screen.getByText("Paste")).toBeInTheDocument();
    expect(screen.getByText("Duplicate")).toBeInTheDocument();
    expect(screen.getByText("Delete")).toBeInTheDocument();
    expect(screen.getByText("Bring to Front")).toBeInTheDocument();
    expect(screen.getByText("Bring Forward")).toBeInTheDocument();
    expect(screen.getByText("Send Backward")).toBeInTheDocument();
    expect(screen.getByText("Send to Back")).toBeInTheDocument();
    expect(screen.getByText("Group")).toBeInTheDocument();
    expect(screen.getByText("Ungroup")).toBeInTheDocument();
  });

  it("positions at specified coordinates", () => {
    const ref = createMockEditorRef();
    render(
      <ContextMenu
        x={150}
        y={250}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={() => {}}
        clipboardRef={makeClipboardRef()}
      />,
    );
    const menu = screen.getByTestId("context-menu");
    expect(menu.style.left).toBe("150px");
    expect(menu.style.top).toBe("250px");
  });

  it("calls onClose when Escape is pressed", () => {
    const onClose = vi.fn();
    const ref = createMockEditorRef();
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={onClose}
        clipboardRef={makeClipboardRef()}
      />,
    );
    fireEvent.keyDown(document, { key: "Escape" });
    expect(onClose).toHaveBeenCalled();
  });

  it("calls onClose on outside click", () => {
    const onClose = vi.fn();
    const ref = createMockEditorRef();
    render(
      <div>
        <div data-testid="outside">Outside</div>
        <ContextMenu
          x={0}
          y={0}
          editorRef={ref}
          onSceneChanged={() => {}}
          onClose={onClose}
          clipboardRef={makeClipboardRef()}
        />
      </div>,
    );
    fireEvent.mouseDown(screen.getByTestId("outside"));
    expect(onClose).toHaveBeenCalled();
  });

  it("disables Copy/Delete/Duplicate when nothing is selected", () => {
    const ref = createMockEditorRef({
      get_selected_ids: vi.fn().mockReturnValue("[]"),
    });
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={() => {}}
        clipboardRef={makeClipboardRef()}
      />,
    );
    expect(screen.getByText("Copy")).toBeDisabled();
    expect(screen.getByText("Delete")).toBeDisabled();
    expect(screen.getByText("Duplicate")).toBeDisabled();
  });

  it("enables Delete when a node is selected", () => {
    const ref = createMockEditorRef({
      get_selected_ids: vi.fn().mockReturnValue('["node-1"]'),
      get_node_json: vi.fn().mockReturnValue(
        JSON.stringify({
          id: "node-1",
          name: "Frame",
          kind: "Frame",
          x: 0,
          y: 0,
          width: 100,
          height: 100,
        }),
      ),
    });
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={() => {}}
        clipboardRef={makeClipboardRef()}
      />,
    );
    expect(screen.getByText("Delete")).not.toBeDisabled();
  });

  it("calls delete_node and onClose when Delete is clicked", () => {
    const onClose = vi.fn();
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_selected_ids: vi.fn().mockReturnValue('["node-1"]'),
      get_node_json: vi.fn().mockReturnValue(
        JSON.stringify({
          id: "node-1",
          name: "Frame",
          kind: "Frame",
          x: 0,
          y: 0,
          width: 100,
          height: 100,
        }),
      ),
    });
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={onChanged}
        onClose={onClose}
        clipboardRef={makeClipboardRef()}
      />,
    );
    fireEvent.click(screen.getByText("Delete"));
    expect(ref.current.execute_tool_call).toHaveBeenCalledWith(
      "delete_node",
      JSON.stringify({ node_id: "node-1" }),
    );
    expect(onChanged).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("enables Group when 2+ nodes are selected", () => {
    const ref = createMockEditorRef({
      get_selected_ids: vi.fn().mockReturnValue('["a","b"]'),
      get_node_json: vi.fn().mockReturnValue(
        JSON.stringify({
          id: "a",
          name: "Frame",
          kind: "Frame",
          x: 0,
          y: 0,
          width: 100,
          height: 100,
        }),
      ),
    });
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={() => {}}
        clipboardRef={makeClipboardRef()}
      />,
    );
    expect(screen.getByText("Group")).not.toBeDisabled();
    expect(screen.getByText("Ungroup")).toBeDisabled();
  });

  it("enables Ungroup when a Group is selected", () => {
    const ref = createMockEditorRef({
      get_selected_ids: vi.fn().mockReturnValue('["g"]'),
      get_node_json: vi.fn().mockReturnValue(
        JSON.stringify({
          id: "g",
          name: "Group",
          kind: "Group",
          x: 0,
          y: 0,
          width: 200,
          height: 200,
        }),
      ),
    });
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={() => {}}
        onClose={() => {}}
        clipboardRef={makeClipboardRef()}
      />,
    );
    expect(screen.getByText("Ungroup")).not.toBeDisabled();
  });

  it("calls move_to_front when Bring to Front is clicked", () => {
    const onClose = vi.fn();
    const onChanged = vi.fn();
    const ref = createMockEditorRef({
      get_selected_ids: vi.fn().mockReturnValue('["node-1"]'),
      get_node_json: vi.fn().mockReturnValue(
        JSON.stringify({
          id: "node-1",
          name: "Frame",
          kind: "Frame",
          x: 0,
          y: 0,
          width: 100,
          height: 100,
        }),
      ),
    });
    render(
      <ContextMenu
        x={0}
        y={0}
        editorRef={ref}
        onSceneChanged={onChanged}
        onClose={onClose}
        clipboardRef={makeClipboardRef()}
      />,
    );
    fireEvent.click(screen.getByText("Bring to Front"));
    expect(ref.current.execute_tool_call).toHaveBeenCalledWith(
      "move_to_front",
      JSON.stringify({ node_id: "node-1" }),
    );
    expect(onChanged).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });
});
