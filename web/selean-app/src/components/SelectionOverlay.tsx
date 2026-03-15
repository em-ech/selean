import { useEffect, useRef, useState } from "react";
import type { CameraInfo, SelectionBounds, SeleanEditor } from "../wasm/types";
import { useResizeDrag } from "../hooks/useResizeDrag";
import { useRotationDrag } from "../hooks/useRotationDrag";
import { worldToScreen, worldDimsToScreen } from "../utils/camera";
import { colors } from "../theme";

interface SelectionOverlayProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  onSceneChanged?: () => void;
}

/** Handle size in CSS pixels. */
const HANDLE_SIZE = 8;
/** Selection box border color. */
const SELECTION_COLOR = colors.accent;
/** Rotation handle offset above the top edge (CSS px). */
const ROTATION_HANDLE_OFFSET = 24;
/** Rotation handle size in CSS pixels. */
const ROTATION_HANDLE_SIZE = 10;

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
 * HTML overlay that renders selection bounding boxes, resize handles,
 * and a rotation handle on top of the WebGPU canvas.
 * Updates on every animation frame.
 */
export function SelectionOverlay({
  editorRef,
  onSceneChanged,
}: SelectionOverlayProps) {
  const [bounds, setBounds] = useState<SelectionBounds[]>([]);
  const [camera, setCamera] = useState<CameraInfo | null>(null);
  const rafRef = useRef(0);
  const lastVersionRef = useRef(-1);

  const { getHandleProps, isDragging } = useResizeDrag({
    editorRef,
    bounds,
    camera,
    onSceneChanged,
  });

  const { rotationHandleProps, isRotating } = useRotationDrag({
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
          const version = editor.scene_version();
          if (version !== lastVersionRef.current) {
            lastVersionRef.current = version;
            setBounds(editor.get_selected_bounds());
            setCamera(editor.get_camera());
          }
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

  // For multi-select, compute union bounding box.
  const unionBox = bounds.length > 1 ? computeUnionBounds(bounds) : null;

  return (
    <div style={overlayStyle}>
      {bounds.map((b, i) => (
        <SelectionBox
          key={i}
          bounds={b}
          camera={camera}
          getHandleProps={getHandleProps}
          rotationHandleProps={rotationHandleProps}
          isDragging={isDragging}
          isRotating={isRotating}
          showRotationHandle={bounds.length === 1}
        />
      ))}
      {unionBox && <UnionBox bounds={unionBox} camera={camera} />}
    </div>
  );
}

interface SelectionBoxProps {
  bounds: SelectionBounds;
  camera: CameraInfo;
  getHandleProps: (index: number) => {
    onPointerDown: (e: React.PointerEvent) => void;
  };
  rotationHandleProps: {
    onPointerDown: (e: React.PointerEvent) => void;
    onPointerMove: (e: React.PointerEvent) => void;
    onPointerUp: (e: React.PointerEvent) => void;
  };
  isDragging: boolean;
  isRotating: boolean;
  showRotationHandle: boolean;
}

function SelectionBox({
  bounds,
  camera,
  getHandleProps,
  rotationHandleProps,
  isDragging,
  isRotating,
  showRotationHandle,
}: SelectionBoxProps) {
  const { x: screenX, y: screenY } = worldToScreen(bounds.x, bounds.y, camera);
  const { w: screenW, h: screenH } = worldDimsToScreen(
    bounds.width,
    bounds.height,
    camera.zoom,
  );

  const half = HANDLE_SIZE / 2;
  const interacting = isDragging || isRotating;

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
            background: colors.white,
            border: `1px solid ${SELECTION_COLOR}`,
            pointerEvents: interacting ? "none" : "auto",
            cursor: HANDLE_CURSORS[i],
          }}
          {...getHandleProps(i)}
        />
      ))}
      {showRotationHandle && (
        <>
          {/* Stem line from top-center to rotation handle */}
          <div
            style={{
              position: "absolute",
              left: screenW / 2,
              top: -ROTATION_HANDLE_OFFSET,
              width: 1,
              height: ROTATION_HANDLE_OFFSET,
              background: SELECTION_COLOR,
              pointerEvents: "none",
            }}
          />
          {/* Rotation handle circle */}
          <div
            data-testid="rotation-handle"
            style={{
              position: "absolute",
              left: screenW / 2 - ROTATION_HANDLE_SIZE / 2,
              top: -ROTATION_HANDLE_OFFSET - ROTATION_HANDLE_SIZE / 2,
              width: ROTATION_HANDLE_SIZE,
              height: ROTATION_HANDLE_SIZE,
              borderRadius: "50%",
              background: colors.white,
              border: `1.5px solid ${SELECTION_COLOR}`,
              pointerEvents: interacting ? "none" : "auto",
              cursor: "grab",
            }}
            {...rotationHandleProps}
          />
        </>
      )}
    </div>
  );
}

function UnionBox({
  bounds,
  camera,
}: {
  bounds: { x: number; y: number; width: number; height: number };
  camera: CameraInfo;
}) {
  const { x: screenX, y: screenY } = worldToScreen(bounds.x, bounds.y, camera);
  const { w: screenW, h: screenH } = worldDimsToScreen(
    bounds.width,
    bounds.height,
    camera.zoom,
  );

  return (
    <div
      data-testid="union-box"
      style={{
        position: "absolute",
        left: screenX,
        top: screenY,
        width: screenW,
        height: screenH,
        border: `1px dashed ${SELECTION_COLOR}`,
        pointerEvents: "none",
      }}
    />
  );
}

function computeUnionBounds(bounds: SelectionBounds[]): {
  x: number;
  y: number;
  width: number;
  height: number;
} {
  let minX = Infinity,
    minY = Infinity,
    maxX = -Infinity,
    maxY = -Infinity;
  for (const b of bounds) {
    minX = Math.min(minX, b.x);
    minY = Math.min(minY, b.y);
    maxX = Math.max(maxX, b.x + b.width);
    maxY = Math.max(maxY, b.y + b.height);
  }
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
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
