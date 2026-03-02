import { useCallback, useEffect, useRef, useState } from "react";
import type { CameraInfo, SeleanEditor } from "../wasm/types";

interface InlineTextEditorProps {
  nodeId: string;
  initialContent: string;
  bounds: { x: number; y: number; width: number; height: number };
  fontSize: number;
  editorRef: React.RefObject<SeleanEditor | null>;
  onCommit: (content: string) => void;
  onCancel: () => void;
}

/**
 * Inline text editor overlay. Renders a textarea positioned over the
 * node's bounding box in screen space. Commits on blur, cancels on Escape.
 */
export function InlineTextEditor({
  nodeId,
  initialContent,
  bounds,
  fontSize,
  editorRef,
  onCommit,
  onCancel,
}: InlineTextEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState(initialContent);
  const [screenRect, setScreenRect] = useState({ x: 0, y: 0, w: 0, h: 0 });
  const [scaledFontSize, setScaledFontSize] = useState(fontSize);
  const committedRef = useRef(false);

  // Suppress unused var warning for nodeId in case it's needed for future features.
  void nodeId;

  // Compute screen position from world-space bounds and camera.
  const updatePosition = useCallback(() => {
    const editor = editorRef.current;
    if (!editor) return;

    try {
      const camera: CameraInfo = JSON.parse(editor.get_camera_json());
      const dpr = window.devicePixelRatio || 1;
      const zoom = camera.zoom;

      const sx = ((bounds.x - camera.pan_x) * zoom) / dpr;
      const sy = ((bounds.y - camera.pan_y) * zoom) / dpr;
      const sw = (bounds.width * zoom) / dpr;
      const sh = (bounds.height * zoom) / dpr;

      setScreenRect({ x: sx, y: sy, w: sw, h: sh });
      setScaledFontSize((fontSize * zoom) / dpr);
    } catch {
      // camera read failed
    }
  }, [editorRef, bounds, fontSize]);

  // Initial position and RAF polling for pan/zoom updates.
  useEffect(() => {
    updatePosition();
    let rafId = 0;
    const poll = () => {
      updatePosition();
      rafId = requestAnimationFrame(poll);
    };
    rafId = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(rafId);
  }, [updatePosition]);

  // Auto-focus on mount.
  useEffect(() => {
    textareaRef.current?.focus();
    textareaRef.current?.select();
  }, []);

  const handleBlur = useCallback(() => {
    if (committedRef.current) return;
    committedRef.current = true;
    onCommit(value);
  }, [value, onCommit]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      e.stopPropagation();
      if (e.key === "Escape") {
        committedRef.current = true;
        onCancel();
      }
    },
    [onCancel],
  );

  return (
    <textarea
      ref={textareaRef}
      data-testid="inline-text-editor"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onBlur={handleBlur}
      onKeyDown={handleKeyDown}
      style={{
        position: "absolute",
        left: screenRect.x,
        top: screenRect.y,
        width: screenRect.w,
        height: screenRect.h,
        fontSize: scaledFontSize,
        fontFamily: "Inter, sans-serif",
        color: "white",
        background: "rgba(0,0,0,0.3)",
        border: "2px solid #4a4a8a",
        outline: "none",
        resize: "none",
        padding: 2,
        margin: 0,
        zIndex: 100,
        overflow: "hidden",
        lineHeight: 1.4,
      }}
    />
  );
}
