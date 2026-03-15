import { useCallback, useEffect, useRef, useState } from "react";
import { useWorkspace } from "../hooks/useWorkspace";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";

export function WorkspaceSelector() {
  const { workspaces, activeWorkspace, switchWorkspace, createWorkspace } =
    useWorkspace();
  const [isOpen, setIsOpen] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [newName, setNewName] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Close dropdown on outside click.
  useEffect(() => {
    if (!isOpen) return;
    const handler = (e: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(e.target as Node)
      ) {
        setIsOpen(false);
        setIsCreating(false);
        setNewName("");
        setCreateError(null);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [isOpen]);

  const handleSelect = useCallback(
    async (id: string) => {
      await switchWorkspace(id);
      setIsOpen(false);
    },
    [switchWorkspace],
  );

  const handleCreate = useCallback(async () => {
    const trimmed = newName.trim();
    if (!trimmed) return;
    setCreateError(null);
    try {
      await createWorkspace(trimmed);
      setNewName("");
      setIsCreating(false);
      setIsOpen(false);
    } catch (err) {
      setCreateError(
        err instanceof Error ? err.message : "Failed to create workspace",
      );
    }
  }, [newName, createWorkspace]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter") {
        void handleCreate();
      } else if (e.key === "Escape") {
        setIsCreating(false);
        setNewName("");
        setCreateError(null);
      }
    },
    [handleCreate],
  );

  if (!activeWorkspace && workspaces.length === 0) {
    return null;
  }

  return (
    <div ref={containerRef} style={containerStyle}>
      <button
        style={triggerStyle}
        onClick={() => setIsOpen((o) => !o)}
        aria-label="Switch workspace"
      >
        {activeWorkspace?.name ?? "Workspace"}
        <span style={caretStyle}>{isOpen ? "\u25B4" : "\u25BE"}</span>
      </button>
      {isOpen && (
        <div style={dropdownStyle} role="listbox">
          {workspaces.map((ws) => (
            <button
              key={ws.id}
              style={{
                ...itemStyle,
                ...(ws.id === activeWorkspace?.id ? activeItemStyle : {}),
              }}
              role="option"
              aria-selected={ws.id === activeWorkspace?.id}
              onClick={() => void handleSelect(ws.id)}
            >
              {ws.name}
            </button>
          ))}
          <div style={dividerStyle} />
          {isCreating ? (
            <div style={createFormStyle}>
              <input
                style={inputStyle}
                type="text"
                placeholder="Workspace name"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                onKeyDown={handleKeyDown}
                autoFocus
              />
              <div style={createButtonsStyle}>
                <button
                  style={confirmBtnStyle}
                  onClick={() => void handleCreate()}
                >
                  Create
                </button>
                <button
                  style={cancelBtnStyle}
                  onClick={() => {
                    setIsCreating(false);
                    setNewName("");
                    setCreateError(null);
                  }}
                >
                  Cancel
                </button>
              </div>
              {createError && <div style={errorStyle}>{createError}</div>}
            </div>
          ) : (
            <button
              style={newWorkspaceBtnStyle}
              onClick={() => setIsCreating(true)}
            >
              + New Workspace
            </button>
          )}
        </div>
      )}
    </div>
  );
}

const containerStyle: React.CSSProperties = {
  position: "relative",
};

const triggerStyle: React.CSSProperties = {
  background: "transparent",
  border: `1px solid ${colors.border}`,
  color: colors.text,
  padding: `${spacing.xs}px 10px`,
  borderRadius: radii.sm,
  cursor: "pointer",
  fontSize: fontSizes.base,
  display: "flex",
  alignItems: "center",
  gap: 6,
};

const caretStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  color: colors.textDim,
};

const dropdownStyle: React.CSSProperties = {
  position: "absolute",
  top: `calc(100% + ${spacing.xs}px)`,
  left: 0,
  minWidth: 200,
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.md,
  boxShadow: shadows.lg,
  zIndex: 100,
  overflow: "hidden",
};

const itemStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  background: "transparent",
  border: "none",
  color: colors.text,
  padding: `${spacing.sm}px ${spacing.md}px`,
  textAlign: "left",
  cursor: "pointer",
  fontSize: fontSizes.base,
};

const activeItemStyle: React.CSSProperties = {
  background: colors.accentLight,
  color: colors.accent,
};

const dividerStyle: React.CSSProperties = {
  height: 1,
  background: colors.border,
};

const newWorkspaceBtnStyle: React.CSSProperties = {
  display: "block",
  width: "100%",
  background: "transparent",
  border: "none",
  color: colors.textMuted,
  padding: `${spacing.sm}px ${spacing.md}px`,
  textAlign: "left",
  cursor: "pointer",
  fontSize: fontSizes.base,
};

const createFormStyle: React.CSSProperties = {
  padding: `${spacing.sm}px ${spacing.md}px`,
  display: "flex",
  flexDirection: "column",
  gap: 6,
};

const inputStyle: React.CSSProperties = {
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  color: colors.text,
  padding: `${spacing.xs}px ${spacing.sm}px`,
  borderRadius: radii.sm,
  fontSize: fontSizes.base,
  outline: "none",
};

const createButtonsStyle: React.CSSProperties = {
  display: "flex",
  gap: spacing.xs,
};

const confirmBtnStyle: React.CSSProperties = {
  background: colors.accent,
  border: "none",
  color: colors.white,
  padding: `${spacing.xs}px 10px`,
  borderRadius: radii.sm,
  cursor: "pointer",
  fontSize: fontSizes.sm,
};

const cancelBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: `1px solid ${colors.border}`,
  color: colors.textMuted,
  padding: `${spacing.xs}px 10px`,
  borderRadius: radii.sm,
  cursor: "pointer",
  fontSize: fontSizes.sm,
};

const errorStyle: React.CSSProperties = {
  color: colors.danger,
  fontSize: fontSizes.xs,
};
