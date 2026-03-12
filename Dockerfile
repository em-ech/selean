# Stage 1: Build Rust backend + WASM
FROM rust:1.88-bookworm AS rust-builder
WORKDIR /app
RUN curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
# Copy workspace files first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
# Build release binary
RUN cargo build --release --bin selean-server
# Build WASM package
RUN wasm-pack build crates/selean-wasm --target web --out-dir /app/wasm-pkg

# Stage 2: Build frontend
FROM node:20-bookworm AS frontend-builder
WORKDIR /app/web/selean-app
COPY web/selean-app/package.json web/selean-app/package-lock.json ./
RUN npm ci
COPY web/selean-app/ ./
# Copy WASM build output into the frontend source tree
COPY --from=rust-builder /app/wasm-pkg/ ./src/wasm/pkg/
RUN npm run build

# Stage 3: Final runtime image
FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app

# Copy server binary
COPY --from=rust-builder /app/target/release/selean-server /app/selean-server

# Copy frontend build output
COPY --from=frontend-builder /app/web/selean-app/dist /app/static

# NOTE: Fonts are not baked into the image. Mount a volume at /app/fonts/
# containing .ttf or .otf files for font serving (e.g. -v ./fonts:/app/fonts).

# Default environment
ENV PORT=8080
ENV STATIC_DIR=/app/static
ENV SELEAN_DATA_DIR=/app/data/rooms
ENV SELEAN_ASSET_DIR=/app/data/assets
ENV RUST_LOG=info

EXPOSE 8080

# Create data directories
RUN mkdir -p /app/data/rooms /app/data/assets /app/fonts

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8080/api/health || exit 1

CMD ["/app/selean-server"]
