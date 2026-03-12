import { useEffect, useRef, useState } from "react";
import type {
  CameraInfo,
  NodeBoundsInfo,
  SelectionBounds,
  SeleanEditor,
} from "../wasm/types";
import { worldToScreen } from "../utils/camera";

/** Snap threshold in world units. */
const SNAP_THRESHOLD = 5;

/** Guide line color. */
const GUIDE_COLOR = "#ff4488";

interface SnapGuidesProps {
  editorRef: React.RefObject<SeleanEditor | null>;
  isDragging: boolean;
}

interface GuideLine {
  axis: "x" | "y";
  worldPos: number;
}

/**
 * Renders snap guide lines when a node is being dragged near other nodes'
 * edges or centers. Only visible during active drag operations.
 */
export function SnapGuides({ editorRef, isDragging }: SnapGuidesProps) {
  const [guides, setGuides] = useState<GuideLine[]>([]);
  const [camera, setCamera] = useState<CameraInfo | null>(null);
  const rafRef = useRef(0);

  useEffect(() => {
    if (!isDragging) {
      setGuides([]);
      return;
    }

    function poll() {
      const editor = editorRef.current;
      if (!editor) {
        rafRef.current = requestAnimationFrame(poll);
        return;
      }

      try {
        const cam = editor.get_camera();
        setCamera(cam);

        const selectedBounds = editor.get_selected_bounds();
        if (selectedBounds.length === 0) {
          setGuides([]);
          rafRef.current = requestAnimationFrame(poll);
          return;
        }

        const sel = unionBounds(selectedBounds);
        // Use typed narrow query instead of full-scene JSON serialization.
        const targets: NodeBoundsInfo[] = editor.get_snap_targets();

        const newGuides: GuideLine[] = [];

        for (const node of targets) {
          const edges = getEdges(node);
          const selEdges = getEdgesFromRect(sel);

          // Check X alignment (vertical guides)
          for (const sx of selEdges.xs) {
            for (const nx of edges.xs) {
              if (Math.abs(sx - nx) < SNAP_THRESHOLD) {
                newGuides.push({ axis: "x", worldPos: nx });
              }
            }
          }

          // Check Y alignment (horizontal guides)
          for (const sy of selEdges.ys) {
            for (const ny of edges.ys) {
              if (Math.abs(sy - ny) < SNAP_THRESHOLD) {
                newGuides.push({ axis: "y", worldPos: ny });
              }
            }
          }
        }

        // Deduplicate guides within threshold
        const dedupedGuides = deduplicateGuides(newGuides);
        setGuides(dedupedGuides);
      } catch {
        setGuides([]);
      }
      rafRef.current = requestAnimationFrame(poll);
    }

    rafRef.current = requestAnimationFrame(poll);
    return () => cancelAnimationFrame(rafRef.current);
  }, [editorRef, isDragging]);

  if (!isDragging || guides.length === 0 || !camera) return null;

  return (
    <div style={overlayStyle}>
      {guides.map((g, i) => {
        if (g.axis === "x") {
          const { x } = worldToScreen(g.worldPos, 0, camera);
          return (
            <div
              key={`x-${i}`}
              style={{
                position: "absolute",
                left: x,
                top: 0,
                width: 1,
                height: "100%",
                background: GUIDE_COLOR,
                pointerEvents: "none",
              }}
            />
          );
        }
        const { y } = worldToScreen(0, g.worldPos, camera);
        return (
          <div
            key={`y-${i}`}
            style={{
              position: "absolute",
              left: 0,
              top: y,
              width: "100%",
              height: 1,
              background: GUIDE_COLOR,
              pointerEvents: "none",
            }}
          />
        );
      })}
    </div>
  );
}

function getEdges(node: NodeBoundsInfo) {
  return {
    xs: [node.x, node.x + node.width / 2, node.x + node.width],
    ys: [node.y, node.y + node.height / 2, node.y + node.height],
  };
}

function getEdgesFromRect(r: {
  x: number;
  y: number;
  width: number;
  height: number;
}) {
  return {
    xs: [r.x, r.x + r.width / 2, r.x + r.width],
    ys: [r.y, r.y + r.height / 2, r.y + r.height],
  };
}

function unionBounds(bounds: SelectionBounds[]) {
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

function deduplicateGuides(guides: GuideLine[]): GuideLine[] {
  const seen = new Set<string>();
  return guides.filter((g) => {
    const key = `${g.axis}:${Math.round(g.worldPos)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

const overlayStyle: React.CSSProperties = {
  position: "absolute",
  top: 0,
  left: 0,
  width: "100%",
  height: "100%",
  pointerEvents: "none",
  overflow: "hidden",
  zIndex: 15,
};
