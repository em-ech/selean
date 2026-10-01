# Contributing to Selean

## Prerequisites

- Rust 1.85+ (edition 2024)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/) (`brew install wasm-pack` or `cargo install wasm-pack`)
- Node.js 20+
- Chrome 113+ or Edge 113+ (WebGPU)
- PostgreSQL 16 (optional, for accounts and workspaces)

## Development Setup

```bash
# Build and test backend
cargo build --workspace
cargo test --workspace

# Build the WASM engine (re-run after changing any Rust crate)
cd web/selean-app
npm install
npm run wasm:build

# API server on http://localhost:8080 (second terminal, from the repository root)
cargo run -p selean-server

# Frontend on http://localhost:3000 (in web/selean-app)
npm run dev
```

The server loads `.env` from the directory it is started in; `ANTHROPIC_API_KEY` is only needed for AI chat. With no `JWT_SECRET` the app runs in guest mode with no login. See the README for auth mode and Docker.

## Before Submitting a PR

All of these must pass (CI enforces them):

```bash
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace
cargo audit
cd web/selean-app && npx tsc --noEmit && npx vitest run && npm audit --omit=dev
```

CI runs clippy with the latest stable toolchain, so run `rustup update stable` before concluding clippy is clean. Advisories that cannot be fixed by upgrading are listed, with the reason, in `.cargo/audit.toml`.

## Pull Request Process

1. Fork the repository and create a branch from `main`
2. Make your changes with tests
3. Ensure CI passes (format, clippy, build, test, security audit, frontend type-check, tests and audit, Docker build)
4. Open a PR against `main`
5. One approval required before merge

## Commit Messages

Use conventional commits:

- `feat:` new feature
- `fix:` bug fix
- `refactor:` code change that neither fixes a bug nor adds a feature
- `docs:` documentation only
- `test:` adding or updating tests
- `chore:` maintenance (CI, deps, config)

## Code Style

**Rust**: Workspace clippy config enforces `pedantic` lints, `unwrap_used = deny`, and 100-line function limit. Use `.expect("reason")` instead of `.unwrap()`. See `Cargo.toml` `[workspace.lints.clippy]` for the full config.

**TypeScript**: Strict mode enabled. All components use theme tokens from `theme.ts` instead of hardcoded values.

## Testing

Tests are non-negotiable. Add tests for new features and bug fixes. The project has 1800+ Rust tests and 400+ frontend tests.

## Architecture

See `CLAUDE.md` for crate structure, design decisions, and the mutation/render pipeline. Key points:

- All mutations go through `CommandDescriptor` (JSON wire format)
- Adding a command requires only an engine `Command` impl + descriptor variant
- AI chat, manual editing, file imports, and collab all share the same undo stack
