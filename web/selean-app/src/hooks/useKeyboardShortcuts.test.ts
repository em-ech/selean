import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, fireEvent } from "@testing-library/react";
import {
  useKeyboardShortcuts,
  pasteNode,
  type UseKeyboardShortcutsParams,
} from "./useKeyboardShortcuts";
import { createMockEditor, makeNodeInfo } from "../test/mock-editor";
import type { SeleanEditor, NodeInfo } from "../wasm/types";

function setup(
  editorOverrides: Partial<Record<keyof SeleanEditor, unknown>> = {},
  hookOverrides: Partial<UseKeyboardShortcutsParams> = {},
) {
  const editor = createMockEditor(editorOverrides);
  const editorRef = { current: editor };
  const onSceneChanged = vi.fn();
  const setActiveTool = vi.fn();
  const handleToolChange = vi.fn();
  const clipboardRef: React.MutableRefObject<NodeInfo | null> = {
    current: null,
  };
  const onUndoRedoTick = vi.fn();

  const params: UseKeyboardShortcutsParams = {
    editorRef,
    isReady: true,
    onSceneChanged,
    activeTool: "select",
    setActiveTool,
    handleToolChange,
    clipboardRef,
    onUndoRedoTick,
    ...hookOverrides,
  };

  renderHook(() => useKeyboardShortcuts(params));

  return {
    editor,
    editorRef,
    onSceneChanged,
    setActiveTool,
    handleToolChange,
    clipboardRef,
    onUndoRedoTick,
  };
}

/** Fire a keydown on window with the given options. */
function press(key: string, opts: Partial<KeyboardEventInit> = {}) {
  fireEvent.keyDown(window, { key, ...opts });
}

const node1 = makeNodeInfo({
  id: "node-1",
  name: "Rectangle",
  kind: "Frame",
  x: 100,
  y: 200,
  width: 300,
  height: 150,
  fill: [0.5, 0.5, 0.5, 1],
});

const textNode = makeNodeInfo({
  id: "text-1",
  name: "Title",
  kind: "Text",
  x: 50,
  y: 50,
  width: 200,
  height: 40,
  text_content: "Hello",
  font_size: 24,
});

const groupNode = makeNodeInfo({
  id: "group-1",
  name: "Group 1",
  kind: "Group",
  x: 0,
  y: 0,
  width: 500,
  height: 500,
  children: ["node-1", "text-1"],
});

describe("pasteNode", () => {
  it("creates a node at +10 offset with 'copy' suffix", () => {
    const editor = createMockEditor();
    pasteNode(editor, node1);

    expect(editor.execute_tool_call).toHaveBeenCalledWith(
      "create_node",
      expect.any(String),
    );

    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.name).toBe("Rectangle copy");
    expect(args.kind).toBe("Frame");
    expect(args.x).toBe(110);
    expect(args.y).toBe(210);
    expect(args.width).toBe(300);
    expect(args.height).toBe(150);
    expect(args.fill_r).toBe(0.5);
    expect(args.fill_g).toBe(0.5);
    expect(args.fill_b).toBe(0.5);
    expect(args.fill_a).toBe(1);
  });

  it("includes text_content and font_size for text nodes", () => {
    const editor = createMockEditor();
    pasteNode(editor, textNode);

    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.text_content).toBe("Hello");
    expect(args.font_size).toBe(24);
  });

  it("includes asset_ref for image nodes", () => {
    const imageNode = makeNodeInfo({
      kind: "Image",
      asset_ref: "img_123",
    });
    const editor = createMockEditor();
    pasteNode(editor, imageNode);

    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.asset_ref).toBe("img_123");
  });

  it("includes path_data for vector nodes", () => {
    const vecNode = makeNodeInfo({
      kind: "Vector",
      path_data: "M0 0 L100 100",
    });
    const editor = createMockEditor();
    pasteNode(editor, vecNode);

    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.path_data).toBe("M0 0 L100 100");
  });

  it("omits fill fields when fill is null", () => {
    const noFillNode = makeNodeInfo({ fill: null });
    const editor = createMockEditor();
    pasteNode(editor, noFillNode);

    const args = JSON.parse(
      (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
    );
    expect(args.fill_r).toBeUndefined();
  });
});

describe("useKeyboardShortcuts", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  describe("undo/redo", () => {
    it("Cmd+Z calls undo and onSceneChanged", () => {
      const { editor, onSceneChanged, onUndoRedoTick } = setup();
      press("z", { metaKey: true });

      expect(editor.undo).toHaveBeenCalledOnce();
      expect(onSceneChanged).toHaveBeenCalledOnce();
      expect(onUndoRedoTick).toHaveBeenCalledOnce();
    });

    it("Ctrl+Z calls undo (Windows/Linux)", () => {
      const { editor, onSceneChanged } = setup();
      press("z", { ctrlKey: true });

      expect(editor.undo).toHaveBeenCalledOnce();
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+Shift+Z calls redo and onSceneChanged", () => {
      const { editor, onSceneChanged, onUndoRedoTick } = setup();
      press("z", { metaKey: true, shiftKey: true });

      expect(editor.redo).toHaveBeenCalledOnce();
      expect(onSceneChanged).toHaveBeenCalledOnce();
      expect(onUndoRedoTick).toHaveBeenCalledOnce();
    });

    it("Cmd+Y calls redo", () => {
      const { editor } = setup();
      press("y", { metaKey: true });

      expect(editor.redo).toHaveBeenCalledOnce();
    });
  });

  describe("delete", () => {
    it("Delete key deletes selected nodes", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi
          .fn()
          .mockReturnValue(JSON.stringify(["node-1", "node-2"])),
      });

      press("Delete");

      expect(editor.execute_tool_call).toHaveBeenCalledTimes(2);
      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "delete_node",
        JSON.stringify({ node_id: "node-1" }),
      );
      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "delete_node",
        JSON.stringify({ node_id: "node-2" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Backspace key deletes selected nodes", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      press("Backspace");

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "delete_node",
        JSON.stringify({ node_id: "node-1" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("does not call onSceneChanged when no nodes selected", () => {
      const { onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue("[]"),
      });

      press("Delete");

      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("copy/paste", () => {
    it("Cmd+C copies first selected node to clipboard", () => {
      const { clipboardRef } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("c", { metaKey: true });

      expect(clipboardRef.current).toEqual(node1);
    });

    it("Cmd+C does nothing when no selection", () => {
      const { clipboardRef } = setup({
        get_selected_ids: vi.fn().mockReturnValue("[]"),
      });

      press("c", { metaKey: true });

      expect(clipboardRef.current).toBeNull();
    });

    it("Cmd+V pastes from clipboard", () => {
      const { editor, onSceneChanged, clipboardRef } = setup();

      clipboardRef.current = node1;
      press("v", { metaKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "create_node",
        expect.any(String),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+V does nothing when clipboard empty", () => {
      const { editor, onSceneChanged } = setup();

      press("v", { metaKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("grouping", () => {
    it("Cmd+G groups when >= 2 nodes selected", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi
          .fn()
          .mockReturnValue(JSON.stringify(["node-1", "node-2"])),
      });

      press("g", { metaKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "group_nodes",
        JSON.stringify({ node_ids: ["node-1", "node-2"] }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+G does nothing with fewer than 2 nodes", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      press("g", { metaKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });

    it("Cmd+Shift+G ungroups a Group node", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["group-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(groupNode)),
      });

      press("g", { metaKey: true, shiftKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "ungroup_node",
        JSON.stringify({ node_id: "group-1" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+Shift+G does nothing for non-Group node", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("g", { metaKey: true, shiftKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });

    it("Cmd+Shift+G does nothing with multiple selections", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi
          .fn()
          .mockReturnValue(JSON.stringify(["node-1", "node-2"])),
      });

      press("g", { metaKey: true, shiftKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("z-order", () => {
    it("Cmd+] calls move_forward", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      press("]", { metaKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "move_forward",
        JSON.stringify({ node_id: "node-1" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+[ calls move_backward", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      press("[", { metaKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "move_backward",
        JSON.stringify({ node_id: "node-1" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+Shift+] calls move_to_front", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      press("}", { metaKey: true, shiftKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "move_to_front",
        JSON.stringify({ node_id: "node-1" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+Shift+[ calls move_to_back", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      press("{", { metaKey: true, shiftKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "move_to_back",
        JSON.stringify({ node_id: "node-1" }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("z-order shortcuts do nothing with multiple selections", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi
          .fn()
          .mockReturnValue(JSON.stringify(["node-1", "node-2"])),
      });

      press("]", { metaKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("duplicate", () => {
    it("Cmd+D duplicates the selected node", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("d", { metaKey: true });

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "create_node",
        expect.any(String),
      );
      const args = JSON.parse(
        (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
      );
      expect(args.name).toBe("Rectangle copy");
      expect(args.x).toBe(110);
      expect(args.y).toBe(210);
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+D does nothing with no selection", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue("[]"),
      });

      press("d", { metaKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });

    it("Cmd+D does nothing when node JSON is null", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue("null"),
      });

      press("d", { metaKey: true });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("arrow key nudge", () => {
    it("ArrowRight nudges by 1px", () => {
      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("ArrowRight");

      expect(editor.execute_tool_call).toHaveBeenCalledWith(
        "set_bounds",
        JSON.stringify({
          node_id: "node-1",
          x: 101,
          y: 200,
          width: 300,
          height: 150,
        }),
      );
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("ArrowLeft nudges by -1px", () => {
      const { editor } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("ArrowLeft");

      const args = JSON.parse(
        (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
      );
      expect(args.x).toBe(99);
      expect(args.y).toBe(200);
    });

    it("ArrowUp nudges by -1px on y", () => {
      const { editor } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("ArrowUp");

      const args = JSON.parse(
        (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
      );
      expect(args.x).toBe(100);
      expect(args.y).toBe(199);
    });

    it("ArrowDown nudges by +1px on y", () => {
      const { editor } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("ArrowDown");

      const args = JSON.parse(
        (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
      );
      expect(args.x).toBe(100);
      expect(args.y).toBe(201);
    });

    it("Shift+Arrow nudges by 10px", () => {
      const { editor } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("ArrowRight", { shiftKey: true });

      const args = JSON.parse(
        (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
      );
      expect(args.x).toBe(110);
      expect(args.y).toBe(200);
    });

    it("Shift+ArrowUp nudges by -10px on y", () => {
      const { editor } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
        get_node_json: vi.fn().mockReturnValue(JSON.stringify(node1)),
      });

      press("ArrowUp", { shiftKey: true });

      const args = JSON.parse(
        (editor.execute_tool_call as ReturnType<typeof vi.fn>).mock.calls[0][1],
      );
      expect(args.x).toBe(100);
      expect(args.y).toBe(190);
    });

    it("nudges multiple selected nodes", () => {
      const node2 = makeNodeInfo({
        id: "node-2",
        x: 400,
        y: 100,
        width: 200,
        height: 200,
      });

      const { editor, onSceneChanged } = setup({
        get_selected_ids: vi
          .fn()
          .mockReturnValue(JSON.stringify(["node-1", "node-2"])),
        get_node_json: vi.fn().mockImplementation((id: string) => {
          if (id === "node-1") return JSON.stringify(node1);
          if (id === "node-2") return JSON.stringify(node2);
          return "null";
        }),
      });

      press("ArrowRight");

      expect(editor.execute_tool_call).toHaveBeenCalledTimes(2);
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("does not call onSceneChanged when no nodes selected", () => {
      const { onSceneChanged } = setup({
        get_selected_ids: vi.fn().mockReturnValue("[]"),
      });

      press("ArrowRight");

      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("escape", () => {
    it("resets tool to select when non-select tool active", () => {
      const { setActiveTool, onSceneChanged } = setup(
        {},
        { activeTool: "frame" },
      );

      press("Escape");

      expect(setActiveTool).toHaveBeenCalledWith("select");
      expect(onSceneChanged).not.toHaveBeenCalled();
    });

    it("clears selection when select tool active", () => {
      const { editor, onSceneChanged, setActiveTool } = setup(
        {},
        { activeTool: "select" },
      );

      press("Escape");

      expect(editor.clear_selection).toHaveBeenCalledOnce();
      expect(onSceneChanged).toHaveBeenCalledOnce();
      expect(setActiveTool).not.toHaveBeenCalled();
    });
  });

  describe("tool shortcuts", () => {
    it("v key activates select tool", () => {
      const { setActiveTool } = setup();
      press("v");
      expect(setActiveTool).toHaveBeenCalledWith("select");
    });

    it("f key activates frame tool", () => {
      const { setActiveTool } = setup();
      press("f");
      expect(setActiveTool).toHaveBeenCalledWith("frame");
    });

    it("t key activates text tool", () => {
      const { setActiveTool } = setup();
      press("t");
      expect(setActiveTool).toHaveBeenCalledWith("text");
    });

    it("i key triggers handleToolChange('image')", () => {
      const { handleToolChange } = setup();
      press("i");
      expect(handleToolChange).toHaveBeenCalledWith("image");
    });

    it("uppercase V also activates select tool", () => {
      const { setActiveTool } = setup();
      press("V");
      expect(setActiveTool).toHaveBeenCalledWith("select");
    });

    it("tool shortcuts do not fire with meta key held", () => {
      const { setActiveTool, handleToolChange } = setup();
      press("f", { metaKey: true });

      // 'f' with meta should not trigger tool switch (no browser default
      // for Cmd+F is handled here, but the key combo doesn't match tool logic)
      expect(setActiveTool).not.toHaveBeenCalled();
      expect(handleToolChange).not.toHaveBeenCalled();
    });
  });

  describe("zoom shortcuts", () => {
    it("Cmd+= zooms in", () => {
      const { editor, onSceneChanged } = setup();
      press("=", { metaKey: true });

      expect(editor.zoom_by).toHaveBeenCalledWith(1.25);
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd++ zooms in", () => {
      const { editor, onSceneChanged } = setup();
      press("+", { metaKey: true });

      expect(editor.zoom_by).toHaveBeenCalledWith(1.25);
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+- zooms out", () => {
      const { editor, onSceneChanged } = setup();
      press("-", { metaKey: true });

      expect(editor.zoom_by).toHaveBeenCalledWith(0.8);
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+0 fits to all", () => {
      const { editor, onSceneChanged } = setup();
      press("0", { metaKey: true });

      expect(editor.fit_to_all).toHaveBeenCalledOnce();
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });

    it("Cmd+1 zooms to 100%", () => {
      const { editor, onSceneChanged } = setup();
      press("1", { metaKey: true });

      expect(editor.zoom_to).toHaveBeenCalledWith(1.0);
      expect(onSceneChanged).toHaveBeenCalledOnce();
    });
  });

  describe("input suppression", () => {
    it("suppresses single-key shortcuts when INPUT focused", () => {
      const { setActiveTool } = setup();

      const input = document.createElement("input");
      document.body.appendChild(input);
      input.focus();

      fireEvent.keyDown(input, { key: "v" });

      expect(setActiveTool).not.toHaveBeenCalled();
      document.body.removeChild(input);
    });

    it("suppresses Delete when TEXTAREA focused", () => {
      const { editor } = setup({
        get_selected_ids: vi.fn().mockReturnValue(JSON.stringify(["node-1"])),
      });

      const textarea = document.createElement("textarea");
      document.body.appendChild(textarea);
      textarea.focus();

      fireEvent.keyDown(textarea, { key: "Delete" });

      expect(editor.execute_tool_call).not.toHaveBeenCalled();
      document.body.removeChild(textarea);
    });

    it("allows Cmd+Z even when input focused", () => {
      const { editor } = setup();

      const input = document.createElement("input");
      document.body.appendChild(input);
      input.focus();

      fireEvent.keyDown(input, { key: "z", metaKey: true });

      expect(editor.undo).toHaveBeenCalledOnce();
      document.body.removeChild(input);
    });

    it("allows zoom shortcuts when input focused", () => {
      const { editor } = setup();

      const input = document.createElement("input");
      document.body.appendChild(input);
      input.focus();

      fireEvent.keyDown(input, { key: "=", metaKey: true });

      expect(editor.zoom_by).toHaveBeenCalledWith(1.25);
      document.body.removeChild(input);
    });
  });

  describe("isReady guard", () => {
    it("does not register listener when not ready", () => {
      const editor = createMockEditor();
      const editorRef = { current: editor };
      const onSceneChanged = vi.fn();
      const clipboardRef: React.MutableRefObject<NodeInfo | null> = {
        current: null,
      };

      renderHook(() =>
        useKeyboardShortcuts({
          editorRef,
          isReady: false,
          onSceneChanged,
          activeTool: "select",
          setActiveTool: vi.fn(),
          handleToolChange: vi.fn(),
          clipboardRef,
          onUndoRedoTick: vi.fn(),
        }),
      );

      press("z", { metaKey: true });

      expect(editor.undo).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("null editor ref", () => {
    it("does nothing when editor ref is null", () => {
      const editorRef = { current: null };
      const onSceneChanged = vi.fn();
      const clipboardRef: React.MutableRefObject<NodeInfo | null> = {
        current: null,
      };

      renderHook(() =>
        useKeyboardShortcuts({
          editorRef: editorRef as React.RefObject<null>,
          isReady: true,
          onSceneChanged,
          activeTool: "select",
          setActiveTool: vi.fn(),
          handleToolChange: vi.fn(),
          clipboardRef,
          onUndoRedoTick: vi.fn(),
        }),
      );

      press("z", { metaKey: true });

      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });

  describe("cleanup", () => {
    it("unregisters listener on unmount", () => {
      const editor = createMockEditor();
      const editorRef = { current: editor };
      const onSceneChanged = vi.fn();
      const clipboardRef: React.MutableRefObject<NodeInfo | null> = {
        current: null,
      };

      const { unmount } = renderHook(() =>
        useKeyboardShortcuts({
          editorRef,
          isReady: true,
          onSceneChanged,
          activeTool: "select",
          setActiveTool: vi.fn(),
          handleToolChange: vi.fn(),
          clipboardRef,
          onUndoRedoTick: vi.fn(),
        }),
      );

      unmount();

      press("z", { metaKey: true });

      expect(editor.undo).not.toHaveBeenCalled();
      expect(onSceneChanged).not.toHaveBeenCalled();
    });
  });
});
