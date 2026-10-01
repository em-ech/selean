# Selean

## Overview

Selean is a design tool that combines the capabilities of Canva, Figma, and Adobe InDesign into a single platform with AI-native design workflows.

### Core Capabilities

1. **AI Chat-Prompted Design**: Users start designs via chat prompts. The LLM generates scene graph operations (add nodes, set properties, arrange layouts) to produce complete designs.

2. **Import/Export Interoperability**: Bidirectional support for PowerPoint (.pptx), Adobe InDesign (.indd/.idml), Figma (REST API), and native Selean format.

3. **AI-Assisted Editing**: Users prompt modifications to existing designs ("make the heading larger", "change the color scheme to dark mode") and the LLM applies targeted changes via the command system.

4. **Manual Direct Manipulation**: Full Canva/InDesign-style visual editing: drag to move/resize, property inspector, click-to-select, multi-select, alignment, layer management.

5. **Hybrid Workflow**: AI and manual editing interleave freely on the same document, same undo stack, same scene graph.

### Design Principles

- Import fidelity: a PPTX or Figma file should look as close to the original as possible.
- Export fidelity: round-tripping through Selean should not degrade a file.
- The AI chat operates on the same document, undo stack, and scene graph as manual editing.
- Performance targets: 60fps rendering, sub-100ms command execution.

## Architecture

### Mutation Pipeline

All mutation sources converge on a single wire format:

```
[React UI edits]  --\
[LLM tool calls]  ----> CommandDescriptor (JSON) --> Box<dyn Command> --> CommandHistory --> SceneGraph
[File imports]    --/
[Collab ops]      --/
```

`CommandDescriptor` is a `#[serde(tag = "type")]` enum with 31 variants. Adding a new command requires: (1) engine `Command` impl, (2) descriptor variant. Nothing else changes.

### Render Pipeline

Three sequential phases per frame:

1. **DFS Traversal**: Walks the scene graph depth-first, populating texture atlases, computing render order, building the clip stack. Uses `SmallVec<[NodeId; 16]>` for stack-allocated child iteration.

2. **Draw List Construction**: Finalizes UV coordinates, patches clip state, coalesces adjacent same-type commands via `merge_adjacent()`.

3. **Render Pass Execution**: Submits batched wgpu commands. `RectPipeline` for SDF rounded rectangles, `TextPipeline` for SDF glyphs, `TexturedQuadPipeline` for images/vectors. Non-native blend modes use shader-based GPU pipelines per the W3C Compositing spec. Effects use Dual Kawase blur with ping-pong half-resolution textures.

### Scene Graph

`SceneGraph` owns all `SceneNode` instances in a `HashMap<NodeId, SceneNode>` with:

- **Dirty Flags**: 8-bit bitmask per node gating incremental updates. Propagation uses an explicit stack.
- **World Transforms**: 3x2 affine matrices computed lazily. Scroll offsets injected as translation matrices.
- **Spatial Index**: R-tree (via `rstar`) for O(log n) viewport culling and hit testing.
- **Clipping**: Three modes (Scissor, Stencil, ShaderRect) with hierarchical DFS traversal.

### Key Design Decisions

- **Command pattern**: Per-mutation old-state capture. Redo calls `execute()` again. Camera and selection excluded from undo (ephemeral UI state).
- **Platform-agnostic input**: `InputEvent` types carry no platform API references. Consumers translate from winit/SDL/web.
- **Persistence**: JSON format for human readability. Transient state (spatial index, world transforms, dirty flags) excluded from serialization and rebuilt on load. 7 structural invariants validated before deserialization.
- **Scroll containers**: Scroll offset is a transform injection, not a separate coordinate space. Composes naturally with existing transforms.

## File Layout

```
crates/
  selean-common/           # Shared types (NodeId, PageId, errors)
  selean-engine/           # Core rendering engine
    src/
      scene/               # SceneNode, SceneGraph, Transform2D, DirtyFlags, ClipMode
      renderer/            # Renderer, RectPipeline, TexturedQuadPipeline, BlendPipeline,
                           # BlurPipeline, Camera, TextureAtlas, DrawList, ClipStack
      text/                # TextSystem, FontRegistry, shaper, layout, SDF, GlyphCache
      image/               # ImageSystem, loader, cache
      vector/              # VectorSystem, SVG path parser, rasterizer, cache
      input/               # InputEvent, InteractionEvent, InputHandler, SelectionSet
      command/             # Command trait, 15 property + 5 hierarchy commands,
                           # CommandGroup, CommandHistory
      persistence/         # Document save/load, format validation, multi-page support
      spatial/             # SpatialIndex (R-tree)
    benches/               # 18 Criterion benchmark groups
    tests/                 # Cross-module integration tests
  selean-wasm/             # WASM binding layer (SeleanEditor, EditorState, queries)
  selean-llm/              # LLM tool definitions (41 tools) and execution mapping
  selean-codegen/          # Scene graph to React + Tailwind CSS code generation
  selean-pptx/             # PowerPoint import/export
  selean-idml/             # InDesign IDML import/export
  selean-figma/            # Figma REST API import, interchange export
  selean-collab/           # Collaborative editing protocol, OpLog, operation inverses
  selean-server/           # Axum HTTP server (auth, GitHub OAuth, routes, storage, WebSocket,
                           # chat providers: OpenAI-compatible and Anthropic)
  selean-db/               # PostgreSQL database (sqlx, migrations, models, queries)

web/
  selean-app/              # React + TypeScript frontend (Vite)
    src/
      components/          # Canvas, ChatPanel, LeftSidebar, FloatingToolbar, ColorPicker,
                           # HeaderBar, LayerPanel, FileMenu, CodePanel, ContextMenu,
                           # InlineTextEditor, SelectionOverlay, PageBar, ErrorBoundary,
                           # ErrorToast, CollabBar, PresenceOverlay, ExportDialog,
                           # WorkspaceSelector, MemberManager, SnapGuides
      contexts/            # EditorContext (editorRef + onSceneChanged provider)
      hooks/               # useSeleanEditor, useSelection, useCreationTool, useResizeDrag,
                           # useMoveDrag, useAutoSave, useKeyboardShortcuts, useCollabSession,
                           # useFileOperations, useFontLoader, useWorkspace, useGitHub,
                           # useChatEngine, useResizablePanel, useTheme, useCommandDispatch
      auth/                # AuthContext, LoginPage, ProtectedRoute
      collab/              # WsClient, OperationBuffer, CollabContext
      data/                # Element presets, template definitions
      wasm/                # TypeScript types for the WASM bindings; pkg/ is wasm-pack output
                           # (generated by `npm run wasm:build`, not checked in)
      types/               # Shared types (ToolType, NodeKind)
      utils/               # Camera transforms, color conversions, clipboard
      theme.ts             # Design tokens (light/dark palettes, spacing, radii, shadows)

figma-plugin/              # Figma plugin for importing Selean interchange format
```

## Development

```bash
# Run locally (see README "Getting Started" for details)
cd web/selean-app && npm install && npm run wasm:build   # needs wasm-pack; re-run after Rust changes
cargo run -p selean-server                               # repo root; API on :8080, loads .env
cd web/selean-app && npm run dev                         # frontend on :3000, proxies /api to :8080

# Pre-push checklist (CI enforces all of these)
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace
cargo audit
cd web/selean-app && npx tsc --noEmit && npx vitest run && npm audit --omit=dev
```

- AI chat needs no key: by default it calls a local Ollama server (`http://localhost:11434/v1`, model `qwen2.5:7b`) through the OpenAI-compatible provider. `LLM_PROVIDER`, `LLM_BASE_URL`, `LLM_API_KEY` and `LLM_MODEL` select another host or model; Claude is opt-in with `LLM_PROVIDER=anthropic` and `ANTHROPIC_API_KEY`. The server starts either way; if the model server is not reachable, chat returns an error that says how to fix it.
- Chat providers live in `crates/selean-server/src/provider/` (one module per provider behind the `ChatProvider` trait); `chat.rs` owns the HTTP round trip.
- With neither `JWT_SECRET` nor `SELEAN_AUTH_SECRET` set, auth is off: `GET /api/auth/status` returns `{"auth_enabled": false}` and the frontend runs as a guest with no login screen.
- `tsc` and Vitest do not need the WASM bundle (Vitest aliases it to `src/test/wasm-stub.ts`).

### Workspace Clippy Config

- `pedantic = warn` (priority -1)
- `unwrap_used = deny`: use `.expect("reason")` everywhere, including tests
- `expect_used = warn`
- Functions over 100 lines must be split
- Struct constructor fields must match definition order

See `Cargo.toml` `[workspace.lints.clippy]` for the full config.
