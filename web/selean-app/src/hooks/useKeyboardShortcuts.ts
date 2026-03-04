import { useEffect } from "react";
import type { SeleanEditor, NodeInfo } from "../wasm/types";
import type { ToolType } from "../components/Toolbar";

/** Tags that should suppress single-key shortcuts. */
const INPUT_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);

export interface UseKeyboardShortcutsParams {
  editorRef: React.RefObject<SeleanEditor | null>;
  isReady: boolean;
  onSceneChanged: () => void;
  activeTool: ToolType;
  setActiveTool: (tool: ToolType) => void;
  handleToolChange: (tool: ToolType) => void;
  clipboardRef: React.MutableRefObject<NodeInfo | null>;
  onUndoRedoTick: () => void;
}

/** Creates a copy of a node at an offset position. */
export function pasteNode(
  editor: { execute_tool_call: (name: string, args: string) => string },
  node: NodeInfo,
): void {
  const args: Record<string, unknown> = {
    name: `${node.name} copy`,
    kind: node.kind,
    x: node.x + 10,
    y: node.y + 10,
    width: node.width,
    height: node.height,
  };

  if (node.fill) {
    args.fill_r = node.fill[0];
    args.fill_g = node.fill[1];
    args.fill_b = node.fill[2];
    args.fill_a = node.fill[3];
  }

  if (node.text_content) {
    args.text_content = node.text_content;
  }

  if (node.font_size) {
    args.font_size = node.font_size;
  }

  if (node.asset_ref) {
    args.asset_ref = node.asset_ref;
  }

  if (node.path_data) {
    args.path_data = node.path_data;
  }

  editor.execute_tool_call("create_node", JSON.stringify(args));
}

/**
 * Registers a global keydown listener that handles all editor keyboard
 * shortcuts: undo/redo, zoom, delete, copy/paste, grouping, z-order,
 * duplicate, arrow-nudge, escape, and single-key tool switches.
 */
export function useKeyboardShortcuts({
  editorRef,
  isReady,
  onSceneChanged,
  activeTool,
  setActiveTool,
  handleToolChange,
  clipboardRef,
  onUndoRedoTick,
}: UseKeyboardShortcutsParams): void {
  useEffect(() => {
    if (!isReady) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      const editor = editorRef.current;
      if (!editor) return;

      const tag = (e.target as HTMLElement).tagName;
      const isInputFocused = INPUT_TAGS.has(tag);
      const isCtrlOrMeta = e.ctrlKey || e.metaKey;

      // Undo: Cmd+Z
      if (isCtrlOrMeta && e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        editor.undo();
        onSceneChanged();
        onUndoRedoTick();
        return;
      }

      // Redo: Cmd+Shift+Z or Cmd+Y
      if (
        (isCtrlOrMeta && e.key === "z" && e.shiftKey) ||
        (isCtrlOrMeta && e.key === "y")
      ) {
        e.preventDefault();
        editor.redo();
        onSceneChanged();
        onUndoRedoTick();
        return;
      }

      // Zoom: Cmd+= (zoom in)
      if (isCtrlOrMeta && (e.key === "=" || e.key === "+")) {
        e.preventDefault();
        editor.zoom_by(1.25);
        onSceneChanged();
        return;
      }

      // Zoom: Cmd+- (zoom out)
      if (isCtrlOrMeta && e.key === "-") {
        e.preventDefault();
        editor.zoom_by(0.8);
        onSceneChanged();
        return;
      }

      // Zoom: Cmd+0 (fit to all)
      if (isCtrlOrMeta && e.key === "0") {
        e.preventDefault();
        editor.fit_to_all();
        onSceneChanged();
        return;
      }

      // Zoom: Cmd+1 (zoom to 100%)
      if (isCtrlOrMeta && e.key === "1") {
        e.preventDefault();
        editor.zoom_to(1.0);
        onSceneChanged();
        return;
      }

      // Skip remaining shortcuts when focused on text input
      if (isInputFocused) return;

      // Delete / Backspace: delete selected nodes
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          for (const id of ids) {
            editor.execute_tool_call(
              "delete_node",
              JSON.stringify({ node_id: id }),
            );
          }
          if (ids.length > 0) onSceneChanged();
        } catch (err) {
          console.warn("shortcut:delete failed", err);
        }
        return;
      }

      // Cmd+C: Copy
      if (isCtrlOrMeta && e.key === "c") {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length > 0) {
            const nodeJson = editor.get_node_json(ids[0]);
            if (nodeJson !== "null") {
              clipboardRef.current = JSON.parse(nodeJson);
            }
          }
        } catch (err) {
          console.warn("shortcut:copy failed", err);
        }
        return;
      }

      // Cmd+V: Paste
      if (isCtrlOrMeta && e.key === "v") {
        e.preventDefault();
        const node = clipboardRef.current;
        if (node) {
          pasteNode(editor, node);
          onSceneChanged();
        }
        return;
      }

      // Cmd+G: Group selected nodes
      if (isCtrlOrMeta && e.key === "g" && !e.shiftKey) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length >= 2) {
            editor.execute_tool_call(
              "group_nodes",
              JSON.stringify({ node_ids: ids }),
            );
            onSceneChanged();
          }
        } catch (err) {
          console.warn("shortcut:group failed", err);
        }
        return;
      }

      // Cmd+Shift+G: Ungroup selected group
      if (isCtrlOrMeta && e.key === "g" && e.shiftKey) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length === 1) {
            const nodeJson = editor.get_node_json(ids[0]);
            if (nodeJson !== "null") {
              const node: NodeInfo = JSON.parse(nodeJson);
              if (node.kind === "Group") {
                editor.execute_tool_call(
                  "ungroup_node",
                  JSON.stringify({ node_id: ids[0] }),
                );
                onSceneChanged();
              }
            }
          }
        } catch (err) {
          console.warn("shortcut:ungroup failed", err);
        }
        return;
      }

      // Cmd+]: Bring Forward
      if (isCtrlOrMeta && e.key === "]" && !e.shiftKey) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length === 1) {
            editor.execute_tool_call(
              "move_forward",
              JSON.stringify({ node_id: ids[0] }),
            );
            onSceneChanged();
          }
        } catch (err) {
          console.warn("shortcut:move-forward failed", err);
        }
        return;
      }

      // Cmd+[: Send Backward
      if (isCtrlOrMeta && e.key === "[" && !e.shiftKey) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length === 1) {
            editor.execute_tool_call(
              "move_backward",
              JSON.stringify({ node_id: ids[0] }),
            );
            onSceneChanged();
          }
        } catch (err) {
          console.warn("shortcut:move-backward failed", err);
        }
        return;
      }

      // Cmd+Shift+]: Bring to Front
      if (isCtrlOrMeta && e.key === "}" && e.shiftKey) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length === 1) {
            editor.execute_tool_call(
              "move_to_front",
              JSON.stringify({ node_id: ids[0] }),
            );
            onSceneChanged();
          }
        } catch (err) {
          console.warn("shortcut:move-to-front failed", err);
        }
        return;
      }

      // Cmd+Shift+[: Send to Back
      if (isCtrlOrMeta && e.key === "{" && e.shiftKey) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length === 1) {
            editor.execute_tool_call(
              "move_to_back",
              JSON.stringify({ node_id: ids[0] }),
            );
            onSceneChanged();
          }
        } catch (err) {
          console.warn("shortcut:move-to-back failed", err);
        }
        return;
      }

      // Cmd+D: Duplicate
      if (isCtrlOrMeta && e.key === "d") {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length > 0) {
            const nodeJson = editor.get_node_json(ids[0]);
            if (nodeJson !== "null") {
              const node: NodeInfo = JSON.parse(nodeJson);
              pasteNode(editor, node);
              onSceneChanged();
            }
          }
        } catch (err) {
          console.warn("shortcut:duplicate failed", err);
        }
        return;
      }

      // Arrow keys: Nudge selected nodes
      if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          const step = e.shiftKey ? 10 : 1;
          let dx = 0;
          let dy = 0;
          if (e.key === "ArrowLeft") dx = -step;
          if (e.key === "ArrowRight") dx = step;
          if (e.key === "ArrowUp") dy = -step;
          if (e.key === "ArrowDown") dy = step;

          for (const id of ids) {
            const json = editor.get_node_json(id);
            if (json === "null") continue;
            const node: NodeInfo = JSON.parse(json);
            editor.execute_tool_call(
              "set_bounds",
              JSON.stringify({
                node_id: id,
                x: node.x + dx,
                y: node.y + dy,
                width: node.width,
                height: node.height,
              }),
            );
          }
          if (ids.length > 0) onSceneChanged();
        } catch (err) {
          console.warn("shortcut:nudge failed", err);
        }
        return;
      }

      // Escape: Deselect all, or reset tool to select
      if (e.key === "Escape") {
        e.preventDefault();
        if (activeTool !== "select") {
          setActiveTool("select");
        } else {
          editor.clear_selection();
          onSceneChanged();
        }
        return;
      }

      // Single-key tool shortcuts (no modifiers, not in input)
      if (!isCtrlOrMeta && !e.altKey) {
        const lower = e.key.toLowerCase();
        if (lower === "v") {
          setActiveTool("select");
          return;
        }
        if (lower === "f") {
          setActiveTool("frame");
          return;
        }
        if (lower === "t") {
          setActiveTool("text");
          return;
        }
        if (lower === "i") {
          handleToolChange("image");
          return;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [
    isReady,
    editorRef,
    onSceneChanged,
    activeTool,
    setActiveTool,
    handleToolChange,
    clipboardRef,
    onUndoRedoTick,
  ]);
}
