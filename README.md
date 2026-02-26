# Selean

A GPU-accelerated 2D rendering engine for design tools, written in Rust.

Selean provides the core rendering and scene management layer for a design platform. It uses WebGPU (via `wgpu`) for hardware-accelerated rendering with SDF-based text and shape rendering, a scene graph with dirty flag tracking for incremental updates, and an R-tree spatial index for viewport culling and hit testing.

## Architecture

```
                        +--------------------+
                        |   Host Application |
                        |  (winit/SDL/web)   |
                        +---------+----------+
                                  |
                    InputEvent    |    save()/load()
                   (platform     |    (JSON persistence)
                    agnostic)    |
                                 v
              +------------------+------------------+
              |            selean-engine             |
              |                                     |
              |  +-----------+    +--------------+  |
              |  |   Input   |--->|   Command    |  |
              |  |  Handler  |    |   History    |  |
              |  +-----------+    |  (undo/redo) |  |
              |       |           +--------------+  |
              |       v                |             |
              |  +---------+    +------v---------+  |
              |  | Camera  |    |  Scene Graph   |  |
              |  | (pan,   |    |  +----------+  |  |
              |  |  zoom)  |    |  | SceneNode|  |  |
              |  +---------+    |  | DirtyFlags|  |  |
              |       |         |  | Transform |  |  |
              |       |         |  +----------+  |  |
              |       |         |       |        |  |
              |       |         | +------------+ |  |
              |       |         | |Spatial Index| |  |
              |       |         | |  (R-tree)   | |  |
              |       |         | +------------+ |  |
              |       |         +----------------+  |
              |       |                |             |
              |       v                v             |
              |  +------------------------------+   |
              |  |      Renderer (3-phase)       |   |
              |  |                               |   |
              |  |  Phase 1: DFS traversal       |   |
              |  |    atlas population           |   |
              |  |    render order                |   |
              |  |    clip stack                  |   |
              |  |                               |   |
              |  |  Phase 2: Draw list           |   |
              |  |    UV finalization            |   |
              |  |    clip patching              |   |
              |  |    merge_adjacent()           |   |
              |  |                               |   |
              |  |  Phase 3: Render pass         |   |
              |  |    wgpu command submission    |   |
              |  +------------------------------+   |
              |       |          |          |        |
              |  +----v---+ +---v----+ +---v-----+  |
              |  |  Rect  | |  Text  | |Textured |  |
              |  |Pipeline| |Pipeline| |  Quad   |  |
              |  | (SDF)  | | (SDF)  | |Pipeline |  |
              |  +--------+ +--------+ +---------+  |
              |                                     |
              +------------------+------------------+
                                 |
                                 v
                        +--------+--------+
                        |      wgpu       |
                        | (Vulkan/Metal/  |
                        |  DX12/WebGPU)   |
                        +-----------------+
```

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

An undo/redo system built on the Command pattern. Each mutation captures its previous state on execute, enabling undo without a separate `redo()` method (redo simply re-executes). Supports batched operations via `CommandGroup` with automatic rollback on partial failure.

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
      command/         Undo/redo command history
      persistence/     JSON document save/load with validation
      spatial/         R-tree spatial index
    benches/           Criterion benchmarks (17 groups)
```

## Building

```sh
cargo build
```

## Testing

```sh
cargo test
```

571 tests across both crates, covering unit, integration, and property-based testing (via `proptest`).

## Benchmarks

```sh
cargo bench -p selean-engine
```

17 Criterion benchmark groups covering scene graph operations, spatial indexing, rendering pipeline phases, and scroll container performance across configurable scene sizes.

## Requirements

- Rust 1.85+
- GPU with Vulkan, Metal, DX12, or WebGPU support (via `wgpu`)

## License

MIT OR Apache-2.0
