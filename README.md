# Selean

A GPU-accelerated design platform built in Rust with a browser-based editor. Combines a WebGPU rendering engine, an LLM chat assistant (Claude), and a React frontend into a unified design tool where manual edits, AI-driven modifications, and file imports all flow through the same mutation pipeline.

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
            |                      +--------+----------+
            | CommandDescriptor (JSON)       |
            v                                | Claude API
+-----------+---------------+      +--------v----------+
|       selean-engine       |      |    selean-llm      |
|                           |      |                    |
|  Scene Graph              |      |  Tool definitions  |
|  Renderer (3-phase)       |      |  map_tool_call()   |
|  Input Handler            |      |  is_read_only()    |
|  Command History          |      +--------------------+
|  CommandDescriptor        |
|  Persistence              |
+---------------------------+
            |
            v
    +-------+--------+
    |      wgpu      |
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
```

`CommandDescriptor` is a `#[serde(tag = "type")]` enum in `selean-engine` with 28 variants mirroring every `Command` type. Adding a new command requires: (1) engine `Command` impl, (2) descriptor variant. Nothing else changes in LLM/import/export.

### Render Pipeline

The renderer processes one frame through three sequential phases:

1. **DFS Traversal**: Walks the scene graph depth-first, populating texture atlases (glyphs, images, vector rasterizations), computing render order, and building the clip stack for nested clipping regions.

2. **Draw List Construction**: Finalizes UV coordinates from atlas packing, patches clip state onto draw commands, and coalesces adjacent same-type commands via `merge_adjacent()` to minimize draw calls.

3. **Render Pass Execution**: Submits batched wgpu commands. Three specialized pipelines handle different node types: `RectPipeline` for SDF rounded rectangles, `TextPipeline` for SDF glyph rendering, and `TexturedQuadPipeline` for images and rasterized vectors. Non-native blend modes (Multiply, Screen, Overlay, etc.) use shader-based GPU pipelines with intermediate render targets per the W3C Compositing spec.

### Scene Graph

The `SceneGraph` is the central data structure. It owns all `SceneNode` instances in a `HashMap<NodeId, SceneNode>`, maintains parent/child relationships, and integrates:

- **Dirty Flags**: An 8-bit bitmask per node (geometry, style, children, layout, text, effects, transform, clip) that gates incremental updates. Only nodes with non-zero flags are processed during rendering.
- **World Transforms**: 3x2 affine matrices computed lazily by walking the transform hierarchy. Scroll offsets are injected as translation matrices into child transforms.
- **Spatial Index**: An R-tree (via `rstar`) over world-space AABBs for O(log n) viewport culling and hit testing.
- **Clipping**: Three modes (Scissor, Stencil, ShaderRect) with hierarchical DFS traversal. Scroll containers auto-clip children via implicit Scissor.

### Input Handling

The input system translates platform-agnostic `InputEvent` types into high-level `InteractionEvent` types (hover, click, drag, selection, scroll, camera pan/zoom). It is stateful, tracking gestures across frames with threshold-based drag recognition. Scroll events route through ancestor scroll containers before falling back to camera panning.

### Command System

An undo/redo system built on the Command pattern. Each mutation captures its previous state on execute, enabling undo without a separate `redo()` method (redo simply re-executes). Supports batched operations via `CommandGroup` with automatic rollback on partial failure. 28 command descriptor variants covering property, hierarchy, rotation, grouping, z-order, and kind-specific mutations. Alignment and distribution operations group multiple `SetBounds` commands for single-step undo.

### LLM Integration

`selean-llm` defines 36 Claude API tools (3 read-only queries, 4 page management, 1 alignment, 1 rotation, 3 grouping, 4 z-order, 20 write mutations). `selean-server` proxies chat to the Claude API with tool definitions and scene context, streaming responses via SSE. The frontend executes tool calls through the same `CommandDescriptor` pipeline, so LLM mutations participate in undo/redo.

### Persistence

Documents serialize to JSON via `serde`. Transient state (spatial index, world transforms, dirty flags) is excluded and rebuilt on load. A 7-rule structural validator checks integrity before reconstruction. Auto-save writes to localStorage every 5 seconds when dirty. Import/export is supported for PowerPoint (.pptx), InDesign IDML (.idml), and Figma (REST API).

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
- Text properties: content, font family, font weight, font style, text align, line height, text color
- Image section with asset ref display
- Vector section with path data display

### File Operations

- New, Save (.selean), Open (.selean)
- Import/Export PowerPoint (.pptx)
- Import/Export InDesign (.idml)
- Import Figma (REST API, by URL or file key)
- Auto-save to localStorage (5-second interval)

### AI Chat

- Claude-powered chat sidebar with streaming responses
- Scene-aware tool execution (36 tools)
- All AI mutations flow through undo/redo

### Rendering

- SDF rounded rectangles with configurable corner radii
- SDF text rendering with variable font weight (Inter)
- Image and SVG vector rendering
- 11 non-native blend modes via GPU shader pipelines
- Hierarchical clipping (Scissor, Stencil, ShaderRect)
- Scroll containers with content clipping

## Workspace Structure

```
crates/
  selean-common/       Shared types (NodeId, error types)
  selean-engine/       Core rendering engine
    src/
      scene/           Scene graph, transforms, dirty flags, clipping, alignment
      renderer/        wgpu pipelines, draw list, camera, texture atlas, blend modes
      text/            HarfBuzz shaping, SDF glyph rendering, atlas packing
      image/           Image decoding and caching
      vector/          SVG path parsing, tiny-skia rasterization
      input/           Platform-agnostic input and interaction handling
      command/         Undo/redo history, 28 CommandDescriptor variants
      persistence/     JSON document save/load with validation
      spatial/         R-tree spatial index
    benches/           Criterion benchmarks (17 groups)
  selean-wasm/         WASM binding layer (SeleanEditor, EditorState, queries)
  selean-llm/          LLM tool definitions (36 tools) and execution mapping
  selean-pptx/         PowerPoint (.pptx) import/export
  selean-idml/         InDesign (.idml) import/export
  selean-figma/        Figma REST API import
  selean-server/       Axum HTTP server (Claude API proxy, SSE streaming, file import/export routes)

web/
  selean-app/          React + TypeScript frontend (Vite)
    src/
      components/      Canvas, ChatSidebar, PropertyInspector, FileMenu, LayerPanel,
                       AlignmentBar, ContextMenu, InlineTextEditor, SelectionOverlay,
                       Toolbar, PageBar, ErrorBoundary
      hooks/           useSeleanEditor, useSelection, useCreationTool, useResizeDrag,
                       useMoveDrag, useAutoSave
      wasm/            TypeScript type stubs for WASM bindings
      theme.ts         Shared design tokens (colors, font sizes)
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
cargo test
```

1117 Rust tests across 8 crates, covering unit, integration, and property-based testing (via `proptest`).

Frontend tests (164 tests via Vitest):

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
