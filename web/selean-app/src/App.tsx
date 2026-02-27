import { useCallback, useRef, useState } from "react";
import { Canvas, type InteractionEvent } from "./components/Canvas";
import { ChatSidebar } from "./components/ChatSidebar";
import { PropertyInspector } from "./components/PropertyInspector";
import { useSeleanEditor } from "./hooks/useSeleanEditor";
import { useSelection } from "./hooks/useSelection";
import { colors, fontSizes } from "./theme";

/** Interaction event types that should trigger a selection refresh. */
const REFRESH_EVENT_TYPES = new Set([
  "SelectionChanged",
  "Clicked",
  "ClickedCanvas",
]);

export function App() {
  const [undoRedoTick, setUndoRedoTick] = useState(0);

  // Keyboard undo/redo callback. Uses ref inside useSeleanEditor,
  // so it picks up the latest refresh even though refresh is defined after.
  const onKeyboardUndoRedo = useCallback(() => {
    refreshRef.current();
    setUndoRedoTick((t) => t + 1);
  }, []);

  const { editorRef, status, error } = useSeleanEditor(
    "selean-canvas",
    onKeyboardUndoRedo,
  );
  const { selectedNode, refresh } = useSelection(editorRef, status === "ready");

  // Stable ref for refresh, used by onKeyboardUndoRedo.
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;

  const onSceneChanged = useCallback(() => {
    refresh();
  }, [refresh]);

  const handleInteractionEvents = useCallback(
    (events: InteractionEvent[]) => {
      for (const event of events) {
        if (REFRESH_EVENT_TYPES.has(event.type)) {
          refresh();
          return;
        }
      }
    },
    [refresh],
  );

  const handleUndo = useCallback(() => {
    editorRef.current?.undo();
    refresh();
    setUndoRedoTick((t) => t + 1);
  }, [editorRef, refresh]);

  const handleRedo = useCallback(() => {
    editorRef.current?.redo();
    refresh();
    setUndoRedoTick((t) => t + 1);
  }, [editorRef, refresh]);

  const canUndo =
    status === "ready" && (editorRef.current?.can_undo() ?? false);
  const canRedo =
    status === "ready" && (editorRef.current?.can_redo() ?? false);
  // Force re-evaluation when undo/redo tick changes.
  void undoRedoTick;

  return (
    <div style={rootStyle}>
      <header style={headerStyle}>
        <span style={{ fontWeight: 600 }}>Selean</span>
        <span style={statusStyle}>
          {status === "loading" && "Initializing..."}
          {status === "ready" && "Ready"}
          {status === "error" && `Error: ${error}`}
          {status === "unsupported" && "WebGPU not supported"}
        </span>
        {status === "ready" && (
          <div style={toolbarStyle}>
            <button
              style={{
                ...buttonStyle,
                opacity: canUndo ? 1 : 0.4,
                cursor: canUndo ? "pointer" : "default",
              }}
              onClick={handleUndo}
              disabled={!canUndo}
            >
              Undo
            </button>
            <button
              style={{
                ...buttonStyle,
                opacity: canRedo ? 1 : 0.4,
                cursor: canRedo ? "pointer" : "default",
              }}
              onClick={handleRedo}
              disabled={!canRedo}
            >
              Redo
            </button>
          </div>
        )}
      </header>
      <div style={bodyStyle}>
        {status === "ready" && (
          <ChatSidebar editorRef={editorRef} onSceneChanged={onSceneChanged} />
        )}
        <div style={canvasAreaStyle}>
          <Canvas
            canvasId="selean-canvas"
            editorRef={editorRef}
            status={status}
            onInteractionEvents={handleInteractionEvents}
          />
        </div>
        {status === "ready" && (
          <PropertyInspector
            node={selectedNode}
            editorRef={editorRef}
            onSceneChanged={onSceneChanged}
          />
        )}
      </div>
    </div>
  );
}

const rootStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
};

const headerStyle: React.CSSProperties = {
  height: 40,
  display: "flex",
  alignItems: "center",
  padding: "0 16px",
  borderBottom: `1px solid ${colors.border}`,
  fontSize: fontSizes.lg,
  gap: 16,
  flexShrink: 0,
};

const statusStyle: React.CSSProperties = {
  color: colors.textDim,
  fontSize: fontSizes.base,
};

const toolbarStyle: React.CSSProperties = {
  marginLeft: "auto",
  display: "flex",
  gap: 8,
};

const bodyStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  overflow: "hidden",
};

const canvasAreaStyle: React.CSSProperties = {
  flex: 1,
  position: "relative",
  minWidth: 0,
};

const buttonStyle: React.CSSProperties = {
  background: colors.border,
  border: `1px solid ${colors.borderHover}`,
  color: colors.text,
  padding: "4px 12px",
  borderRadius: 4,
  cursor: "pointer",
  fontSize: fontSizes.base,
};
