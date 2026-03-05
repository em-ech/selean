import { useEffect, useRef, useState } from "react";
import type { CameraInfo, SelectionBounds, SeleanEditor } from "../wasm/types";
import { useResizeDrag } from "../hooks/useResizeDrag";
import { worldToScreen, worldDimsToScreen } from "../utils/camera";

interface SelectionOverlayProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged?: () => void;
}

/** Handle size in CSS pixels. */
const HANDLE_SIZE = 8;
/** Selection box border color. */
const SELECTION_COLOR = "#4a90d9";

/** Cursor style per handle index. */
const HANDLE_CURSORS: string[] = [
  "nwse-resize", // 0: top-left
  "ns-resize", // 1: top-center
  "nesw-resize", // 2: top-right
  "ew-resize", // 3: middle-right
  "nwse-resize", // 4: bottom-right
  "ns-resize", // 5: bottom-center
  "nesw-resize", // 6: bottom-left
  "ew-resize", // 7: middle-left
];

/**
 * HTML overlay that renders selection bounding boxes and resize handles
 * on top of the WebGPU canvas. Updates on every animation frame.
 */
export function SelectionOverlay({
  editorRef,
  onSceneChanged,
}: SelectionOverlayProps) {
  const [bounds, setBounds] = useState<SelectionBounds[]>([]);
  const [camera, setCamera] = useState<CameraInfo | null>(null);
  const rafRef = useRef(0);

  const { getHandleProps, isDragging } = useResizeDrag({
    editorRef,
    bounds,
    camera,
    onSceneChanged,
  });

  useEffect(() => {
    function poll() {
      const editor = editorRef.current;
      if (editor) {
        try {
          const boundsJson = editor.get_selected_bounds_json();
          const cameraJson = editor.get_camera_json();
          setBounds(JSON.parse(boundsJson));
          setCamera(JSON.parse(cameraJson));
        } catch (e) {
          console.warn("selection-overlay:get-bounds failed", e);
        }
      }
      rafRef.current = requestAnimationFrame(poll);
    }
    rafRef.current = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(rafRef.current);
  }, [editorRef]);

  if (!camera || bounds.length === 0) return null;

  return (
    <div style={overlayStyle}>
      {bounds.map((b, i) => (
        <SelectionBox
          key={i}
          bounds={b}
          camera={camera}
          getHandleProps={getHandleProps}
          isDragging={isDragging}
        />
      ))}
    </div>
  );
}

interface SelectionBoxProps {
  bounds: SelectionBounds;
  camera: CameraInfo;
  getHandleProps: (index: number) => {
    onPointerDown: (e: React.PointerEvent) => void;
  };
  isDragging: boolean;
}

function SelectionBox({
  bounds,
  camera,
  getHandleProps,
  isDragging,
}: SelectionBoxProps) {
  const { x: screenX, y: screenY } = worldToScreen(bounds.x, bounds.y, camera);
  const { w: screenW, h: screenH } = worldDimsToScreen(
    bounds.width,
    bounds.height,
    camera.zoom,
  );

  const half = HANDLE_SIZE / 2;

  const handles = [
    { x: -half, y: -half }, // top-left
    { x: screenW / 2 - half, y: -half }, // top-center
    { x: screenW - half, y: -half }, // top-right
    { x: screenW - half, y: screenH / 2 - half }, // middle-right
    { x: screenW - half, y: screenH - half }, // bottom-right
    { x: screenW / 2 - half, y: screenH - half }, // bottom-center
    { x: -half, y: screenH - half }, // bottom-left
    { x: -half, y: screenH / 2 - half }, // middle-left
  ];

  return (
    <div
      style={{
        position: "absolute",
        left: screenX,
        top: screenY,
        width: screenW,
        height: screenH,
        border: `1px solid ${SELECTION_COLOR}`,
        pointerEvents: "none",
      }}
    >
      {handles.map((h, i) => (
        <div
          key={i}
          data-handle-index={i}
          style={{
            position: "absolute",
            left: h.x,
            top: h.y,
            width: HANDLE_SIZE,
            height: HANDLE_SIZE,
            background: "#fff",
            border: `1px solid ${SELECTION_COLOR}`,
            pointerEvents: isDragging ? "none" : "auto",
            cursor: HANDLE_CURSORS[i],
          }}
          {...getHandleProps(i)}
        />
      ))}
    </div>
  );
}

const overlayStyle: React.CSSProperties = {
  position: "absolute",
  top: 0,
  left: 0,
  width: "100%",
  height: "100%",
  pointerEvents: "none",
  overflow: "hidden",
};
