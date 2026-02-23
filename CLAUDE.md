# Selean Engine Session Notes

## Current Phase: 6 Complete (Scroll Containers)

### Completed Phases

- Phase 1: Rect rendering (SDF rounded corners)
- Phase 2: SDF text rendering
- Phase 3: Image and vector rendering
- Phase 4: Scene graph, R-tree spatial index, dirty flags, 2D affine transforms, blend modes
- Phase 5: Clipping and masking (Scissor, Stencil, ShaderRect)
- Phase 6: Scroll containers

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

- 458 tests passing (16 common + 442 engine)
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
      spatial/             # Spatial indexing
        index.rs           # SpatialIndex (R-tree via rstar)
    benches/
      bench_utils.rs       # SceneConfig, generate_scene
      engine_benchmarks.rs # 17 Criterion benchmark groups
```
