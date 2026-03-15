import { useCallback, useEffect, useMemo, useState } from "react";
import { Highlight, themes } from "prism-react-renderer";
import { colors, fontSizes, radii, spacing } from "../theme";
import { ExportDialog } from "./ExportDialog";
import type { SeleanEditor } from "../wasm/types";

interface CodePanelProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  refreshKey: number;
}

interface ProjectFile {
  path: string;
  content: string;
  language: string;
}

interface DesignTokens {
  colors: { name: string; hex: string }[];
  fonts: { family: string; weights: number[] }[];
}

type TabId = "component" | "project" | "tokens";

const TABS: { id: TabId; label: string }[] = [
  { id: "component", label: "Component" },
  { id: "project", label: "Project" },
  { id: "tokens", label: "Tokens" },
];

export function CodePanel({ editorRef, refreshKey }: CodePanelProps) {
  const [activeTab, setActiveTab] = useState<TabId>("component");
  const [code, setCode] = useState("");
  const [projectFiles, setProjectFiles] = useState<ProjectFile[]>([]);
  const [selectedFile, setSelectedFile] = useState("");
  const [tokens, setTokens] = useState<DesignTokens | null>(null);
  const [copied, setCopied] = useState(false);
  const [showExport, setShowExport] = useState(false);

  // Re-generate on refreshKey or tab change
  useEffect(() => {
    const editor = editorRef.current;
    if (!editor) return;

    try {
      if (activeTab === "component") {
        setCode(editor.generate_code());
      } else if (activeTab === "project") {
        const json = editor.generate_project_json();
        const parsed = JSON.parse(json) as { files?: ProjectFile[] };
        const files = parsed.files ?? [];
        setProjectFiles(files);
        if (files.length > 0 && !selectedFile) {
          setSelectedFile(files[0].path);
        }
      } else {
        const json = editor.extract_design_tokens_json();
        setTokens(JSON.parse(json) as DesignTokens);
      }
    } catch (e) {
      console.warn("code-panel: generation failed", e);
    }
  }, [refreshKey, activeTab, editorRef, selectedFile]);

  const currentCode = useMemo(() => {
    if (activeTab === "component") return code;
    if (activeTab === "project") {
      const file = projectFiles.find((f) => f.path === selectedFile);
      return file?.content ?? "";
    }
    return "";
  }, [activeTab, code, projectFiles, selectedFile]);

  const currentLanguage = useMemo(() => {
    if (activeTab === "component") return "tsx";
    if (activeTab === "project") {
      const file = projectFiles.find((f) => f.path === selectedFile);
      return file?.language ?? "tsx";
    }
    return "tsx";
  }, [activeTab, projectFiles, selectedFile]);

  const handleCopy = useCallback(async () => {
    await navigator.clipboard.writeText(currentCode);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [currentCode]);

  return (
    <div style={panelStyle}>
      {/* Tab bar */}
      <div style={tabBarStyle}>
        {TABS.map((tab) => (
          <button
            key={tab.id}
            style={activeTab === tab.id ? activeTabStyle : tabStyle}
            onClick={() => setActiveTab(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Copy button for component and project tabs */}
      {activeTab !== "tokens" && (
        <div style={toolbarStyle}>
          <span style={fileLabel}>
            {activeTab === "project" && selectedFile
              ? selectedFile
              : "Active Page"}
          </span>
          <div style={toolbarActionsStyle}>
            <button
              style={exportBtnStyle}
              onClick={() => setShowExport(true)}
              data-testid="export-button"
            >
              Export
            </button>
            <button
              style={copyBtnStyle}
              onClick={handleCopy}
              data-testid="copy-button"
            >
              {copied ? "Copied" : "Copy"}
            </button>
          </div>
        </div>
      )}
      {activeTab === "tokens" && (
        <div style={toolbarStyle}>
          <span style={fileLabel}>Design Tokens</span>
          <button
            style={exportBtnStyle}
            onClick={() => setShowExport(true)}
            data-testid="export-button-tokens"
          >
            Export
          </button>
        </div>
      )}

      {/* Content area */}
      <div style={contentStyle}>
        {activeTab === "component" && (
          <CodeBlock code={currentCode} language={currentLanguage} />
        )}

        {activeTab === "project" && (
          <div style={projectLayout}>
            <div style={fileTreeStyle}>
              {projectFiles.map((f) => (
                <button
                  key={f.path}
                  style={
                    selectedFile === f.path ? activeFileStyle : fileItemStyle
                  }
                  onClick={() => setSelectedFile(f.path)}
                >
                  {f.path}
                </button>
              ))}
              {projectFiles.length === 0 && (
                <div style={emptyStyle}>No files</div>
              )}
            </div>
            <div style={fileContentStyle}>
              <CodeBlock code={currentCode} language={currentLanguage} />
            </div>
          </div>
        )}

        {activeTab === "tokens" && tokens && <TokensView tokens={tokens} />}
        {activeTab === "tokens" && !tokens && (
          <div style={emptyStyle}>No design tokens</div>
        )}
      </div>

      {showExport && (
        <ExportDialog
          editorRef={editorRef}
          onClose={() => setShowExport(false)}
        />
      )}
    </div>
  );
}

/** Syntax-highlighted code block using prism-react-renderer. */
function CodeBlock({ code, language }: { code: string; language: string }) {
  if (!code) {
    return <div style={emptyStyle}>No code generated</div>;
  }

  return (
    <Highlight theme={themes.vsDark} code={code} language={language}>
      {({ style, tokens: lines, getLineProps, getTokenProps }) => (
        <pre
          style={{ ...style, background: "transparent", margin: 0 }}
          data-testid="code-block"
        >
          {lines.map((line, i) => (
            <div key={i} {...getLineProps({ line })}>
              <span style={lineNumberStyle}>{i + 1}</span>
              {line.map((token, key) => (
                <span key={key} {...getTokenProps({ token })} />
              ))}
            </div>
          ))}
        </pre>
      )}
    </Highlight>
  );
}

/** Design tokens display with color swatches and font list. */
function TokensView({ tokens }: { tokens: DesignTokens }) {
  return (
    <div style={tokensContainer}>
      {tokens.colors.length > 0 && (
        <div>
          <div style={tokensSectionHeader}>Colors</div>
          <div style={colorGrid}>
            {tokens.colors.map((c) => (
              <div key={c.name} style={colorItem}>
                <div
                  style={{
                    ...colorSwatch,
                    backgroundColor: c.hex,
                  }}
                  data-testid={`color-swatch-${c.name}`}
                />
                <div style={colorLabel}>{c.name}</div>
                <div style={colorHex}>{c.hex}</div>
              </div>
            ))}
          </div>
        </div>
      )}

      {tokens.fonts.length > 0 && (
        <div>
          <div style={tokensSectionHeader}>Fonts</div>
          {tokens.fonts.map((f) => (
            <div key={f.family} style={fontItem}>
              <div style={fontFamily}>{f.family}</div>
              <div style={fontWeights}>
                {f.weights.map((w) => (
                  <span key={w} style={fontWeightBadge}>
                    {w}
                  </span>
                ))}
              </div>
            </div>
          ))}
        </div>
      )}

      {tokens.colors.length === 0 && tokens.fonts.length === 0 && (
        <div style={emptyStyle}>No design tokens found</div>
      )}
    </div>
  );
}

// --- Styles ---

const panelStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
  background: colors.bg,
  color: colors.text,
};

const tabBarStyle: React.CSSProperties = {
  display: "flex",
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const tabStyle: React.CSSProperties = {
  flex: 1,
  padding: `${spacing.sm}px 0`,
  background: "transparent",
  border: "none",
  borderBottomWidth: 2,
  borderBottomStyle: "solid",
  borderBottomColor: "transparent",
  color: colors.textMuted,
  cursor: "pointer",
  fontSize: fontSizes.sm,
  fontWeight: 500,
};

const activeTabStyle: React.CSSProperties = {
  ...tabStyle,
  color: colors.text,
  borderBottomColor: colors.accent,
};

const toolbarStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: `6px ${spacing.md}px`,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const fileLabel: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textDim,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const toolbarActionsStyle: React.CSSProperties = {
  display: "flex",
  gap: 6,
  alignItems: "center",
  flexShrink: 0,
};

const exportBtnStyle: React.CSSProperties = {
  background: colors.accent,
  border: "none",
  color: colors.white,
  padding: "3px 10px",
  borderRadius: radii.sm,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  fontWeight: 500,
  flexShrink: 0,
};

const copyBtnStyle: React.CSSProperties = {
  background: colors.accentLight,
  border: `1px solid ${colors.accent}`,
  color: colors.text,
  padding: "3px 10px",
  borderRadius: radii.sm,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  fontWeight: 500,
  flexShrink: 0,
};

const contentStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  fontSize: fontSizes.sm,
};

const lineNumberStyle: React.CSSProperties = {
  color: colors.textFaint,
  userSelect: "none",
  display: "inline-block",
  width: spacing.xxl,
  textAlign: "right",
  marginRight: spacing.lg,
  fontSize: fontSizes.xs,
};

const emptyStyle: React.CSSProperties = {
  padding: `${spacing.xl}px ${spacing.md}px`,
  color: colors.textFaint,
  fontSize: fontSizes.sm,
};

// Project tab styles

const projectLayout: React.CSSProperties = {
  display: "flex",
  height: "100%",
};

const fileTreeStyle: React.CSSProperties = {
  width: 140,
  borderRight: `1px solid ${colors.border}`,
  overflowY: "auto",
  flexShrink: 0,
};

const fileItemStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  textAlign: "left",
  padding: "6px 10px",
  background: "transparent",
  border: "none",
  color: colors.textDim,
  cursor: "pointer",
  fontSize: fontSizes.xs,
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
};

const activeFileStyle: React.CSSProperties = {
  ...fileItemStyle,
  background: colors.accentLight,
  color: colors.text,
};

const fileContentStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
};

// Tokens tab styles

const tokensContainer: React.CSSProperties = {
  padding: spacing.md,
  display: "flex",
  flexDirection: "column",
  gap: spacing.lg,
};

const tokensSectionHeader: React.CSSProperties = {
  fontWeight: 600,
  fontSize: fontSizes.sm,
  color: colors.textMuted,
  marginBottom: spacing.sm,
  textTransform: "uppercase",
  letterSpacing: 1,
};

const colorGrid: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: spacing.sm,
};

const colorItem: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  gap: spacing.xs,
  width: 60,
};

const colorSwatch: React.CSSProperties = {
  width: spacing.xxl,
  height: spacing.xxl,
  borderRadius: radii.md,
  border: `1px solid ${colors.border}`,
};

const colorLabel: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.text,
  textAlign: "center",
  overflow: "hidden",
  textOverflow: "ellipsis",
  whiteSpace: "nowrap",
  width: "100%",
};

const colorHex: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textFaint,
};

const fontItem: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: "6px 0",
  borderBottom: `1px solid ${colors.border}`,
};

const fontFamily: React.CSSProperties = {
  fontSize: fontSizes.sm,
  color: colors.text,
};

const fontWeights: React.CSSProperties = {
  display: "flex",
  gap: spacing.xs,
};

const fontWeightBadge: React.CSSProperties = {
  background: colors.surfaceAlt,
  color: colors.textDim,
  padding: "2px 6px",
  borderRadius: radii.sm,
  fontSize: fontSizes.xs,
};
