import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { colors, fontSizes, radii, spacing } from "../theme";
import { showError } from "./ErrorToast";
import type { SeleanEditor, TreeNode } from "../wasm/types";

interface LayerPanelProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  refreshTick: number;
}

/** Node kind to SVG icon path mapping. */
const KIND_ICONS: Record<string, string> = {
  Rect: "M3 3h10v10H3z",
  Frame: "M3 3h10v10H3zM3 6h10",
  Text: "M3 3h10M8 3v10",
  Image: "M3 3h10v10H3zM3 10l3-3 2 2 2-3 3 4",
  Group: "M2 4h5v5H2zM9 7h5v5H9z",
  Vector: "M3 13L8 3l5 10",
};

/**
 * Enhanced layer panel with search filter, inline rename,
 * expand/collapse for groups, and SVG icons.
 */
export function LayerPanel({
  editorRef,
  onSceneChanged,
  refreshTick,
}: LayerPanelProps) {
  const [tree, setTree] = useState<TreeNode[]>([]);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());

  // Debounce search input by 200ms.
  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), 200);
    return () => clearTimeout(timer);
  }, [search]);

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

  const handleRename = useCallback(
    (nodeId: string, newName: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      if (!newName.trim()) {
        showError("Layer name cannot be empty");
        setRenamingId(null);
        return;
      }
      editor.execute_command(
        JSON.stringify({
          type: "SetName",
          node_id: nodeId,
          name: newName.trim(),
        }),
      );
      onSceneChanged();
      setRenamingId(null);
    },
    [editorRef, onSceneChanged],
  );

  const toggleCollapse = useCallback((nodeId: string) => {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(nodeId)) {
        next.delete(nodeId);
      } else {
        next.add(nodeId);
      }
      return next;
    });
  }, []);

  /** Filter tree nodes by search query (case-insensitive). */
  const filterTree = (nodes: TreeNode[], query: string): TreeNode[] => {
    if (!query) return nodes;
    const lower = query.toLowerCase();
    const matches: TreeNode[] = [];
    for (const node of nodes) {
      const childMatches = filterTree(node.children, query);
      if (
        node.name.toLowerCase().includes(lower) ||
        node.kind.toLowerCase().includes(lower) ||
        childMatches.length > 0
      ) {
        matches.push({ ...node, children: childMatches });
      }
    }
    return matches;
  };

  const filteredTree = useMemo(
    () => filterTree(tree, debouncedSearch),
    [tree, debouncedSearch],
  );

  return (
    <div style={panelStyle}>
      <div style={headerStyle}>
        <span style={headerTitleStyle}>Layers</span>
      </div>
      <div style={searchContainerStyle}>
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill="none"
          stroke={colors.textDim}
          strokeWidth="2"
          style={{ flexShrink: 0 }}
        >
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Filter layers..."
          style={searchInputStyle}
        />
      </div>
      <div style={listStyle}>
        {filteredTree.length === 0 && (
          <div style={emptyStyle}>
            {search ? "No matching layers" : "No layers"}
          </div>
        )}
        {filteredTree.map((node) => (
          <LayerItem
            key={node.id}
            node={node}
            depth={0}
            onSelect={handleSelect}
            onToggleVisible={handleToggleVisible}
            onMoveUp={handleMoveUp}
            onMoveDown={handleMoveDown}
            onStartRename={setRenamingId}
            renamingId={renamingId}
            onCommitRename={handleRename}
            onCancelRename={() => setRenamingId(null)}
            collapsed={collapsed}
            onToggleCollapse={toggleCollapse}
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
  onStartRename: (id: string) => void;
  renamingId: string | null;
  onCommitRename: (id: string, name: string) => void;
  onCancelRename: () => void;
  collapsed: Set<string>;
  onToggleCollapse: (id: string) => void;
}

function LayerItem({
  node,
  depth,
  onSelect,
  onToggleVisible,
  onMoveUp,
  onMoveDown,
  onStartRename,
  renamingId,
  onCommitRename,
  onCancelRename,
  collapsed,
  onToggleCollapse,
}: LayerItemProps) {
  const renameInputRef = useRef<HTMLInputElement>(null);
  const [renameValue, setRenameValue] = useState(node.name);
  const isRenaming = renamingId === node.id;
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsed.has(node.id);

  useEffect(() => {
    if (isRenaming && renameInputRef.current) {
      renameInputRef.current.focus();
      renameInputRef.current.select();
    }
  }, [isRenaming]);

  useEffect(() => {
    setRenameValue(node.name);
  }, [node.name]);

  const iconPath = KIND_ICONS[node.kind] ?? KIND_ICONS.Rect;

  return (
    <>
      <div
        style={{
          ...itemStyle,
          paddingLeft: spacing.sm + depth * 16,
          opacity: node.visible ? 1 : 0.4,
        }}
        onClick={() => onSelect(node.id)}
        onDoubleClick={() => onStartRename(node.id)}
      >
        {/* Expand/collapse toggle for nodes with children */}
        {hasChildren ? (
          <button
            style={expandBtnStyle}
            onClick={(e) => {
              e.stopPropagation();
              onToggleCollapse(node.id);
            }}
          >
            <svg
              width="10"
              height="10"
              viewBox="0 0 10 10"
              fill="none"
              stroke="currentColor"
              strokeWidth="1.5"
            >
              {isCollapsed ? (
                <polyline points="3 1 7 5 3 9" />
              ) : (
                <polyline points="1 3 5 7 9 3" />
              )}
            </svg>
          </button>
        ) : (
          <span style={expandPlaceholderStyle} />
        )}

        {/* Node type icon */}
        <svg
          width="14"
          height="14"
          viewBox="0 0 16 16"
          fill="none"
          stroke={colors.textDim}
          strokeWidth="1.2"
          style={{ flexShrink: 0 }}
        >
          <path d={iconPath} />
        </svg>

        {/* Name (inline rename on double-click) */}
        {isRenaming ? (
          <input
            ref={renameInputRef}
            type="text"
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
            onBlur={() => onCommitRename(node.id, renameValue)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                onCommitRename(node.id, renameValue);
              } else if (e.key === "Escape") {
                onCancelRename();
              }
            }}
            onClick={(e) => e.stopPropagation()}
            style={renameInputStyle}
          />
        ) : (
          <span style={nameStyle} title={node.name}>
            {node.name}
          </span>
        )}

        {/* Z-order buttons */}
        <button
          style={actionBtnStyle}
          title="Move forward"
          onClick={(e) => {
            e.stopPropagation();
            onMoveUp(node.id);
          }}
        >
          <svg
            width="10"
            height="10"
            viewBox="0 0 10 10"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          >
            <polyline points="2 7 5 3 8 7" />
          </svg>
        </button>
        <button
          style={actionBtnStyle}
          title="Move backward"
          onClick={(e) => {
            e.stopPropagation();
            onMoveDown(node.id);
          }}
        >
          <svg
            width="10"
            height="10"
            viewBox="0 0 10 10"
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
          >
            <polyline points="2 3 5 7 8 3" />
          </svg>
        </button>

        {/* Visibility toggle */}
        <button
          style={actionBtnStyle}
          title={node.visible ? "Hide" : "Show"}
          onClick={(e) => {
            e.stopPropagation();
            onToggleVisible(node.id, node.visible);
          }}
        >
          {node.visible ? (
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z" />
              <circle cx="12" cy="12" r="3" />
            </svg>
          ) : (
            <svg
              width="12"
              height="12"
              viewBox="0 0 24 24"
              fill="none"
              stroke="currentColor"
              strokeWidth="2"
            >
              <path d="M17.94 17.94A10.07 10.07 0 0 1 12 20c-7 0-11-8-11-8a18.45 18.45 0 0 1 5.06-5.94M9.9 4.24A9.12 9.12 0 0 1 12 4c7 0 11 8 11 8a18.5 18.5 0 0 1-2.16 3.19m-6.72-1.07a3 3 0 1 1-4.24-4.24" />
              <line x1="1" y1="1" x2="23" y2="23" />
            </svg>
          )}
        </button>
      </div>

      {/* Render children if not collapsed */}
      {hasChildren &&
        !isCollapsed &&
        node.children.map((child) => (
          <LayerItem
            key={child.id}
            node={child}
            depth={depth + 1}
            onSelect={onSelect}
            onToggleVisible={onToggleVisible}
            onMoveUp={onMoveUp}
            onMoveDown={onMoveDown}
            onStartRename={onStartRename}
            renamingId={renamingId}
            onCommitRename={onCommitRename}
            onCancelRename={onCancelRename}
            collapsed={collapsed}
            onToggleCollapse={onToggleCollapse}
          />
        ))}
    </>
  );
}

// --- Styles ---

const panelStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
  height: "100%",
  background: colors.bg,
};

const headerStyle: React.CSSProperties = {
  height: 36,
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: `0 ${spacing.md}px`,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const headerTitleStyle: React.CSSProperties = {
  fontWeight: 600,
  fontSize: fontSizes.sm,
  color: colors.text,
};

const searchContainerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.xs,
  padding: `${spacing.xs}px ${spacing.sm}px`,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const searchInputStyle: React.CSSProperties = {
  flex: 1,
  background: "transparent",
  border: "none",
  outline: "none",
  color: colors.text,
  fontSize: fontSizes.xs,
};

const listStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
};

const emptyStyle: React.CSSProperties = {
  padding: `${spacing.xl}px ${spacing.md}px`,
  color: colors.textFaint,
  fontSize: fontSizes.xs,
  textAlign: "center",
};

const itemStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.xs,
  padding: `3px ${spacing.sm}px`,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  color: colors.text,
  borderBottom: `1px solid ${colors.surface}`,
  minHeight: 28,
};

const expandBtnStyle: React.CSSProperties = {
  width: 14,
  height: 14,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "transparent",
  border: "none",
  cursor: "pointer",
  color: colors.textDim,
  padding: 0,
  flexShrink: 0,
};

const expandPlaceholderStyle: React.CSSProperties = {
  width: 14,
  height: 14,
  flexShrink: 0,
};

const nameStyle: React.CSSProperties = {
  flex: 1,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const renameInputStyle: React.CSSProperties = {
  flex: 1,
  background: colors.surface,
  border: `1px solid ${colors.accent}`,
  borderRadius: radii.sm,
  color: colors.text,
  fontSize: fontSizes.xs,
  padding: "1px 4px",
  outline: "none",
  minWidth: 0,
};

const actionBtnStyle: React.CSSProperties = {
  width: 18,
  height: 18,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "transparent",
  border: "none",
  color: colors.textDim,
  cursor: "pointer",
  padding: 0,
  flexShrink: 0,
  borderRadius: radii.sm,
};
