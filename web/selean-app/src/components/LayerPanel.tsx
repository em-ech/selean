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
    } catch {
      // WASM not ready
    }
  }, [editorRef, refreshTick]);

  const handleSelect = useCallback(
    (nodeId: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      // Use a click on the node's position to select it.
      // For now, get the node info and use execute_tool_call to set selection
      // indirectly. The real selection happens via pointer events on canvas.
      // For the layer panel, we'll need a direct selection API.
      // Workaround: use get_node_json to find bounds, then simulate click.
      const nodeJson = editor.get_node_json(nodeId);
      if (nodeJson === "null") return;
      const node = JSON.parse(nodeJson);
      // Simulate a click at the node's center position
      const cx = node.x + node.width / 2;
      const cy = node.y + node.height / 2;
      // Convert world to screen using camera
      try {
        const cam = JSON.parse(editor.get_camera_json());
        const sx = (cx - cam.pan_x) * cam.zoom + cam.viewport_width / 2;
        const sy = (cy - cam.pan_y) * cam.zoom + cam.viewport_height / 2;
        editor.on_pointer_down(sx, sy, 0, false, false, false, false);
        editor.on_pointer_up(sx, sy, 0, false, false, false, false);
        onSceneChanged();
      } catch {
        // Camera query failed
      }
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
}

function LayerItem({ node, depth, onSelect, onToggleVisible }: LayerItemProps) {
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

const visToggleStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: colors.textDim,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  padding: "2px 4px",
  flexShrink: 0,
};
