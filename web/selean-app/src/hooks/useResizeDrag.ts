import { useCallback, useRef, useState } from "react";
import type { CollabSession } from "./useCollabSession";
import type { CameraInfo, SelectionBounds, SeleanEditor } from "../wasm/types";

/** Minimum node size in world units. */
const MIN_SIZE = 10;

interface DragState {
  handleIndex: number;
  nodeId: string;
  originalBounds: { x: number; y: number; width: number; height: number };
  startClientX: number;
  startClientY: number;
  /** Descriptors applied during this resize, for collab group submission. */
  descriptors: Record<string, unknown>[];
}

interface UseResizeDragOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  bounds: SelectionBounds[];
  camera: CameraInfo | null;
  onSceneChanged?: () => void;
  collab?: CollabSession | null;
  activePageId?: string;
}

/**
 * Resize drag hook for selection handles. Returns handle props to spread onto
 * each handle div and an `isDragging` flag to suppress RAF polling flicker.
 *
 * Handle index layout:
 *   0=top-left, 1=top-center, 2=top-right, 3=middle-right,
 *   4=bottom-right, 5=bottom-center, 6=bottom-left, 7=middle-left
 */
export function useResizeDrag({
  editorRef,
  bounds,
  camera,
  onSceneChanged,
  collab,
  activePageId,
}: UseResizeDragOptions) {
  const dragRef = useRef<DragState | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  const handlePointerDown = useCallback(
    (handleIndex: number, e: React.PointerEvent) => {
      if (!camera || bounds.length === 0) return;

      // For single selection, use bounds[0]. For multi-select the handle
      // belongs to the first selected node (consistent with the overlay).
      const b = bounds[0];
      if (!b) return;

      e.preventDefault();
      e.stopPropagation();
      (e.target as HTMLElement).setPointerCapture(e.pointerId);

      dragRef.current = {
        handleIndex,
        nodeId: b.node_id,
        originalBounds: { x: b.x, y: b.y, width: b.width, height: b.height },
        startClientX: e.clientX,
        startClientY: e.clientY,
        descriptors: [],
      };
      setIsDragging(true);

      editorRef.current?.begin_group("Resize");
    },
    [editorRef, bounds, camera],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag || !camera) return;

      const editor = editorRef.current;
      if (!editor) return;

      const dpr =
        typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
      const deltaClientX = e.clientX - drag.startClientX;
      const deltaClientY = e.clientY - drag.startClientY;

      // Convert client-space delta to world-space delta.
      // Client pixels * dpr = physical pixels. Physical / zoom = world.
      // But client delta maps directly: delta_world = delta_client / (zoom / dpr)
      // Actually: screen CSS px -> physical px -> world. delta_world = delta_css * dpr / zoom.
      // However, for overlays in CSS space, the zoom already accounts for the
      // fact that camera viewport is in physical pixels. So:
      //   screen delta (CSS px) * dpr / zoom = world delta
      const worldDx = (deltaClientX * dpr) / camera.zoom;
      const worldDy = (deltaClientY * dpr) / camera.zoom;

      const { x, y, width, height } = drag.originalBounds;
      const newBounds = computeResizedBounds(
        drag.handleIndex,
        x,
        y,
        width,
        height,
        worldDx,
        worldDy,
      );

      const descriptor = {
        type: "SetBounds",
        node_id: drag.nodeId,
        bounds: {
          x: newBounds.x,
          y: newBounds.y,
          width: newBounds.width,
          height: newBounds.height,
        },
      };
      editor.execute_command(JSON.stringify(descriptor));
      drag.descriptors.push(descriptor);

      onSceneChanged?.();
    },
    [editorRef, camera, onSceneChanged],
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag) return;
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      dragRef.current = null;
      setIsDragging(false);

      editorRef.current?.end_group();
      if (
        collab?.status === "connected" &&
        activePageId &&
        drag.descriptors.length > 0
      ) {
        collab.submitOpGroup(drag.descriptors, activePageId, "Resize");
      }
      onSceneChanged?.();
    },
    [editorRef, onSceneChanged, collab, activePageId],
  );

  const getHandleProps = useCallback(
    (index: number) => ({
      onPointerDown: (e: React.PointerEvent) => handlePointerDown(index, e),
      onPointerMove: handlePointerMove,
      onPointerUp: handlePointerUp,
    }),
    [handlePointerDown, handlePointerMove, handlePointerUp],
  );

  return { getHandleProps, isDragging };
}

/**
 * Given the original bounds and a world-space delta, computes new bounds
 * based on which handle is being dragged.
 *
 * Handle index mapping:
 *   0=top-left, 1=top-center, 2=top-right, 3=middle-right,
 *   4=bottom-right, 5=bottom-center, 6=bottom-left, 7=middle-left
 */
export function computeResizedBounds(
  handleIndex: number,
  origX: number,
  origY: number,
  origW: number,
  origH: number,
  dx: number,
  dy: number,
): { x: number; y: number; width: number; height: number } {
  let x = origX;
  let y = origY;
  let w = origW;
  let h = origH;

  // Which edges this handle moves.
  const movesLeft = handleIndex === 0 || handleIndex === 6 || handleIndex === 7;
  const movesRight =
    handleIndex === 2 || handleIndex === 3 || handleIndex === 4;
  const movesTop = handleIndex === 0 || handleIndex === 1 || handleIndex === 2;
  const movesBottom =
    handleIndex === 4 || handleIndex === 5 || handleIndex === 6;

  if (movesLeft) {
    const maxShift = w - MIN_SIZE;
    const shift = Math.min(dx, maxShift);
    x = origX + shift;
    w = origW - shift;
  } else if (movesRight) {
    w = Math.max(origW + dx, MIN_SIZE);
  }

  if (movesTop) {
    const maxShift = h - MIN_SIZE;
    const shift = Math.min(dy, maxShift);
    y = origY + shift;
    h = origH - shift;
  } else if (movesBottom) {
    h = Math.max(origH + dy, MIN_SIZE);
  }

  return { x, y, width: w, height: h };
}
