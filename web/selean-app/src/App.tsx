import { useCallback, useEffect, useRef, useState } from "react";
import { Canvas, type InteractionEvent } from "./components/Canvas";
import { ChatSidebar } from "./components/ChatSidebar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { FileMenu } from "./components/FileMenu";
import { LayerPanel } from "./components/LayerPanel";
import { PageBar } from "./components/PageBar";
import { PropertyInspector } from "./components/PropertyInspector";
import { SelectionOverlay } from "./components/SelectionOverlay";
import { Toolbar, type ToolType } from "./components/Toolbar";
import { useCreationTool } from "./hooks/useCreationTool";
import { useSeleanEditor } from "./hooks/useSeleanEditor";
import { useSelection } from "./hooks/useSelection";
import type { NodeInfo } from "./wasm/types";
import { colors, fontSizes } from "./theme";

/** Interaction event types that should trigger a selection refresh. */
const REFRESH_EVENT_TYPES = new Set([
  "SelectionChanged",
  "Clicked",
  "ClickedCanvas",
]);

/** Tags that should suppress single-key shortcuts. */
const INPUT_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);

export function App() {
  const [undoRedoTick, setUndoRedoTick] = useState(0);
  const [activeTool, setActiveTool] = useState<ToolType>("select");
  const [refreshTick, setRefreshTick] = useState(0);
  const clipboardRef = useRef<NodeInfo | null>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);

  const { editorRef, status, error } = useSeleanEditor("selean-canvas");
  const { selectedNode, refresh } = useSelection(editorRef, status === "ready");

  const onSceneChanged = useCallback(() => {
    refresh();
    setRefreshTick((t) => t + 1);
  }, [refresh]);

  // Handle tool changes, intercepting the image tool to open file picker.
  const handleToolChange = useCallback((tool: ToolType) => {
    if (tool === "image") {
      imageInputRef.current?.click();
      // Don't change active tool; stay on select.
      return;
    }
    setActiveTool(tool);
  }, []);

  const handleImageSelected = useCallback(
    async (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];
      if (!file) return;
      const editor = editorRef.current;
      if (!editor) return;
      try {
        const buffer = await file.arrayBuffer();
        const data = new Uint8Array(buffer);
        const assetRef = `img_${Date.now()}`;
        const ok = editor.register_image_asset(assetRef, data);
        if (ok) {
          // Create an image node at the center of the viewport.
          editor.execute_tool_call(
            "create_node",
            JSON.stringify({
              name: file.name.replace(/\.[^.]+$/, ""),
              kind: "Frame",
              x: 100,
              y: 100,
              width: 400,
              height: 300,
            }),
          );
          onSceneChanged();
        }
      } catch {
        // Image read failed
      }
      e.target.value = "";
    },
    [editorRef, onSceneChanged],
  );

  // Creation tool hook
  const { creationHandlers } = useCreationTool({
    editorRef,
    activeTool,
    onToolReset: useCallback(() => setActiveTool("select"), []),
    onSceneChanged,
  });

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
    onSceneChanged();
    setUndoRedoTick((t) => t + 1);
  }, [editorRef, onSceneChanged]);

  const handleRedo = useCallback(() => {
    editorRef.current?.redo();
    onSceneChanged();
    setUndoRedoTick((t) => t + 1);
  }, [editorRef, onSceneChanged]);

  // Consolidated keyboard shortcuts
  useEffect(() => {
    if (status !== "ready") return;

    const handleKeyDown = (e: KeyboardEvent) => {
      const editor = editorRef.current;
      if (!editor) return;

      const tag = (e.target as HTMLElement).tagName;
      const isInputFocused = INPUT_TAGS.has(tag);
      const isCtrlOrMeta = e.ctrlKey || e.metaKey;

      // Undo: Cmd+Z
      if (isCtrlOrMeta && e.key === "z" && !e.shiftKey) {
        e.preventDefault();
        editor.undo();
        onSceneChanged();
        setUndoRedoTick((t) => t + 1);
        return;
      }

      // Redo: Cmd+Shift+Z or Cmd+Y
      if (
        (isCtrlOrMeta && e.key === "z" && e.shiftKey) ||
        (isCtrlOrMeta && e.key === "y")
      ) {
        e.preventDefault();
        editor.redo();
        onSceneChanged();
        setUndoRedoTick((t) => t + 1);
        return;
      }

      // Skip remaining shortcuts when focused on text input
      if (isInputFocused) return;

      // Delete / Backspace: delete selected nodes
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          for (const id of ids) {
            editor.execute_tool_call(
              "delete_node",
              JSON.stringify({ node_id: id }),
            );
          }
          if (ids.length > 0) onSceneChanged();
        } catch {
          // parse failure
        }
        return;
      }

      // Cmd+C: Copy
      if (isCtrlOrMeta && e.key === "c") {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length > 0) {
            const nodeJson = editor.get_node_json(ids[0]);
            if (nodeJson !== "null") {
              clipboardRef.current = JSON.parse(nodeJson);
            }
          }
        } catch {
          // parse failure
        }
        return;
      }

      // Cmd+V: Paste
      if (isCtrlOrMeta && e.key === "v") {
        e.preventDefault();
        const node = clipboardRef.current;
        if (node) {
          pasteNode(editor, node);
          onSceneChanged();
        }
        return;
      }

      // Cmd+D: Duplicate
      if (isCtrlOrMeta && e.key === "d") {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          if (ids.length > 0) {
            const nodeJson = editor.get_node_json(ids[0]);
            if (nodeJson !== "null") {
              const node: NodeInfo = JSON.parse(nodeJson);
              pasteNode(editor, node);
              onSceneChanged();
            }
          }
        } catch {
          // parse failure
        }
        return;
      }

      // Arrow keys: Nudge selected nodes
      if (["ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight"].includes(e.key)) {
        e.preventDefault();
        try {
          const ids: string[] = JSON.parse(editor.get_selected_ids());
          const step = e.shiftKey ? 10 : 1;
          let dx = 0;
          let dy = 0;
          if (e.key === "ArrowLeft") dx = -step;
          if (e.key === "ArrowRight") dx = step;
          if (e.key === "ArrowUp") dy = -step;
          if (e.key === "ArrowDown") dy = step;

          for (const id of ids) {
            const json = editor.get_node_json(id);
            if (json === "null") continue;
            const node: NodeInfo = JSON.parse(json);
            editor.execute_tool_call(
              "set_bounds",
              JSON.stringify({
                node_id: id,
                x: node.x + dx,
                y: node.y + dy,
                width: node.width,
                height: node.height,
              }),
            );
          }
          if (ids.length > 0) onSceneChanged();
        } catch {
          // parse failure
        }
        return;
      }

      // Escape: Deselect all, or reset tool to select
      if (e.key === "Escape") {
        e.preventDefault();
        if (activeTool !== "select") {
          setActiveTool("select");
        } else {
          editor.clear_selection();
          onSceneChanged();
        }
        return;
      }

      // Single-key tool shortcuts (no modifiers, not in input)
      if (!isCtrlOrMeta && !e.altKey) {
        const lower = e.key.toLowerCase();
        if (lower === "v") {
          setActiveTool("select");
          return;
        }
        if (lower === "f") {
          setActiveTool("frame");
          return;
        }
        if (lower === "t") {
          setActiveTool("text");
          return;
        }
        if (lower === "i") {
          handleToolChange("image");
          return;
        }
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [status, editorRef, onSceneChanged, activeTool, handleToolChange]);

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
        {status === "ready" && (
          <FileMenu editorRef={editorRef} onSceneChanged={onSceneChanged} />
        )}
        <span style={statusStyle}>
          {status === "loading" && "Initializing..."}
          {status === "ready" && "Ready"}
          {status === "error" && `Error: ${error}`}
          {status === "unsupported" && "WebGPU not supported"}
        </span>
        {status === "ready" && (
          <div style={headerToolbarStyle}>
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
      {status === "ready" && (
        <PageBar
          editorRef={editorRef}
          onSceneChanged={onSceneChanged}
          refreshTick={refreshTick}
        />
      )}
      <div style={bodyStyle}>
        {status === "ready" && (
          <ErrorBoundary name="Chat">
            <ChatSidebar editorRef={editorRef} onSceneChanged={onSceneChanged} />
          </ErrorBoundary>
        )}
        {status === "ready" && (
          <Toolbar activeTool={activeTool} onToolChange={handleToolChange} />
        )}
        <div style={canvasAreaStyle}>
          <ErrorBoundary name="Canvas">
            <Canvas
              canvasId="selean-canvas"
              editorRef={editorRef}
              status={status}
              onInteractionEvents={handleInteractionEvents}
            />
          </ErrorBoundary>
          {status === "ready" && <SelectionOverlay editorRef={editorRef} />}
          {creationHandlers && (
            <div style={creationOverlayStyle} {...creationHandlers} />
          )}
        </div>
        {status === "ready" && (
          <div style={rightPanelStyle}>
            <ErrorBoundary name="Layers">
              <LayerPanel
                editorRef={editorRef}
                onSceneChanged={onSceneChanged}
                refreshTick={refreshTick}
              />
            </ErrorBoundary>
            <ErrorBoundary name="Properties">
              <PropertyInspector
                node={selectedNode}
                editorRef={editorRef}
                onSceneChanged={onSceneChanged}
              />
            </ErrorBoundary>
          </div>
        )}
      </div>
      <input
        ref={imageInputRef}
        type="file"
        accept="image/png,image/jpeg,image/webp"
        style={{ display: "none" }}
        onChange={handleImageSelected}
      />
    </div>
  );
}

/** Creates a copy of a node at an offset position. */
function pasteNode(
  editor: { execute_tool_call: (name: string, args: string) => string },
  node: NodeInfo,
) {
  const args: Record<string, unknown> = {
    name: `${node.name} copy`,
    kind: node.kind,
    x: node.x + 10,
    y: node.y + 10,
    width: node.width,
    height: node.height,
  };

  if (node.fill) {
    args.fill_r = node.fill[0];
    args.fill_g = node.fill[1];
    args.fill_b = node.fill[2];
    args.fill_a = node.fill[3];
  }

  if (node.text_content) {
    args.text_content = node.text_content;
  }

  if (node.font_size) {
    args.font_size = node.font_size;
  }

  editor.execute_tool_call("create_node", JSON.stringify(args));
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

const headerToolbarStyle: React.CSSProperties = {
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

const creationOverlayStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  cursor: "crosshair",
  zIndex: 10,
};

const rightPanelStyle: React.CSSProperties = {
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
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
