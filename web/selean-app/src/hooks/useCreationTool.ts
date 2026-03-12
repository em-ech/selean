import { useCallback, useRef, useState } from "react";
import type { ToolType } from "../components/Toolbar";
import type { SeleanEditor } from "../wasm/types";

interface DragState {
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
}

interface UseCreationToolOptions {
  editorRef: React.RefObject<SeleanEditor | null>;
  activeTool: ToolType;
  onToolReset: () => void;
  onSceneChanged: () => void;
}

/**
 * Handles pointer events when a creation tool (frame, text) is active.
 * Returns handlers to attach to the canvas overlay.
 *
 * Coordinates are stored as client-space values during the drag gesture,
 * then converted to physical pixels (via devicePixelRatio and canvas rect)
 * and finally to world-space on pointer up using the WASM camera state.
 */
export function useCreationTool({
  editorRef,
  activeTool,
  onToolReset,
  onSceneChanged,
}: UseCreationToolOptions) {
  const dragRef = useRef<DragState | null>(null);
  const [isDragging, setIsDragging] = useState(false);

  const handlePointerDown = useCallback(
    (e: React.PointerEvent) => {
      if (activeTool === "select") return;
      e.preventDefault();
      e.stopPropagation();
      (e.target as HTMLElement).setPointerCapture(e.pointerId);

      dragRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        currentX: e.clientX,
        currentY: e.clientY,
      };
      setIsDragging(true);
    },
    [activeTool],
  );

  const handlePointerMove = useCallback((e: React.PointerEvent) => {
    if (!dragRef.current) return;
    dragRef.current.currentX = e.clientX;
    dragRef.current.currentY = e.clientY;
  }, []);

  const handlePointerUp = useCallback(
    (e: React.PointerEvent) => {
      const drag = dragRef.current;
      if (!drag) return;
      dragRef.current = null;
      setIsDragging(false);

      const editor = editorRef.current;
      if (!editor) return;

      // Get camera to convert screen coords to world coords
      let camera;
      try {
        camera = editor.get_camera();
      } catch (e) {
        console.warn("creation-tool:get-camera failed", e);
        return;
      }

      // Convert client coords to canvas-relative physical pixels
      const rect = (e.target as HTMLElement).getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      const toPhysical = (cx: number, cy: number) => ({
        x: (cx - rect.left) * dpr,
        y: (cy - rect.top) * dpr,
      });

      const start = toPhysical(drag.startX, drag.startY);
      const end = toPhysical(drag.currentX, drag.currentY);

      const toWorldX = (sx: number) =>
        (sx - camera.viewport_width / 2) / camera.zoom + camera.pan_x;
      const toWorldY = (sy: number) =>
        (sy - camera.viewport_height / 2) / camera.zoom + camera.pan_y;

      const x1 = toWorldX(Math.min(start.x, end.x));
      const y1 = toWorldY(Math.min(start.y, end.y));
      const x2 = toWorldX(Math.max(start.x, end.x));
      const y2 = toWorldY(Math.max(start.y, end.y));

      const w = Math.max(x2 - x1, 10);
      const h = Math.max(y2 - y1, 10);

      if (activeTool === "frame") {
        editor.execute_tool_call(
          "create_node",
          JSON.stringify({
            name: "Frame",
            kind: "Frame",
            x: x1,
            y: y1,
            width: w,
            height: h,
            fill_r: 0.85,
            fill_g: 0.85,
            fill_b: 0.85,
            fill_a: 1.0,
          }),
        );
      } else if (activeTool === "text") {
        editor.execute_tool_call(
          "create_node",
          JSON.stringify({
            name: "Text",
            kind: "Text",
            x: x1,
            y: y1,
            width: Math.max(w, 100),
            height: Math.max(h, 30),
            text_content: "Text",
          }),
        );
      }

      onSceneChanged();
      onToolReset();
    },
    [editorRef, activeTool, onSceneChanged, onToolReset],
  );

  return {
    isDragging,
    creationHandlers:
      activeTool !== "select"
        ? {
            onPointerDown: handlePointerDown,
            onPointerMove: handlePointerMove,
            onPointerUp: handlePointerUp,
          }
        : null,
  };
}
