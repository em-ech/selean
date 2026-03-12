# Selean

A design tool that combines the capabilities of Canva, Figma, and Adobe InDesign into a single platform with AI-native design workflows and design-to-code generation.

## Architecture

```
+---------------------------+
|   React Frontend (Vite)   |
|                           |
|  Canvas  Chat  Inspector  |
|  (WebGPU) (SSE)  (Props)  |
|  CodePanel  ExportDialog  |
+-----------+---------------+
            |
            | wasm-bindgen FFI
            v
+-----------+---------------+      +------------------+
|       selean-wasm         |      |   selean-server   |
|                           |      |     (Axum)        |
|  SeleanEditor (WASM)      |      |                   |
|  EditorState (native)     |      |  POST /api/chat   |
|  Code generation          |      |  POST /api/import |
+-----------+---------------+      |  POST /api/export |
            |                      |  WS   /ws/collab  |
            | CommandDescriptor    |  /api/github/*    |
            v       (JSON)         |  /api/auth/*      |
+-----------+---------------+      +--------+----------+
|       selean-engine       |      |    selean-llm      |
|                           |      |                    |
|  Scene Graph              |      |  Tool definitions  |
|  Renderer (3-phase)       |      |  map_tool_call()   |
|  Input Handler            |      |  is_read_only()    |
|  Command History          |      +--------------------+
|  Persistence              |
+---------------------------+      +--------------------+
            |                      |   selean-collab    |
            v                      |                    |
    +-------+--------+            |  OpLog, inverse    |
    |      wgpu      |            |  ops, protocol     |
    | (Vulkan/Metal/ |            +--------------------+
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

`CommandDescriptor` is a `#[serde(tag = "type")]` enum with 29 variants mirroring every `Command` type. Adding a new command requires: (1) engine `Command` impl, (2) descriptor variant. Nothing else changes in LLM/import/export/collab.

### Render Pipeline

Three sequential phases per frame:

1. **DFS Traversal**: Walks the scene graph depth-first, populating texture atlases (glyphs, images, vector rasterizations), computing render order, and building the clip stack. Uses `SmallVec<[NodeId; 16]>` for stack-allocated child iteration.

2. **Draw List Construction**: Finalizes UV coordinates from atlas packing, patches clip state onto draw commands, and coalesces adjacent same-type commands via `merge_adjacent()` to minimize draw calls.

3. **Render Pass Execution**: Submits batched wgpu commands. `RectPipeline` for SDF rounded rectangles, `TextPipeline` for SDF glyph rendering, `TexturedQuadPipeline` for images and rasterized vectors. Non-native blend modes use shader-based GPU pipelines with intermediate render targets per the W3C Compositing spec. Drop shadow and blur effects use Dual Kawase blur with ping-pong half-resolution textures.

### Scene Graph

The `SceneGraph` owns all `SceneNode` instances in a `HashMap<NodeId, SceneNode>`, maintains parent/child relationships, and integrates:

- **Dirty Flags**: 8-bit bitmask per node gating incremental updates. Dirty propagation uses an explicit stack.
- **World Transforms**: 3x2 affine matrices computed lazily. Scroll offsets injected as translation matrices into child transforms.
- **Spatial Index**: R-tree (via `rstar`) for O(log n) viewport culling and hit testing.
- **Clipping**: Three modes (Scissor, Stencil, ShaderRect) with hierarchical DFS traversal.

## Features

### Visual Design Editor

- GPU-accelerated 2D rendering engine (wgpu, 60fps target)
- SDF text rendering with multi-font support and variable weight
- 13 blend modes (Normal, Add, Multiply, Screen, Overlay, etc. via multi-pass GPU shaders)
- Visual effects (drop shadow, Gaussian blur via Dual Kawase pipeline)
- Linear and radial gradients with arbitrary color stops
- Full manual editing: drag-to-move, resize handles, rotation, grouping, z-order, alignment
- Multi-page documents with per-page undo/redo
- Keyboard shortcuts, context menu, inline text editing
- Selection overlay, property inspector, layer panel, page bar

### AI Chat-Prompted Design

- Claude-powered chat sidebar with streaming responses
- 41 tool definitions covering node creation, property changes, layout, grouping, effects
- AI and manual editing share the same undo stack and scene graph

### Import/Export

| Format             | Import                    | Export                   |
| ------------------ | ------------------------- | ------------------------ |
| PowerPoint (.pptx) | Yes                       | Yes                      |
| InDesign (.idml)   | Yes                       | Yes                      |
| InDesign (.indd)   | Yes (via InDesign Server) | --                       |
| Figma (REST API)   | Yes                       | Yes (interchange format) |
| Native Selean      | Yes                       | Yes                      |

### Design-to-Code

Live code generation panel (similar to Figma dev mode / Lovable):

- Scene graph to React + Tailwind CSS conversion in real time
- Per-page component view with syntax highlighting
- Full project generation (Vite + React + Tailwind)
- Design token extraction (colors, fonts)
- Three export modes:
  - **GitHub Push**: Push to a repo via Git Data API, optional PR creation
  - **Claude Code**: ZIP with CLAUDE.md containing design tokens, component inventory, and conventions
  - **Download ZIP**: Raw Vite project files

### GitHub Integration

- GitHub OAuth login (alternative to email/password)
- Repository listing, creation, and code push
- Commit to configurable branch with optional pull request
- Account linking (email users can connect GitHub later)

### Real-Time Collaboration

- WebSocket-based collaborative editing with server-authoritative sequencing
- Presence overlay (cursors, selections)
- Offline operation buffering with automatic sync on reconnect
- Per-user undo/redo stacks

### SaaS Infrastructure

- JWT authentication with token refresh (email/password + GitHub OAuth)
- Workspace-based multi-tenancy with RBAC (Viewer/Editor/Admin/Owner)
- Billing tiers (Free/Pro/Team/Enterprise) with quota enforcement
- Asset storage (S3/R2/MinIO + local filesystem fallback)
- Docker deployment with PostgreSQL, MinIO, and health checks
- Environment-driven CORS, graceful shutdown, CI/CD to ghcr.io

## Workspace Structure

```
crates/
  selean-common/       Shared types (NodeId, PageId, errors)
  selean-engine/       GPU rendering engine, scene graph, input, undo/redo, persistence
  selean-wasm/         WASM binding layer (SeleanEditor, EditorState, queries, codegen)
  selean-llm/          LLM tool definitions (41 tools) and execution mapping
  selean-codegen/      Design-to-code engine (React + Tailwind generation, CLAUDE.md)
  selean-pptx/         PowerPoint (.pptx) import/export
  selean-idml/         InDesign (.idml) import/export
  selean-figma/        Figma REST API import, interchange export
  selean-collab/       Collaborative editing protocol, OpLog, operation inverses
  selean-server/       Axum HTTP server (auth, GitHub, routes, storage, WebSocket)
  selean-db/           PostgreSQL database (sqlx, migrations, models, queries)

web/
  selean-app/          React + TypeScript frontend (Vite)
    src/
      components/      Canvas, ChatSidebar, PropertyInspector, FileMenu, LayerPanel,
                       AlignmentBar, ContextMenu, InlineTextEditor, SelectionOverlay,
                       Toolbar, PageBar, ErrorBoundary, ErrorToast, CollabBar,
                       PresenceOverlay, CodePanel, ExportDialog, WorkspaceSelector,
                       MemberManager
      hooks/           useSeleanEditor, useSelection, useCreationTool, useResizeDrag,
                       useMoveDrag, useAutoSave, useKeyboardShortcuts, useCollabSession,
                       useFileOperations, useFontLoader, useWorkspace, useGitHub
      auth/            AuthContext, LoginPage, ProtectedRoute, api
      collab/          WsClient, OperationBuffer, CollabContext
      wasm/            TypeScript type stubs for WASM bindings
      theme.ts         Shared design tokens (colors, font sizes)

figma-plugin/          Figma plugin for importing Selean interchange format
```

## Getting Started

### Prerequisites

- Rust 1.85+ (edition 2024)
- Node.js 18+
- PostgreSQL 16 (optional, for SaaS features)
- GPU with Vulkan, Metal, DX12, or WebGPU support
- Chrome 113+ or Edge 113+ (WebGPU required)

### Development

```bash
# Backend
cargo build --workspace
cargo test --workspace

# Frontend
cd web/selean-app
npm install
npm run dev

# Lint
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
```

### Docker

```bash
cp .env.example .env
# Edit .env with your configuration
docker compose up
```

### Environment Variables

| Variable               | Required               | Description                     |
| ---------------------- | ---------------------- | ------------------------------- |
| `DATABASE_URL`         | For SaaS features      | PostgreSQL connection string    |
| `JWT_SECRET`           | For auth               | Secret for JWT signing          |
| `ANTHROPIC_API_KEY`    | For AI chat            | Anthropic API key               |
| `GITHUB_CLIENT_ID`     | For GitHub integration | GitHub OAuth App client ID      |
| `GITHUB_CLIENT_SECRET` | For GitHub integration | GitHub OAuth App client secret  |
| `S3_BUCKET`            | For cloud storage      | S3/R2/MinIO bucket name         |
| `FIGMA_ACCESS_TOKEN`   | For Figma import       | Figma personal access token     |
| `INDESIGN_SERVER_URL`  | For .indd import       | InDesign Server URL             |
| `ALLOWED_ORIGINS`      | For CORS               | Comma-separated allowed origins |

See [.env.example](.env.example) for the full list.

## Testing

```bash
# Rust (1682 tests across 11 crates)
cargo test --workspace

# Frontend (410 tests via Vitest)
cd web/selean-app && npx vitest run

# Benchmarks (17 groups)
cargo bench -p selean-engine
```

## License

MIT OR Apache-2.0
