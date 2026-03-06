# Selean

A GPU-accelerated design platform built in Rust with a browser-based editor. Combines a WebGPU rendering engine, an LLM chat assistant (Claude), real-time collaborative editing, and a React frontend into a unified design tool where manual edits, AI-driven modifications, and file imports all flow through the same mutation pipeline.

## Architecture

```
+---------------------------+
|   React Frontend (Vite)   |
|                           |
|  Canvas  Chat  Inspector  |
|  (WebGPU) (SSE)  (Props)  |
+-----------+---------------+
            |
            | wasm-bindgen FFI
            v
+-----------+---------------+      +------------------+
|       selean-wasm         |      |   selean-server   |
|                           |      |     (Axum)        |
|  SeleanEditor (WASM)      |      |                   |
|  EditorState (native)     |      |  POST /api/chat   |
|  Event forwarding         |      |  GET  /api/tools  |
+-----------+---------------+      |  POST /api/import |
            |                      |  POST /api/export |
            |                      |  WS   /ws/collab  |
            | CommandDescriptor    +--------+----------+
            v       (JSON)                  |
+-----------+---------------+      +--------v----------+
|       selean-engine       |      |    selean-llm      |
|                           |      |                    |
|  Scene Graph              |      |  Tool definitions  |
|  Renderer (3-phase)       |      |  map_tool_call()   |
|  Input Handler            |      |  is_read_only()    |
|  Command History          |      +--------------------+
|  CommandDescriptor        |
|  Persistence              |      +--------------------+
+---------------------------+      |   selean-collab    |
            |                      |                    |
            v                      |  OpLog, inverse    |
    +-------+--------+            |  ops, protocol     |
    |      wgpu      |            +--------------------+
    | (Vulkan/Metal/ |
    |  DX12/WebGPU)  |
    +----------------+
```

### Mutation Pipeline

All mutation sources converge on a single wire format:

```
[React UI edits]  --\
[LLM tool calls]  ----> CommandDescriptor (JSON) --> Box<dyn Command> --> CommandHistory --> SceneGraph
[File imports]    --/
[Collab ops]      --/
```

`CommandDescriptor` is a `#[serde(tag = "type")]` enum in `selean-engine` with 29 variants mirroring every `Command` type. Adding a new command requires: (1) engine `Command` impl, (2) descriptor variant. Nothing else changes in LLM/import/export/collab.

### Render Pipeline

The renderer processes one frame through three sequential phases:

1. **DFS Traversal**: Walks the scene graph depth-first, populating texture atlases (glyphs, images, vector rasterizations), computing render order, and building the clip stack for nested clipping regions. Uses `SmallVec<[NodeId; 16]>` for stack-allocated child iteration to reduce heap allocations.

2. **Draw List Construction**: Finalizes UV coordinates from atlas packing, patches clip state onto draw commands, and coalesces adjacent same-type commands via `merge_adjacent()` to minimize draw calls.

3. **Render Pass Execution**: Submits batched wgpu commands. Three specialized pipelines handle different node types: `RectPipeline` for SDF rounded rectangles, `TextPipeline` for SDF glyph rendering, and `TexturedQuadPipeline` for images and rasterized vectors. Non-native blend modes (Multiply, Screen, Overlay, etc.) use shader-based GPU pipelines with intermediate render targets per the W3C Compositing spec. Drop shadow and blur effects use a Dual Kawase blur pipeline with ping-pong half-resolution textures.

### Scene Graph

The `SceneGraph` is the central data structure. It owns all `SceneNode` instances in a `HashMap<NodeId, SceneNode>`, maintains parent/child relationships, and integrates:

- **Dirty Flags**: An 8-bit bitmask per node (geometry, style, children, layout, text, effects, transform, clip) that gates incremental updates. Only nodes with non-zero flags are processed during rendering. Dirty propagation uses an explicit stack instead of recursion for deep hierarchies.
- **World Transforms**: 3x2 affine matrices computed lazily by walking the transform hierarchy. Scroll offsets are injected as translation matrices into child transforms.
- **Spatial Index**: An R-tree (via `rstar`) over world-space AABBs for O(log n) viewport culling and hit testing.
- **Clipping**: Three modes (Scissor, Stencil, ShaderRect) with hierarchical DFS traversal. Scroll containers auto-clip children via implicit Scissor.

### Input Handling

The input system translates platform-agnostic `InputEvent` types into high-level `InteractionEvent` types (hover, click, drag, selection, scroll, camera pan/zoom). It is stateful, tracking gestures across frames with threshold-based drag recognition. Scroll events route through ancestor scroll containers before falling back to camera panning.

### Command System

An undo/redo system built on the Command pattern. Each mutation captures its previous state on execute, enabling undo without a separate `redo()` method (redo simply re-executes). Supports batched operations via `CommandGroup` with automatic rollback on partial failure. 29 command descriptor variants covering property, hierarchy, rotation, grouping, z-order, effects, and kind-specific mutations. Alignment and distribution operations group multiple `SetBounds` commands for single-step undo.

### LLM Integration

`selean-llm` defines 41 Claude API tools (3 read-only queries, 4 page management, 1 alignment, 7 z-order/group, 3 effects, 23 write mutations). `selean-server` proxies chat to the Claude API with tool definitions and scene context, streaming responses via SSE. The frontend executes tool calls through the same `CommandDescriptor` pipeline, so LLM mutations participate in undo/redo.

### Real-Time Collaboration

`selean-collab` provides the protocol and operation log for collaborative editing. The server manages rooms with server-authoritative operation sequencing. Each room maintains a canonical `Document`, an `OpLog`, and per-user undo/redo stacks. The frontend uses a WebSocket client with exponential backoff reconnection and an operation buffer for offline resilience. Presence (cursors, selections) is broadcast to all room participants.

### Persistence

Documents serialize to JSON via `serde`. Transient state (spatial index, world transforms, dirty flags) is excluded and rebuilt on load. A 7-rule structural validator checks integrity before reconstruction. Auto-save writes to localStorage every 5 seconds when dirty. Import/export is supported for PowerPoint (.pptx), InDesign IDML (.idml), InDesign (.indd via server bridge), and Figma (REST API import, interchange export).

## Features

### Direct Manipulation

- Click-to-select with selection overlay and 8 resize handles
- Drag-to-move nodes on canvas with command grouping for single undo
- Inline text editing via double-click on Text nodes
- Arrow key nudge (1px, Shift+Arrow for 10px)
- Right-click context menu (Copy, Paste, Duplicate, Delete, z-order, Group, Ungroup)

### Canvas Controls

- Zoom in/out (Cmd+=, Cmd+-)
- Fit to all (Cmd+0)
- Zoom to 100% (Cmd+1)
- Middle-mouse camera pan
- Scroll-wheel zoom

### Layer Management

- Layer panel with visibility toggle and inline rename
- Z-order controls: Bring to Front/Back, Bring Forward/Backward (Cmd+]/[, Cmd+Shift+]/[)
- Group/Ungroup (Cmd+G, Cmd+Shift+G)

### Property Inspector

- Position, size, fill, stroke, opacity, corner radius, blend mode, clip mode
- Rotation field with degree input
- Gradient fills (linear and radial, up to 4 color stops)
- Text properties: content, font family, font weight, font style, text align, line height, text color
- Effects: drop shadow (color, offset, blur radius) and blur (radius)
- Image section with asset ref display
- Vector section with path data display

### File Operations

- New, Save (.selean), Open (.selean)
- Import/Export PowerPoint (.pptx)
- Import/Export InDesign (.idml)
- Import InDesign (.indd, via server bridge)
- Import Figma (REST API, by URL or file key)
- Export Figma (interchange format for Figma plugin)
- Auto-save to localStorage (5-second interval)

### AI Chat

- Claude-powered chat sidebar with streaming responses
- Scene-aware tool execution (41 tools)
- All AI mutations flow through undo/redo

### Real-Time Collaboration

- WebSocket-based collaborative editing with server-authoritative sequencing
- Presence overlay showing remote cursors and selections
- Offline operation buffering with automatic sync on reconnect
- Per-user undo/redo stacks

### Rendering

- SDF rounded rectangles with configurable corner radii
- SDF text rendering with variable font weight (Inter) and multi-font support via `FontRegistry`
- Gradient fills (linear and radial) with up to 4 color stops
- Image and SVG vector rendering
- 11 non-native blend modes via GPU shader pipelines
- Drop shadow and Gaussian blur effects (Dual Kawase pipeline)
- Hierarchical clipping (Scissor, Stencil, ShaderRect)
- Scroll containers with content clipping

### Error Handling

- ErrorBoundary components around major UI sections with retry
- Global error toast notification system with auto-dismiss
- Body size limits on server routes (1 MiB JSON, 50 MiB uploads)
- Graceful mutex recovery in collaborative editing (no panics on poisoned locks)

## Workspace Structure

```
crates/
  selean-common/       Shared types (NodeId, error types)
  selean-engine/       Core rendering engine
    src/
      scene/           Scene graph, transforms, dirty flags, clipping, alignment
      renderer/        wgpu pipelines, draw list, camera, texture atlas, blend modes, blur
      text/            HarfBuzz shaping, SDF glyph rendering, atlas packing, FontRegistry
      image/           Image decoding and caching
      vector/          SVG path parsing, tiny-skia rasterization
      input/           Platform-agnostic input and interaction handling
      command/         Undo/redo history, 29 CommandDescriptor variants
      persistence/     JSON document save/load with validation
      spatial/         R-tree spatial index
    benches/           Criterion benchmarks (17 groups)
    tests/             Integration tests (commands + persistence roundtrips)
  selean-wasm/         WASM binding layer (SeleanEditor, EditorState, queries)
  selean-llm/          LLM tool definitions (41 tools) and execution mapping
  selean-pptx/         PowerPoint (.pptx) import/export
  selean-idml/         InDesign (.idml) import/export
  selean-figma/        Figma REST API import, interchange export
  selean-collab/       Collaborative editing protocol, OpLog, operation inverses
  selean-server/       Axum HTTP server (Claude API proxy, SSE streaming, file routes, WS collab)

web/
  selean-app/          React + TypeScript frontend (Vite)
    src/
      components/      Canvas, ChatSidebar, PropertyInspector, FileMenu, LayerPanel,
                       AlignmentBar, ContextMenu, InlineTextEditor, SelectionOverlay,
                       Toolbar, PageBar, ErrorBoundary, ErrorToast, CollabBar,
                       PresenceOverlay
      hooks/           useSeleanEditor, useSelection, useCreationTool, useResizeDrag,
                       useMoveDrag, useAutoSave, useKeyboardShortcuts, useCollabSession,
                       useFileOperations, useFontLoader
      collab/          WsClient, OperationBuffer, CollabContext
      wasm/            TypeScript type stubs for WASM bindings
      theme.ts         Shared design tokens (colors, font sizes)

figma-plugin/          Figma plugin for importing Selean interchange format
```

## Building

```sh
cargo build
```

For the frontend:

```sh
cd web/selean-app
npm install
npm run dev
```

## Testing

```sh
cargo test --workspace
```

1531 Rust tests across 9 crates, covering unit, integration, and property-based testing.

Frontend tests (351 tests via Vitest):

```sh
cd web/selean-app
npx vitest run
```

Frontend type checking:

```sh
cd web/selean-app
npx tsc --noEmit
```

## Benchmarks

```sh
cargo bench -p selean-engine
```

17 Criterion benchmark groups covering scene graph operations, spatial indexing, rendering pipeline phases, and scroll container performance across configurable scene sizes.

## Requirements

- Rust 1.85+
- Node.js 18+ (for the frontend)
- GPU with Vulkan, Metal, DX12, or WebGPU support (via `wgpu`)
- Chrome 113+ or Edge 113+ for the browser editor (WebGPU required)

## License

MIT OR Apache-2.0
