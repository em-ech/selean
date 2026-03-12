import { useCallback, useRef, useState } from "react";
import type { CollabSession } from "./useCollabSession";
import type {
  CameraInfo,
  NodeInfo,
  SelectionBounds,
  SeleanEditor,
} from "../wasm/types";
import { worldToScreen } from "../utils/camera";

/** Snap increment in degrees when shift key is held. */
const SNAP_INCREMENT = 15;

interface RotationDragState {
  nodeId: string;
  centerScreenX: number;
  centerScreenY: number;
  startAngle: number;
  originalRotation: number;
  descriptors: Record<string, unknown>[];
}

interface UseRotationDragOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  bounds: SelectionBounds[];
  camera: CameraInfo | null;
  onSceneChanged?: () => void;
  collab?: CollabSession | null;
  activePageId?: string;
}

/**
 * Computes the angle in radians from a center point to a pointer position.
 * Returns the angle using `Math.atan2(dy, dx)`.
 */
export function computeRotation(
  centerX: number,
  centerY: number,
  pointerX: number,
  pointerY: number,
): number {
  return Math.atan2(pointerY - centerY, pointerX - centerX);
}

/**
 * Snaps a degree value to the nearest increment of `SNAP_INCREMENT`.
 */
function snapDegrees(degrees: number): number {
  return Math.round(degrees / SNAP_INCREMENT) * SNAP_INCREMENT;
}

/**
 * Rotation drag hook for the selection overlay rotation handle.
 *
 * Returns `rotationHandleProps` (onPointerDown, onPointerMove, onPointerUp)
 * to spread onto the rotation handle element, and an `isRotating` flag.
 */
export function useRotationDrag({
  editorRef,
  bounds,
  camera,
  onSceneChanged,
  collab,
  activePageId,
}: UseRotationDragOptions) {
  const dragRef = useRef<RotationDragState | null>(null);
  const [isRotating, setIsRotating] = useState(false);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (!camera || bounds.length === 0) return;

      const b = bounds[0];
      if (!b) return;

      const editor = editorRef.current;
      if (!editor) return;

      e.preventDefault();
      e.stopPropagation();
      (e.target as HTMLElement).setPointerCapture(e.pointerId);

      // Compute the node center in world space, then convert to screen space.
      const worldCenterX = b.x + b.width / 2;
      const worldCenterY = b.y + b.height / 2;
      const screenCenter = worldToScreen(worldCenterX, worldCenterY, camera);

      // Read the current rotation from the node's transform.
      let originalRotation = 0;
      try {
        const nodeJson = editor.get_node_json(b.node_id);
        const node: NodeInfo = JSON.parse(nodeJson);
        const [a, bVal] = node.transform;
        originalRotation =
          Math.round(Math.atan2(bVal, a) * (180 / Math.PI) * 10) / 10;
      } catch {
        // Fall back to 0 if node data is unavailable.
      }

      const startAngle = computeRotation(
        screenCenter.x,
        screenCenter.y,
        e.clientX,
        e.clientY,
      );

      dragRef.current = {
        nodeId: b.node_id,
        centerScreenX: screenCenter.x,
        centerScreenY: screenCenter.y,
        startAngle,
        originalRotation,
        descriptors: [],
      };
      setIsRotating(true);

      editor.begin_group("Rotate");
    },
    [editorRef, bounds, camera],
  );

  const onPointerMove = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag) return;

      const editor = editorRef.current;
      if (!editor) return;

      const currentAngle = computeRotation(
        drag.centerScreenX,
        drag.centerScreenY,
        e.clientX,
        e.clientY,
      );

      const deltaRadians = currentAngle - drag.startAngle;
      const deltaDegrees = deltaRadians * (180 / Math.PI);

      let angleDegrees = drag.originalRotation + deltaDegrees;

      // Shift key: snap to 15-degree increments.
      if (e.shiftKey) {
        angleDegrees = snapDegrees(angleDegrees);
      }

      // Normalize to [-360, 360] range.
      angleDegrees = ((angleDegrees % 360) + 360) % 360;
      if (angleDegrees > 180) angleDegrees -= 360;

      // Round to one decimal place for clean values.
      angleDegrees = Math.round(angleDegrees * 10) / 10;

      const result = editor.execute_tool_call(
        "set_rotation",
        JSON.stringify({
          node_id: drag.nodeId,
          angle_degrees: angleDegrees,
        }),
      );

      // Only track as a descriptor if the command succeeded (non-error result).
      if (result && !result.startsWith("Error")) {
        const descriptor = {
          type: "SetRotation",
          node_id: drag.nodeId,
          angle_degrees: angleDegrees,
        };
        drag.descriptors.push(descriptor);
      }

      onSceneChanged?.();
    },
    [editorRef, onSceneChanged],
  );

  const onPointerUp = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag) return;

      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
      dragRef.current = null;
      setIsRotating(false);

      editorRef.current?.end_group();

      if (
        collab?.status === "connected" &&
        activePageId &&
        drag.descriptors.length > 0
      ) {
        collab.submitOpGroup(drag.descriptors, activePageId, "Rotate");
      }

      onSceneChanged?.();
    },
    [editorRef, onSceneChanged, collab, activePageId],
  );

  const rotationHandleProps = {
    onPointerDown,
    onPointerMove,
    onPointerUp,
  };

  return { rotationHandleProps, isRotating };
}
