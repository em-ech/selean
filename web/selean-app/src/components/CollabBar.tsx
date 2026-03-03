/**
 * Displays collaboration status and participant list in the header.
 */

import type { ConnectionStatus } from "../collab/ws-client";
import type { Participant } from "../collab/types";
import { colorForUser } from "./PresenceOverlay";
import { colors, fontSizes } from "../theme";

interface CollabBarProps {
  status: ConnectionStatus;
  participants: Participant[];
  hasPendingOps: boolean;
  onConnect?: () => void;
  onDisconnect?: () => void;
}

export function CollabBar({
  status,
  participants,
  hasPendingOps,
  onConnect,
  onDisconnect,
}: CollabBarProps) {
  return (
    <div style={containerStyle} data-testid="collab-bar">
      <div style={statusContainerStyle}>
        <span
          style={{
            ...dotStyle,
            backgroundColor: status === "connected" ? "#2ecc71" : "#95a5a6",
          }}
          data-testid="collab-status-dot"
        />
        <span style={statusTextStyle} data-testid="collab-status-text">
          {status === "connected" &&
            (hasPendingOps ? "Saving..." : "Connected")}
          {status === "connecting" && "Connecting..."}
          {status === "disconnected" && "Offline"}
        </span>
      </div>

      {status === "disconnected" && onConnect && (
        <button
          style={buttonStyle}
          onClick={onConnect}
          data-testid="collab-connect-btn"
        >
          Join
        </button>
      )}

      {status === "connected" && onDisconnect && (
        <button
          style={buttonStyle}
          onClick={onDisconnect}
          data-testid="collab-disconnect-btn"
        >
          Leave
        </button>
      )}

      {participants.length > 0 && (
        <div style={participantListStyle} data-testid="participant-list">
          {participants.map((p) => (
            <span
              key={p.session_id}
              style={{
                ...avatarStyle,
                backgroundColor: colorForUser(p.user_id),
              }}
              title={p.display_name}
              data-testid={`participant-${p.session_id}`}
            >
              {p.display_name.charAt(0).toUpperCase()}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

const containerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 8,
};

const statusContainerStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: 4,
};

const dotStyle: React.CSSProperties = {
  width: 8,
  height: 8,
  borderRadius: "50%",
  flexShrink: 0,
};

const statusTextStyle: React.CSSProperties = {
  fontSize: fontSizes.sm,
  color: colors.textDim,
};

const buttonStyle: React.CSSProperties = {
  background: colors.border,
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: "2px 8px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: fontSizes.sm,
};

const participantListStyle: React.CSSProperties = {
  display: "flex",
  gap: 2,
};

const avatarStyle: React.CSSProperties = {
  width: 24,
  height: 24,
  borderRadius: "50%",
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  color: "#fff",
  fontSize: 12,
  fontWeight: 600,
};
