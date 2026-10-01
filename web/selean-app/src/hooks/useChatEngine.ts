import { useCallback, useEffect, useRef, useState } from "react";
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
export interface ConversationMessage {
  role: "user" | "assistant";
  content: unknown;
}

/** Display-only message shown in the chat UI. */
export interface DisplayMessage {
  role: "user" | "assistant";
  text: string;
}

/** Tool execution event for UI display. */
export interface ToolExecution {
  name: string;
  status: "running" | "done" | "error";
}

interface UseChatEngineOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
}

interface UseChatEngineReturn {
  displayMessages: DisplayMessage[];
  input: string;
  setInput: (value: string) => void;
  loading: boolean;
  streamingText: string | null;
  activeTools: ToolExecution[];
  sendMessage: (textOverride?: string) => Promise<void>;
  abort: () => void;
  clearHistory: () => void;
}

/**
 * Extracted chat engine logic. Handles SSE streaming, client-driven tool
 * loop, conversation state management, and tool execution. UI-agnostic;
 * used by ChatPanel for rendering.
 */
export function useChatEngine({
  editorRef,
  onSceneChanged,
}: UseChatEngineOptions): UseChatEngineReturn {
  const [displayMessages, setDisplayMessages] = useState<DisplayMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [streamingText, setStreamingText] = useState<string | null>(null);
  const [activeTools, setActiveTools] = useState<ToolExecution[]>([]);
  const abortRef = useRef<AbortController | null>(null);
  const conversationRef = useRef<ConversationMessage[]>([]);

  useEffect(() => {
    return () => {
      abortRef.current?.abort();
    };
  }, []);

  const abort = useCallback(() => {
    abortRef.current?.abort();
    abortRef.current = null;
    setLoading(false);
    setStreamingText((current) => {
      if (current) {
        setDisplayMessages((prev) => [
          ...prev,
          { role: "assistant", text: current },
        ]);
      }
      return null;
    });
    setActiveTools([]);
  }, []);

  const clearHistory = useCallback(() => {
    conversationRef.current = [];
    setDisplayMessages([]);
    setStreamingText(null);
    setActiveTools([]);
  }, []);

  /**
   * Sends a single request to the server and reads back the SSE stream.
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

          let event: ChatEvent;
          try {
            event = JSON.parse(data);
          } catch (e) {
            console.warn("chat:sse-parse failed", e);
            continue;
          }
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
          } else if (event.type === "error") {
            // The server streams failures (missing API key, Claude API
            // errors) as an error event; surface it in the chat.
            throw new Error(event.message);
          }
        }
      }

      return { events, rawBlocks, stopReason };
    },
    [],
  );

  const sendMessage = useCallback(
    async (textOverride?: string) => {
      const text = (textOverride ?? input).trim();
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
      setActiveTools([]);

      const onTextDelta = (delta: string) => {
        setStreamingText((prev) => (prev === null ? delta : prev + delta));
      };

      try {
        let sceneSummary = editor.get_scene_json();

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

          const consolidated = consolidateTextBlocks(rawBlocks);

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

            setActiveTools((prev) => [
              ...prev,
              { name: event.name, status: "running" },
            ]);

            const resultJson = editor.execute_tool_call(
              event.name,
              JSON.stringify(event.input),
            );
            onSceneChanged();

            setActiveTools((prev) =>
              prev.map((t) =>
                t.name === event.name && t.status === "running"
                  ? { ...t, status: "done" }
                  : t,
              ),
            );

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

          sceneSummary = editor.get_scene_json();

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
        setActiveTools([]);
      }
    },
    [input, loading, editorRef, onSceneChanged, sendRequest],
  );

  return {
    displayMessages,
    input,
    setInput,
    loading,
    streamingText,
    activeTools,
    sendMessage,
    abort,
    clearHistory,
  };
}
