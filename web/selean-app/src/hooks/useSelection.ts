import { useCallback, useEffect, useRef, useState } from "react";
import type { NodeInfo, SeleanEditor } from "../wasm/types";

/** Polling interval in milliseconds. 100ms = 10Hz, sufficient for UI updates. */
const POLL_INTERVAL_MS = 100;

/**
 * Tracks the currently selected node and provides its full info.
 *
 * Polls the WASM editor at a throttled interval for selection changes.
 * When a node is selected, fetches its full properties for the inspector.
 *
 * Returns the selected NodeInfo (or null) and a `refresh` callback that
 * can be called after mutations to force an immediate update.
 */
export function useSelection(
  editorRef: React.RefObject<SeleanEditor | null>,
  ready: boolean,
) {
  const [selectedNode, setSelectedNode] = useState<NodeInfo | null>(null);
  const lastSelectedIdRef = useRef<string | null>(null);
  const refreshCounterRef = useRef(0);
  const [, setRefreshCounter] = useState(0);

  const refresh = useCallback(() => {
    refreshCounterRef.current += 1;
    setRefreshCounter(refreshCounterRef.current);
  }, []);

  useEffect(() => {
    if (!ready) return;

    let timerId: ReturnType<typeof setTimeout> | null = null;

    function poll() {
      const editor = editorRef.current;
      if (!editor) {
        timerId = setTimeout(poll, POLL_INTERVAL_MS);
        return;
      }

      try {
        const idsJson = editor.get_selected_ids();
        const ids: string[] = JSON.parse(idsJson);

        if (ids.length === 0) {
          if (lastSelectedIdRef.current !== null) {
            lastSelectedIdRef.current = null;
            setSelectedNode(null);
          }
        } else {
          const firstId = ids[0];
          if (
            firstId !== lastSelectedIdRef.current ||
            refreshCounterRef.current > 0
          ) {
            lastSelectedIdRef.current = firstId;
            refreshCounterRef.current = 0;
            const nodeJson = editor.get_node_json(firstId);
            if (nodeJson !== "null") {
              const node: NodeInfo = JSON.parse(nodeJson);
              setSelectedNode(node);
            } else {
              setSelectedNode(null);
            }
          }
        }
      } catch {
        // WASM call failed (e.g. during teardown). Skip this poll cycle.
      }

      timerId = setTimeout(poll, POLL_INTERVAL_MS);
    }

    timerId = setTimeout(poll, 0);
    return () => {
      if (timerId !== null) {
        clearTimeout(timerId);
      }
    };
  }, [ready, editorRef]);

  return { selectedNode, refresh };
}
