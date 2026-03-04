import { useCallback, useRef } from "react";
import type { CollabSession } from "./useCollabSession";
import type { InteractionEvent } from "../components/Canvas";
import type { NodeInfo, SeleanEditor } from "../wasm/types";

interface DragState {
  nodeIds: string[];
  /** Accumulated total delta since drag start (for absolute positioning). */
  accDx: number;
  accDy: number;
  /** Descriptors applied during this drag, for collab group submission. */
  descriptors: Record<string, unknown>[];
}

interface UseMoveDragOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
  collab?: CollabSession | null;
  activePageId?: string;
}

/**
 * Applies a world-space delta to bounds, returning new position values.
 * Pure function exported for testing.
 */
export function applyMoveDelta(
  bounds: { x: number; y: number; width: number; height: number },
  dx: number,
  dy: number,
): { x: number; y: number; width: number; height: number } {
  return {
    x: bounds.x + dx,
    y: bounds.y + dy,
    width: bounds.width,
    height: bounds.height,
  };
}

/**
 * Hook that handles drag-to-move for selected nodes on the canvas.
 *
 * Consumes DragStarted/DragMoved/DragEnded interaction events from the
 * WASM input handler and translates them into SetBounds commands grouped
 * as a single undo entry.
 */
export function useMoveDrag({
  editorRef,
  onSceneChanged,
  collab,
  activePageId,
}: UseMoveDragOptions) {
  const dragRef = useRef<DragState | null>(null);

  const handleDragEvent = useCallback(
    (event: InteractionEvent) => {
      const editor = editorRef.current;
      if (!editor) return;

      if (event.type === "DragStarted") {
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length === 0) return;
          editor.begin_group("Move");
          dragRef.current = {
            nodeIds: ids,
            accDx: 0,
            accDy: 0,
            descriptors: [],
          };
        } catch (e) {
          console.warn("move-drag:begin failed", e);
        }
        return;
      }

      if (event.type === "DragMoved") {
        const drag = dragRef.current;
        if (!drag) return;

        const dx = (event.delta_x as number) ?? 0;
        const dy = (event.delta_y as number) ?? 0;
        if (dx === 0 && dy === 0) return;

        drag.accDx += dx;
        drag.accDy += dy;

        for (const id of drag.nodeIds) {
          try {
            const json = editor.get_node_json(id);
            if (json === "null") continue;
            const node: NodeInfo = JSON.parse(json);
            const newBounds = applyMoveDelta(node, dx, dy);
            const descriptor = {
              type: "SetBounds",
              node_id: id,
              bounds: {
                x: newBounds.x,
                y: newBounds.y,
                width: newBounds.width,
                height: newBounds.height,
              },
            };
            editor.execute_command(JSON.stringify(descriptor));
            drag.descriptors.push(descriptor);
          } catch (e) {
            console.warn("move-drag:update failed", e);
          }
        }
        onSceneChanged();
        return;
      }

      if (event.type === "DragEnded") {
        const drag = dragRef.current;
        if (!drag) return;
        dragRef.current = null;
        editor.end_group();
        if (
          collab?.status === "connected" &&
          activePageId &&
          drag.descriptors.length > 0
        ) {
          collab.submitOpGroup(drag.descriptors, activePageId, "Move");
        }
        onSceneChanged();
      }
    },
    [editorRef, onSceneChanged, collab, activePageId],
  );

  return { handleDragEvent };
}
