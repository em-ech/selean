import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Canvas, type InteractionEvent } from "./components/Canvas";
import { ChatPanel } from "./components/ChatPanel";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { ErrorToast } from "./components/ErrorToast";
import { FloatingToolbar } from "./components/FloatingToolbar";
import { HeaderBar } from "./components/HeaderBar";
import { LayerPanel } from "./components/LayerPanel";
import { LeftSidebar } from "./components/LeftSidebar";
import { PageBar } from "./components/PageBar";
import { PresenceOverlay } from "./components/PresenceOverlay";
import { SelectionOverlay } from "./components/SelectionOverlay";
import { CodePanel } from "./components/CodePanel";
import { ContextMenu } from "./components/ContextMenu";
import { InlineTextEditor } from "./components/InlineTextEditor";
import { MemberManager } from "./components/MemberManager";
import { SnapGuides } from "./components/SnapGuides";
import { CollabContext } from "./collab/CollabContext";
import { EditorProvider } from "./contexts/EditorContext";
import { useAutoSave } from "./hooks/useAutoSave";
import { useCollabSession } from "./hooks/useCollabSession";
import { useCreationTool } from "./hooks/useCreationTool";
import { useKeyboardShortcuts } from "./hooks/useKeyboardShortcuts";
import { useMoveDrag } from "./hooks/useMoveDrag";
import { WorkspaceProvider, useWorkspace } from "./hooks/useWorkspace";
import { useSeleanEditor } from "./hooks/useSeleanEditor";
import { useSelection } from "./hooks/useSelection";
import type { NodeInfo } from "./wasm/types";
import { worldToScreen, worldDimsToScreen } from "./utils/camera";
import { colors } from "./theme";
import { useResizablePanel } from "./hooks/useResizablePanel";
import { NodeKind, type ToolType } from "./types/editor";

/** Interaction event types that should trigger a selection refresh. */
const REFRESH_EVENT_TYPES = new Set([
  "SelectionChanged",
  "Clicked",
  "ClickedCanvas",
]);

export function App() {
  return (
    <WorkspaceProvider>
      <AppContent />
    </WorkspaceProvider>
  );
}

type RightView = "design" | "code";

function AppContent() {
  const { activeWorkspace } = useWorkspace();
  const [undoRedoTick, setUndoRedoTick] = useState(0);
  const [activeTool, setActiveTool] = useState<ToolType>("select");
  const [refreshTick, setRefreshTick] = useState(0);
  const clipboardRef = useRef<NodeInfo | null>(null);
  const [contextMenuPos, setContextMenuPos] = useState<{
    x: number;
    y: number;
  } | null>(null);
  const [editingNodeId, setEditingNodeId] = useState<string | null>(null);
  const [showMembers, setShowMembers] = useState(false);
  const [rightView, setRightView] = useState<RightView>("design");
  const lastClickNodeIdRef = useRef<string | null>(null);
  const lastClickTimeRef = useRef(0);

  // Panel toggle state
  const [showLeftSidebar, setShowLeftSidebar] = useState(
    () => localStorage.getItem("selean-panel-sidebar") !== "false",
  );
  const [showChat, setShowChat] = useState(
    () => localStorage.getItem("selean-panel-chat") !== "false",
  );
  const [showLayers, setShowLayers] = useState(
    () => localStorage.getItem("selean-panel-layers") !== "false",
  );

  // Persist panel states.
  useEffect(() => {
    localStorage.setItem("selean-panel-sidebar", String(showLeftSidebar));
  }, [showLeftSidebar]);
  useEffect(() => {
    localStorage.setItem("selean-panel-chat", String(showChat));
  }, [showChat]);
  useEffect(() => {
    localStorage.setItem("selean-panel-layers", String(showLayers));
  }, [showLayers]);

  // Resizable panel widths
  const sidebarResize = useResizablePanel({
    initialWidth: 260,
    minWidth: 200,
    maxWidth: 400,
    storageKey: "selean-sidebar-width",
    edge: "right",
  });
  const chatResize = useResizablePanel({
    initialWidth: 380,
    minWidth: 300,
    maxWidth: 600,
    storageKey: "selean-chat-width",
    edge: "right",
  });
  const layersResize = useResizablePanel({
    initialWidth: 180,
    minWidth: 120,
    maxWidth: 400,
    storageKey: "selean-layers-width",
    edge: "left",
  });

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

  // Track shift key state for axis-lock during drag.
  const shiftKeyRef = useRef(false);
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "Shift") shiftKeyRef.current = true;
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === "Shift") shiftKeyRef.current = false;
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, []);

  // Move drag hook
  const { handleDragEvent, isDragging: isMoveDragging } = useMoveDrag({
    editorRef,
    onSceneChanged,
    collab,
    activePageId,
    shiftKeyRef,
  });

  // Creation tool hook (kept for context menu creation path)
  const { creationHandlers } = useCreationTool({
    editorRef,
    activeTool,
    onToolReset: useCallback(() => setActiveTool("select"), []),
    onSceneChanged,
  });

  const handleContextMenu = useCallback((cx: number, cy: number) => {
    // Clamp to viewport to prevent off-screen overflow.
    const menuW = 180;
    const menuH = 300;
    const x = Math.min(cx, window.innerWidth - menuW);
    const y = Math.min(cy, window.innerHeight - menuH);
    setContextMenuPos({ x, y });
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
              try {
                const json = editor.get_node_json(clickedId);
                if (json !== "null") {
                  const node: NodeInfo = JSON.parse(json);
                  if (node.kind === NodeKind.Text) {
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

  const toggleChat = useCallback(() => setShowChat((v) => !v), []);
  const toggleSidebar = useCallback(() => setShowLeftSidebar((v) => !v), []);
  const toggleLayers = useCallback(() => setShowLayers((v) => !v), []);

  useKeyboardShortcuts({
    editorRef,
    isReady: status === "ready",
    onSceneChanged,
    activeTool,
    setActiveTool,
    handleToolChange: useCallback(() => {}, []),
    clipboardRef,
    onUndoRedoTick: handleUndoRedoTick,
    onToggleChat: toggleChat,
    onToggleSidebar: toggleSidebar,
    onToggleLayers: toggleLayers,
  });

  // Compute screen bounds for FloatingToolbar positioning.
  const floatingToolbarBounds = useMemo(() => {
    if (!selectedNode || status !== "ready" || !editorRef.current)
      return undefined;
    try {
      const cam = editorRef.current.get_camera();
      const pos = worldToScreen(selectedNode.x, selectedNode.y, cam);
      const dims = worldDimsToScreen(
        selectedNode.width,
        selectedNode.height,
        cam.zoom,
      );
      return { x: pos.x, y: pos.y, width: dims.w, height: dims.h };
    } catch {
      return undefined;
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedNode, status, refreshTick]);

  const canUndo =
    status === "ready" && (editorRef.current?.can_undo() ?? false);
  const canRedo =
    status === "ready" && (editorRef.current?.can_redo() ?? false);
  void undoRedoTick;

  const handleCollabConnect = useCallback(() => {
    const userId = crypto.randomUUID();
    const roomId = "default-room";
    collab.connect(roomId, userId, `User ${userId.slice(0, 4)}`);
  }, [collab]);

  const handleCollabDisconnect = useCallback(() => {
    collab.disconnect();
  }, [collab]);

  const editorContextValue = useMemo(
    () => ({ editorRef, onSceneChanged }),
    [editorRef, onSceneChanged],
  );

  return (
    <>
      <CollabContext.Provider value={collab}>
        <EditorProvider value={editorContextValue}>
          <div style={rootStyle}>
            <HeaderBar
              editorRef={editorRef}
              status={status}
              error={error}
              rightView={rightView}
              onRightViewChange={setRightView}
              canUndo={canUndo}
              canRedo={canRedo}
              onUndo={handleUndo}
              onRedo={handleRedo}
              showLayers={showLayers}
              onToggleLayers={toggleLayers}
              onShowMembers={useCallback(() => setShowMembers(true), [])}
              onSceneChanged={onSceneChanged}
              onClearAutoSave={clearSavedDocument}
              collabStatus={collab.status}
              collabParticipants={collab.participants}
              collabHasPendingOps={collab.hasPendingOps}
              onCollabConnect={handleCollabConnect}
              onCollabDisconnect={handleCollabDisconnect}
            />

            {/* Body */}
            <div style={bodyStyle}>
            {/* Left Sidebar: Elements / Templates / Uploads */}
            {status === "ready" && (
              <div
                style={{ position: "relative", display: "flex", flexShrink: 0 }}
              >
                <ErrorBoundary name="Sidebar">
                  <LeftSidebar
                    editorRef={editorRef}
                    onSceneChanged={onSceneChanged}
                    isOpen={showLeftSidebar}
                    onToggle={() => setShowLeftSidebar((v) => !v)}
                    activeWorkspaceId={activeWorkspace?.id}
                    width={sidebarResize.width}
                  />
                </ErrorBoundary>
                {showLeftSidebar && <div {...sidebarResize.handleProps} />}
              </div>
            )}

            {/* Chat Panel */}
            {status === "ready" && (
              <div
                style={{ position: "relative", display: "flex", flexShrink: 0 }}
              >
                <ErrorBoundary name="Chat">
                  <ChatPanel
                    editorRef={editorRef}
                    onSceneChanged={onSceneChanged}
                    isOpen={showChat}
                    onToggle={() => setShowChat((v) => !v)}
                    width={chatResize.width}
                  />
                </ErrorBoundary>
                {showChat && <div {...chatResize.handleProps} />}
              </div>
            )}

            {/* Main content area: Design (canvas + layers) or Code */}
            <div style={mainAreaStyle}>
              {rightView === "design" ? (
                <>
                  <div style={designRowStyle}>
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
                      {status === "ready" && (
                        <SnapGuides
                          editorRef={editorRef}
                          isDragging={isMoveDragging}
                        />
                      )}
                      {status === "ready" && collab.status === "connected" && (
                        <PresenceOverlay
                          editorRef={editorRef}
                          presences={presenceOverlayData}
                          activePageId={activePageId}
                        />
                      )}
                      {status === "ready" && (
                        <FloatingToolbar
                          node={selectedNode}
                          selectedIds={selectedIds}
                          editorRef={editorRef}
                          onSceneChanged={onSceneChanged}
                          isDragging={isMoveDragging}
                          isEditing={editingNodeId !== null}
                          screenBounds={floatingToolbarBounds}
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
                            if (node.kind !== NodeKind.Text) return null;
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
                        <div
                          style={creationOverlayStyle}
                          {...creationHandlers}
                        />
                      )}
                    </div>

                    {/* Layers panel (collapsible right drawer within design view) */}
                    {status === "ready" && showLayers && (
                      <div
                        style={{
                          ...layersPanelStyle,
                          width: layersResize.width,
                          position: "relative",
                        }}
                      >
                        <div {...layersResize.handleProps} />
                        <ErrorBoundary name="Layers">
                          <LayerPanel
                            editorRef={editorRef}
                            onSceneChanged={onSceneChanged}
                            refreshTick={refreshTick}
                          />
                        </ErrorBoundary>
                      </div>
                    )}
                  </div>
                  {/* end designRowStyle */}
                </>
              ) : (
                /* Code view */
                status === "ready" && (
                  <ErrorBoundary name="Code">
                    <CodePanel editorRef={editorRef} refreshKey={refreshTick} />
                  </ErrorBoundary>
                )
              )}

              {/* Page bar at the bottom of main area */}
              {status === "ready" && rightView === "design" && (
                <PageBar
                  editorRef={editorRef}
                  onSceneChanged={onSceneChanged}
                  refreshTick={refreshTick}
                />
              )}
            </div>
          </div>
          </div>
        </EditorProvider>
      </CollabContext.Provider>
      {showMembers && <MemberManager onClose={() => setShowMembers(false)} />}
      <ErrorToast />
    </>
  );
}

// --- Styles ---

const rootStyle: React.CSSProperties = {
  width: "100%",
  height: "100%",
  display: "flex",
  flexDirection: "column",
  background: colors.bg,
  color: colors.text,
  fontFamily:
    '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
};

const bodyStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  overflow: "hidden",
};

const mainAreaStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  flexDirection: "column",
  overflow: "hidden",
  minWidth: 0,
};

const designRowStyle: React.CSSProperties = {
  flex: 1,
  display: "flex",
  overflow: "hidden",
  minHeight: 0,
};

const canvasAreaStyle: React.CSSProperties = {
  flex: 1,
  position: "relative",
  minWidth: 0,
  minHeight: 0,
  background: colors.canvasBg,
};

const creationOverlayStyle: React.CSSProperties = {
  position: "absolute",
  inset: 0,
  cursor: "crosshair",
  zIndex: 10,
};

const layersPanelStyle: React.CSSProperties = {
  width: 180,
  minWidth: 120,
  maxWidth: 400,
  borderLeft: `1px solid ${colors.border}`,
  overflow: "hidden",
  display: "flex",
  flexDirection: "column",
  flexShrink: 0,
  background: colors.bg,
};
