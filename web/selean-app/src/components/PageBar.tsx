import { useCallback, useEffect, useState } from "react";
import { colors, fontSizes } from "../theme";
import type { PageInfo, SeleanEditor } from "../wasm/types";

interface PageBarProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  refreshTick: number;
}

/**
 * Horizontal page tabs below the header.
 * Click a tab to switch pages, "+" to add a new page.
 */
export function PageBar({
  editorRef,
  onSceneChanged,
  refreshTick,
}: PageBarProps) {
  const [pages, setPages] = useState<PageInfo[]>([]);
  const [activePageId, setActivePageId] = useState<string | null>(null);

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

  const handleAddPage = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;
    const name = `Page ${pages.length + 1}`;
    const id = editor.add_page(name, 1920, 1080);
    editor.set_active_page(id);
    setActivePageId(id);
    onSceneChanged();
  }, [editorRef, pages.length, onSceneChanged]);

  const handleRemovePage = useCallback(
    (e: React.MouseEvent, pageId: string) => {
      e.stopPropagation();
      const editor = editorRef.current;
      if (!editor) return;
      if (pages.length <= 1) return;
      const ok = editor.remove_page(pageId);
      if (ok) {
        // Switch to first remaining page
        const remaining = pages.filter((p) => p.id !== pageId);
        if (remaining.length > 0) {
          editor.set_active_page(remaining[0].id);
          setActivePageId(remaining[0].id);
        }
        onSceneChanged();
      }
    },
    [editorRef, pages, onSceneChanged],
  );

  if (pages.length === 0) return null;

  return (
    <div style={barStyle}>
      {pages.map((page) => (
        <div
          key={page.id}
          onClick={() => handlePageClick(page.id)}
          style={{
            ...tabStyle,
            borderBottom:
              page.id === activePageId
                ? `2px solid ${colors.accent}`
                : "2px solid transparent",
            color: page.id === activePageId ? colors.text : colors.textDim,
          }}
        >
          <span>{page.name}</span>
          {pages.length > 1 && (
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
    </div>
  );
}

const barStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  height: 32,
  borderBottom: `1px solid ${colors.border}`,
  padding: "0 8px",
  gap: 2,
  flexShrink: 0,
  overflow: "auto",
};

const tabStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 4,
  padding: "4px 12px",
  fontSize: fontSizes.sm,
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
  width: 24,
  height: 24,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  borderRadius: 4,
  flexShrink: 0,
};
