# Selean Engine Session Notes

## Current Phase: 5 (Clipping & Masking)

Full plan file: `~/.claude/plans/cheeky-stirring-aurora.md`

### Completed Steps

**Step 1: ClipMode + ClipRect + DirtyFlags** (done)

- Created `scene/clip.rs` with `ClipMode` enum (None/Scissor/Stencil/ShaderRect) and `ClipRect` struct
- Added `CLIP = 0x80` dirty flag, updated `ALL` to `0xFF`
- Added `clip_mode: ClipMode` field to `SceneNode`, default `None`
- Added `set_clip_mode()` to `SceneGraph`
- 22 tests for ClipRect math

**Step 2: Clip-aware hit testing** (done)

- Added `ancestor_clip_chain()`, `point_passes_clip()`, `point_inside_rounded_rect()` to SceneGraph
- Modified `hit_test()` to walk ancestor clip chain and reject clipped points
- SDF rounded-rect check for stencil clips on Frame nodes
- 12 new tests for clip-aware hit testing

**Step 3: Stencil texture + DepthStencilState infrastructure** (done)

- Added `STENCIL_NOOP` constant to `shared.rs`
- Modified `create_pipeline_with_blend()` to accept `depth_stencil` and `color_writes` params
- Added `create_stencil_texture()` helper (Depth24PlusStencil8 format)
- Stencil texture created on `Renderer::new()`, recreated on `resize()`
- Render pass attaches stencil with `Clear(0)` / `Discard`
- All pipelines use no-op stencil state (rendering unchanged)

**CI fixes** (done, committed with step 3)

- Fixed `cargo fmt`, clippy pedantic lints across all modules

Commit: `ebb3aaa` pushed to origin/main

**Step 4: Stencil pipeline variants** (done)

- Added `STENCIL_TEST`, `STENCIL_WRITE`, `STENCIL_DECREMENT` constants to `shared.rs`
- `RectPipeline`: 4 new variants (normal/add stencil test, stencil write, stencil decrement)
- `TextPipeline`: 2 new variants (normal/add stencil test)
- `TexturedQuadPipeline`: 2 new variants (normal/add stencil test)
- Added `select_pipeline(blend, stencil_test) -> &RenderPipeline` to all 3 pipeline structs
- Added `stencil_write_pipeline()` and `stencil_decrement_pipeline()` to `RectPipeline`
- Exported new constants from `renderer/mod.rs`
- 400 tests passing, zero clippy warnings

**Step 5: Shader + instance data for ShaderRect clip** (done)

- Added `clip_rect: [f32; 4]` to all 3 instance types (RectInstance 96->112, GlyphInstance 88->104, TexturedQuadInstance 88->104)
- Added `@location(N) clip_rect: vec4<f32>` to all 3 WGSL instance structs
- Added `world_pos: vec2<f32>` and `clip_rect: vec4<f32>` to all 3 VertexOutput structs
- Added `out.world_pos = world_pos` and `out.clip_rect = inst.clip_rect` passthrough in all 3 vertex shaders
- Added discard logic at start of all 3 fragment shaders
- All construction sites use `ClipRect::INFINITE.to_array()` sentinel (no discard)
- Updated size assertions, added clip_rect value assertions to existing tests
- 400 tests passing, zero clippy warnings

**Step 6: ClipStack + DrawList + hierarchical DFS** (done)

- Created `renderer/clip_stack.rs`: `ClipStack` with push/pop for scissor, stencil, shader_rect modes; `ResolvedClipState` snapshot type; 15 tests
- Created `renderer/draw_list.rs`: `DrawCommand` enum (9 variants), `DrawList` with `merge_adjacent()` coalescing; 12 tests
- Added alpha discard to `rect.wgsl` (`if outer_alpha <= 0.0 { discard }`) so stencil writes respect rounded corners
- Added `ordered_instances` vec + methods to `RectBatch`, `TextBatch`, `TexturedQuadBatch` for hierarchical instance ordering
- Added `finalize_uvs_ordered()` to `TextBatch` and `TexturedQuadBatch` for ordered UV finalization
- Replaced flat `prepare()`/`render()` with hierarchical DFS in `pipeline.rs`:
  - `RenderOrderEntry` enum bridges DFS traversal and draw list construction
  - `dfs_visit()` walks scene graph depth-first, pushes/pops clip state, populates atlases, builds render order
  - `build_draw_list()` patches `clip_rect` on instances, emits `DrawCommand`s
  - `execute_draw_list()` iterates draw commands sequentially in render pass
- Viewport culling during DFS (skip subtrees outside effective clip rect)
- `merge_adjacent()` ensures no-clip scenes have identical draw call count to previous flat approach
- Updated `renderer/mod.rs` exports for `ClipStack`, `ResolvedClipState`, `DrawCommand`, `DrawList`
- 412 tests passing (16 common + 396 engine), zero clippy warnings

### Remaining Steps

**Step 7: Benchmarks + final verification**

- Add `clip_fraction` to `SceneConfig`, random clip modes on generated nodes
- Add `bench_clip_traversal` group
- Final: `cargo test --workspace && cargo clippy --workspace && cargo bench --no-run`

### Test Count

- 412 tests passing (16 common + 396 engine) as of step 6 completion

### Key Architecture Notes

- Rendering uses hierarchical DFS traversal (replaced flat draw order in step 6)
- Two-phase pipeline: Phase 1 (DFS: atlas population, render order), Phase 2 (UV finalization, clip patching, DrawList construction)
- `DrawList::merge_adjacent()` coalesces contiguous same-type draw commands for minimal draw calls
- Stencil nesting uses IncrementClamp/DecrementClamp (max 255 depth)
- ShaderRect uses per-instance `clip_rect` with fragment discard
- Scissor uses GPU `set_scissor_rect()` (zero overhead, no rounded corners)
- Pipeline variant table in plan file covers all blend x stencil combinations
- `render_frame()` is the single entry point: takes `&mut SceneGraph` + `&TextureView`
