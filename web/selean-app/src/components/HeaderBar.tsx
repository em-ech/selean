import React from "react";
import { CollabBar } from "./CollabBar";
import { FileMenu } from "./FileMenu";
import { WorkspaceSelector } from "./WorkspaceSelector";
import { useAuth } from "../auth/AuthContext";
import { useTheme } from "../hooks/useTheme";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";
import type { SeleanEditor } from "../wasm/types";
import type { ConnectionStatus } from "../collab/ws-client";
import type { Participant } from "../collab/types";

type RightView = "design" | "code";

interface HeaderBarProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  status: string;
  error: string | null;
  rightView: RightView;
  onRightViewChange: (view: RightView) => void;
  canUndo: boolean;
  canRedo: boolean;
  onUndo: () => void;
  onRedo: () => void;
  showLayers: boolean;
  onToggleLayers: () => void;
  onShowMembers: () => void;
  onSceneChanged: () => void;
  onClearAutoSave: () => void;
  collabStatus: ConnectionStatus;
  collabParticipants: Participant[];
  collabHasPendingOps: boolean;
  onCollabConnect: () => void;
  onCollabDisconnect: () => void;
}

export const HeaderBar = React.memo(function HeaderBar({
  editorRef,
  status,
  error,
  rightView,
  onRightViewChange,
  canUndo,
  canRedo,
  onUndo,
  onRedo,
  showLayers,
  onToggleLayers,
  onShowMembers,
  onSceneChanged,
  onClearAutoSave,
  collabStatus,
  collabParticipants,
  collabHasPendingOps,
  onCollabConnect,
  onCollabDisconnect,
}: HeaderBarProps) {
  const { user } = useAuth();
  const { mode: themeMode, toggle: toggleTheme } = useTheme();

  return (
    <header style={headerStyle}>
      <div style={headerLeftStyle}>
        <span style={logoStyle}>Selean</span>
        {status === "ready" && (
          <FileMenu
            editorRef={editorRef}
            onSceneChanged={onSceneChanged}
            onClearAutoSave={onClearAutoSave}
          />
        )}
        {status === "ready" && <WorkspaceSelector />}
      </div>

      <div style={headerCenterStyle}>
        {status === "ready" && (
          <div style={viewToggleStyle}>
            <button
              style={{
                ...viewToggleBtnStyle,
                ...(rightView === "design" ? viewToggleActiveStyle : {}),
              }}
              onClick={() => onRightViewChange("design")}
            >
              Design
            </button>
            <button
              style={{
                ...viewToggleBtnStyle,
                ...(rightView === "code" ? viewToggleActiveStyle : {}),
              }}
              onClick={() => onRightViewChange("code")}
            >
              Code
            </button>
          </div>
        )}
        {status !== "ready" && (
          <span style={statusStyle}>
            {status === "loading" && "Initializing..."}
            {status === "error" && `Error: ${error}`}
            {status === "unsupported" && "WebGPU not supported"}
          </span>
        )}
      </div>

      <div style={headerRightStyle}>
        {status === "ready" && (
          <>
            <button
              data-interactive
              style={{
                ...headerBtnStyle,
                opacity: canUndo ? 1 : 0.4,
                cursor: canUndo ? "pointer" : "default",
              }}
              onClick={onUndo}
              disabled={!canUndo}
              title="Undo"
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <polyline points="1 4 1 10 7 10" />
                <path d="M3.51 15a9 9 0 1 0 2.13-9.36L1 10" />
              </svg>
            </button>
            <button
              data-interactive
              style={{
                ...headerBtnStyle,
                opacity: canRedo ? 1 : 0.4,
                cursor: canRedo ? "pointer" : "default",
              }}
              onClick={onRedo}
              disabled={!canRedo}
              title="Redo"
            >
              <svg
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
              >
                <polyline points="23 4 23 10 17 10" />
                <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" />
              </svg>
            </button>
            <CollabBar
              status={collabStatus}
              participants={collabParticipants}
              hasPendingOps={collabHasPendingOps}
              onConnect={onCollabConnect}
              onDisconnect={onCollabDisconnect}
            />
            <button
              data-interactive
              style={{
                ...headerBtnStyle,
                ...(showLayers
                  ? {
                      background: colors.accentLight,
                      color: colors.accent,
                      borderColor: colors.accent,
                    }
                  : {}),
              }}
              onClick={onToggleLayers}
              title="Toggle layers panel"
            >
              Layers
            </button>
            <button
              data-interactive
              style={headerBtnStyle}
              onClick={onShowMembers}
            >
              Share
            </button>
            <button
              data-interactive
              style={headerBtnStyle}
              onClick={toggleTheme}
              title={`Switch to ${themeMode === "light" ? "dark" : "light"} mode`}
            >
              {themeMode === "light" ? (
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
                </svg>
              ) : (
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <circle cx="12" cy="12" r="5" />
                  <line x1="12" y1="1" x2="12" y2="3" />
                  <line x1="12" y1="21" x2="12" y2="23" />
                  <line x1="4.22" y1="4.22" x2="5.64" y2="5.64" />
                  <line x1="18.36" y1="18.36" x2="19.78" y2="19.78" />
                  <line x1="1" y1="12" x2="3" y2="12" />
                  <line x1="21" y1="12" x2="23" y2="12" />
                  <line x1="4.22" y1="19.78" x2="5.64" y2="18.36" />
                  <line x1="18.36" y1="5.64" x2="19.78" y2="4.22" />
                </svg>
              )}
            </button>
            {user && <span style={userStyle}>{user.display_name}</span>}
          </>
        )}
      </div>
    </header>
  );
});

// --- Styles ---

const headerStyle: React.CSSProperties = {
  height: 48,
  display: "flex",
  alignItems: "center",
  padding: `0 ${spacing.lg}px`,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
  background: colors.bg,
  gap: spacing.lg,
};

const headerLeftStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.md,
  flex: 1,
};

const logoStyle: React.CSSProperties = {
  fontWeight: 700,
  fontSize: fontSizes.xl,
  color: colors.accent,
  letterSpacing: "-0.02em",
};

const headerCenterStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const viewToggleStyle: React.CSSProperties = {
  display: "flex",
  background: colors.surface,
  borderRadius: radii.md,
  padding: 2,
  border: `1px solid ${colors.border}`,
};

const viewToggleBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  padding: `${spacing.xs}px ${spacing.lg}px`,
  fontSize: fontSizes.sm,
  fontWeight: 500,
  color: colors.textDim,
  cursor: "pointer",
  borderRadius: radii.md - 2,
};

const viewToggleActiveStyle: React.CSSProperties = {
  background: colors.bg,
  color: colors.text,
  fontWeight: 600,
  boxShadow: shadows.sm,
};

const statusStyle: React.CSSProperties = {
  color: colors.textDim,
  fontSize: fontSizes.sm,
};

const headerRightStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.sm,
  flex: 1,
  justifyContent: "flex-end",
};

const headerBtnStyle: React.CSSProperties = {
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  color: colors.text,
  padding: `${spacing.xs}px ${spacing.md}px`,
  borderRadius: radii.md,
  cursor: "pointer",
  fontSize: fontSizes.sm,
  fontWeight: 500,
  display: "flex",
  alignItems: "center",
  gap: spacing.xs,
};

const userStyle: React.CSSProperties = {
  fontSize: fontSizes.sm,
  color: colors.textDim,
};
