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
+-----------+---------------+      |  GET  /api/health |
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

`CommandDescriptor` is a `#[serde(tag = "type")]` enum in `selean-engine` mirroring every `Command` type. Adding a new command requires: (1) engine `Command` impl, (2) descriptor variant. Nothing else changes in LLM/import/export.

### Render Pipeline

The renderer processes one frame through three sequential phases:

1. **DFS Traversal**: Walks the scene graph depth-first, populating texture atlases (glyphs, images, vector rasterizations), computing render order, and building the clip stack for nested clipping regions.

2. **Draw List Construction**: Finalizes UV coordinates from atlas packing, patches clip state onto draw commands, and coalesces adjacent same-type commands via `merge_adjacent()` to minimize draw calls.

3. **Render Pass Execution**: Submits batched wgpu commands. Three specialized pipelines handle different node types: `RectPipeline` for SDF rounded rectangles, `TextPipeline` for SDF glyph rendering, and `TexturedQuadPipeline` for images and rasterized vectors.

### Scene Graph

The `SceneGraph` is the central data structure. It owns all `SceneNode` instances in a `HashMap<NodeId, SceneNode>`, maintains parent/child relationships, and integrates:

- **Dirty Flags**: An 8-bit bitmask per node (geometry, style, children, layout, text, effects, transform, clip) that gates incremental updates. Only nodes with non-zero flags are processed during rendering.
- **World Transforms**: 3x2 affine matrices computed lazily by walking the transform hierarchy. Scroll offsets are injected as translation matrices into child transforms.
- **Spatial Index**: An R-tree (via `rstar`) over world-space AABBs for O(log n) viewport culling and hit testing.
- **Clipping**: Three modes (Scissor, Stencil, ShaderRect) with hierarchical DFS traversal. Scroll containers auto-clip children via implicit Scissor.

### Input Handling

The input system translates platform-agnostic `InputEvent` types into high-level `InteractionEvent` types (hover, click, drag, selection, scroll, camera pan/zoom). It is stateful, tracking gestures across frames with threshold-based drag recognition. Scroll events route through ancestor scroll containers before falling back to camera panning.

### Command System

An undo/redo system built on the Command pattern. Each mutation captures its previous state on execute, enabling undo without a separate `redo()` method (redo simply re-executes). Supports batched operations via `CommandGroup` with automatic rollback on partial failure. 20 command types covering property, hierarchy, and kind-specific mutations.

### LLM Integration

`selean-llm` defines 13 Claude API tools (3 read-only queries, 10 write mutations). `selean-server` proxies chat to the Claude API with tool definitions and scene context, streaming responses via SSE. The frontend executes tool calls through the same `CommandDescriptor` pipeline, so LLM mutations participate in undo/redo.

### Persistence

Documents serialize to JSON via `serde`. Transient state (spatial index, world transforms, dirty flags) is excluded and rebuilt on load. A 7-rule structural validator checks integrity before reconstruction.

## Workspace Structure

```
crates/
  selean-common/       Shared types (NodeId, error types)
  selean-engine/       Core rendering engine
    src/
      scene/           Scene graph, transforms, dirty flags, clipping
      renderer/        wgpu pipelines, draw list, camera, texture atlas
      text/            HarfBuzz shaping, SDF glyph rendering, atlas packing
      image/           Image decoding and caching
      vector/          SVG path parsing, tiny-skia rasterization
      input/           Platform-agnostic input and interaction handling
      command/         Undo/redo history, CommandDescriptor wire format
      persistence/     JSON document save/load with validation
      spatial/         R-tree spatial index
    benches/           Criterion benchmarks (17 groups)
  selean-wasm/         WASM binding layer (SeleanEditor, EditorState, queries)
  selean-llm/          LLM tool definitions and execution mapping
  selean-server/       Axum HTTP server (Claude API proxy, SSE streaming)

web/
  selean-app/          React + TypeScript frontend (Vite)
    src/
      components/      Canvas, ChatSidebar, PropertyInspector
      hooks/           useSeleanEditor, useSelection
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

702 tests across 5 crates, covering unit, integration, and property-based testing (via `proptest`).

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
