/**
 * Renders remote participant cursors and selection highlights on the canvas.
 *
 * Cursor positions (world coordinates) are transformed to screen coordinates
 * using the current camera state.
 */

import type { SeleanEditor } from "../wasm/types";
import type { CursorPosition, Participant } from "../collab/types";
import { colors } from "../theme";
import { worldToScreen } from "../utils/camera";

/** Presence data for a single remote participant. */
export interface RemotePresence {
  participant: Participant;
  pageId: string;
  cursor: CursorPosition | null;
  selectedNodeIds: string[];
}

interface PresenceOverlayProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  presences: RemotePresence[];
  activePageId: string;
}

/** Deterministic color from a user ID string. */
export function colorForUser(userId: string): string {
  const PRESENCE_COLORS = [
    "#e74c3c",
    "#3498db",
    "#2ecc71",
    "#f39c12",
    "#9b59b6",
    "#1abc9c",
    "#e67e22",
    "#e91e63",
  ];
  let hash = 0;
  for (let i = 0; i < userId.length; i++) {
    hash = (hash * 31 + userId.charCodeAt(i)) | 0;
  }
  return PRESENCE_COLORS[Math.abs(hash) % PRESENCE_COLORS.length];
}

export function PresenceOverlay({
  editorRef,
  presences,
  activePageId,
}: PresenceOverlayProps) {
  const editor = editorRef.current;
  if (!editor) return null;

  let camera: CameraInfo;
  try {
    camera = editor.get_camera();
  } catch (e) {
    console.warn("presence-overlay:get-camera failed", e);
    return null;
  }

  // Filter to participants on the same page.
  const visiblePresences = presences.filter(
    (p) => p.pageId === activePageId && p.cursor !== null,
  );

  if (visiblePresences.length === 0) return null;

  return (
    <div style={overlayStyle} data-testid="presence-overlay">
      {visiblePresences.map((presence) => {
        const cursor = presence.cursor;
        if (!cursor) return null;
        const screen = worldToScreen(cursor.x, cursor.y, camera);
        const userColor = colorForUser(presence.participant.user_id);

        return (
          <div
            key={presence.participant.session_id}
            style={{
              position: "absolute",
              left: screen.x,
              top: screen.y,
              pointerEvents: "none",
            }}
            data-testid={`cursor-${presence.participant.session_id}`}
          >
            <svg
              width="16"
              height="20"
              viewBox="0 0 16 20"
              style={{ display: "block" }}
            >
              <path
                d="M0 0 L16 12 L6 12 L0 20 Z"
                fill={userColor}
                stroke={colors.bg}
                strokeWidth="1"
              />
            </svg>
            <span
              style={{
                ...labelStyle,
                backgroundColor: userColor,
              }}
            >
              {presence.participant.display_name}
            </span>
          </div>
        );
      })}
    </div>
  );
}

const overlayStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  pointerEvents: "none",
  overflow: "hidden",
  zIndex: 50,
};

const labelStyle: React.CSSProperties = {
  display: "inline-block",
  color: "#fff",
  fontSize: 11,
  padding: "1px 4px",
  borderRadius: 3,
  marginLeft: 4,
  whiteSpace: "nowrap",
};
