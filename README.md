# Selean

A design tool that combines the capabilities of Canva, Figma, and Adobe InDesign into a single platform with AI-native design workflows and design-to-code generation.

## Architecture

```
+---------------------------+
|   React Frontend (Vite)   |
|                           |
| LeftSidebar ChatPanel     |
| Canvas  FloatingToolbar   |
| LayerPanel  CodePanel     |
+-----------+---------------+
            |
            | wasm-bindgen FFI
            v
+-----------+---------------+      +------------------+
|       selean-wasm         |      |   selean-server   |
|                           |      |     (Axum)        |
|  SeleanEditor (WASM)      |      |                   |
|  EditorState (native)     |      |  POST /api/chat   |
|  Code generation          |      |  /api/import/*    |
+-----------+---------------+      |  /api/export/*    |
            |                      |  WS   /api/ws     |
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

Server routes: `POST /api/chat` (SSE), `POST /api/import/{pptx,idml,indd,figma}`, `POST /api/export/{pptx,idml,figma}`, WebSocket `/api/ws` (collaboration), `GET /api/auth/status`, plus `/api/auth/*`, `/api/github/*`, `/api/documents`, `/api/workspaces`, `/api/assets` and `/api/fonts/{family}`.

### Mutation Pipeline

All mutation sources converge on a single wire format:

```
[React UI edits]  --\
[LLM tool calls]  ----> CommandDescriptor (JSON) --> Box<dyn Command> --> CommandHistory --> SceneGraph
[File imports]    --/
[Collab ops]      --/
```

`CommandDescriptor` is a `#[serde(tag = "type")]` enum with 31 variants mirroring every `Command` type. Adding a new command requires: (1) engine `Command` impl, (2) descriptor variant. Nothing else changes in LLM/import/export/collab.

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
- Canva-style color picker (hex input, hue/saturation spectrum, opacity slider, gradient modes, 24 preset swatches)
- Full manual editing: drag-to-move, resize handles, rotation, grouping, z-order, alignment
- Floating contextual toolbar (fill, stroke, opacity, font controls, alignment, delete)
- Multi-page documents with per-page undo/redo
- Keyboard shortcuts, context menu, inline text editing
- Layer panel with search filter, inline rename, expand/collapse
- 6 starter templates (social post, presentation, business card, poster, flyer, resume)
- Light theme with dark mode toggle

### AI Chat-Prompted Design

- Lovable-style chat panel with streaming responses and suggestion chips
- Runs on open models through a local Ollama server by default, with no API key; Claude and other OpenAI-compatible hosts are optional
- 41 tool definitions covering node creation, property changes, layout, grouping, effects
- AI and manual editing share the same undo stack and scene graph
- Chat and manual editing given equal weight in the UI layout

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
      components/      Canvas, ChatPanel, LeftSidebar, FloatingToolbar, ColorPicker,
                       LayerPanel, FileMenu, ContextMenu, InlineTextEditor,
                       SelectionOverlay, PageBar, ErrorBoundary, ErrorToast,
                       CollabBar, PresenceOverlay, CodePanel, ExportDialog,
                       WorkspaceSelector, MemberManager, SnapGuides
      hooks/           useSeleanEditor, useSelection, useCreationTool, useResizeDrag,
                       useMoveDrag, useAutoSave, useKeyboardShortcuts, useCollabSession,
                       useFileOperations, useFontLoader, useWorkspace, useGitHub,
                       useChatEngine, useResizablePanel, useTheme
      auth/            AuthContext, LoginPage, ProtectedRoute, api
      collab/          WsClient, OperationBuffer, CollabContext
      wasm/            TypeScript types for the WASM bindings; pkg/ is wasm-pack output (generated, not checked in)
      types/           Shared types (ToolType)
      utils/           Camera transforms, color conversions, clipboard
      theme.ts         Shared design tokens (light/dark palettes, spacing, radii, shadows)

figma-plugin/          Figma plugin for importing Selean interchange format
```

## Getting Started

### Prerequisites

- Rust 1.85+ (edition 2024)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) (`brew install wasm-pack` or `cargo install wasm-pack`); it adds the `wasm32-unknown-unknown` Rust target on first use
- Node.js 20+
- Chrome 113+ or Edge 113+ (WebGPU is required; the editor does not start without it)
- [Ollama](https://ollama.com/download) (optional, only for AI chat; no API key needed)
- PostgreSQL 16 (optional, only for accounts and workspaces)

### Run locally

Three steps: build the WASM engine, start the API server, start the frontend.

```bash
# 1. Build the WASM engine into web/selean-app/src/wasm/pkg/.
#    Re-run this after changing any Rust crate.
cd web/selean-app
npm install
npm run wasm:build

# 2. In a second terminal, from the repository root: start the API server
#    on http://localhost:8080.
cp .env.example .env        # optional: nothing in it is required
cargo run -p selean-server

# 3. Back in web/selean-app: start the frontend on http://localhost:3000.
npm run dev
```

Open http://localhost:3000 in Chrome or Edge. The Vite dev server proxies `/api` to the server on port 8080.

- `npm run dev` and `npm run build` stop with a message if the WASM bundle has not been built.
- The server reads `.env` from the directory it is started in (or a parent directory). Variables already set in the shell take precedence over `.env`.
- AI chat uses a local Ollama server and no API key; see "AI chat" below. Without Ollama the server still starts and everything except AI chat works; the chat panel shows an error saying how to start it.
- Documents auto-save to the browser's `localStorage` every 5 seconds and are restored on reload.

To run from a single process instead, build the frontend and let the server serve it:

```bash
cd web/selean-app && npm run wasm:build && npm run build
cd ../.. && cargo run -p selean-server      # serves web/selean-app/dist on http://localhost:8080
```

### AI chat

The chat runs on open models by default: the server talks to an [Ollama](https://ollama.com) server on your machine through its OpenAI-compatible API. It needs no API key and costs nothing.

Status: the request translation, response and stream parsing, and error messages are covered by automated tests against a stand-in server. The live path against a real Ollama server has not been tested yet. Claude (see "Optional: bring your own key") is the only provider the chat has been used with end to end.

#### Run the AI chat for free

1. Install Ollama from https://ollama.com/download and start it (open the app, or run `ollama serve` in a terminal).
2. Download the default model (4.7 GB):

   ```bash
   ollama pull qwen2.5:7b
   ```

3. Give the model room for Selean's tool definitions. The 41 tools are about 20 KB of JSON in every request, which by our estimate is more than the 4k-token context Ollama uses by default on machines with under 24 GiB of VRAM; with too little context Ollama cuts the prompt short and the model stops calling tools. Raise the context length in the Ollama app's settings, or start the server with:

   ```bash
   OLLAMA_CONTEXT_LENGTH=16000 ollama serve
   ```

4. Start Selean as described in "Run locally". Nothing needs to be set in `.env`.
5. Check the server log for this line:

   ```
   AI chat uses an OpenAI-compatible server base_url="http://localhost:11434/v1" model="qwen2.5:7b"
   ```

   Then type a prompt in the chat panel, for example "add a blue rectangle". The reply streams into the panel and the shapes appear on the canvas.

The default model is `qwen2.5:7b` because Ollama lists it as supporting tool calling and its 4.7 GB download fits a typical laptop.

#### Use a different open model

Set `LLM_MODEL` in `.env` (or the shell) to any model you have pulled that Ollama lists with the `tools` label, then restart the server:

```bash
ollama pull qwen2.5:14b
LLM_MODEL=qwen2.5:14b cargo run -p selean-server
```

| Model         | Download | Notes                                           |
| ------------- | -------- | ----------------------------------------------- |
| `llama3.2`    | 2.0 GB   | Smallest of the three; expect weaker tool calls |
| `qwen2.5:7b`  | 4.7 GB   | Default                                         |
| `qwen2.5:14b` | 9.0 GB   | Larger; needs correspondingly more memory       |

Sizes are the download sizes on Ollama's model pages. A model needs at least that much free memory to run, plus more for a longer context.

#### What to expect

Small local models are weaker at tool calling than Claude. Typical symptoms:

- The model picks the wrong tool, or describes a change without making it (no tool call).
- The tool arguments are malformed. Selean then runs the tool with empty arguments, and the tool reports an error.
- The model ignores the current scene, usually because the context length is too short (step 3).

What to try: a larger model, a shorter and more specific prompt (one change at a time), and a longer context length.

#### Troubleshooting

Errors appear in the chat panel.

| Message                                                                 | Fix                                                                                      |
| ----------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `AI chat could not reach Ollama at http://localhost:11434/v1. ...`      | Ollama is not running. Open the Ollama app or run `ollama serve`                         |
| `The model ... is not downloaded. Run ollama pull ... and try again.`   | Run the `ollama pull` command shown in the message                                       |
| `The model ... does not support tool calling, which AI chat needs. ...` | Set `LLM_MODEL` to a model with the `tools` label on its Ollama page                     |
| `No chat completions API was found at ... Check LLM_BASE_URL; ...`      | `LLM_BASE_URL` must end with the API root, `/v1` for Ollama                              |
| `AI chat could not reach the model server at ... Check LLM_BASE_URL ...` | Shown for a server that is not on Ollama's port: check `LLM_BASE_URL` and that it is up |
| `The model server at ... rejected the API key. Check LLM_API_KEY.`      | Hosted providers only: fix the key in `.env`                                             |

The server keeps a request open for at most 120 seconds, so the first prompt can time out while Ollama is still loading a large model; send it again.

#### Optional: bring your own key

Skip this section unless you want a hosted model. Keys go in your local `.env`, which is git-ignored; never commit one. Each provider reads only its own key.

**OpenRouter, or any other OpenAI-compatible host.** OpenRouter requires an account key even for its free models. Pick a model that supports tool calling.

```bash
LLM_PROVIDER=openai-compatible
LLM_BASE_URL=https://openrouter.ai/api/v1
LLM_API_KEY=            # your OpenRouter key
LLM_MODEL=              # a model ID from openrouter.ai/models
```

**Anthropic.** Claude is the provider the chat was built and tested with.

```bash
LLM_PROVIDER=anthropic
ANTHROPIC_API_KEY=      # your Anthropic key
```

`ANTHROPIC_API_KEY` on its own does not select Claude: without `LLM_PROVIDER=anthropic` the server uses local Ollama and logs a warning that the key is unused. Tool-calling quality varies by model on every OpenAI-compatible host. No hosted OpenAI-compatible provider has been tested live.

### Guest mode and auth mode

The server decides, and the frontend asks it through `GET /api/auth/status`.

- **Guest mode (default).** With neither `JWT_SECRET` nor `SELEAN_AUTH_SECRET` set, the server accepts every request and the app opens straight into the editor with no login. There are no accounts or workspaces in this mode. If the server is not running, the app cannot tell that auth is off and shows the login screen.
- **Auth mode.** Set `JWT_SECRET` and `DATABASE_URL` (PostgreSQL; migrations run at startup). The app shows the login screen, with signup and login backed by the database. `JWT_SECRET` without a database enables the login screen, but signup and login return 503.
- `SELEAN_AUTH_SECRET` is a shared bearer token for API clients. The web app cannot send it, so it is not a way to protect the browser app.

### Docker

```bash
cp .env.example .env        # optional: nothing in it is required
docker compose up --build
```

This builds the server, the WASM engine and the frontend into one image and starts it with PostgreSQL and MinIO on http://localhost:8080. Compose sets `DATABASE_URL` and a default `JWT_SECRET`, so this path runs in auth mode: sign up on the login screen first. Change `JWT_SECRET` for anything other than local use. Assets are stored on the local volume unless the `S3_*` variables are set.

AI chat in Docker uses the Ollama server on the host through `http://host.docker.internal:11434/v1`. This path is untested. On Linux, Ollama must listen on an address the container can reach (`OLLAMA_HOST=0.0.0.0 ollama serve`).

### Environment Variables

| Variable               | Required for                | Description                                                        |
| ---------------------- | --------------------------- | ------------------------------------------------------------------ |
| `LLM_PROVIDER`         | --                          | `openai-compatible` (default) or `anthropic`                       |
| `LLM_BASE_URL`         | --                          | OpenAI-compatible API root (default `http://localhost:11434/v1`)   |
| `LLM_MODEL`            | --                          | Model on that server (default `qwen2.5:7b`)                        |
| `LLM_API_KEY`          | Hosted OpenAI-compatible    | Key for that server. Not needed for local Ollama                   |
| `ANTHROPIC_API_KEY`    | `LLM_PROVIDER=anthropic`    | Anthropic API key. Only read when Anthropic is selected            |
| `ANTHROPIC_MODEL`      | --                          | Claude model (default `claude-sonnet-4-6`)                         |
| `JWT_SECRET`           | Auth mode                   | Secret for JWT signing; enables the login screen                   |
| `DATABASE_URL`         | Auth mode, workspaces       | PostgreSQL connection string                                       |
| `SELEAN_AUTH_SECRET`   | --                          | Shared bearer token for API clients (not usable from the web app)  |
| `PORT`                 | --                          | Server port (default `8080`)                                       |
| `STATIC_DIR`           | --                          | Built frontend to serve (default `web/selean-app/dist`)            |
| `ALLOWED_ORIGINS`      | CORS                        | Comma-separated allowed origins (default `http://localhost:3000`)  |
| `GITHUB_CLIENT_ID`     | GitHub integration          | GitHub OAuth App client ID                                         |
| `GITHUB_CLIENT_SECRET` | GitHub integration          | GitHub OAuth App client secret                                     |
| `S3_BUCKET`            | S3/R2/MinIO asset storage   | Bucket name; with `S3_REGION`, `S3_ENDPOINT`, `S3_ACCESS_KEY`, `S3_SECRET_KEY` |
| `FIGMA_ACCESS_TOKEN`   | Figma import                | Figma personal access token                                        |
| `INDESIGN_SERVER_URL`  | .indd import                | URL of an Adobe InDesign Server; without it .indd import returns 501 |

See [.env.example](.env.example) for the full list.

### Development checks

```bash
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace
cd web/selean-app && npx tsc --noEmit && npx vitest run
```

The TypeScript check and the Vitest suite do not need the WASM bundle.

## Testing

```bash
# Rust (1920 tests across 11 crates)
cargo test --workspace

# Frontend (461 tests via Vitest)
cd web/selean-app && npx vitest run

# Benchmarks (18 groups)
cargo bench -p selean-engine
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for development setup, PR process, and code style guidelines.

## Security

See [SECURITY.md](SECURITY.md) for reporting vulnerabilities.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
