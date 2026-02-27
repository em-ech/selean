import { useEffect, useRef, useState } from "react";
import type { CameraInfo, SelectionBounds, SeleanEditor } from "../wasm/types";

interface SelectionOverlayProps {
  editorRef: React.RefObject<SeleanEditor | null>;
}

/** Handle size in CSS pixels. */
const HANDLE_SIZE = 8;
/** Selection box border color. */
const SELECTION_COLOR = "#4a90d9";

/**
 * HTML overlay that renders selection bounding boxes and resize handles
 * on top of the WebGPU canvas. Updates on every animation frame.
 */
export function SelectionOverlay({ editorRef }: SelectionOverlayProps) {
  const [bounds, setBounds] = useState<SelectionBounds[]>([]);
  const [camera, setCamera] = useState<CameraInfo | null>(null);
  const rafRef = useRef(0);

  useEffect(() => {
    function poll() {
      const editor = editorRef.current;
      if (editor) {
        try {
          const boundsJson = editor.get_selected_bounds_json();
          const cameraJson = editor.get_camera_json();
          setBounds(JSON.parse(boundsJson));
          setCamera(JSON.parse(cameraJson));
        } catch {
          // WASM call failed
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
        <SelectionBox key={i} bounds={b} camera={camera} />
      ))}
    </div>
  );
}

interface SelectionBoxProps {
  bounds: SelectionBounds;
  camera: CameraInfo;
}

function SelectionBox({ bounds, camera }: SelectionBoxProps) {
  const screenX =
    (bounds.x - camera.pan_x) * camera.zoom + camera.viewport_width / 2;
  const screenY =
    (bounds.y - camera.pan_y) * camera.zoom + camera.viewport_height / 2;
  const screenW = bounds.width * camera.zoom;
  const screenH = bounds.height * camera.zoom;

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
          style={{
            position: "absolute",
            left: h.x,
            top: h.y,
            width: HANDLE_SIZE,
            height: HANDLE_SIZE,
            background: "#fff",
            border: `1px solid ${SELECTION_COLOR}`,
            pointerEvents: "auto",
            cursor: "pointer",
          }}
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
