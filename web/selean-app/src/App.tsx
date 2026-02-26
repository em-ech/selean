import { useCallback } from "react";
import { Canvas } from "./components/Canvas";
import { ChatSidebar } from "./components/ChatSidebar";
import { PropertyInspector } from "./components/PropertyInspector";
import { useSeleanEditor } from "./hooks/useSeleanEditor";
import { useSelection } from "./hooks/useSelection";
import { colors, fontSizes } from "./theme";

export function App() {
  const { editorRef, status, error } = useSeleanEditor("selean-canvas");
  const { selectedNode, refresh } = useSelection(editorRef, status === "ready");

  const onSceneChanged = useCallback(() => {
    refresh();
  }, [refresh]);

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
              style={buttonStyle}
              onClick={() => editorRef.current?.undo()}
            >
              Undo
            </button>
            <button
              style={buttonStyle}
              onClick={() => editorRef.current?.redo()}
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
