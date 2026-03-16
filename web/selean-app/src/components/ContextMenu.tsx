import { useCallback, useEffect, useRef } from "react";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";
import type { NodeInfo, SeleanEditor } from "../wasm/types";
import { pasteNode } from "../utils/clipboard";
import { NodeKind } from "../types/editor";

interface ContextMenuProps {
  x: number;
  y: number;
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  onClose: () => void;
  clipboardRef: React.MutableRefObject<NodeInfo | null>;
}

interface MenuItem {
  label: string;
  action: () => void;
  disabled?: boolean;
  separator?: false;
}

interface Separator {
  separator: true;
}

type MenuEntry = MenuItem | Separator;

/**
 * Right-click context menu for the canvas. Positioned at click coordinates.
 * Supports Copy, Paste, Duplicate, Delete, z-order, Group, Ungroup.
 */
export function ContextMenu({
  x,
  y,
  editorRef,
  onSceneChanged,
  onClose,
  clipboardRef,
}: ContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

  // Close on outside click or Escape.
  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("mousedown", handleClick);
    document.addEventListener("keydown", handleKey);
    return () => {
      document.removeEventListener("mousedown", handleClick);
      document.removeEventListener("keydown", handleKey);
    };
  }, [onClose]);

  const editor = editorRef.current;
  const selectedIds = getSelectedIds(editor);
  const selectedNode = getFirstSelectedNode(editor, selectedIds);
  const isGroup = selectedNode?.kind === NodeKind.Group;
  const hasSelection = selectedIds.length > 0;
  const hasMultiSelection = selectedIds.length >= 2;

  const handleCopy = useCallback(() => {
    if (!editor || selectedIds.length === 0) return;
    const json = editor.get_node_json(selectedIds[0]);
    if (json !== "null") {
      clipboardRef.current = JSON.parse(json);
    }
    onClose();
  }, [editor, selectedIds, onClose]);

  const handlePaste = useCallback(() => {
    const node = clipboardRef.current;
    if (!editor || !node) return;
    pasteNode(editor, node);
    onSceneChanged();
    onClose();
  }, [editor, onSceneChanged, onClose]);

  const handleDuplicate = useCallback(() => {
    if (!editor || !selectedNode) return;
    pasteNode(editor, selectedNode);
    onSceneChanged();
    onClose();
  }, [editor, selectedNode, onSceneChanged, onClose]);

  const handleDelete = useCallback(() => {
    if (!editor) return;
    for (const id of selectedIds) {
      editor.execute_tool_call("delete_node", JSON.stringify({ node_id: id }));
    }
    if (selectedIds.length > 0) onSceneChanged();
    onClose();
  }, [editor, selectedIds, onSceneChanged, onClose]);

  const handleZOrder = useCallback(
    (tool: string) => {
      if (!editor || selectedIds.length !== 1) return;
      editor.execute_tool_call(
        tool,
        JSON.stringify({ node_id: selectedIds[0] }),
      );
      onSceneChanged();
      onClose();
    },
    [editor, selectedIds, onSceneChanged, onClose],
  );

  const handleGroup = useCallback(() => {
    if (!editor || selectedIds.length < 2) return;
    editor.execute_tool_call(
      "group_nodes",
      JSON.stringify({ node_ids: selectedIds }),
    );
    onSceneChanged();
    onClose();
  }, [editor, selectedIds, onSceneChanged, onClose]);

  const handleUngroup = useCallback(() => {
    if (!editor || selectedIds.length !== 1 || !isGroup) return;
    editor.execute_tool_call(
      "ungroup_node",
      JSON.stringify({ node_id: selectedIds[0] }),
    );
    onSceneChanged();
    onClose();
  }, [editor, selectedIds, isGroup, onSceneChanged, onClose]);

  const entries: MenuEntry[] = [
    { label: "Copy", action: handleCopy, disabled: !hasSelection },
    { label: "Paste", action: handlePaste, disabled: !clipboardRef.current },
    { label: "Duplicate", action: handleDuplicate, disabled: !hasSelection },
    { label: "Delete", action: handleDelete, disabled: !hasSelection },
    { separator: true },
    {
      label: "Bring to Front",
      action: () => handleZOrder("move_to_front"),
      disabled: selectedIds.length !== 1,
    },
    {
      label: "Bring Forward",
      action: () => handleZOrder("move_forward"),
      disabled: selectedIds.length !== 1,
    },
    {
      label: "Send Backward",
      action: () => handleZOrder("move_backward"),
      disabled: selectedIds.length !== 1,
    },
    {
      label: "Send to Back",
      action: () => handleZOrder("move_to_back"),
      disabled: selectedIds.length !== 1,
    },
    { separator: true },
    { label: "Group", action: handleGroup, disabled: !hasMultiSelection },
    {
      label: "Ungroup",
      action: handleUngroup,
      disabled: !(selectedIds.length === 1 && isGroup),
    },
  ];

  return (
    <div
      ref={menuRef}
      style={{ ...menuStyle, left: x, top: y }}
      data-testid="context-menu"
    >
      {entries.map((entry, i) =>
        "separator" in entry && entry.separator ? (
          <div key={`sep-${i}`} style={separatorStyle} />
        ) : (
          <button
            key={(entry as MenuItem).label}
            style={{
              ...itemBtnStyle,
              opacity: (entry as MenuItem).disabled ? 0.4 : 1,
              cursor: (entry as MenuItem).disabled ? "default" : "pointer",
            }}
            disabled={(entry as MenuItem).disabled}
            onClick={(entry as MenuItem).action}
          >
            {(entry as MenuItem).label}
          </button>
        ),
      )}
    </div>
  );
}

function getSelectedIds(editor: SeleanEditor | null): string[] {
  if (!editor) return [];
  try {
    return editor.get_selected_ids();
  } catch (e) {
    console.warn("context-menu:get-selected-ids failed", e);
    return [];
  }
}

function getFirstSelectedNode(
  editor: SeleanEditor | null,
  ids: string[],
): NodeInfo | null {
  if (!editor || ids.length === 0) return null;
  try {
    const json = editor.get_node_json(ids[0]);
    if (json === "null") return null;
    return JSON.parse(json);
  } catch (e) {
    console.warn("context-menu:get-node failed", e);
    return null;
  }
}

const menuStyle: React.CSSProperties = {
  position: "fixed",
  zIndex: 1000,
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.md,
  padding: `${spacing.xs}px 0`,
  minWidth: 160,
  boxShadow: shadows.lg,
};

const itemBtnStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  background: "none",
  border: "none",
  color: colors.text,
  fontSize: fontSizes.sm,
  padding: `6px ${spacing.md}px`,
  textAlign: "left",
  cursor: "pointer",
};

const separatorStyle: React.CSSProperties = {
  height: 1,
  background: colors.border,
  margin: `${spacing.xs}px 0`,
};
