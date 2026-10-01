import { useCallback, useEffect, useRef } from "react";
import { colors, fontSizes, spacing } from "../theme";
import type { EditorStatus, SeleanEditor } from "../wasm/types";

interface CanvasProps {
  canvasId: string;
  editorRef: React.RefObject<SeleanEditor | null>;
  status: EditorStatus;
  onInteractionEvents?: (events: InteractionEvent[]) => void;
  onContextMenu?: (x: number, y: number) => void;
}

/** Subset of InteractionEvent variants relevant to the frontend. */
export interface InteractionEvent {
  type: string;
  [key: string]: unknown;
}

/**
 * Canvas component that hosts the WebGPU render surface and forwards
 * pointer/wheel events to the WASM editor. Parses the JSON interaction
 * events returned by WASM handlers and propagates them via callback.
 */
export function Canvas({
  canvasId,
  editorRef,
  status,
  onInteractionEvents,
  onContextMenu: onContextMenuProp,
}: CanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number>(0);

  // Render loop (stops after 5 consecutive failures to prevent freeze)
  useEffect(() => {
    if (status !== "ready") return;
    let failCount = 0;

    function frame() {
      try {
        editorRef.current?.render();
        failCount = 0;
      } catch (e) {
        failCount++;
        if (failCount <= 3) {
          console.warn("canvas:render-frame failed", e);
        }
        if (failCount >= 5) {
          console.error(
            "canvas: render loop stopped after 5 consecutive failures",
          );
          return;
        }
      }
      rafRef.current = requestAnimationFrame(frame);
    }
    rafRef.current = requestAnimationFrame(frame);

    return () => cancelAnimationFrame(rafRef.current);
  }, [status, editorRef]);

  // Resize observer
  useEffect(() => {
    const container = containerRef.current;
    if (!container || status !== "ready") return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        const canvas = document.getElementById(
          canvasId,
        ) as HTMLCanvasElement | null;
        if (canvas) {
          canvas.width = width * devicePixelRatio;
          canvas.height = height * devicePixelRatio;
          canvas.style.width = `${width}px`;
          canvas.style.height = `${height}px`;
          editorRef.current?.resize(
            width * devicePixelRatio,
            height * devicePixelRatio,
          );
        }
      }
    });

    observer.observe(container);
    return () => observer.disconnect();
  }, [canvasId, editorRef, status]);

  const emitEvents = useCallback(
    (events: InteractionEvent[]) => {
      if (!onInteractionEvents) return;
      if (events && events.length > 0) {
        onInteractionEvents(events);
      }
    },
    [onInteractionEvents],
  );

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const rect = (e.target as HTMLElement).getBoundingClientRect();
        const x = (e.clientX - rect.left) * devicePixelRatio;
        const y = (e.clientY - rect.top) * devicePixelRatio;
        const events = editor.on_pointer_move(
          x,
          y,
          e.shiftKey,
          e.ctrlKey,
          e.altKey,
          e.metaKey,
        );
        emitEvents(events);
      } catch {
        // Silently ignore pointer move failures to prevent UI freeze.
      }
    },
    [editorRef, emitEvents],
  );

  // Track whether a pointer is currently down on the canvas so we can
  // listen for pointermove/pointerup on window (instead of using pointer
  // capture, which steals events from sidebar buttons).
  const activePointerRef = useRef<number | null>(null);
  const canvasRectRef = useRef<DOMRect | null>(null);

  const handlePointerDown = useCallback(
    (e: React.PointerEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const rect = (e.target as HTMLElement).getBoundingClientRect();
        canvasRectRef.current = rect;
        const x = (e.clientX - rect.left) * devicePixelRatio;
        const y = (e.clientY - rect.top) * devicePixelRatio;
        const events = editor.on_pointer_down(
          x,
          y,
          e.button,
          e.shiftKey,
          e.ctrlKey,
          e.altKey,
          e.metaKey,
        );
        emitEvents(events);
        if (e.button === 0) {
          activePointerRef.current = e.pointerId;
        }
      } catch (err) {
        console.warn("canvas:pointer-down failed", err);
      }
    },
    [editorRef, emitEvents],
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      activePointerRef.current = null;
      try {
        const rect =
          canvasRectRef.current ??
          (e.target as HTMLElement).getBoundingClientRect();
        const x = (e.clientX - rect.left) * devicePixelRatio;
        const y = (e.clientY - rect.top) * devicePixelRatio;
        const events = editor.on_pointer_up(
          x,
          y,
          e.button,
          e.shiftKey,
          e.ctrlKey,
          e.altKey,
          e.metaKey,
        );
        emitEvents(events);
      } catch (err) {
        console.warn("canvas:pointer-up failed", err);
      }
    },
    [editorRef, emitEvents],
  );

  // Window-level listeners that keep a drag alive once the pointer leaves
  // the canvas (e.g. over the sidebar or an overlay). Moves and the release
  // are forwarded to the editor and the returned events are emitted, so
  // DragMoved/DragEnded still reach the drag hooks. Events that target the
  // canvas itself are skipped: the React handlers above already handled them.
  useEffect(() => {
    const isOutsideDrag = (e: PointerEvent) =>
      activePointerRef.current === e.pointerId &&
      (e.target as HTMLElement | null)?.id !== canvasId;

    const handleWindowPointerMove = (e: PointerEvent) => {
      if (!isOutsideDrag(e)) return;
      const editor = editorRef.current;
      const rect = canvasRectRef.current;
      if (!editor || !rect) return;
      try {
        const x = (e.clientX - rect.left) * devicePixelRatio;
        const y = (e.clientY - rect.top) * devicePixelRatio;
        const events = editor.on_pointer_move(
          x,
          y,
          e.shiftKey,
          e.ctrlKey,
          e.altKey,
          e.metaKey,
        );
        emitEvents(events);
      } catch {
        // Silently ignore pointer move failures to prevent UI freeze.
      }
    };

    const handleWindowPointerUp = (e: PointerEvent) => {
      if (!isOutsideDrag(e)) return;
      activePointerRef.current = null;
      const editor = editorRef.current;
      const rect = canvasRectRef.current;
      if (!editor || !rect) return;
      try {
        const x = (e.clientX - rect.left) * devicePixelRatio;
        const y = (e.clientY - rect.top) * devicePixelRatio;
        const events = editor.on_pointer_up(
          x,
          y,
          e.button,
          e.shiftKey,
          e.ctrlKey,
          e.altKey,
          e.metaKey,
        );
        emitEvents(events);
      } catch (err) {
        console.warn("canvas:pointer-up failed", err);
      }
    };

    window.addEventListener("pointermove", handleWindowPointerMove);
    window.addEventListener("pointerup", handleWindowPointerUp);
    return () => {
      window.removeEventListener("pointermove", handleWindowPointerMove);
      window.removeEventListener("pointerup", handleWindowPointerUp);
    };
  }, [canvasId, editorRef, emitEvents]);

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      e.preventDefault();
      try {
        const rect = (e.target as HTMLElement).getBoundingClientRect();
        const x = (e.clientX - rect.left) * devicePixelRatio;
        const y = (e.clientY - rect.top) * devicePixelRatio;
        const events = editor.on_scroll(
          x,
          y,
          e.deltaX,
          e.deltaY,
          e.shiftKey,
          e.ctrlKey,
          e.altKey,
          e.metaKey,
        );
        emitEvents(events);
      } catch {
        // Silently ignore scroll failures.
      }
    },
    [editorRef, emitEvents],
  );

  return (
    <div
      ref={containerRef}
      style={{ width: "100%", height: "100%", position: "absolute", inset: 0 }}
    >
      <canvas
        id={canvasId}
        style={{ width: "100%", height: "100%", display: "block" }}
        onPointerMove={handlePointerMove}
        onPointerDown={handlePointerDown}
        onPointerUp={handlePointerUp}
        onWheel={handleWheel}
        onContextMenu={(e) => {
          e.preventDefault();
          onContextMenuProp?.(e.clientX, e.clientY);
        }}
      />
      {status === "loading" && (
        <div style={overlayStyle}>Loading editor...</div>
      )}
      {status === "unsupported" && (
        <div style={overlayStyle}>
          <span>WebGPU is not supported in this browser.</span>
          <span style={{ fontSize: fontSizes.base, color: colors.textDim }}>
            Try Chrome 113+ or Edge 113+.
          </span>
        </div>
      )}
    </div>
  );
}

const overlayStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  display: "flex",
  alignItems: "center",
  justifyContent: "center",
  background: "rgba(255, 255, 255, 0.92)",
  color: colors.text,
  fontSize: fontSizes.xl,
  flexDirection: "column",
  gap: spacing.sm,
};
