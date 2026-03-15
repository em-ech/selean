import { useCallback, useEffect, useRef } from "react";
import { colors, fontSizes, radii, shadows, spacing } from "../theme";
import type { SeleanEditor } from "../wasm/types";
import { useChatEngine } from "../hooks/useChatEngine";

const SUGGESTION_CHIPS = [
  "Add a title and subtitle",
  "Create a 3-column layout",
  "Make a social media post",
  "Add a background image",
  "Change the color scheme",
];

interface ChatPanelProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  isOpen: boolean;
  onToggle: () => void;
  /** Dynamic width from resize handle. */
  width?: number;
}

/**
 * Lovable-style chat panel. Full-height, toggleable.
 * Uses useChatEngine for all LLM interaction logic.
 */
export function ChatPanel({
  editorRef,
  onSceneChanged,
  isOpen,
  onToggle,
  width,
}: ChatPanelProps) {
  const {
    displayMessages,
    input,
    setInput,
    loading,
    streamingText,
    activeTools,
    sendMessage,
    abort,
  } = useChatEngine({ editorRef, onSceneChanged });

  const messagesEndRef = useRef<HTMLDivElement>(null);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  useEffect(() => {
    scrollToBottom();
  }, [displayMessages, streamingText, scrollToBottom]);

  const handleChipClick = useCallback(
    (chip: string) => {
      sendMessage(chip);
    },
    [sendMessage],
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        sendMessage();
      }
    },
    [sendMessage],
  );

  if (!isOpen) {
    return (
      <button
        data-interactive
        onClick={onToggle}
        style={collapsedBtnStyle}
        title="Open chat (Cmd+J)"
      >
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
        >
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
        </svg>
      </button>
    );
  }

  const isEmpty = displayMessages.length === 0 && streamingText === null;

  return (
    <div style={{ ...panelStyle, ...(width ? { width } : {}) }}>
      <div style={headerStyle}>
        <span style={headerTitleStyle}>Chat</span>
        <button
          data-interactive
          onClick={onToggle}
          style={closeBtnStyle}
          title="Close chat"
        >
          <svg
            width="14"
            height="14"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path d="M15 18l-6-6 6-6" />
          </svg>
        </button>
      </div>

      <div style={messagesStyle}>
        {isEmpty && (
          <div style={emptyStateStyle}>
            <div style={emptyTitleStyle}>Design with AI</div>
            <div style={emptySubtitleStyle}>
              Describe what you want to create or change. Selean will modify the
              canvas in real time.
            </div>
            <div style={chipsContainerStyle}>
              {SUGGESTION_CHIPS.map((chip) => (
                <button
                  key={chip}
                  onClick={() => handleChipClick(chip)}
                  style={chipStyle}
                >
                  {chip}
                </button>
              ))}
            </div>
          </div>
        )}

        {displayMessages.map((msg, i) => (
          <div
            key={i}
            style={
              msg.role === "user" ? userMsgWrapStyle : assistantMsgWrapStyle
            }
          >
            <div style={msg.role === "user" ? userMsgStyle : assistantMsgStyle}>
              {msg.text}
            </div>
          </div>
        ))}

        {activeTools.length > 0 && (
          <div style={toolsContainerStyle}>
            {activeTools.map((tool, i) => (
              <div key={i} style={toolBadgeStyle}>
                <span style={toolDotStyle(tool.status)} />
                {tool.name.replace(/_/g, " ")}
              </div>
            ))}
          </div>
        )}

        {loading && streamingText === null && (
          <div style={assistantMsgWrapStyle}>
            <div style={thinkingStyle}>
              <span style={thinkingDotStyle} />
              Thinking...
            </div>
          </div>
        )}

        {streamingText !== null && (
          <div style={assistantMsgWrapStyle}>
            <div style={assistantMsgStyle}>{streamingText}</div>
          </div>
        )}

        <div ref={messagesEndRef} />
      </div>

      <div style={inputAreaStyle}>
        <div style={inputWrapperStyle}>
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Ask Selean..."
            style={inputFieldStyle}
            disabled={loading}
            rows={1}
          />
          <div style={inputActionsStyle}>
            {loading ? (
              <button
                data-interactive
                onClick={abort}
                style={abortBtnStyle}
                title="Stop"
              >
                <svg
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="currentColor"
                >
                  <rect x="6" y="6" width="12" height="12" rx="2" />
                </svg>
              </button>
            ) : (
              <button
                data-interactive
                onClick={() => sendMessage()}
                disabled={!input.trim()}
                style={{
                  ...sendBtnStyle,
                  opacity: !input.trim() ? 0.4 : 1,
                }}
                title="Send"
              >
                <svg
                  width="16"
                  height="16"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <line x1="12" y1="19" x2="12" y2="5" />
                  <polyline points="5 12 12 5 19 12" />
                </svg>
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

// --- Styles ---

const panelStyle: React.CSSProperties = {
  width: 380,
  minWidth: 300,
  maxWidth: 500,
  display: "flex",
  flexDirection: "column",
  background: colors.bg,
  borderRight: `1px solid ${colors.border}`,
  overflow: "hidden",
  flexShrink: 0,
};

const headerStyle: React.CSSProperties = {
  height: 48,
  display: "flex",
  alignItems: "center",
  justifyContent: "space-between",
  padding: `0 ${spacing.lg}px`,
  borderBottom: `1px solid ${colors.border}`,
  flexShrink: 0,
};

const headerTitleStyle: React.CSSProperties = {
  fontWeight: 600,
  fontSize: fontSizes.lg,
  color: colors.text,
};

const closeBtnStyle: React.CSSProperties = {
  background: "transparent",
  border: "none",
  cursor: "pointer",
  color: colors.textDim,
  padding: spacing.xs,
  borderRadius: radii.sm,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
};

const collapsedBtnStyle: React.CSSProperties = {
  width: 40,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "transparent",
  border: "none",
  borderRight: `1px solid ${colors.border}`,
  cursor: "pointer",
  color: colors.textDim,
  padding: `${spacing.md}px 0`,
  flexShrink: 0,
  alignSelf: "stretch",
};

const messagesStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  padding: spacing.lg,
  display: "flex",
  flexDirection: "column",
  gap: spacing.md,
};

const emptyStateStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  alignItems: "center",
  justifyContent: "center",
  flex: 1,
  padding: spacing.xl,
  textAlign: "center",
};

const emptyTitleStyle: React.CSSProperties = {
  fontSize: fontSizes.xl,
  fontWeight: 600,
  color: colors.text,
  marginBottom: spacing.sm,
};

const emptySubtitleStyle: React.CSSProperties = {
  fontSize: fontSizes.md,
  color: colors.textMuted,
  lineHeight: 1.5,
  marginBottom: spacing.xl,
  maxWidth: 280,
};

const chipsContainerStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: spacing.sm,
  justifyContent: "center",
};

const chipStyle: React.CSSProperties = {
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.full,
  padding: `${spacing.xs}px ${spacing.md}px`,
  fontSize: fontSizes.sm,
  color: colors.textMuted,
  cursor: "pointer",
  whiteSpace: "nowrap",
};

const userMsgWrapStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "flex-end",
};

const assistantMsgWrapStyle: React.CSSProperties = {
  display: "flex",
  justifyContent: "flex-start",
};

const userMsgStyle: React.CSSProperties = {
  background: colors.userBubble,
  color: colors.userBubbleText,
  borderRadius: `${radii.lg}px ${radii.lg}px ${radii.sm}px ${radii.lg}px`,
  padding: `${spacing.sm}px ${spacing.md}px`,
  maxWidth: "85%",
  fontSize: fontSizes.md,
  lineHeight: 1.5,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
};

const assistantMsgStyle: React.CSSProperties = {
  background: colors.assistantBubble,
  color: colors.assistantBubbleText,
  borderRadius: `${radii.lg}px ${radii.lg}px ${radii.lg}px ${radii.sm}px`,
  padding: `${spacing.sm}px ${spacing.md}px`,
  maxWidth: "85%",
  fontSize: fontSizes.md,
  lineHeight: 1.5,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
};

const thinkingStyle: React.CSSProperties = {
  ...assistantMsgStyle,
  color: colors.textDim,
  display: "flex",
  alignItems: "center",
  gap: spacing.sm,
};

const thinkingDotStyle: React.CSSProperties = {
  width: 6,
  height: 6,
  borderRadius: "50%",
  background: colors.accent,
  animation: "pulse 1.5s infinite",
};

const toolsContainerStyle: React.CSSProperties = {
  display: "flex",
  flexWrap: "wrap",
  gap: spacing.xs,
  padding: `0 ${spacing.xs}px`,
};

const toolBadgeStyle: React.CSSProperties = {
  display: "inline-flex",
  alignItems: "center",
  gap: spacing.xs,
  background: colors.accentLight,
  color: colors.accent,
  fontSize: fontSizes.xs,
  fontWeight: 500,
  padding: `2px ${spacing.sm}px`,
  borderRadius: radii.full,
};

const toolDotStyle = (
  status: "running" | "done" | "error",
): React.CSSProperties => ({
  width: 6,
  height: 6,
  borderRadius: "50%",
  background:
    status === "running"
      ? colors.accent
      : status === "done"
        ? colors.success
        : colors.danger,
  flexShrink: 0,
});

const inputAreaStyle: React.CSSProperties = {
  padding: spacing.md,
  borderTop: `1px solid ${colors.border}`,
};

const inputWrapperStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "flex-end",
  gap: spacing.sm,
  background: colors.surface,
  border: `1px solid ${colors.border}`,
  borderRadius: radii.lg,
  padding: `${spacing.sm}px ${spacing.md}px`,
  boxShadow: shadows.sm,
};

const inputFieldStyle: React.CSSProperties = {
  flex: 1,
  background: "transparent",
  border: "none",
  color: colors.text,
  fontSize: fontSizes.md,
  lineHeight: 1.5,
  outline: "none",
  resize: "none",
  minHeight: 24,
  maxHeight: 120,
  fontFamily: "inherit",
};

const inputActionsStyle: React.CSSProperties = {
  display: "flex",
  alignItems: "center",
  gap: spacing.xs,
  flexShrink: 0,
};

const sendBtnStyle: React.CSSProperties = {
  width: 32,
  height: 32,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: colors.accent,
  border: "none",
  borderRadius: radii.full,
  cursor: "pointer",
  color: colors.white,
};

const abortBtnStyle: React.CSSProperties = {
  width: 32,
  height: 32,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: colors.danger,
  border: "none",
  borderRadius: radii.full,
  cursor: "pointer",
  color: colors.white,
};
