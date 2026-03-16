import { useCallback, useState } from "react";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";
import type { NodeInfo, SeleanEditor } from "../wasm/types";
import { ColorPicker, type ColorPickerValue } from "./ColorPicker";
import { rgbaArrayToString } from "../utils/color";
import { NodeKind } from "../types/editor";
import { useCommandDispatch } from "../hooks/useCommandDispatch";

/** Alignment icon paths (16x16 viewBox). */
const ALIGN_ICONS: Record<string, string> = {
  Left: "M3 2v12M6 4h7v3H6zM6 9h5v3H6z",
  CenterH: "M8 2v12M5 4h6v3H5zM4 9h8v3H4z",
  Right: "M13 2v12M6 4h7v3H6zM8 9h5v3H8z",
  Top: "M2 3h12M4 6v7h3V6zM9 6v5h3V6z",
  CenterV: "M2 8h12M4 5v6h3V5zM9 4v8h3V4z",
  Bottom: "M2 13h12M4 6v7h3V6zM9 8v5h3V8z",
  DistributeH: "M2 2v12M14 2v12M5 5h2v6H5zM9 5h2v6H9z",
  DistributeV: "M2 2h12M2 14h12M5 5h6v2H5zM5 9h6v2H5z",
};

const ALIGN_TOOLTIPS: Record<string, string> = {
  Left: "Align left",
  CenterH: "Align center",
  Right: "Align right",
  Top: "Align top",
  CenterV: "Align middle",
  Bottom: "Align bottom",
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

interface FloatingToolbarProps {
  node: NodeInfo | null;
  selectedIds: string[];
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  isDragging: boolean;
  isEditing: boolean;
  /** Bounding box in screen coordinates for positioning. */
  screenBounds?: { x: number; y: number; width: number; height: number };
}

/** Build a ColorPickerValue from the current node state. */
function buildColorPickerValue(node: NodeInfo): ColorPickerValue {
  // Check for gradient fill first.
  if (node.gradient) {
    const g = node.gradient;
    const stops = g.stops.map((s) => ({
      position: s.position,
      color: { r: s.color[0], g: s.color[1], b: s.color[2], a: s.color[3] },
    }));
    const baseColor = stops[0]?.color ?? { r: 0, g: 0, b: 0, a: 1 };

    if (g.type === "Linear") {
      return {
        mode: "linear",
        color: baseColor,
        gradient: {
          type: "linear",
          stops,
          start: g.start,
          end: g.end,
        },
      };
    }
    return {
      mode: "radial",
      color: baseColor,
      gradient: {
        type: "radial",
        stops,
        center: g.center,
        radius: g.radius,
      },
    };
  }

  if (!node.fill) {
    return { mode: "none", color: { r: 0, g: 0, b: 0, a: 0 } };
  }

  return {
    mode: "solid",
    color: {
      r: node.fill[0],
      g: node.fill[1],
      b: node.fill[2],
      a: node.fill[3],
    },
  };
}

/**
 * Apply a ColorPickerValue change to the node via commands.
 * Note: Gradient changes use execute_tool_call (writes to undo history)
 * but don't broadcast to collab sessions. This is a known limitation
 * until the WASM layer exposes gradient descriptors for collab sync.
 */
function applyColorPickerValue(
  val: ColorPickerValue,
  nodeId: string,
  editorRef: React.RefObject<SeleanEditor | null>,
  onSceneChanged: () => void,
  executeCommand: (cmd: Record<string, unknown>) => void,
) {
  const editor = editorRef.current;
  if (!editor) return;

  if (val.mode === "none") {
    executeCommand({ type: "SetFill", node_id: nodeId, fill: null });
  } else if (val.mode === "solid") {
    executeCommand({
      type: "SetFill",
      node_id: nodeId,
      fill: { r: val.color.r, g: val.color.g, b: val.color.b, a: val.color.a },
    });
  } else if (val.mode === "linear" && val.gradient) {
    editor.execute_tool_call(
      "set_linear_gradient",
      JSON.stringify({
        node_id: nodeId,
        start_x: val.gradient.start?.[0] ?? 0,
        start_y: val.gradient.start?.[1] ?? 0.5,
        end_x: val.gradient.end?.[0] ?? 1,
        end_y: val.gradient.end?.[1] ?? 0.5,
        stops: val.gradient.stops.map((s) => ({
          position: s.position,
          r: s.color.r,
          g: s.color.g,
          b: s.color.b,
          a: s.color.a,
        })),
      }),
    );
    onSceneChanged();
  } else if (val.mode === "radial" && val.gradient) {
    editor.execute_tool_call(
      "set_radial_gradient",
      JSON.stringify({
        node_id: nodeId,
        center_x: val.gradient.center?.[0] ?? 0.5,
        center_y: val.gradient.center?.[1] ?? 0.5,
        radius: val.gradient.radius ?? 0.5,
        stops: val.gradient.stops.map((s) => ({
          position: s.position,
          r: s.color.r,
          g: s.color.g,
          b: s.color.b,
          a: s.color.a,
        })),
      }),
    );
    onSceneChanged();
  }
}

/**
 * Contextual floating toolbar that appears above selected elements.
 * Shows simplified property controls (fill, stroke, opacity, font, delete)
 * and alignment buttons for multi-select.
 */
export function FloatingToolbar({
  node,
  selectedIds,
  editorRef,
  onSceneChanged,
  isDragging,
  isEditing,
  screenBounds,
}: FloatingToolbarProps) {
  const executeCommand = useCommandDispatch();

  const handleAlign = useCallback(
    (kind: string) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.align_nodes(JSON.stringify(selectedIds), kind);
      onSceneChanged();
    },
    [editorRef, selectedIds, onSceneChanged],
  );

  const handleDelete = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;
    for (const id of selectedIds) {
      editor.execute_tool_call("delete_node", JSON.stringify({ node_id: id }));
    }
    onSceneChanged();
  }, [editorRef, selectedIds, onSceneChanged]);

  const [showColorPicker, setShowColorPicker] = useState(false);
  const [showStrokePicker, setShowStrokePicker] = useState(false);

  // Don't render if nothing selected, dragging, or inline editing.
  if (!node || isDragging || isEditing) return null;
  if (selectedIds.length === 0) return null;

  // Position above selection bounds, or default to top of canvas.
  let toolbarStyle: React.CSSProperties = { ...barBaseStyle };
  if (screenBounds) {
    const topY = screenBounds.y - 52;
    const centerX = screenBounds.x + screenBounds.width / 2;
    // If too close to top, show below instead.
    if (topY < 8) {
      toolbarStyle = {
        ...toolbarStyle,
        top: screenBounds.y + screenBounds.height + 8,
        left: centerX,
        transform: "translateX(-50%)",
      };
    } else {
      toolbarStyle = {
        ...toolbarStyle,
        top: topY,
        left: centerX,
        transform: "translateX(-50%)",
      };
    }
  } else {
    toolbarStyle = {
      ...toolbarStyle,
      top: 12,
      left: "50%",
      transform: "translateX(-50%)",
    };
  }

  const isText = node.kind === NodeKind.Text;
  const showAlignment = selectedIds.length >= 2;

  return (
    <div style={toolbarStyle}>
      {/* Fill color */}
      <div style={{ ...groupStyle, position: "relative" }}>
        <button
          style={swatchBtnStyle}
          title="Fill color"
          onClick={() => setShowColorPicker((v) => !v)}
        >
          <div
            style={{
              ...swatchStyle,
              background: node.fill
                ? rgbaArrayToString(node.fill)
                : "transparent",
              border: node.fill ? "none" : `1px dashed ${colors.borderHover}`,
            }}
          />
        </button>
        {showColorPicker && (
          <ColorPicker
            value={buildColorPickerValue(node)}
            onChange={(val) =>
              applyColorPickerValue(
                val,
                node.id,
                editorRef,
                onSceneChanged,
                executeCommand,
              )
            }
            onClose={() => setShowColorPicker(false)}
          />
        )}
      </div>

      {/* Stroke color */}
      <div style={{ ...groupStyle, position: "relative" }}>
        <button
          style={swatchBtnStyle}
          title="Stroke color"
          onClick={() => setShowStrokePicker((v) => !v)}
        >
          <div
            style={{
              ...swatchStyle,
              background: "transparent",
              border: node.stroke
                ? `3px solid ${rgbaArrayToString(node.stroke)}`
                : `1px dashed ${colors.borderHover}`,
              width: 20,
              height: 20,
            }}
          />
        </button>
        {showStrokePicker && (
          <ColorPicker
            value={
              node.stroke
                ? {
                    mode: "solid",
                    color: {
                      r: node.stroke[0],
                      g: node.stroke[1],
                      b: node.stroke[2],
                      a: node.stroke[3],
                    },
                  }
                : { mode: "none", color: { r: 0, g: 0, b: 0, a: 0 } }
            }
            onChange={(val) => {
              if (val.mode === "none") {
                executeCommand({
                  type: "SetStroke",
                  node_id: node.id,
                  stroke: null,
                });
              } else {
                executeCommand({
                  type: "SetStroke",
                  node_id: node.id,
                  stroke: {
                    r: val.color.r,
                    g: val.color.g,
                    b: val.color.b,
                    a: val.color.a,
                  },
                });
              }
            }}
            onClose={() => setShowStrokePicker(false)}
          />
        )}
      </div>

      <div style={dividerStyle} />

      {/* Opacity */}
      <div style={groupStyle}>
        <input
          type="number"
          value={Math.round(node.opacity * 100)}
          min={0}
          max={100}
          step={5}
          onChange={(e) => {
            const pct = parseFloat(e.target.value);
            if (!isNaN(pct)) {
              executeCommand({
                type: "SetOpacity",
                node_id: node.id,
                opacity: Math.max(0, Math.min(1, pct / 100)),
              });
            }
          }}
          style={numberInputStyle}
          title="Opacity"
        />
        <span style={suffixStyle}>%</span>
      </div>

      {/* Text controls */}
      {isText && (
        <>
          <div style={dividerStyle} />

          {/* Font family */}
          <div style={groupStyle}>
            <input
              type="text"
              value={node.font_family ?? "Inter"}
              onChange={(e) => {
                executeCommand({
                  type: "SetFontFamily",
                  node_id: node.id,
                  font_family: e.target.value,
                });
              }}
              style={{ ...numberInputStyle, width: 72, textAlign: "left" }}
              title="Font family"
            />
          </div>

          {/* Font size */}
          <div style={groupStyle}>
            <input
              type="number"
              value={node.font_size ?? 14}
              min={1}
              max={200}
              step={1}
              onChange={(e) => {
                const size = parseFloat(e.target.value);
                if (!isNaN(size) && size > 0) {
                  executeCommand({
                    type: "SetFontSize",
                    node_id: node.id,
                    font_size: size,
                  });
                }
              }}
              style={{ ...numberInputStyle, width: 44 }}
              title="Font size"
            />
            <span style={suffixStyle}>px</span>
          </div>

          {/* Bold toggle */}
          <button
            style={{
              ...alignBtnStyle,
              fontWeight: (node.font_weight ?? 400) >= 700 ? 800 : 400,
              fontSize: fontSizes.sm,
              color:
                (node.font_weight ?? 400) >= 700
                  ? colors.accent
                  : colors.textDim,
            }}
            title="Bold"
            onClick={() => {
              const current = node.font_weight ?? 400;
              executeCommand({
                type: "SetFontWeight",
                node_id: node.id,
                font_weight: current >= 700 ? 400 : 700,
              });
            }}
          >
            B
          </button>

          {/* Italic toggle */}
          <button
            style={{
              ...alignBtnStyle,
              fontStyle: node.font_style === "Italic" ? "italic" : "normal",
              fontSize: fontSizes.sm,
              color:
                node.font_style === "Italic" ? colors.accent : colors.textDim,
            }}
            title="Italic"
            onClick={() => {
              executeCommand({
                type: "SetFontStyle",
                node_id: node.id,
                font_style: node.font_style === "Italic" ? "Normal" : "Italic",
              });
            }}
          >
            I
          </button>
        </>
      )}

      {/* Alignment buttons (multi-select only) */}
      {showAlignment && (
        <>
          <div style={dividerStyle} />
          <div style={groupStyle}>
            {ALIGNMENTS.map((kind) => (
              <button
                key={kind}
                data-interactive
                style={alignBtnStyle}
                onClick={() => handleAlign(kind)}
                title={ALIGN_TOOLTIPS[kind]}
              >
                <svg width={14} height={14} viewBox="0 0 16 16" fill="none">
                  <path
                    d={ALIGN_ICONS[kind]}
                    stroke="currentColor"
                    strokeWidth={1.5}
                  />
                </svg>
              </button>
            ))}
          </div>
        </>
      )}

      <div style={dividerStyle} />

      {/* Delete */}
      <button
        data-interactive
        onClick={handleDelete}
        style={deleteBtnStyle}
        title="Delete"
      >
        <svg
          width="14"
          height="14"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <polyline points="3 6 5 6 21 6" />
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" />
        </svg>
      </button>
    </div>
  );
}

// --- Styles ---

const barBaseStyle: React.CSSProperties = {
  position: "absolute",
  display: "flex",
  alignItems: "center",
  gap: spacing.xs,
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.lg,
  padding: `${spacing.xs}px ${spacing.sm}px`,
  boxShadow: shadows.toolbar,
  zIndex: 30,
  whiteSpace: "nowrap",
};

const groupStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 2,
};

const dividerStyle: React.CSSProperties = {
  width: 1,
  height: 20,
  background: colors.border,
  margin: `0 ${spacing.xs}px`,
};

const swatchStyle: React.CSSProperties = {
  width: 22,
  height: 22,
  borderRadius: radii.sm,
  cursor: "pointer",
  flexShrink: 0,
};

const swatchBtnStyle: React.CSSProperties = {
  position: "relative",
  display: "flex",
  alignItems: "center",
  cursor: "pointer",
  background: "transparent",
  border: "none",
  padding: 0,
};

const numberInputStyle: React.CSSProperties = {
  width: 40,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.sm,
  color: colors.text,
  fontSize: fontSizes.sm,
  padding: "2px 4px",
  textAlign: "center",
  outline: "none",
};

const suffixStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textDim,
};

const alignBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: `1px solid ${colors.border}`,
  color: colors.textMuted,
  padding: 3,
  borderRadius: radii.sm,
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const deleteBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: colors.danger,
  padding: spacing.xs,
  borderRadius: radii.sm,
  cursor: "pointer",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};
