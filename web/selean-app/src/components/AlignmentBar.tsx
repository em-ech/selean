import { useCallback } from "react";
import { colors, fontSizes } from "../theme";
import type { SeleanEditor } from "../wasm/types";

const ALIGNMENTS = [
  { kind: "Left", label: "L" },
  { kind: "CenterH", label: "CH" },
  { kind: "Right", label: "R" },
  { kind: "Top", label: "T" },
  { kind: "CenterV", label: "CV" },
  { kind: "Bottom", label: "B" },
  { kind: "DistributeH", label: "DH" },
  { kind: "DistributeV", label: "DV" },
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
      {ALIGNMENTS.map(({ kind, label }) => (
        <button
          key={kind}
          style={buttonStyle}
          onClick={() => handleAlign(kind)}
          title={kind}
        >
          {label}
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
  padding: "2px 6px",
  borderRadius: 3,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  fontWeight: 600,
};
