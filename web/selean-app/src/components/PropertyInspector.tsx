import { useCallback } from "react";
import { colors, fontSizes } from "../theme";
import type { NodeInfo, SeleanEditor } from "../wasm/types";

interface PropertyInspectorProps {
  node: NodeInfo | null;
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

/**
 * Right panel showing editable properties of the selected node.
 * When no node is selected, shows a placeholder message.
 */
export function PropertyInspector({
  node,
  editorRef,
  onSceneChanged,
}: PropertyInspectorProps) {
  const executeCommand = useCallback(
    (command: Record<string, unknown>) => {
      const editor = editorRef.current;
      if (!editor) return;
      editor.execute_command(JSON.stringify(command));
      onSceneChanged();
    },
    [editorRef, onSceneChanged],
  );

  if (!node) {
    return (
      <div style={panelStyle}>
        <div style={headerStyle}>Properties</div>
        <div style={emptyStyle}>No node selected</div>
      </div>
    );
  }

  return (
    <div style={panelStyle}>
      <div style={headerStyle}>Properties</div>
      <div style={contentStyle}>
        <Section label="Identity">
          <Field label="Name">
            <TextInput
              value={node.name}
              onCommit={(name) =>
                executeCommand({ type: "SetName", node_id: node.id, name })
              }
            />
          </Field>
          <Field label="Kind">
            <span style={readonlyStyle}>{node.kind}</span>
          </Field>
          <Field label="ID">
            <span style={{ ...readonlyStyle, fontSize: 10 }}>{node.id}</span>
          </Field>
        </Section>

        <Section label="Position & Size">
          <div style={gridStyle}>
            <Field label="X">
              <NumberInput
                value={node.x}
                onCommit={(x) =>
                  executeCommand({
                    type: "SetBounds",
                    node_id: node.id,
                    bounds: {
                      x,
                      y: node.y,
                      width: node.width,
                      height: node.height,
                    },
                  })
                }
              />
            </Field>
            <Field label="Y">
              <NumberInput
                value={node.y}
                onCommit={(y) =>
                  executeCommand({
                    type: "SetBounds",
                    node_id: node.id,
                    bounds: {
                      x: node.x,
                      y,
                      width: node.width,
                      height: node.height,
                    },
                  })
                }
              />
            </Field>
            <Field label="W">
              <NumberInput
                value={node.width}
                onCommit={(width) =>
                  executeCommand({
                    type: "SetBounds",
                    node_id: node.id,
                    bounds: {
                      x: node.x,
                      y: node.y,
                      width,
                      height: node.height,
                    },
                  })
                }
              />
            </Field>
            <Field label="H">
              <NumberInput
                value={node.height}
                onCommit={(height) =>
                  executeCommand({
                    type: "SetBounds",
                    node_id: node.id,
                    bounds: { x: node.x, y: node.y, width: node.width, height },
                  })
                }
              />
            </Field>
          </div>
        </Section>

        <Section label="Appearance">
          <Field label="Fill">
            <ColorInput
              value={node.fill}
              onCommit={(fill) =>
                executeCommand({
                  type: "SetFill",
                  node_id: node.id,
                  fill: fill
                    ? { r: fill[0], g: fill[1], b: fill[2], a: fill[3] }
                    : null,
                })
              }
            />
          </Field>
          <Field label="Stroke">
            <ColorInput
              value={node.stroke}
              onCommit={(stroke) =>
                executeCommand({
                  type: "SetStroke",
                  node_id: node.id,
                  stroke: stroke
                    ? { r: stroke[0], g: stroke[1], b: stroke[2], a: stroke[3] }
                    : null,
                })
              }
            />
          </Field>
          <Field label="Opacity">
            <NumberInput
              value={Math.round(node.opacity * 100)}
              min={0}
              max={100}
              step={1}
              onCommit={(pct) =>
                executeCommand({
                  type: "SetOpacity",
                  node_id: node.id,
                  opacity: pct / 100,
                })
              }
              suffix="%"
            />
          </Field>
          <Field label="Visible">
            <input
              type="checkbox"
              checked={node.visible}
              onChange={(e) =>
                executeCommand({
                  type: "SetVisible",
                  node_id: node.id,
                  visible: e.target.checked,
                })
              }
            />
          </Field>
          <Field label="Blend">
            <span style={readonlyStyle}>{node.blend_mode}</span>
          </Field>
        </Section>

        {node.kind === "Frame" && (
          <Section label="Frame">
            <Field label="Radius">
              <span style={readonlyStyle}>
                {node.corner_radius.map((r) => r.toFixed(0)).join(", ")}
              </span>
            </Field>
          </Section>
        )}

        {node.kind === "Text" && (
          <Section label="Text">
            <Field label="Content">
              <TextInput
                value={node.text_content ?? ""}
                onCommit={(content) =>
                  executeCommand({
                    type: "SetTextContent",
                    node_id: node.id,
                    content,
                  })
                }
              />
            </Field>
            <Field label="Size">
              <NumberInput
                value={node.font_size ?? 16}
                min={1}
                onCommit={(font_size) =>
                  executeCommand({
                    type: "SetFontSize",
                    node_id: node.id,
                    font_size,
                  })
                }
                suffix="px"
              />
            </Field>
          </Section>
        )}
      </div>
    </div>
  );
}

// --- Sub-components ---

function Section({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div style={{ marginBottom: 16 }}>
      <div style={sectionLabelStyle}>{label}</div>
      {children}
    </div>
  );
}

function Field({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div style={fieldStyle}>
      <span style={fieldLabelStyle}>{label}</span>
      <div style={{ flex: 1, minWidth: 0 }}>{children}</div>
    </div>
  );
}

function TextInput({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (v: string) => void;
}) {
  return (
    <input
      type="text"
      defaultValue={value}
      key={value}
      style={inputStyle}
      onBlur={(e) => {
        if (e.target.value !== value) onCommit(e.target.value);
      }}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          const target = e.target as HTMLInputElement;
          if (target.value !== value) onCommit(target.value);
          target.blur();
        }
      }}
    />
  );
}

function NumberInput({
  value,
  onCommit,
  min,
  max,
  step = 1,
  suffix,
}: {
  value: number;
  onCommit: (v: number) => void;
  min?: number;
  max?: number;
  step?: number;
  suffix?: string;
}) {
  const display = Number.isInteger(value) ? String(value) : value.toFixed(1);
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
      <input
        type="number"
        defaultValue={display}
        key={display}
        min={min}
        max={max}
        step={step}
        style={{ ...inputStyle, width: "100%" }}
        onBlur={(e) => {
          const n = parseFloat(e.target.value);
          if (!isNaN(n) && n !== value) onCommit(n);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            const n = parseFloat((e.target as HTMLInputElement).value);
            if (!isNaN(n) && n !== value) onCommit(n);
            (e.target as HTMLElement).blur();
          }
        }}
      />
      {suffix && <span style={{ fontSize: 11, color: "#888" }}>{suffix}</span>}
    </div>
  );
}

function ColorInput({
  value,
  onCommit,
}: {
  value: [number, number, number, number] | null;
  onCommit: (v: [number, number, number, number] | null) => void;
}) {
  const hex = value ? rgbaToHex(value) : "#000000";
  const hasColor = value !== null;

  return (
    <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
      <input
        type="color"
        value={hex}
        style={{
          width: 24,
          height: 24,
          padding: 0,
          border: "none",
          cursor: "pointer",
        }}
        onChange={(e) => {
          const rgba = hexToRgba(e.target.value, value?.[3] ?? 1);
          onCommit(rgba);
        }}
      />
      <span style={{ fontSize: 11, color: "#aaa", fontFamily: "monospace" }}>
        {hasColor ? hex.toUpperCase() : "none"}
      </span>
      {hasColor && (
        <button
          style={clearBtnStyle}
          onClick={() => onCommit(null)}
          title="Clear color"
        >
          x
        </button>
      )}
    </div>
  );
}

// --- Color utilities ---

function rgbaToHex(rgba: [number, number, number, number]): string {
  const [r, g, b] = rgba;
  const toHex = (v: number) =>
    Math.round(v * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${toHex(r)}${toHex(g)}${toHex(b)}`;
}

function hexToRgba(
  hex: string,
  alpha: number,
): [number, number, number, number] {
  const r = parseInt(hex.slice(1, 3), 16) / 255;
  const g = parseInt(hex.slice(3, 5), 16) / 255;
  const b = parseInt(hex.slice(5, 7), 16) / 255;
  return [r, g, b, alpha];
}

// --- Styles ---

const panelStyle: React.CSSProperties = {
  width: 280,
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

const contentStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  padding: 12,
};

const emptyStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: colors.textFaint,
  fontSize: fontSizes.md,
};

const sectionLabelStyle: React.CSSProperties = {
  fontSize: fontSizes.sm,
  fontWeight: 600,
  color: colors.textDim,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
  marginBottom: 8,
};

const fieldStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  marginBottom: 6,
  gap: 8,
};

const fieldLabelStyle: React.CSSProperties = {
  width: 52,
  fontSize: fontSizes.base,
  color: colors.textMuted,
  flexShrink: 0,
};

const readonlyStyle: React.CSSProperties = {
  fontSize: fontSizes.base,
  color: "#ccc",
};

const inputStyle: React.CSSProperties = {
  background: colors.surface,
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: "3px 6px",
  borderRadius: 3,
  fontSize: fontSizes.base,
  width: "100%",
  outline: "none",
};

const gridStyle: React.CSSProperties = {
  display: "grid",
  gridTemplateColumns: "1fr 1fr",
  gap: 6,
};

const clearBtnStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: colors.textDim,
  cursor: "pointer",
  fontSize: fontSizes.base,
  padding: "0 4px",
};
