import { useCallback, useEffect, useRef, useState } from "react";
import { colors, fontSizes } from "../theme";
import { authFetch } from "../utils/api";
import type { ChatEvent, SeleanEditor } from "../wasm/types";

/** Maximum number of tool-loop iterations before stopping. */
const MAX_TOOL_ITERATIONS = 10;

/**
 * Merges consecutive text content blocks into a single block.
 * Many small streaming text deltas should be one block in conversation
 * history to avoid wasting tokens.
 */
export function consolidateTextBlocks(blocks: unknown[]): unknown[] {
  const result: unknown[] = [];
  for (const block of blocks) {
    const b = block as Record<string, unknown>;
    const last = result[result.length - 1] as
      | Record<string, unknown>
      | undefined;
    if (b.type === "text" && last?.type === "text") {
      last.text = (last.text as string) + (b.text as string);
    } else {
      result.push({ ...b });
    }
  }
  return result;
}

/**
 * A structured message in the conversation history.
 * `content` is either a plain string or an array of content blocks
 * (text / tool_use / tool_result) for Claude API structured messages.
 */
interface ConversationMessage {
  role: "user" | "assistant";
  content: unknown;
}

/** Display-only message shown in the sidebar. */
interface DisplayMessage {
  role: "user" | "assistant";
  text: string;
}

interface ChatSidebarProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

/**
 * Left sidebar with chat interface for LLM-driven design modifications.
 * Sends messages to the server, which proxies to Claude API.
 * Implements a client-driven tool loop: when Claude responds with
 * stop_reason "tool_use", the frontend executes tools locally via
 * execute_tool_call, collects results, and sends a follow-up request
 * with the full conversation until Claude finishes.
 */
export function ChatSidebar({ editorRef, onSceneChanged }: ChatSidebarProps) {
  const [displayMessages, setDisplayMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [streamingText, setStreamingText] = useState<string | null>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const abortRef = useRef<AbortController | null>(null);
  // Full structured conversation for the Claude API.
  const conversationRef = useRef<ConversationMessage[]>([]);

  useEffect(() => {
    return () => {
      abortRef.current?.abort();
    };
  }, []);

  const scrollToBottom = useCallback(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, []);

  /**
   * Sends a single request to the server and reads back the SSE stream.
   * Returns the parsed events and the raw content blocks for structured
   * conversation history. Invokes `onTextDelta` for each text chunk as
   * it arrives so the UI can display streaming text.
   */
  const sendRequest = useCallback(
    async (
      messages: ConversationMessage[],
      sceneSummary: string,
      signal: AbortSignal,
      onTextDelta?: (delta: string) => void,
    ): Promise<{
      events: ChatEvent[];
      rawBlocks: unknown[];
      stopReason: string;
    }> => {
      const response = await authFetch("/api/chat", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        signal,
        body: JSON.stringify({
          messages: messages.map((m) => ({
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

      const events: ChatEvent[] = [];
      const rawBlocks: unknown[] = [];
      let stopReason = "end_turn";
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
            const event: ChatEvent = JSON.parse(data);
            events.push(event);

            if (event.type === "text") {
              rawBlocks.push({ type: "text", text: event.text });
              onTextDelta?.(event.text);
            } else if (event.type === "tool_use") {
              rawBlocks.push({
                type: "tool_use",
                id: event.id,
                name: event.name,
                input: event.input,
              });
            } else if (event.type === "done") {
              stopReason = event.stop_reason;
            }
          } catch (e) {
            console.warn("chat:sse-parse failed", e);
          }
        }
      }

      return { events, rawBlocks, stopReason };
    },
    [],
  );

  const sendMessage = useCallback(async () => {
    const text = input.trim();
    if (!text || loading) return;

    const editor = editorRef.current;
    if (!editor) return;

    abortRef.current?.abort();
    const controller = new AbortController();
    abortRef.current = controller;

    setDisplayMessages((prev) => [...prev, { role: "user", text }]);
    setInput("");
    setLoading(true);
    setStreamingText(null);

    const onTextDelta = (delta: string) => {
      setStreamingText((prev) => (prev === null ? delta : prev + delta));
    };

    try {
      let sceneSummary = editor.get_scene_json();

      // Add user message to structured conversation.
      conversationRef.current = [
        ...conversationRef.current,
        { role: "user", content: text },
      ];

      let iteration = 0;

      while (iteration < MAX_TOOL_ITERATIONS) {
        iteration++;

        const { events, rawBlocks, stopReason } = await sendRequest(
          conversationRef.current,
          sceneSummary,
          controller.signal,
          onTextDelta,
        );

        // Consolidate many small text deltas into fewer blocks.
        const consolidated = consolidateTextBlocks(rawBlocks);

        // Add assistant response to conversation as structured content blocks.
        conversationRef.current = [
          ...conversationRef.current,
          { role: "assistant", content: consolidated },
        ];

        if (stopReason !== "tool_use") {
          break;
        }

        // Execute tool calls and build tool_result messages.
        const toolResults: unknown[] = [];
        for (const event of events) {
          if (event.type !== "tool_use") continue;

          const resultJson = editor.execute_tool_call(
            event.name,
            JSON.stringify(event.input),
          );
          onSceneChanged();

          let resultContent: string;
          try {
            const parsed = JSON.parse(resultJson);
            resultContent = JSON.stringify(parsed.result ?? parsed);
          } catch (e) {
            console.warn("chat:tool-result-parse failed", e);
            resultContent = resultJson;
          }

          toolResults.push({
            type: "tool_result",
            tool_use_id: event.id,
            content: resultContent,
          });
        }

        // Refresh scene summary so Claude sees updated state.
        sceneSummary = editor.get_scene_json();

        // Add tool results as a user message (Claude API expects this).
        conversationRef.current = [
          ...conversationRef.current,
          { role: "user", content: toolResults },
        ];
      }

      // Finalize: move streaming text into display messages.
      setStreamingText((current) => {
        if (current) {
          setDisplayMessages((prev) => [
            ...prev,
            { role: "assistant", text: current },
          ]);
        }
        return null;
      });
    } catch (err) {
      if (err instanceof DOMException && err.name === "AbortError") {
        return;
      }
      const message = err instanceof Error ? err.message : String(err);
      setStreamingText(null);
      setDisplayMessages((prev) => [
        ...prev,
        { role: "assistant", text: `Error: ${message}` },
      ]);
    } finally {
      abortRef.current = null;
      setLoading(false);
      scrollToBottom();
    }
  }, [input, loading, editorRef, onSceneChanged, scrollToBottom, sendRequest]);

  return (
    <div style={panelStyle}>
      <div style={headerStyle}>Chat</div>
      <div style={messagesStyle}>
        {displayMessages.length === 0 && !streamingText && (
          <div style={emptyStyle}>
            Ask me to modify the design. For example: "Make the red box blue" or
            "Add a new green rectangle".
          </div>
        )}
        {displayMessages.map((msg, i) => (
          <div
            key={i}
            style={msg.role === "user" ? userMsgStyle : assistantMsgStyle}
          >
            <div style={msgRoleStyle}>
              {msg.role === "user" ? "You" : "Assistant"}
            </div>
            <div style={msgTextStyle}>{msg.text}</div>
          </div>
        ))}
        {loading && streamingText === null && (
          <div style={assistantMsgStyle}>
            <div style={msgRoleStyle}>Assistant</div>
            <div style={{ ...msgTextStyle, color: colors.textDim }}>
              Thinking...
            </div>
          </div>
        )}
        {streamingText !== null && (
          <div style={assistantMsgStyle}>
            <div style={msgRoleStyle}>Assistant</div>
            <div style={msgTextStyle}>{streamingText}</div>
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
