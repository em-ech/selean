import { useCallback } from "react";
import { colors } from "../theme";
import type { SeleanEditor } from "../wasm/types";

/** Inline SVG icons for alignment actions (16x16 viewBox). */
const ICONS: Record<string, string> = {
  Left: "M3 2v12M6 4h7v3H6zM6 9h5v3H6z",
  CenterH: "M8 2v12M5 4h6v3H5zM4 9h8v3H4z",
  Right: "M13 2v12M6 4h7v3H6zM8 9h5v3H8z",
  Top: "M2 3h12M4 6v7h3V6zM9 6v5h3V6z",
  CenterV: "M2 8h12M4 5v6h3V5zM9 4v8h3V4z",
  Bottom: "M2 13h12M4 6v7h3V6zM9 8v5h3V8z",
  DistributeH: "M2 2v12M14 2v12M5 5h2v6H5zM9 5h2v6H9z",
  DistributeV: "M2 2h12M2 14h12M5 5h6v2H5zM5 9h6v2H5z",
};

const TOOLTIPS: Record<string, string> = {
  Left: "Align left edges",
  CenterH: "Align horizontal centers",
  Right: "Align right edges",
  Top: "Align top edges",
  CenterV: "Align vertical centers",
  Bottom: "Align bottom edges",
  DistributeH: "Distribute horizontally",
  DistributeV: "Distribute vertically",
};

const ALIGNMENTS = [
  "Left",
  "CenterH",
  "Right",
  "Top",
  "CenterV",
  "Bottom",
  "DistributeH",
  "DistributeV",
] as const;

interface AlignmentBarProps {
  selectedIds: string[];
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

export function AlignmentBar({
  selectedIds,
  editorRef,
  onSceneChanged,
}: AlignmentBarProps) {
  const handleAlign = useCallback(
    (kind: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.align_nodes(JSON.stringify(selectedIds), kind);
      onSceneChanged();
    },
    [editorRef, selectedIds, onSceneChanged],
  );

  if (selectedIds.length < 2) return null;

  return (
    <div style={barStyle}>
      {ALIGNMENTS.map((kind) => (
        <button
          key={kind}
          style={buttonStyle}
          onClick={() => handleAlign(kind)}
          title={TOOLTIPS[kind]}
        >
          <svg width={16} height={16} viewBox="0 0 16 16" fill="none">
            <path d={ICONS[kind]} stroke="currentColor" strokeWidth={1.5} />
          </svg>
        </button>
      ))}
    </div>
  );
}

const barStyle: React.CSSProperties = {
  position: "absolute",
  top: 8,
  left: "50%",
  transform: "translateX(-50%)",
  display: "flex",
  gap: 4,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: 6,
  padding: "4px 8px",
  zIndex: 20,
};

const buttonStyle: React.CSSProperties = {
  background: "none",
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: 4,
  borderRadius: 3,
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};
