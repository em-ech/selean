import { useCallback, useEffect, useState } from "react";
import { colors, fontSizes } from "../theme";
import type { SeleanEditor, TreeNode } from "../wasm/types";

interface LayerPanelProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  refreshTick: number;
}

/**
 * Layer panel showing the scene tree hierarchy.
 * Click to select, toggle visibility.
 */
export function LayerPanel({
  editorRef,
  onSceneChanged,
  refreshTick,
}: LayerPanelProps) {
  const [tree, setTree] = useState<TreeNode[]>([]);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor) return;
    try {
      const json = editor.get_scene_tree_json();
      setTree(JSON.parse(json));
    } catch (e) {
      console.warn("layer-panel:get-scene-tree failed", e);
    }
  }, [editorRef, refreshTick]);

  const handleSelect = useCallback(
    (nodeId: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.select_node_by_id(nodeId);
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  const handleToggleVisible = useCallback(
    (nodeId: string, currentVisible: boolean) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.execute_tool_call(
        "set_visible",
        JSON.stringify({ node_id: nodeId, visible: !currentVisible }),
      );
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  const handleMoveUp = useCallback(
    (nodeId: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.execute_tool_call(
        "move_forward",
        JSON.stringify({ node_id: nodeId }),
      );
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  const handleMoveDown = useCallback(
    (nodeId: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.execute_tool_call(
        "move_backward",
        JSON.stringify({ node_id: nodeId }),
      );
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  return (
    <div style={panelStyle}>
      <div style={headerStyle}>Layers</div>
      <div style={listStyle}>
        {tree.length === 0 && <div style={emptyStyle}>No layers</div>}
        {tree.map((node) => (
          <LayerItem
            key={node.id}
            node={node}
            depth={0}
            onSelect={handleSelect}
            onToggleVisible={handleToggleVisible}
            onMoveUp={handleMoveUp}
            onMoveDown={handleMoveDown}
          />
        ))}
      </div>
    </div>
  );
}

interface LayerItemProps {
  node: TreeNode;
  depth: number;
  onSelect: (id: string) => void;
  onToggleVisible: (id: string, current: boolean) => void;
  onMoveUp: (id: string) => void;
  onMoveDown: (id: string) => void;
}

function LayerItem({
  node,
  depth,
  onSelect,
  onToggleVisible,
  onMoveUp,
  onMoveDown,
}: LayerItemProps) {
  return (
    <>
      <div
        style={{
          ...itemStyle,
          paddingLeft: 8 + depth * 16,
          opacity: node.visible ? 1 : 0.4,
        }}
        onClick={() => onSelect(node.id)}
      >
        <span style={kindBadgeStyle}>{node.kind[0]}</span>
        <span style={nameStyle}>{node.name}</span>
        <button
          style={zOrderBtnStyle}
          title="Move forward"
          onClick={(e) => {
            e.stopPropagation();
            onMoveUp(node.id);
          }}
        >
          ^
        </button>
        <button
          style={zOrderBtnStyle}
          title="Move backward"
          onClick={(e) => {
            e.stopPropagation();
            onMoveDown(node.id);
          }}
        >
          v
        </button>
        <button
          style={visToggleStyle}
          onClick={(e) => {
            e.stopPropagation();
            onToggleVisible(node.id, node.visible);
          }}
        >
          {node.visible ? "V" : "H"}
        </button>
      </div>
      {node.children.map((child) => (
        <LayerItem
          key={child.id}
          node={child}
          depth={depth + 1}
          onSelect={onSelect}
          onToggleVisible={onToggleVisible}
          onMoveUp={onMoveUp}
          onMoveDown={onMoveDown}
        />
      ))}
    </>
  );
}

const panelStyle: React.CSSProperties = {
  width: 200,
  borderLeft: `1px solid ${colors.border}`,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

const headerStyle: React.CSSProperties = {
  height: 36,
  display: "flex",
  alignItems: "center",
  padding: "0 12px",
  fontWeight: 600,
  fontSize: fontSizes.md,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const listStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
};

const emptyStyle: React.CSSProperties = {
  padding: "20px 12px",
  color: colors.textFaint,
  fontSize: fontSizes.sm,
};

const itemStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 6,
  padding: "4px 8px",
  cursor: "pointer",
  fontSize: fontSizes.sm,
  borderBottom: `1px solid ${colors.border}`,
};

const kindBadgeStyle: React.CSSProperties = {
  width: 16,
  height: 16,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: colors.border,
  borderRadius: 2,
  fontSize: fontSizes.xs,
  fontWeight: 600,
  color: colors.textDim,
  flexShrink: 0,
};

const nameStyle: React.CSSProperties = {
  flex: 1,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const zOrderBtnStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: colors.textDim,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  padding: "2px 2px",
  flexShrink: 0,
  lineHeight: 1,
};

const visToggleStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: colors.textDim,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  padding: "2px 4px",
  flexShrink: 0,
};
