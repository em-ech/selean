# Selean Engine Session Notes

## Current Phase: 8 Complete (Undo/Redo System)

### Completed Phases

- Phase 1: Rect rendering (SDF rounded corners)
- Phase 2: SDF text rendering
- Phase 3: Image and vector rendering
- Phase 4: Scene graph, R-tree spatial index, dirty flags, 2D affine transforms, blend modes
- Phase 5: Clipping and masking (Scissor, Stencil, ShaderRect)
- Phase 6: Scroll containers
- Phase 7: Input handling
- Phase 8: Undo/redo system

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

- 535 tests passing (16 common + 519 engine)
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
      spatial/             # Spatial indexing
        index.rs           # SpatialIndex (R-tree via rstar)
    benches/
      bench_utils.rs       # SceneConfig, generate_scene
      engine_benchmarks.rs # 17 Criterion benchmark groups
```
