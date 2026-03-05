import { useCallback, useRef, useState } from "react";
import type { NodeInfo, SeleanEditor } from "../wasm/types";

/**
 * Tracks the currently selected node and provides its full info.
 *
 * Event-driven: call `refresh()` after any interaction or mutation that
 * may change selection state. Does not poll.
 *
 * Returns the selected NodeInfo (or null) and a `refresh` callback.
 */
export function useSelection(
  editorRef: React.RefObject<SeleanEditor | null>,
  ready: boolean,
) {
  const [selectedNode, setSelectedNode] = useState<NodeInfo | null>(null);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const lastSelectedIdRef = useRef<string | null>(null);

  const refresh = useCallback(() => {
    if (!ready) return;
    const editor = editorRef.current;
    if (!editor) return;

    try {
      const idsJson = editor.get_selected_ids();
      const ids: string[] = JSON.parse(idsJson);

      setSelectedIds(ids);

      if (ids.length === 0) {
        if (lastSelectedIdRef.current !== null) {
          lastSelectedIdRef.current = null;
          setSelectedNode(null);
        }
      } else {
        const firstId = ids[0];
        if (firstId !== lastSelectedIdRef.current) {
          lastSelectedIdRef.current = firstId;
        }
        // Always re-read node properties on refresh (properties may have changed).
        const nodeJson = editor.get_node_json(firstId);
        if (nodeJson !== "null") {
          const node: NodeInfo = JSON.parse(nodeJson);
          setSelectedNode(node);
        } else {
          setSelectedNode(null);
        }
      }
    } catch (e) {
      console.warn("selection:poll failed", e);
    }
  }, [ready, editorRef]);

  return { selectedNode, selectedIds, refresh };
}
