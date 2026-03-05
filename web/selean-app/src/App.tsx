import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Canvas, type InteractionEvent } from "./components/Canvas";
import { ChatSidebar } from "./components/ChatSidebar";
import { CollabBar } from "./components/CollabBar";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { FileMenu } from "./components/FileMenu";
import { LayerPanel } from "./components/LayerPanel";
import { PageBar } from "./components/PageBar";
import { PresenceOverlay } from "./components/PresenceOverlay";
import { PropertyInspector } from "./components/PropertyInspector";
import { SelectionOverlay } from "./components/SelectionOverlay";
import { Toolbar, type ToolType } from "./components/Toolbar";
import { AlignmentBar } from "./components/AlignmentBar";
import { ContextMenu } from "./components/ContextMenu";
import { InlineTextEditor } from "./components/InlineTextEditor";
import { CollabContext } from "./collab/CollabContext";
import { useAutoSave } from "./hooks/useAutoSave";
import { useCollabSession } from "./hooks/useCollabSession";
import { useCreationTool } from "./hooks/useCreationTool";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useMoveDrag } from "./hooks/useMoveDrag";
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

export function App() {
  const [undoRedoTick, setUndoRedoTick] = useState(0);
  const [activeTool, setActiveTool] = useState<ToolType>("select");
  const [refreshTick, setRefreshTick] = useState(0);
  const clipboardRef = useRef<NodeInfo | null>(null);
  const imageInputRef = useRef<HTMLInputElement>(null);
  const [contextMenuPos, setContextMenuPos] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [editingNodeId, setEditingNodeId] = useState<string | null>(null);
  const lastClickNodeIdRef = useRef<string | null>(null);
  const lastClickTimeRef = useRef(0);

  const { editorRef, status, error } = useSeleanEditor("selean-canvas");
  const { selectedNode, selectedIds, refresh } = useSelection(
    editorRef,
    status === "ready",
  );

  const collabOnRemoteChange = useCallback(() => {
    refresh();
    setRefreshTick((t) => t + 1);
  }, [refresh]);

  const collab = useCollabSession({
    editorRef,
    onRemoteChange: collabOnRemoteChange,
  });

  const activePageId = useMemo(() => {
    if (status !== "ready" || !editorRef.current) return "";
    return editorRef.current.active_page_id();
    // Re-derive when refreshTick changes (page switches).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, editorRef, refreshTick]);

  const { markDirty, loadSavedDocument, clearSavedDocument } = useAutoSave({
    editorRef,
    isReady: status === "ready",
  });

  const onSceneChanged = useCallback(() => {
    refresh();
    setRefreshTick((t) => t + 1);
    markDirty();
  }, [refresh, markDirty]);

  // Auto-load saved document on mount.
  useEffect(() => {
    if (status !== "ready") return;
    const editor = editorRef.current;
    if (!editor) return;
    const saved = loadSavedDocument();
    if (saved) {
      editor.import_document(saved);
      onSceneChanged();
    }
  }, [status]); // eslint-disable-line react-hooks/exhaustive-deps

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
              kind: "Image",
              x: 100,
              y: 100,
              width: 400,
              height: 300,
              asset_ref: assetRef,
            }),
          );
          onSceneChanged();
        }
      } catch (e) {
        console.warn("image upload failed", e);
      }
      e.target.value = "";
    },
    [editorRef, onSceneChanged],
  );

  // Convert collab remote presences to PresenceOverlay format.
  const presenceOverlayData = useMemo(
    () =>
      collab.remotePresences.map((p) => ({
        participant: {
          session_id: p.sessionId,
          user_id: p.userId,
          display_name: p.displayName,
        },
        pageId: p.pageId,
        cursor: p.cursor,
        selectedNodeIds: p.selectedNodeIds,
      })),
    [collab.remotePresences],
  );

  // Move drag hook
  const { handleDragEvent } = useMoveDrag({
    editorRef,
    onSceneChanged,
    collab,
    activePageId,
  });

  // Creation tool hook
  const { creationHandlers } = useCreationTool({
    editorRef,
    activeTool,
    onToolReset: useCallback(() => setActiveTool("select"), []),
    onSceneChanged,
  });

  const handleContextMenu = useCallback((cx: number, cy: number) => {
    setContextMenuPos({ x: cx, y: cy });
  }, []);

  const handleCloseContextMenu = useCallback(() => {
    setContextMenuPos(null);
  }, []);

  const handleInteractionEvents = useCallback(
    (events: InteractionEvent[]) => {
      const editor = editorRef.current;
      for (const event of events) {
        if (
          event.type === "DragStarted" ||
          event.type === "DragMoved" ||
          event.type === "DragEnded"
        ) {
          handleDragEvent(event);
          continue;
        }

        // Double-click detection for inline text editing.
        if (event.type === "Clicked" && editor) {
          const clickedId = event.node_id as string | undefined;
          if (clickedId) {
            const now = Date.now();
            if (
              clickedId === lastClickNodeIdRef.current &&
              now - lastClickTimeRef.current < 300
            ) {
              // Double-click detected. Check if it's a Text node.
              try {
                const json = editor.get_node_json(clickedId);
                if (json !== "null") {
                  const node: NodeInfo = JSON.parse(json);
                  if (node.kind === "Text") {
                    setEditingNodeId(clickedId);
                    lastClickNodeIdRef.current = null;
                    lastClickTimeRef.current = 0;
                    refresh();
                    return;
                  }
                }
              } catch (e) {
                console.warn("double-click node parse failed", e);
              }
            }
            lastClickNodeIdRef.current = clickedId;
            lastClickTimeRef.current = now;
          } else {
            lastClickNodeIdRef.current = null;
            lastClickTimeRef.current = 0;
          }
        }

        if (REFRESH_EVENT_TYPES.has(event.type)) {
          refresh();
          return;
        }
      }
    },
    [refresh, handleDragEvent, editorRef],
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

  const handleUndoRedoTick = useCallback(() => {
    setUndoRedoTick((t) => t + 1);
  }, []);

  useKeyboardShortcuts({
    editorRef,
    isReady: status === "ready",
    onSceneChanged,
    activeTool,
    setActiveTool,
    handleToolChange,
    clipboardRef,
    onUndoRedoTick: handleUndoRedoTick,
  });

  const canUndo =
    status === "ready" && (editorRef.current?.can_undo() ?? false);
  const canRedo =
    status === "ready" && (editorRef.current?.can_redo() ?? false);
  // Force re-evaluation when undo/redo tick changes.
  void undoRedoTick;

  const handleCollabConnect = useCallback(() => {
    const userId = crypto.randomUUID();
    const roomId = "default-room";
    collab.connect(roomId, userId, `User ${userId.slice(0, 4)}`);
  }, [collab]);

  const handleCollabDisconnect = useCallback(() => {
    collab.disconnect();
  }, [collab]);

  return (
    <CollabContext.Provider value={collab}>
      <div style={rootStyle}>
        <header style={headerStyle}>
          <span style={{ fontWeight: 600 }}>Selean</span>
          {status === "ready" && (
            <FileMenu
              editorRef={editorRef}
              onSceneChanged={onSceneChanged}
              onClearAutoSave={clearSavedDocument}
            />
          )}
          <span style={statusStyle}>
            {status === "loading" && "Initializing..."}
            {status === "ready" && "Ready"}
            {status === "error" && `Error: ${error}`}
            {status === "unsupported" && "WebGPU not supported"}
          </span>
          {status === "ready" && (
            <CollabBar
              status={collab.status}
              participants={collab.participants}
              hasPendingOps={collab.hasPendingOps}
              onConnect={handleCollabConnect}
              onDisconnect={handleCollabDisconnect}
            />
          )}
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
              <ChatSidebar
                editorRef={editorRef}
                onSceneChanged={onSceneChanged}
              />
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
                onContextMenu={handleContextMenu}
              />
            </ErrorBoundary>
            {status === "ready" && (
              <SelectionOverlay
                editorRef={editorRef}
                onSceneChanged={onSceneChanged}
              />
            )}
            {status === "ready" && collab.status === "connected" && (
              <PresenceOverlay
                editorRef={editorRef}
                presences={presenceOverlayData}
                activePageId={activePageId}
              />
            )}
            {editingNodeId &&
              status === "ready" &&
              (() => {
                const editor = editorRef.current;
                if (!editor) return null;
                try {
                  const json = editor.get_node_json(editingNodeId);
                  if (json === "null") return null;
                  const node: NodeInfo = JSON.parse(json);
                  if (node.kind !== "Text") return null;
                  return (
                    <InlineTextEditor
                      nodeId={editingNodeId}
                      initialContent={node.text_content ?? ""}
                      bounds={{
                        x: node.x,
                        y: node.y,
                        width: node.width,
                        height: node.height,
                      }}
                      fontSize={node.font_size ?? 16}
                      editorRef={editorRef}
                      onCommit={(content) => {
                        editor.execute_tool_call(
                          "set_text",
                          JSON.stringify({
                            node_id: editingNodeId,
                            content,
                          }),
                        );
                        setEditingNodeId(null);
                        onSceneChanged();
                      }}
                      onCancel={() => setEditingNodeId(null)}
                    />
                  );
                } catch (e) {
                  console.warn("inline text editor setup failed", e);
                  return null;
                }
              })()}
            {status === "ready" && (
              <AlignmentBar
                selectedIds={selectedIds}
                editorRef={editorRef}
                onSceneChanged={onSceneChanged}
              />
            )}
            {contextMenuPos && (
              <ContextMenu
                x={contextMenuPos.x}
                y={contextMenuPos.y}
                editorRef={editorRef}
                onSceneChanged={onSceneChanged}
                onClose={handleCloseContextMenu}
                clipboardRef={clipboardRef}
              />
            )}
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
                  activePageId={activePageId}
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
    </CollabContext.Provider>
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
