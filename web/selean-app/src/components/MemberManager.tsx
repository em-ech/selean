import { useCallback, useState } from "react";
import { useWorkspace } from "../hooks/useWorkspace";
import { colors, fontSizes } from "../theme";

interface MemberManagerProps {
  onClose: () => void;
}

const ROLES = ["viewer", "editor", "admin"];

export function MemberManager({ onClose }: MemberManagerProps) {
  const {
    activeWorkspace,
    members,
    userRole,
    inviteMember,
    updateMemberRole,
    removeMember,
  } = useWorkspace();
  const [inviteEmail, setInviteEmail] = useState("");
  const [inviteRole, setInviteRole] = useState("editor");
  const [error, setError] = useState<string | null>(null);

  const isAdmin = userRole === "admin" || userRole === "owner";

  const handleInvite = useCallback(async () => {
    const trimmed = inviteEmail.trim();
    if (!trimmed) return;
    setError(null);
    try {
      await inviteMember(trimmed, inviteRole);
      setInviteEmail("");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to invite member");
    }
  }, [inviteEmail, inviteRole, inviteMember]);

  const handleRoleChange = useCallback(
    async (userId: string, role: string) => {
      try {
        await updateMemberRole(userId, role);
      } catch (err) {
        setError(err instanceof Error ? err.message : "Failed to update role");
      }
    },
    [updateMemberRole],
  );

  const handleRemove = useCallback(
    async (userId: string) => {
      try {
        await removeMember(userId);
      } catch (err) {
        setError(
          err instanceof Error ? err.message : "Failed to remove member",
        );
      }
    },
    [removeMember],
  );

  return (
    <div style={overlayStyle} onClick={onClose}>
      <div style={modalStyle} onClick={(e) => e.stopPropagation()}>
        <div style={headerStyle}>
          <span style={titleStyle}>
            {activeWorkspace?.name ?? "Workspace"} Members
          </span>
          <button style={closeBtnStyle} onClick={onClose} aria-label="Close">
            x
          </button>
        </div>

        <div style={memberListStyle}>
          {members.map((member) => {
            const isOwner = member.role === "owner";
            return (
              <div
                key={member.user_id}
                style={{
                  ...memberRowStyle,
                  ...(isOwner ? ownerRowStyle : {}),
                }}
              >
                <div style={memberInfoStyle}>
                  <span style={memberNameStyle}>{member.display_name}</span>
                  <span style={memberEmailStyle}>{member.email}</span>
                </div>
                {isOwner ? (
                  <span style={ownerBadgeStyle}>Owner</span>
                ) : isAdmin ? (
                  <div style={memberActionsStyle}>
                    <select
                      style={roleSelectStyle}
                      value={member.role}
                      onChange={(e) =>
                        void handleRoleChange(member.user_id, e.target.value)
                      }
                    >
                      {ROLES.map((r) => (
                        <option key={r} value={r}>
                          {r}
                        </option>
                      ))}
                    </select>
                    <button
                      style={removeBtnStyle}
                      onClick={() => void handleRemove(member.user_id)}
                    >
                      Remove
                    </button>
                  </div>
                ) : (
                  <span style={roleLabelStyle}>{member.role}</span>
                )}
              </div>
            );
          })}
        </div>

        {isAdmin && (
          <div style={inviteFormStyle}>
            <div style={inviteRowStyle}>
              <input
                style={inviteInputStyle}
                type="email"
                placeholder="Email address"
                value={inviteEmail}
                onChange={(e) => setInviteEmail(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") void handleInvite();
                }}
              />
              <select
                style={roleSelectStyle}
                value={inviteRole}
                onChange={(e) => setInviteRole(e.target.value)}
              >
                {ROLES.map((r) => (
                  <option key={r} value={r}>
                    {r}
                  </option>
                ))}
              </select>
              <button
                style={inviteBtnStyle}
                onClick={() => void handleInvite()}
              >
                Invite
              </button>
            </div>
          </div>
        )}

        {error && <div style={errorStyle}>{error}</div>}
      </div>
    </div>
  );
}

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
  width: 480,
  maxHeight: "70vh",
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

const memberListStyle: React.CSSProperties = {
  flex: 1,
  overflowY: "auto",
  padding: "8px 0",
};

const memberRowStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: "8px 16px",
};

const ownerRowStyle: React.CSSProperties = {
  background: colors.surfaceAlt,
};

const memberInfoStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  gap: 2,
};

const memberNameStyle: React.CSSProperties = {
  color: colors.text,
  fontSize: fontSizes.base,
};

const memberEmailStyle: React.CSSProperties = {
  color: colors.textDim,
  fontSize: fontSizes.sm,
};

const ownerBadgeStyle: React.CSSProperties = {
  color: colors.textMuted,
  fontSize: fontSizes.sm,
  fontWeight: 600,
  padding: "2px 8px",
  border: `1px solid ${colors.border}`,
  borderRadius: 4,
};

const memberActionsStyle: React.CSSProperties = {
  display: "flex",
  gap: 6,
  alignItems: "center",
};

const roleSelectStyle: React.CSSProperties = {
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  color: colors.text,
  padding: "3px 6px",
  borderRadius: 4,
  fontSize: fontSizes.sm,
};

const roleLabelStyle: React.CSSProperties = {
  color: colors.textMuted,
  fontSize: fontSizes.sm,
};

const removeBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: `1px solid ${colors.border}`,
  color: "#e55",
  padding: "3px 8px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: fontSizes.sm,
};

const inviteFormStyle: React.CSSProperties = {
  padding: "12px 16px",
  borderTop: `1px solid ${colors.border}`,
};

const inviteRowStyle: React.CSSProperties = {
  display: "flex",
  gap: 6,
  alignItems: "center",
};

const inviteInputStyle: React.CSSProperties = {
  flex: 1,
  background: colors.bg,
  border: `1px solid ${colors.border}`,
  color: colors.text,
  padding: "6px 8px",
  borderRadius: 4,
  fontSize: fontSizes.base,
  outline: "none",
};

const inviteBtnStyle: React.CSSProperties = {
  background: colors.accent,
  border: "none",
  color: colors.text,
  padding: "6px 12px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: fontSizes.sm,
};

const errorStyle: React.CSSProperties = {
  color: "#e55",
  fontSize: fontSizes.sm,
  padding: "8px 16px",
};
