# Selean

## Business Requirements

Selean is a design tool that combines the capabilities of Canva, Figma, and Adobe InDesign into a single platform with AI-native design workflows.

### Core Capabilities

1. **AI Chat-Prompted Design**: Users can start a design from scratch via chat prompts. The LLM interprets intent and generates scene graph operations (add nodes, set properties, arrange layouts) to produce a complete design.

2. **Import/Export Interoperability**: Full bidirectional support for:
   - PowerPoint (.pptx)
   - Adobe InDesign (.indd / .idml)
   - Figma (.fig / API)
   - Native Selean format

   Users bring existing work into Selean from any tool and export back to any format.

3. **AI-Assisted Editing**: A working designer spends more time editing than creating from scratch. Users can prompt modifications to an existing design ("make the heading larger", "change the color scheme to dark mode", "swap the layout to two columns") and the LLM applies targeted changes via the command system.

4. **Manual Direct Manipulation**: Full Canva/InDesign-style visual editing: drag to move/resize, property inspector panels, click-to-select, multi-select, alignment tools, layer management. The manual editing path must be as capable as the AI path.

5. **Hybrid Workflow**: AI and manual editing are interleaved freely. A user can prompt a layout, manually fine-tune positions, prompt a color change, manually adjust text, and undo/redo across both AI and manual actions seamlessly.

### Design Principles

- Import fidelity matters: a PPTX or Figma file should look as close to the original as possible on import.
- Export fidelity matters equally: round-tripping a file through Selean should not degrade it.
- The AI chat is not a separate mode. It operates on the same document, same undo stack, same scene graph.
- Performance targets: 60fps rendering, sub-100ms command execution, real-time collaborative editing (future).

# Engine Session Notes

## Current Phase: 11 Complete (End-to-End Editor Wiring)

### Phase 11 Summary (End-to-End Editor Wiring)

**What was added:**

- WASM rendering wired: `SeleanEditor` stores `surface` + `surface_config`, `render()` acquires texture and calls `render_frame()`, `resize()` reconfigures the surface
- `execute_tool_call(name, args_json)` on `EditorState` and `SeleanEditor`: routes LLM tool calls through `selean_llm::map_tool_call()` in Rust, handles read-only tools (`get_scene_summary`, `get_node`, `query_nodes`) and mutation tools, wraps multi-descriptor calls in command groups
- `query_nodes_json()` in queries.rs: filters nodes by optional name pattern (case-insensitive) and optional kind, returns `NodeSummary` array
- Client-driven chat tool loop in `ChatSidebar.tsx`: detects `stop_reason: "tool_use"`, executes tools via `execute_tool_call`, sends tool_result messages back to server, loops up to 10 iterations
- Complete property inspector: added editable fields for `stroke_width`, `corner_radius`, `blend_mode` (dropdown), `clip_mode` (dropdown), `font_family`, `font_weight`, `font_style` (dropdown), `text_align` (dropdown), `line_height`, `text_color` (color picker)
- Event-driven selection: `Canvas.tsx` parses interaction events from WASM pointer handlers and propagates via `onInteractionEvents` callback, `App.tsx` triggers `refresh()` on `SelectionChanged`/`Clicked`/`ClickedCanvas`
- Undo/redo refresh: button handlers and keyboard shortcuts call `refresh()` after undo/redo, `can_undo()`/`can_redo()` exposed via `#[wasm_bindgen]` for button disabled state
- `uuid` crate `js` feature added for WASM target
- 13 new tests (5 `query_nodes_json` + 8 `execute_tool_call`) bringing total to 828

### Completed Phases

- Phase 11: End-to-end editor wiring (WASM rendering, chat tool loop, tool call bridge, property inspector, event-driven selection, undo/redo refresh)

- Phase 1: Rect rendering (SDF rounded corners)
- Phase 2: SDF text rendering
- Phase 3: Image and vector rendering
- Phase 4: Scene graph, R-tree spatial index, dirty flags, 2D affine transforms, blend modes
- Phase 5: Clipping and masking (Scissor, Stencil, ShaderRect)
- Phase 6: Scroll containers
- Phase 7: Input handling
- Phase 8: Undo/redo system
- Phase 9: Serialization / Persistence
- Phase 10: Multi-page documents, PPTX import/export, extended text properties
- Phase 11: End-to-end editor wiring

### Phase 10 Summary (Multi-page Documents, PPTX, Extended Text Properties)

**What was added (in progress):**

- `FontStyle` enum (Normal, Italic) and `TextAlign` enum (Left, Center, Right, Justify) on `SceneNodeKind::Text`
- 6 new text fields: `font_family`, `font_weight`, `font_style`, `text_align`, `line_height`, `text_color`
- 7 new property commands + descriptors for the new text fields, plus `SetCornerRadiusCommand`
- `Document` struct (multi-page container) and `Page` struct (owns a SceneGraph)
- v2 persistence format with `PageData`, backward-compatible v1 loading
- `save_document()` / `load_document()` for full multi-page persistence
- `selean-pptx` crate: bidirectional PPTX import/export (EMU conversion, shape/text parsing, OOXML ZIP)
- 9 new LLM tool definitions (total 22 tools)
- Server routes: `POST /api/import/pptx`, `POST /api/export/pptx`
- TypeScript types: `PageInfo`, `TreeNode`, new editor methods

**What remains (not yet built):**

- InDesign (.indd/.idml) import/export
- Figma (.fig / API) import/export
- WASM-side implementations for page management methods declared in TypeScript types
- Full manual editing UI (drag handles, alignment tools, property inspector bindings)

### Phase 9 Summary (Serialization / Persistence)

**What was added:**

- `Serialize`/`Deserialize` derives on all scene types: `BlendMode`, `BoundingBox`, `Color`, `SceneNodeKind`, `ClipMode`, `SceneNode`
- Manual `Serialize`/`Deserialize` impl for `Transform2D` using `raw()`/`from_raw()`
- `#[serde(skip)]` on transient fields: `dirty` (defaults to `ALL`), `world_transform` (defaults to identity)
- `SceneGraph::from_document_state(nodes, roots)` constructor that rebuilds spatial index and recomputes world transforms
- `SceneGraph::nodes()` accessor for the internal node map
- `persistence/format.rs`: `DocumentFormat`, `SceneGraphData` with `from_graph()`, `validate()` (7 rules), `into_graph()`
- `persistence/mod.rs`: `PersistenceError` (UnsupportedVersion, InvalidScene, Json), `save()`, `save_pretty()`, `load()`
- `FORMAT_VERSION = 1`, timestamps via `std::time::SystemTime`
- 36 tests: 18 in format.rs (10 roundtrip + 8 validation rejection), 18 in mod.rs (10 save/load roundtrip + 4 field checks + 4 rejection)

**Design decisions:**

- JSON format chosen for human readability and debugging. Binary format can be added later behind the same `DocumentFormat` abstraction.
- Transient state (spatial index, world transforms, dirty flags, z-indices) is excluded from serialization and rebuilt on load.
- `validate()` checks 7 structural invariants before `into_graph()`: roots exist, children exist, parent consistency, root parent is None, no root in children, no cycles (DFS), no orphans.
- `from_document_state` sets `z_dirty` and `has_any_transform_dirty` to trigger full recomputation on first frame.
- No new dependencies (serde_json and thiserror were added in steps 1-4).

### Phase 8 Summary (Undo/Redo System)

**What was added:**

- `command/` module with 6 files: `mod.rs`, `traits.rs`, `property.rs`, `hierarchy.rs`, `batch.rs`, `history.rs`
- `Command` trait: `execute(&mut self, &mut SceneGraph) -> bool`, `undo(&mut self, &mut SceneGraph) -> bool`, `description(&self) -> &str`. Debug supertrait, object-safe.
- 11 macro-generated property commands via `define_property_command!`: `SetBoundsCommand`, `SetFillCommand`, `SetStrokeCommand`, `SetStrokeWidthCommand`, `SetOpacityCommand`, `SetVisibleCommand`, `SetNameCommand`, `SetBlendModeCommand`, `SetClipModeCommand`, `SetTransformCommand`
- 1 macro-generated scroll command via `define_scroll_offset_command!`: `SetScrollOffsetCommand`
- 4 manual kind-specific commands: `SetTextContentCommand`, `SetFontSizeCommand`, `SetPathDataCommand`, `SetAssetRefCommand`
- 5 hierarchy commands: `AddRootCommand`, `AddChildCommand`, `RemoveNodeCommand` (full subtree DFS snapshot), `ReparentCommand`, `ReorderChildrenCommand`
- `CommandGroup`: vec of `Box<dyn Command>`, executes forward, undoes in reverse, rollback on partial failure
- `CommandHistory`: undo/redo stacks, `execute()`, `undo()`, `redo()`, `begin_group()`/`end_group()`/`cancel_group()`, configurable max size (default 100)
- `SceneGraph::insert_root_at(node, index)`, `insert_child_at(parent_id, child, index)`, `reparent_to_root(node_id, index)` helper methods
- 45 tests across the command module (2 traits + 17 property + 14 hierarchy + 4 batch + 8 history)

**Design decisions:**

- Command pattern with per-mutation old-state capture. No `redo()` method: redo calls `execute()` again, which re-captures old state.
- Camera and selection excluded from undo (viewport concern and ephemeral UI state, not document state).
- `RemoveNodeCommand` uses DFS to capture `Vec<NodeSnapshot>` (node clone + parent_id + child_index) on execute. Undo re-inserts top-down via `insert_root_at`/`insert_child_at` to restore exact positions.
- `reparent_to_root()` added to SceneGraph (not in original plan) to support undoing a reparent that moved a root node into a subtree.
- `CommandGroup` rolls back already-executed commands on partial execute failure.
- `CommandHistory` blocks undo/redo during active group. New execute clears redo stack (no branching history).
- No new dependencies added.

### Phase 7 Summary (Input Handling)

**What was added:**

- `input/` module with 4 files: `mod.rs`, `event.rs`, `state.rs`, `handler.rs`
- `PointerButton` enum (Left, Right, Middle) and `Modifiers` struct (shift, ctrl, alt, meta)
- `InputEvent` enum: `PointerMove`, `PointerDown`, `PointerUp`, `ScrollDelta` (platform-agnostic, screen-space coordinates)
- `InteractionEvent` enum: `HoverChanged`, `Clicked`, `ClickedCanvas`, `SelectionChanged`, `DragStarted`, `DragMoved`, `DragEnded`, `ScrollApplied`, `CameraPanned`, `CameraZoomed`
- `SelectionSet` with `select_one()`, `toggle()`, `clear()`, `contains()`, `ids()`, `is_empty()`, `len()`
- `DragPhase` (tracks node, start positions, threshold state), `CameraPanPhase` (tracks start screen/pan)
- `InteractionState`: `hover_target`, `selection`, `drag`, `camera_pan`, `drag_threshold` (default 4.0px)
- `InputHandler` with `new()`, `with_drag_threshold()`, `handle_event()`, `state()`, `state_mut()`
- `Camera::pan_by(dx, dy)` for delta-based viewport translation
- `Camera::zoom_at(factor, screen_x, screen_y)` for focus-aware zoom
- `SceneGraph::parent(id)` for ancestor walking in scroll routing
- 38 tests across the input module (6 event + 12 state + 20 handler)

**Design decisions:**

- Platform-agnostic boundary: `InputEvent` types carry no platform API references. Consumers translate from winit/SDL/web.
- Stateful handler: `InputHandler` owns `InteractionState` to track gestures across event frames. Purely reactive, no event queuing.
- Borrowing pattern: `handle_event()` requires `&mut SceneGraph` + `&mut Camera` to prevent stale references.
- Priority ordering: camera pan (middle-button) > active drag > hover detection. Scroll routing walks ancestors via `parent()`, consuming delta at each scrollable container, then falls back to camera pan.
- Drag gesture recognition: threshold-based (configurable). Below threshold = click, above = drag. Tracks screen-space for threshold check, world-space for delta computation.
- Selection: Shift-click toggles individual nodes. Plain click replaces selection. Canvas click deselects all (only emits `SelectionChanged` if selection was non-empty).
- No new dependencies added.

### Phase 6 Summary (Scroll Containers)

**What was added:**

- `scroll_offset: [f32; 2]` field on `SceneNode` (default `[0.0, 0.0]`)
- `BoundingBox::union()` for computing content bounds
- `set_scroll_offset()`, `scroll_offset()` accessors on `SceneGraph`
- `compute_content_bounds()`, `max_scroll()`, `set_scroll_offset_clamped()` on `SceneGraph`
- Scroll translation injection in `recompute_world_transform_recursive`: children see `parent_world * translation(-offset_x, -offset_y)`
- Implicit scissor clipping: scroll containers with `ClipMode::None` get an automatic `Scissor` clip in DFS and hit testing
- `ancestor_clip_chain()` and `point_passes_clip()` updated for implicit scroll clips
- `dfs_visit()` computes effective clip mode: explicit clip takes precedence, scroll implies Scissor, otherwise None
- `scroll_fraction` config in benchmarks, Group 17 `bench_scroll_containers` with 4 benchmarks across 3 tiers

**Design decisions:**

- Scroll offset is a transform injection, not a separate coordinate space. Composes naturally with existing transforms and nested scroll containers.
- No new dirty flag. Scroll offset changes reuse `TRANSFORM` since they require world transform recomputation on all descendants.
- Content bounds computed on demand via `compute_content_bounds()`. No per-frame overhead.
- No event/input handling. The caller sets scroll offset directly.

### Test Count

- 828 tests passing (16 common + 650 engine + 46 llm + 58 pptx + 16 server + 42 wasm)
- 17 benchmark groups

### Key Architecture Notes

- Rendering uses hierarchical DFS traversal (replaced flat draw order in Phase 5)
- Three-phase pipeline: Phase 1 (DFS: atlas population, render order), Phase 2 (UV finalization, clip patching, DrawList construction), Phase 3 (render pass execution)
- `DrawList::merge_adjacent()` coalesces contiguous same-type draw commands for minimal draw calls
- Stencil nesting uses IncrementClamp/DecrementClamp (max 255 depth)
- ShaderRect uses per-instance `clip_rect` with fragment discard
- Scissor uses GPU `set_scissor_rect()` (zero overhead, no rounded corners)
- Scroll containers inject translation into child world transforms and auto-clip via implicit Scissor
- `render_frame()` is the single entry point: takes `&mut SceneGraph` + `&TextureView`

### File Layout

```
crates/
  selean-common/           # Shared types and errors
    src/
      error/mod.rs         # SeleanError, EngineError
      types/id.rs          # NodeId, TokenId, ProjectId, etc.
  selean-engine/           # Core rendering engine
    src/
      scene/               # Scene graph
        node.rs            # SceneNode, SceneNodeKind, BoundingBox, Color, BlendMode
        store.rs           # SceneGraph (central store, transforms, hit testing, scroll)
        transform.rs       # Transform2D (3x2 affine matrix)
        dirty.rs           # DirtyFlags (8-bit bitmask)
        clip.rs            # ClipMode, ClipRect
      renderer/            # GPU rendering
        pipeline.rs        # Renderer (top-level orchestrator, DFS traversal)
        rect_pipeline.rs   # RectPipeline, RectInstance, RectBatch
        textured_quad.rs   # TexturedQuadPipeline, TexturedQuadInstance, TexturedQuadBatch
        shared.rs          # SharedPipelineResources, PersistentInstanceBuffer
        clip_stack.rs      # ClipStack, ResolvedClipState
        draw_list.rs       # DrawCommand, DrawList
        camera.rs          # Camera, CameraUniform
        texture_atlas.rs   # TextureAtlas<CHANNELS>
        gpu.rs             # GpuContext, GpuContextDescriptor
        quad.rs            # Unit quad vertices/indices
        shaders/           # WGSL shaders (rect, text, textured_quad)
      text/                # Text subsystem
        mod.rs             # TextSystem (coordinator)
        font.rs            # FontData (Inter Regular embedded)
        shaper.rs          # shape_text (rustybuzz)
        layout.rs          # layout_text, PositionedGlyph
        sdf.rs             # generate_glyph_sdf (Felzenszwalb EDT)
        cache.rs           # GlyphCache
        atlas.rs           # GlyphAtlas (TextureAtlas<1>)
        packer.rs          # ShelfPacker
        pipeline.rs        # TextPipeline, GlyphInstance, TextBatch
      image/               # Image subsystem
        mod.rs             # ImageSystem (coordinator)
        loader.rs          # decode_image, decode_image_resized
        cache.rs           # ImageCache
      vector/              # Vector subsystem
        mod.rs             # VectorSystem (coordinator)
        parser.rs          # parse_path_data (SVG path commands)
        rasterizer.rs      # rasterize_path (tiny-skia)
        cache.rs           # VectorCache
      input/               # Input handling
        mod.rs             # Module re-exports
        event.rs           # InputEvent, InteractionEvent, PointerButton, Modifiers
        state.rs           # SelectionSet, DragPhase, CameraPanPhase, InteractionState
        handler.rs         # InputHandler (event processing, hit testing, scroll routing)
      command/             # Undo/redo system
        mod.rs             # Module declarations and re-exports
        traits.rs          # Command trait
        property.rs        # 15 property commands (11 macro + 4 manual)
        hierarchy.rs       # 5 hierarchy commands (add, remove, reparent, reorder)
        batch.rs           # CommandGroup (multi-command undo unit)
        history.rs         # CommandHistory (undo/redo stacks, grouping)
      persistence/         # Document save/load
        mod.rs             # PersistenceError, save(), save_pretty(), load(), save_document(), load_document()
        format.rs          # DocumentFormat, SceneGraphData, PageData, validate(), FORMAT_VERSION=2
        document.rs        # Document (multi-page container, active page tracking)
        page.rs            # Page (id, name, dimensions, owns SceneGraph)
      spatial/             # Spatial indexing
        index.rs           # SpatialIndex (R-tree via rstar)
    benches/
      bench_utils.rs       # SceneConfig, generate_scene
      engine_benchmarks.rs # 17 Criterion benchmark groups
  selean-pptx/             # PPTX import/export
    src/
      lib.rs               # PptxError, import_pptx(), export_pptx()
      coord.rs             # EMU/pixel conversion, OOXML color parsing, font size conversion
      import/
        mod.rs             # ZIP reading, slide parsing, Page creation
        shape.rs           # <p:sp> to SceneNode conversion
        text.rs            # <a:txBody> run parsing (font, color, alignment)
      export/
        mod.rs             # OOXML ZIP building (Content_Types, rels, presentation, slides)
        shape.rs           # SceneNode to <p:sp> XML conversion
```
