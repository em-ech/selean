import { useCallback, useEffect, useRef } from "react";
import { colors, fontSizes } from "../theme";
import type { EditorStatus, SeleanEditor } from "../wasm/types";

interface CanvasProps {
  canvasId: string;
  editorRef: React.RefObject<SeleanEditor | null>;
  status: EditorStatus;
}

/**
 * Canvas component that hosts the WebGPU render surface and forwards
 * pointer/wheel events to the WASM editor.
 */
export function Canvas({ canvasId, editorRef, status }: CanvasProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number>(0);

  // Render loop
  useEffect(() => {
    if (status !== "ready") return;

    function frame() {
      try {
        editorRef.current?.render();
      } catch {
        // GPU context lost or WASM error. Skip frame.
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

  const handlePointerMove = useCallback(
    (e: React.PointerEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      const rect = (e.target as HTMLElement).getBoundingClientRect();
      const x = (e.clientX - rect.left) * devicePixelRatio;
      const y = (e.clientY - rect.top) * devicePixelRatio;
      editor.on_pointer_move(x, y, e.shiftKey, e.ctrlKey, e.altKey, e.metaKey);
    },
    [editorRef],
  );

  const handlePointerDown = useCallback(
    (e: React.PointerEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      const rect = (e.target as HTMLElement).getBoundingClientRect();
      const x = (e.clientX - rect.left) * devicePixelRatio;
      const y = (e.clientY - rect.top) * devicePixelRatio;
      editor.on_pointer_down(
        x,
        y,
        e.button,
        e.shiftKey,
        e.ctrlKey,
        e.altKey,
        e.metaKey,
      );
      (e.target as HTMLElement).setPointerCapture(e.pointerId);
    },
    [editorRef],
  );

  const handlePointerUp = useCallback(
    (e: React.PointerEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      const rect = (e.target as HTMLElement).getBoundingClientRect();
      const x = (e.clientX - rect.left) * devicePixelRatio;
      const y = (e.clientY - rect.top) * devicePixelRatio;
      editor.on_pointer_up(
        x,
        y,
        e.button,
        e.shiftKey,
        e.ctrlKey,
        e.altKey,
        e.metaKey,
      );
      (e.target as HTMLElement).releasePointerCapture(e.pointerId);
    },
    [editorRef],
  );

  const handleWheel = useCallback(
    (e: React.WheelEvent) => {
      const editor = editorRef.current;
      if (!editor) return;
      e.preventDefault();
      const rect = (e.target as HTMLElement).getBoundingClientRect();
      const x = (e.clientX - rect.left) * devicePixelRatio;
      const y = (e.clientY - rect.top) * devicePixelRatio;
      editor.on_scroll(
        x,
        y,
        e.deltaX,
        e.deltaY,
        e.shiftKey,
        e.ctrlKey,
        e.altKey,
        e.metaKey,
      );
    },
    [editorRef],
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
        onContextMenu={(e) => e.preventDefault()}
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
  background: "rgba(26, 26, 46, 0.9)",
  fontSize: fontSizes.xl,
  flexDirection: "column",
  gap: 8,
};
