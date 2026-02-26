import { useCallback, useEffect, useRef, useState } from "react";
import { colors, fontSizes } from "../theme";
import type { ChatMessage, SeleanEditor } from "../wasm/types";

interface ChatSidebarProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

/**
 * Left sidebar with chat interface for LLM-driven design modifications.
 * Sends messages to the server, which proxies to Claude API.
 * Tool calls are executed against the WASM editor.
 */
export function ChatSidebar({ editorRef, onSceneChanged }: ChatSidebarProps) {
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);

  // Cancel in-flight request on unmount
  useEffect(() => {
    return () => {
      abortRef.current?.abort();
    };
  }, []);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  const sendMessage = useCallback(async () => {
    const text = input.trim();
    if (!text || loading) return;

    const editor = editorRef.current;
    if (!editor) return;

    // Abort any previous in-flight request
    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    const userMessage: ChatMessage = { role: "user", content: text };
    setMessages((prev) => [...prev, userMessage]);
    setInput("");
    setLoading(true);

    try {
      const sceneSummary = editor.get_scene_json();
      const allMessages = [...messages, userMessage];

      const response = await fetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        signal: controller.signal,
        body: JSON.stringify({
          messages: allMessages.map((m) => ({
            role: m.role,
            content: m.content,
          })),
          scene_summary: sceneSummary,
        }),
      });

      if (!response.ok) {
        throw new Error(`Server error: ${response.status}`);
      }

      const reader = response.body?.getReader();
      if (!reader) {
        throw new Error("No response body");
      }

      let assistantText = "";
      const decoder = new TextDecoder();
      let buffer = "";

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() ?? "";

        for (const line of lines) {
          if (!line.startsWith("data: ")) continue;
          const data = line.slice(6).trim();
          if (!data) continue;

          try {
            const event = JSON.parse(data);

            if (event.type === "text") {
              assistantText += event.text;
            } else if (event.type === "tool_use") {
              const result = editor.execute_command(
                JSON.stringify({
                  type: toolNameToCommandType(event.name),
                  ...event.input,
                }),
              );
              if (result) {
                onSceneChanged();
              }
            }
          } catch {
            // Skip malformed SSE events
          }
        }
      }

      if (assistantText) {
        setMessages((prev) => [
          ...prev,
          { role: "assistant", content: assistantText },
        ]);
      }
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") {
        return;
      }
      const message = err instanceof Error ? err.message : String(err);
      setMessages((prev) => [
        ...prev,
        { role: "assistant", content: `Error: ${message}` },
      ]);
    } finally {
      abortRef.current = null;
      setLoading(false);
      scrollToBottom();
    }
  }, [input, loading, messages, editorRef, onSceneChanged, scrollToBottom]);

  return (
    <div style={panelStyle}>
      <div style={headerStyle}>Chat</div>
      <div style={messagesStyle}>
        {messages.length === 0 && (
          <div style={emptyStyle}>
            Ask me to modify the design. For example: "Make the red box blue" or
            "Add a new green rectangle".
          </div>
        )}
        {messages.map((msg, i) => (
          <div
            key={i}
            style={msg.role === "user" ? userMsgStyle : assistantMsgStyle}
          >
            <div style={msgRoleStyle}>
              {msg.role === "user" ? "You" : "Assistant"}
            </div>
            <div style={msgTextStyle}>{msg.content}</div>
          </div>
        ))}
        {loading && (
          <div style={assistantMsgStyle}>
            <div style={msgRoleStyle}>Assistant</div>
            <div style={{ ...msgTextStyle, color: colors.textDim }}>
              Thinking...
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>
      <div style={inputAreaStyle}>
        <input
          type="text"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              sendMessage();
            }
          }}
          placeholder="Describe a change..."
          style={inputFieldStyle}
          disabled={loading}
        />
        <button
          onClick={sendMessage}
          disabled={loading || !input.trim()}
          style={{
            ...sendBtnStyle,
            opacity: loading || !input.trim() ? 0.5 : 1,
          }}
        >
          Send
        </button>
      </div>
    </div>
  );
}

/**
 * Maps LLM tool names to CommandDescriptor type tags.
 * Most tool names match directly; this handles the exceptions.
 */
function toolNameToCommandType(toolName: string): string {
  const mapping: Record<string, string> = {
    set_fill: "SetFill",
    set_bounds: "SetBounds",
    set_text: "SetTextContent",
    set_opacity: "SetOpacity",
    set_visible: "SetVisible",
    set_name: "SetName",
    set_stroke: "SetStroke",
    set_blend_mode: "SetBlendMode",
    create_node: "AddRoot",
    delete_node: "RemoveNode",
  };
  return mapping[toolName] ?? toolName;
}

// --- Styles ---

const panelStyle: React.CSSProperties = {
  width: 320,
  borderRight: `1px solid ${colors.border}`,
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

const messagesStyle: React.CSSProperties = {
  flex: 1,
  overflow: "auto",
  padding: 12,
  display: "flex",
  flexDirection: "column",
  gap: 8,
};

const emptyStyle: React.CSSProperties = {
  color: colors.textFaint,
  fontSize: fontSizes.md,
  lineHeight: 1.5,
  padding: "20px 0",
};

const userMsgStyle: React.CSSProperties = {
  background: colors.border,
  borderRadius: 8,
  padding: "8px 12px",
  alignSelf: "flex-end",
  maxWidth: "85%",
};

const assistantMsgStyle: React.CSSProperties = {
  background: colors.surfaceAlt,
  borderRadius: 8,
  padding: "8px 12px",
  alignSelf: "flex-start",
  maxWidth: "85%",
};

const msgRoleStyle: React.CSSProperties = {
  fontSize: fontSizes.xs,
  fontWeight: 600,
  color: colors.textDim,
  marginBottom: 4,
  textTransform: "uppercase",
  letterSpacing: "0.05em",
};

const msgTextStyle: React.CSSProperties = {
  fontSize: fontSizes.md,
  lineHeight: 1.5,
  whiteSpace: "pre-wrap",
  wordBreak: "break-word",
};

const inputAreaStyle: React.CSSProperties = {
  padding: 12,
  borderTop: `1px solid ${colors.border}`,
  display: "flex",
  gap: 8,
};

const inputFieldStyle: React.CSSProperties = {
  flex: 1,
  background: colors.surface,
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: "8px 12px",
  borderRadius: 6,
  fontSize: fontSizes.md,
  outline: "none",
};

const sendBtnStyle: React.CSSProperties = {
  background: colors.accent,
  border: "none",
  color: colors.text,
  padding: "8px 16px",
  borderRadius: 6,
  cursor: "pointer",
  fontSize: fontSizes.md,
  fontWeight: 600,
};
