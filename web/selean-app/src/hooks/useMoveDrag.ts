import { useCallback, useRef } from "react";
import type { InteractionEvent } from "../components/Canvas";
import type { NodeInfo, SeleanEditor } from "../wasm/types";

interface DragState {
  nodeIds: string[];
  /** Accumulated total delta since drag start (for absolute positioning). */
  accDx: number;
  accDy: number;
}

interface UseMoveDragOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged: () => void;
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
export function useMoveDrag({ editorRef, onSceneChanged }: UseMoveDragOptions) {
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
          dragRef.current = { nodeIds: ids, accDx: 0, accDy: 0 };
        } catch {
          // parse failure
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
            editor.execute_tool_call(
              "set_bounds",
              JSON.stringify({
                node_id: id,
                x: newBounds.x,
                y: newBounds.y,
                width: newBounds.width,
                height: newBounds.height,
              }),
            );
          } catch {
            // node read failed
          }
        }
        onSceneChanged();
        return;
      }

      if (event.type === "DragEnded") {
        if (!dragRef.current) return;
        dragRef.current = null;
        editor.end_group();
        onSceneChanged();
      }
    },
    [editorRef, onSceneChanged],
  );

  return { handleDragEvent };
}
