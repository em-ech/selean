import { useCallback, useRef } from "react";
import { colors, fontSizes } from "../theme";
import type { NodeInfo, SeleanEditor } from "../wasm/types";

interface PropertyInspectorProps {
  node: NodeInfo | null;
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

const BLEND_MODES = [
  "Normal",
  "Add",
  "Multiply",
  "Screen",
  "Overlay",
  "Darken",
  "Lighten",
  "ColorDodge",
  "ColorBurn",
  "HardLight",
  "SoftLight",
  "Difference",
  "Exclusion",
];

const CLIP_MODES = ["None", "Scissor", "Stencil", "ShaderRect"];

const FONT_STYLES = ["Normal", "Italic"];

const TEXT_ALIGNS = ["Left", "Center", "Right", "Justify"];

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
          <Field label="Rotate">
            <NumberInput
              value={extractRotationDegrees(node.transform)}
              min={-360}
              max={360}
              step={1}
              onCommit={(degrees) => {
                const editor = editorRef.current;
                if (!editor) return;
                editor.execute_tool_call(
                  "set_rotation",
                  JSON.stringify({
                    node_id: node.id,
                    angle_degrees: degrees,
                  }),
                );
                onSceneChanged();
              }}
              suffix="deg"
            />
          </Field>
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
          <Field label="Stroke W">
            <NumberInput
              value={node.stroke_width}
              min={0}
              step={0.5}
              onCommit={(width) =>
                executeCommand({
                  type: "SetStrokeWidth",
                  node_id: node.id,
                  width,
                })
              }
              suffix="px"
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
            <SelectInput
              value={node.blend_mode}
              options={BLEND_MODES}
              onCommit={(blend_mode) =>
                executeCommand({
                  type: "SetBlendMode",
                  node_id: node.id,
                  blend_mode,
                })
              }
            />
          </Field>
          <Field label="Clip">
            <SelectInput
              value={node.clip_mode}
              options={CLIP_MODES}
              onCommit={(clip_mode) =>
                executeCommand({
                  type: "SetClipMode",
                  node_id: node.id,
                  clip_mode,
                })
              }
            />
          </Field>
        </Section>

        {node.kind === "Frame" && (
          <Section label="Frame">
            <Field label="Radius">
              <NumberInput
                value={node.corner_radius[0]}
                min={0}
                step={1}
                onCommit={(r) =>
                  executeCommand({
                    type: "SetCornerRadius",
                    node_id: node.id,
                    corner_radius: [r, r, r, r],
                  })
                }
                suffix="px"
              />
            </Field>
          </Section>
        )}

        {node.kind === "Image" && (
          <ImageSection
            node={node}
            editorRef={editorRef}
            onSceneChanged={onSceneChanged}
          />
        )}

        {node.kind === "Vector" && (
          <Section label="Vector">
            <Field label="Path">
              <span style={readonlyStyle} title={node.path_data ?? ""}>
                {node.path_data
                  ? node.path_data.length > 30
                    ? `${node.path_data.slice(0, 30)}...`
                    : node.path_data
                  : "none"}
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
            <Field label="Family">
              <TextInput
                value={node.font_family ?? "Inter"}
                onCommit={(font_family) =>
                  executeCommand({
                    type: "SetFontFamily",
                    node_id: node.id,
                    font_family,
                  })
                }
              />
            </Field>
            <Field label="Weight">
              <NumberInput
                value={node.font_weight ?? 400}
                min={100}
                max={900}
                step={100}
                onCommit={(font_weight) =>
                  executeCommand({
                    type: "SetFontWeight",
                    node_id: node.id,
                    font_weight,
                  })
                }
              />
            </Field>
            <Field label="Style">
              <SelectInput
                value={node.font_style ?? "Normal"}
                options={FONT_STYLES}
                onCommit={(font_style) =>
                  executeCommand({
                    type: "SetFontStyle",
                    node_id: node.id,
                    font_style,
                  })
                }
              />
            </Field>
            <Field label="Align">
              <SelectInput
                value={node.text_align ?? "Left"}
                options={TEXT_ALIGNS}
                onCommit={(text_align) =>
                  executeCommand({
                    type: "SetTextAlign",
                    node_id: node.id,
                    text_align,
                  })
                }
              />
            </Field>
            <Field label="Height">
              <NumberInput
                value={node.line_height ?? 1.2}
                min={0.5}
                max={3.0}
                step={0.1}
                onCommit={(line_height) =>
                  executeCommand({
                    type: "SetLineHeight",
                    node_id: node.id,
                    line_height,
                  })
                }
              />
            </Field>
            <Field label="Color">
              <ColorInput
                value={node.text_color}
                onCommit={(text_color) =>
                  executeCommand({
                    type: "SetTextColor",
                    node_id: node.id,
                    text_color: text_color
                      ? {
                          r: text_color[0],
                          g: text_color[1],
                          b: text_color[2],
                          a: text_color[3],
                        }
                      : null,
                  })
                }
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

function SelectInput({
  value,
  options,
  onCommit,
}: {
  value: string;
  options: string[];
  onCommit: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => {
        if (e.target.value !== value) onCommit(e.target.value);
      }}
      style={selectStyle}
    >
      {options.map((opt) => (
        <option key={opt} value={opt}>
          {opt}
        </option>
      ))}
    </select>
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

function ImageSection({
  node,
  editorRef,
  onSceneChanged,
}: {
  node: NodeInfo;
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}) {
  const replaceInputRef = useRef<HTMLInputElement>(null);

  const handleReplace = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const buffer = await file.arrayBuffer();
        const data = new Uint8Array(buffer);
        const assetRef = `img_${Date.now()}`;
        if (editor.register_image_asset(assetRef, data)) {
          editor.execute_command(
            JSON.stringify({
              type: "SetAssetRef",
              node_id: node.id,
              asset_ref: assetRef,
            }),
          );
          onSceneChanged();
        }
      } catch {
        // Replace failed
      }
      e.target.value = "";
    },
    [editorRef, node.id, onSceneChanged],
  );

  return (
    <Section label="Image">
      <Field label="Asset">
        <span style={readonlyStyle}>{node.asset_ref ?? "none"}</span>
      </Field>
      <Field label="">
        <button
          style={replaceButtonStyle}
          onClick={() => replaceInputRef.current?.click()}
        >
          Replace Image
        </button>
        <input
          ref={replaceInputRef}
          type="file"
          accept="image/png,image/jpeg,image/webp"
          style={{ display: "none" }}
          onChange={handleReplace}
        />
      </Field>
    </Section>
  );
}

// --- Transform utilities ---

/**
 * Extracts the rotation angle in degrees from a 3x2 affine transform.
 * Transform layout: [a, b, c, d, tx, ty] where rotation = atan2(b, a).
 */
export function extractRotationDegrees(
  transform: [number, number, number, number, number, number],
): number {
  const [a, b] = transform;
  const rad = Math.atan2(b, a);
  return Math.round(rad * (180 / Math.PI) * 10) / 10;
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

const selectStyle: React.CSSProperties = {
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

const replaceButtonStyle: React.CSSProperties = {
  background: colors.surface,
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: "3px 8px",
  borderRadius: 3,
  fontSize: fontSizes.sm,
  cursor: "pointer",
};
