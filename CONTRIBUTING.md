# Contributing to Selean

## Prerequisites

- Rust 1.85+ (edition 2024)
- Node.js 18+
- PostgreSQL 16 (optional, for SaaS features)

## Development Setup

```bash
# Build and test backend
cargo build --workspace
cargo test --workspace

# Frontend
cd web/selean-app
npm install
npm run dev
```

## Before Submitting a PR

All of these must pass (CI enforces them):

```bash
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace
cd web/selean-app && npx vitest run
```

## Pull Request Process

1. Fork the repository and create a branch from `main`
2. Make your changes with tests
3. Ensure CI passes (format, clippy, build, test, frontend tests)
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
