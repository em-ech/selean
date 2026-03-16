import { useCallback, useEffect, useRef, useState } from "react";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";
import { showError } from "./ErrorToast";
import type { PageInfo, SeleanEditor } from "../wasm/types";

interface PageBarProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  refreshTick: number;
}

/**
 * Horizontal page tabs below the header.
 * Click a tab to switch pages, double-click to rename,
 * right-click for context menu (duplicate), "+" to add a new page.
 */
export function PageBar({
  editorRef,
  onSceneChanged,
  refreshTick,
}: PageBarProps) {
  const [pages, setPages] = useState<PageInfo[]>([]);
  const [activePageId, setActivePageId] = useState<string | null>(null);
  const [editingPageId, setEditingPageId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState("");
  const [contextMenu, setContextMenu] = useState<{
    pageId: string;
    x: number;
    y: number;
  } | null>(null);
  const editInputRef = useRef<HTMLInputElement>(null);
  const [dragPageId, setDragPageId] = useState<string | null>(null);
  const [dropTargetId, setDropTargetId] = useState<string | null>(null);

  useEffect(() => {
    const editor = editorRef.current;
    if (!editor) return;
    try {
      const parsed: PageInfo[] = JSON.parse(editor.get_pages_json());
      setPages(parsed);
      if (parsed.length > 0 && !activePageId) {
        setActivePageId(parsed[0].id);
      }
    } catch (e) {
      console.warn("page-bar:get-pages failed", e);
    }
  }, [editorRef, refreshTick, activePageId]);

  // Auto-focus rename input.
  useEffect(() => {
    if (editingPageId && editInputRef.current) {
      editInputRef.current.focus();
      editInputRef.current.select();
    }
  }, [editingPageId]);

  // Close context menu on outside click.
  useEffect(() => {
    if (!contextMenu) return;
    const handler = () => setContextMenu(null);
    document.addEventListener("click", handler);
    return () => document.removeEventListener("click", handler);
  }, [contextMenu]);

  const handlePageClick = useCallback(
    (pageId: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.set_active_page(pageId);
      setActivePageId(pageId);
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  const handleDoubleClick = useCallback(
    (pageId: string, currentName: string) => {
      setEditingPageId(pageId);
      setEditingName(currentName);
    },
    [],
  );

  const commitRename = useCallback(() => {
    const editor = editorRef.current;
    if (!editor || !editingPageId) return;
    const trimmed = editingName.trim();
    if (trimmed) {
      try {
        editor.rename_page(editingPageId, trimmed);
        onSceneChanged();
      } catch (err) {
        showError("Failed to rename page");
        console.warn("page-bar:rename failed", err);
      }
    }
    setEditingPageId(null);
  }, [editorRef, editingPageId, editingName, onSceneChanged]);

  const handleRenameKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        commitRename();
      } else if (e.key === "Escape") {
        setEditingPageId(null);
      }
    },
    [commitRename],
  );

  const handleAddPage = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;
    try {
      const name = `Page ${pages.length + 1}`;
      const id = editor.add_page(name, 1920, 1080);
      editor.set_active_page(id);
      setActivePageId(id);
      onSceneChanged();
    } catch (err) {
      showError("Failed to add page");
      console.warn("page-bar:add-page failed", err);
    }
  }, [editorRef, pages.length, onSceneChanged]);

  const handleRemovePage = useCallback(
    (e: React.MouseEvent, pageId: string) => {
      e.stopPropagation();
      const editor = editorRef.current;
      if (!editor) return;
      if (pages.length <= 1) return;
      try {
        const ok = editor.remove_page(pageId);
        if (ok) {
          const remaining = pages.filter((p) => p.id !== pageId);
          if (remaining.length > 0) {
            editor.set_active_page(remaining[0].id);
            setActivePageId(remaining[0].id);
          }
          onSceneChanged();
        }
      } catch (err) {
        showError("Failed to remove page");
        console.warn("page-bar:remove-page failed", err);
      }
    },
    [editorRef, pages, onSceneChanged],
  );

  const handleContextMenu = useCallback(
    (e: React.MouseEvent, pageId: string) => {
      e.preventDefault();
      e.stopPropagation();
      // Clamp to viewport to prevent off-screen overflow.
      const menuW = 120;
      const menuH = 70;
      const x = Math.min(e.clientX, window.innerWidth - menuW);
      const y = Math.min(e.clientY, window.innerHeight - menuH);
      setContextMenu({ pageId, x, y });
    },
    [],
  );

  const handleDuplicate = useCallback(
    (pageId: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const newId = editor.duplicate_page(pageId);
        if (newId) {
          editor.set_active_page(newId);
          setActivePageId(newId);
          onSceneChanged();
        }
      } catch (err) {
        showError("Failed to duplicate page");
        console.warn("page-bar:duplicate failed", err);
      }
      setContextMenu(null);
    },
    [editorRef, onSceneChanged],
  );

  // Drag-to-reorder handlers.
  const handleDragStart = useCallback((e: React.DragEvent, pageId: string) => {
    setDragPageId(pageId);
    e.dataTransfer.effectAllowed = "move";
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent, pageId: string) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    setDropTargetId(pageId);
  }, []);

  const handleDragLeave = useCallback(() => {
    setDropTargetId(null);
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, targetPageId: string) => {
      e.preventDefault();
      setDropTargetId(null);
      setDragPageId(null);
      const editor = editorRef.current;
      if (!editor || !dragPageId || dragPageId === targetPageId) return;

      // Build new order: move dragPageId before targetPageId.
      const ids = pages.map((p) => p.id).filter((id) => id !== dragPageId);
      const targetIdx = ids.indexOf(targetPageId);
      ids.splice(targetIdx, 0, dragPageId);
      editor.reorder_pages(JSON.stringify(ids));
      onSceneChanged();
    },
    [editorRef, dragPageId, pages, onSceneChanged],
  );

  const handleDragEnd = useCallback(() => {
    setDragPageId(null);
    setDropTargetId(null);
  }, []);

  if (pages.length === 0) return null;

  return (
    <div style={barStyle}>
      {pages.map((page) => (
        <div
          key={page.id}
          draggable
          onDragStart={(e) => handleDragStart(e, page.id)}
          onDragOver={(e) => handleDragOver(e, page.id)}
          onDragLeave={handleDragLeave}
          onDrop={(e) => handleDrop(e, page.id)}
          onDragEnd={handleDragEnd}
          onClick={() => handlePageClick(page.id)}
          onDoubleClick={() => handleDoubleClick(page.id, page.name)}
          onContextMenu={(e) => handleContextMenu(e, page.id)}
          style={{
            ...tabStyle,
            borderBottom:
              page.id === activePageId
                ? `2px solid ${colors.accent}`
                : "2px solid transparent",
            color: page.id === activePageId ? colors.text : colors.textDim,
            opacity: dragPageId === page.id ? 0.4 : 1,
            borderLeft:
              dropTargetId === page.id && dragPageId !== page.id
                ? `2px solid ${colors.accent}`
                : "2px solid transparent",
          }}
        >
          {editingPageId === page.id ? (
            <input
              ref={editInputRef}
              value={editingName}
              onChange={(e) => setEditingName(e.target.value)}
              onBlur={commitRename}
              onKeyDown={handleRenameKeyDown}
              onClick={(e) => e.stopPropagation()}
              style={renameInputStyle}
            />
          ) : (
            <span>{page.name}</span>
          )}
          {pages.length > 1 && editingPageId !== page.id && (
            <button
              style={closeButtonStyle}
              onClick={(e) => handleRemovePage(e, page.id)}
              title="Remove page"
            >
              x
            </button>
          )}
        </div>
      ))}
      <button style={addButtonStyle} onClick={handleAddPage} title="Add page">
        +
      </button>
      {contextMenu && (
        <div
          style={{
            ...contextMenuStyle,
            left: contextMenu.x,
            top: contextMenu.y,
          }}
        >
          <button
            style={menuItemStyle}
            onClick={() => handleDuplicate(contextMenu.pageId)}
          >
            Duplicate
          </button>
          <button
            style={menuItemStyle}
            onClick={() => {
              const page = pages.find((p) => p.id === contextMenu.pageId);
              if (page) handleDoubleClick(page.id, page.name);
              setContextMenu(null);
            }}
          >
            Rename
          </button>
        </div>
      )}
    </div>
  );
}

const barStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  height: 32,
  background: colors.bg,
  borderBottom: `1px solid ${colors.border}`,
  padding: `0 ${spacing.sm}px`,
  gap: 2,
  flexShrink: 0,
  overflow: "auto",
  position: "relative",
};

const tabStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.xs,
  padding: `${spacing.xs}px ${spacing.md}px`,
  fontSize: fontSizes.sm,
  color: colors.text,
  cursor: "pointer",
  whiteSpace: "nowrap",
  flexShrink: 0,
};

const closeButtonStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: colors.textFaint,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  padding: "0 2px",
  lineHeight: 1,
};

const addButtonStyle: React.CSSProperties = {
  background: "none",
  border: `1px solid ${colors.border}`,
  color: colors.textDim,
  cursor: "pointer",
  fontSize: fontSizes.md,
  width: spacing.xl,
  height: spacing.xl,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  borderRadius: radii.sm,
  flexShrink: 0,
};

const renameInputStyle: React.CSSProperties = {
  background: colors.surface,
  border: `1px solid ${colors.accent}`,
  color: colors.text,
  fontSize: fontSizes.sm,
  padding: `1px ${spacing.xs}px`,
  outline: "none",
  width: 100,
};

const contextMenuStyle: React.CSSProperties = {
  position: "fixed",
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.sm,
  padding: `${spacing.xs}px 0`,
  zIndex: 1000,
  minWidth: 100,
  boxShadow: shadows.md,
};

const menuItemStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  background: "none",
  border: "none",
  color: colors.text,
  fontSize: fontSizes.sm,
  padding: `${spacing.xs}px ${spacing.md}px`,
  cursor: "pointer",
  textAlign: "left",
};
