import { useCallback, useMemo, useState } from "react";
import JSZip from "jszip";
import { useGitHub } from "../hooks/useGitHub";
import { colors, fontSizes } from "../theme";
import type { SeleanEditor } from "../wasm/types";

interface ExportDialogProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onClose: () => void;
}

type TabId = "github" | "claude" | "zip";

interface ProjectFile {
  path: string;
  content: string;
  language: string;
}

const TABS: { id: TabId; label: string }[] = [
  { id: "github", label: "GitHub" },
  { id: "claude", label: "Claude Code" },
  { id: "zip", label: "Download ZIP" },
];

export function ExportDialog({ editorRef, onClose }: ExportDialogProps) {
  const [activeTab, setActiveTab] = useState<TabId>("github");

  return (
    <div style={overlayStyle} onClick={onClose}>
      <div style={modalStyle} onClick={(e) => e.stopPropagation()}>
        <div style={headerStyle}>
          <span style={titleStyle}>Export Project</span>
          <button
            style={closeBtnStyle}
            onClick={onClose}
            aria-label="Close"
            data-testid="close-button"
          >
            x
          </button>
        </div>

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

        <div style={bodyStyle}>
          {activeTab === "github" && <GitHubTab editorRef={editorRef} />}
          {activeTab === "claude" && <ClaudeCodeTab editorRef={editorRef} />}
          {activeTab === "zip" && <DownloadZipTab editorRef={editorRef} />}
        </div>
      </div>
    </div>
  );
}

/** Collect project files from the editor. Returns empty array if editor is null. */
function getProjectFiles(editor: SeleanEditor | null): ProjectFile[] {
  if (!editor) return [];
  try {
    const json = editor.generate_project_json();
    const parsed = JSON.parse(json) as { files?: ProjectFile[] };
    return parsed.files ?? [];
  } catch {
    return [];
  }
}

/** Build a CLAUDE.md from design tokens and page info. */
function buildClaudeMd(editor: SeleanEditor | null): string {
  const lines: string[] = [
    "# Selean Export",
    "",
    "This project was exported from Selean, a design tool with AI-native workflows.",
    "",
  ];

  if (!editor) return lines.join("\n");

  try {
    const tokensJson = editor.extract_design_tokens_json();
    const tokens = JSON.parse(tokensJson) as {
      colors: { name: string; hex: string }[];
      fonts: { family: string; weights: number[] }[];
    };

    if (tokens.colors.length > 0) {
      lines.push("## Design Tokens", "", "### Colors", "");
      for (const c of tokens.colors) {
        lines.push(`- **${c.name}**: \`${c.hex}\``);
      }
      lines.push("");
    }

    if (tokens.fonts.length > 0) {
      lines.push("### Fonts", "");
      for (const f of tokens.fonts) {
        lines.push(`- **${f.family}**: weights ${f.weights.join(", ")}`);
      }
      lines.push("");
    }
  } catch {
    // Ignore token extraction errors.
  }

  try {
    const pagesJson = editor.get_pages_json();
    const pages = JSON.parse(pagesJson) as {
      id: string;
      name: string;
      width: number;
      height: number;
      node_count: number;
    }[];

    if (pages.length > 0) {
      lines.push("## Pages", "");
      for (const p of pages) {
        lines.push(
          `- **${p.name}** (${p.width}x${p.height}, ${p.node_count} nodes)`,
        );
      }
      lines.push("");
    }
  } catch {
    // Ignore page extraction errors.
  }

  lines.push(
    "## Conventions",
    "",
    "- Vite + React + TypeScript project",
    "- Tailwind CSS for styling",
    "- Components are in `src/components/`",
    "- Design tokens are defined in `src/theme.ts`",
    "",
  );

  return lines.join("\n");
}

async function buildZip(
  files: ProjectFile[],
  claudeMd?: string,
): Promise<Blob> {
  const zip = new JSZip();
  if (claudeMd) {
    zip.file("CLAUDE.md", claudeMd);
  }
  for (const f of files) {
    zip.file(f.path, f.content);
  }
  return zip.generateAsync({ type: "blob" });
}

function triggerDownload(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// --- GitHub Tab ---

function GitHubTab({
  editorRef,
}: {
  editorRef: React.RefObject<SeleanEditor | null>;
}) {
  const { status, repos, loading, connect, disconnect, createRepo, push } =
    useGitHub();
  const [selectedRepo, setSelectedRepo] = useState("");
  const [branch, setBranch] = useState("selean/export");
  const [commitMessage, setCommitMessage] = useState("Export from Selean");
  const [createPr, setCreatePr] = useState(true);
  const [pushing, setPushing] = useState(false);
  const [result, setResult] = useState<{
    success: boolean;
    commit_sha?: string;
    pr_url?: string;
    error?: string;
  } | null>(null);
  const [showNewRepo, setShowNewRepo] = useState(false);
  const [newRepoName, setNewRepoName] = useState("");
  const [newRepoPrivate, setNewRepoPrivate] = useState(true);

  const handlePush = useCallback(async () => {
    if (!selectedRepo) return;
    setPushing(true);
    setResult(null);
    try {
      const files = getProjectFiles(editorRef.current);
      const pushResult = await push(
        selectedRepo,
        branch,
        files.map((f) => ({ path: f.path, content: f.content })),
        commitMessage,
        createPr,
      );
      setResult(pushResult);
    } catch (err) {
      setResult({
        success: false,
        error: err instanceof Error ? err.message : "Push failed",
      });
    } finally {
      setPushing(false);
    }
  }, [selectedRepo, branch, commitMessage, createPr, editorRef, push]);

  const handleCreateRepo = useCallback(async () => {
    if (!newRepoName.trim()) return;
    const repo = await createRepo(newRepoName.trim(), newRepoPrivate);
    if (repo) {
      setSelectedRepo(repo.full_name);
      setShowNewRepo(false);
      setNewRepoName("");
    }
  }, [newRepoName, newRepoPrivate, createRepo]);

  if (loading) {
    return <div style={sectionStyle}>Loading...</div>;
  }

  if (!status.connected) {
    return (
      <div style={sectionStyle}>
        <p style={descStyle}>
          Connect your GitHub account to push exported code directly to a
          repository.
        </p>
        <button style={primaryBtnStyle} onClick={() => void connect()}>
          Connect GitHub
        </button>
      </div>
    );
  }

  return (
    <div style={sectionStyle}>
      <div style={fieldRowStyle}>
        <span style={fieldLabelStyle}>Account</span>
        <div style={accountRowStyle}>
          <span style={accountNameStyle}>{status.login}</span>
          <button style={linkBtnStyle} onClick={() => void disconnect()}>
            Disconnect
          </button>
        </div>
      </div>

      <div style={fieldRowStyle}>
        <span style={fieldLabelStyle}>Repository</span>
        <select
          style={selectStyle}
          value={selectedRepo}
          onChange={(e) => {
            if (e.target.value === "__new__") {
              setShowNewRepo(true);
            } else {
              setSelectedRepo(e.target.value);
              setShowNewRepo(false);
            }
          }}
          data-testid="repo-select"
        >
          <option value="">Select a repository...</option>
          {repos.map((r) => (
            <option key={r.full_name} value={r.full_name}>
              {r.full_name}
              {r.private ? " (private)" : ""}
            </option>
          ))}
          <option value="__new__">+ Create new repository</option>
        </select>
      </div>

      {showNewRepo && (
        <div style={newRepoFormStyle}>
          <input
            style={inputStyle}
            placeholder="Repository name"
            value={newRepoName}
            onChange={(e) => setNewRepoName(e.target.value)}
          />
          <label style={checkboxLabelStyle}>
            <input
              type="checkbox"
              checked={newRepoPrivate}
              onChange={(e) => setNewRepoPrivate(e.target.checked)}
            />
            Private
          </label>
          <button
            style={secondaryBtnStyle}
            onClick={() => void handleCreateRepo()}
          >
            Create
          </button>
        </div>
      )}

      <div style={fieldRowStyle}>
        <span style={fieldLabelStyle}>Branch</span>
        <input
          style={inputStyle}
          value={branch}
          onChange={(e) => setBranch(e.target.value)}
        />
      </div>

      <div style={fieldRowStyle}>
        <span style={fieldLabelStyle}>Commit message</span>
        <input
          style={inputStyle}
          value={commitMessage}
          onChange={(e) => setCommitMessage(e.target.value)}
        />
      </div>

      <label style={checkboxLabelStyle}>
        <input
          type="checkbox"
          checked={createPr}
          onChange={(e) => setCreatePr(e.target.checked)}
        />
        Create pull request
      </label>

      <button
        style={primaryBtnStyle}
        onClick={() => void handlePush()}
        disabled={!selectedRepo || pushing}
        data-testid="push-button"
      >
        {pushing ? "Pushing..." : "Push"}
      </button>

      {result && result.success && (
        <div style={successStyle}>
          Pushed successfully.
          {result.commit_sha && (
            <span> Commit: {result.commit_sha.slice(0, 7)}</span>
          )}
          {result.pr_url && (
            <span>
              {" "}
              <a
                href={result.pr_url}
                target="_blank"
                rel="noreferrer"
                style={linkStyle}
              >
                View PR
              </a>
            </span>
          )}
        </div>
      )}
      {result && !result.success && (
        <div style={errorStyle}>{result.error ?? "Push failed"}</div>
      )}
    </div>
  );
}

// --- Claude Code Tab ---

function ClaudeCodeTab({
  editorRef,
}: {
  editorRef: React.RefObject<SeleanEditor | null>;
}) {
  const [downloading, setDownloading] = useState(false);

  const handleDownload = useCallback(async () => {
    setDownloading(true);
    try {
      const editor = editorRef.current;
      const files = getProjectFiles(editor);
      const claudeMd = buildClaudeMd(editor);
      const blob = await buildZip(files, claudeMd);
      triggerDownload(blob, "selean-claude-code.zip");
    } finally {
      setDownloading(false);
    }
  }, [editorRef]);

  return (
    <div style={sectionStyle}>
      <p style={descStyle}>
        Download a project optimized for Claude Code with a CLAUDE.md containing
        design tokens and conventions.
      </p>
      <button
        style={primaryBtnStyle}
        onClick={() => void handleDownload()}
        disabled={downloading}
        data-testid="claude-download-button"
      >
        {downloading ? "Preparing..." : "Download ZIP"}
      </button>
    </div>
  );
}

// --- Download ZIP Tab ---

function DownloadZipTab({
  editorRef,
}: {
  editorRef: React.RefObject<SeleanEditor | null>;
}) {
  const [downloading, setDownloading] = useState(false);

  const handleDownload = useCallback(async () => {
    setDownloading(true);
    try {
      const files = getProjectFiles(editorRef.current);
      const blob = await buildZip(files);
      triggerDownload(blob, "selean-export.zip");
    } finally {
      setDownloading(false);
    }
  }, [editorRef]);

  return (
    <div style={sectionStyle}>
      <p style={descStyle}>
        Download the generated Vite + React + Tailwind project as a ZIP file.
      </p>
      <button
        style={primaryBtnStyle}
        onClick={() => void handleDownload()}
        disabled={downloading}
        data-testid="zip-download-button"
      >
        {downloading ? "Preparing..." : "Download ZIP"}
      </button>
    </div>
  );
}

// --- Styles ---

const overlayStyle: React.CSSProperties = {
  position: "fixed",
  inset: 0,
  background: "rgba(0,0,0,0.5)",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  zIndex: 200,
};

const modalStyle: React.CSSProperties = {
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: 8,
  width: 520,
  maxHeight: "80vh",
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
};

const headerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: "12px 16px",
  borderBottom: `1px solid ${colors.border}`,
};

const titleStyle: React.CSSProperties = {
  color: colors.text,
  fontSize: fontSizes.lg,
  fontWeight: 600,
};

const closeBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  color: colors.textMuted,
  fontSize: fontSizes.lg,
  cursor: "pointer",
  padding: "2px 6px",
};

const tabBarStyle: React.CSSProperties = {
  display: "flex",
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const tabStyle: React.CSSProperties = {
  flex: 1,
  padding: "8px 0",
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

const bodyStyle: React.CSSProperties = {
  flex: 1,
  overflowY: "auto",
};

const sectionStyle: React.CSSProperties = {
  padding: 16,
  display: "flex",
  flexDirection: "column",
  gap: 12,
};

const descStyle: React.CSSProperties = {
  color: colors.textDim,
  fontSize: fontSizes.base,
  margin: 0,
  lineHeight: 1.5,
};

const fieldRowStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 4,
};

const fieldLabelStyle: React.CSSProperties = {
  color: colors.textMuted,
  fontSize: fontSizes.sm,
  fontWeight: 500,
};

const inputStyle: React.CSSProperties = {
  padding: "6px 8px",
  border: `1px solid ${colors.border}`,
  borderRadius: 4,
  background: colors.bg,
  color: colors.text,
  fontSize: fontSizes.base,
  outline: "none",
};

const selectStyle: React.CSSProperties = {
  ...inputStyle,
  cursor: "pointer",
};

const checkboxLabelStyle: React.CSSProperties = {
  color: colors.textDim,
  fontSize: fontSizes.sm,
  display: "flex",
  alignItems: "center",
  gap: 6,
  cursor: "pointer",
};

const primaryBtnStyle: React.CSSProperties = {
  padding: "8px 16px",
  background: "#3b82f6",
  color: "#fff",
  border: "none",
  borderRadius: 4,
  fontSize: fontSizes.base,
  fontWeight: 600,
  cursor: "pointer",
};

const secondaryBtnStyle: React.CSSProperties = {
  padding: "6px 12px",
  background: colors.accent,
  color: colors.text,
  border: "none",
  borderRadius: 4,
  fontSize: fontSizes.sm,
  cursor: "pointer",
};

const linkBtnStyle: React.CSSProperties = {
  background: "none",
  border: "none",
  color: "#3b82f6",
  cursor: "pointer",
  fontSize: fontSizes.sm,
  padding: 0,
};

const accountRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
};

const accountNameStyle: React.CSSProperties = {
  color: colors.text,
  fontSize: fontSizes.base,
  fontWeight: 500,
};

const newRepoFormStyle: React.CSSProperties = {
  display: "flex",
  gap: 8,
  alignItems: "center",
  padding: "8px 0",
};

const successStyle: React.CSSProperties = {
  color: "#4ade80",
  fontSize: fontSizes.sm,
};

const errorStyle: React.CSSProperties = {
  color: "#e55",
  fontSize: fontSizes.sm,
};

const linkStyle: React.CSSProperties = {
  color: "#3b82f6",
};
