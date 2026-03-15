import { useCallback, useEffect, useRef, useState } from "react";

interface UseResizablePanelOptions {
  /** Initial width in pixels. */
  initialWidth: number;
  /** Minimum width. */
  minWidth: number;
  /** Maximum width. */
  maxWidth: number;
  /** LocalStorage key for persistence (optional). */
  storageKey?: string;
  /** Which edge to drag: "left" means the left edge (panels on the right), "right" means the right edge (panels on the left). */
  edge: "left" | "right";
}

interface UseResizablePanelReturn {
  width: number;
  /** Props to spread on the resize handle element. */
  handleProps: {
    onPointerDown: (e: React.PointerEvent) => void;
    style: React.CSSProperties;
    "data-resize-handle": boolean;
  };
  isResizing: boolean;
}

/**
 * Hook for making a panel width resizable via drag.
 * Persists width to localStorage if a storageKey is provided.
 */
export function useResizablePanel({
  initialWidth,
  minWidth,
  maxWidth,
  storageKey,
  edge,
}: UseResizablePanelOptions): UseResizablePanelReturn {
  const [width, setWidth] = useState(() => {
    if (storageKey) {
      const saved = localStorage.getItem(storageKey);
      if (saved) {
        const parsed = parseInt(saved, 10);
        if (!isNaN(parsed) && parsed >= minWidth && parsed <= maxWidth) {
          return parsed;
        }
      }
    }
    return initialWidth;
  });

  const isResizingRef = useRef(false);
  const [isResizing, setIsResizing] = useState(false);
  const startXRef = useRef(0);
  const startWidthRef = useRef(0);
  const cleanupRef = useRef<(() => void) | null>(null);

  // Clean up listeners on unmount.
  useEffect(() => {
    return () => {
      cleanupRef.current?.();
    };
  }, []);

  // Persist width changes.
  useEffect(() => {
    if (storageKey) {
      localStorage.setItem(storageKey, String(width));
    }
  }, [width, storageKey]);

  const handlePointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      e.stopPropagation();
      isResizingRef.current = true;
      setIsResizing(true);
      startXRef.current = e.clientX;
      startWidthRef.current = width;

      const handlePointerMove = (ev: PointerEvent) => {
        if (!isResizingRef.current) return;
        const dx = ev.clientX - startXRef.current;
        const newWidth =
          edge === "right"
            ? startWidthRef.current + dx
            : startWidthRef.current - dx;
        setWidth(Math.max(minWidth, Math.min(maxWidth, newWidth)));
      };

      const handlePointerUp = () => {
        isResizingRef.current = false;
        setIsResizing(false);
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("pointerup", handlePointerUp);
        cleanupRef.current = null;
      };

      window.addEventListener("pointermove", handlePointerMove);
      window.addEventListener("pointerup", handlePointerUp);

      cleanupRef.current = () => {
        window.removeEventListener("pointermove", handlePointerMove);
        window.removeEventListener("pointerup", handlePointerUp);
      };
    },
    [width, minWidth, maxWidth, edge],
  );

  const handleStyle: React.CSSProperties = {
    position: "absolute",
    top: 0,
    bottom: 0,
    width: 4,
    cursor: "col-resize",
    zIndex: 50,
    ...(edge === "left" ? { left: -2 } : { right: -2 }),
  };

  return {
    width,
    handleProps: {
      onPointerDown: handlePointerDown,
      style: handleStyle,
      "data-resize-handle": true,
    },
    isResizing,
  };
}
