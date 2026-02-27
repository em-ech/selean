import { colors, fontSizes } from "../theme";

export type ToolType = "select" | "frame" | "text" | "image";

interface ToolbarProps {
  activeTool: ToolType;
  onToolChange: (tool: ToolType) => void;
}

const tools: { type: ToolType; label: string; shortcut: string }[] = [
  { type: "select", label: "Select", shortcut: "V" },
  { type: "frame", label: "Frame", shortcut: "F" },
  { type: "text", label: "Text", shortcut: "T" },
  { type: "image", label: "Image", shortcut: "I" },
];

/**
 * Vertical toolbar for switching between editor tools.
 * Sits between the chat sidebar and the canvas.
 */
export function Toolbar({ activeTool, onToolChange }: ToolbarProps) {
  return (
    <div style={toolbarStyle}>
      {tools.map((tool) => (
        <button
          key={tool.type}
          onClick={() => onToolChange(tool.type)}
          style={{
            ...buttonStyle,
            background:
              activeTool === tool.type ? colors.accent : "transparent",
          }}
          title={`${tool.label} (${tool.shortcut})`}
        >
          <span style={labelStyle}>{tool.label[0]}</span>
        </button>
      ))}
    </div>
  );
}

const toolbarStyle: React.CSSProperties = {
  width: 40,
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  padding: "8px 0",
  gap: 4,
  borderRight: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const buttonStyle: React.CSSProperties = {
  width: 32,
  height: 32,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  border: "none",
  borderRadius: 4,
  cursor: "pointer",
  color: colors.text,
  fontSize: fontSizes.lg,
};

const labelStyle: React.CSSProperties = {
  fontWeight: 600,
};
