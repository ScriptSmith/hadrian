# syntax=docker/dockerfile:1.4

# Stage 1: Build frontend assets (UI, Storybook, Docs)
FROM node:24-slim AS frontend-builder

# Install pnpm
RUN corepack enable && corepack prepare pnpm@9 --activate

WORKDIR /app

# Copy package files for dependency caching
COPY ui/package.json ui/pnpm-lock.yaml ./ui/
COPY docs/package.json docs/pnpm-lock.yaml docs/next.config.mjs docs/source.config.ts ./docs/

# Install UI dependencies
WORKDIR /app/ui
RUN --mount=type=cache,id=pnpm,target=/root/.local/share/pnpm/store \
  pnpm install --frozen-lockfile

# Install docs dependencies
WORKDIR /app/docs
RUN --mount=type=cache,id=pnpm,target=/root/.local/share/pnpm/store \
  pnpm install --frozen-lockfile

# Copy source files
WORKDIR /app
COPY ui ./ui
COPY docs ./docs
COPY openapi/hadrian.openapi.json ./openapi/hadrian.openapi.json

ENV NEXT_TELEMETRY_DISABLED=1

# Generate API client and build UI
WORKDIR /app/ui
RUN pnpm run generate-api
RUN pnpm build

# Build Storybook (needed for docs)
RUN pnpm storybook:build

# Build docs (follows symlink to storybook-static)
WORKDIR /app/docs
RUN pnpm build

# Stage 2: Build Rust application
FROM rustlang/rust:nightly-slim AS builder

# Install build dependencies
# Includes SAML libraries (libxml2, libxslt, xmlsec1) for samael crate
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    build-essential \
    cmake \
    curl \
    tar \
    file \
    libxml2-dev \
    libxslt1-dev \
    libxmlsec1-dev \
    libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/hadrian

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./

# Create dummy src to build dependencies
RUN mkdir -p src/bin \
    && echo "fn main() {}" > src/main.rs \
    && echo "fn main() {}" > src/bin/record_fixtures.rs

# Build dependencies only (cached layer)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/hadrian/target \
    cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src
COPY migrations_sqlx ./migrations_sqlx

# Copy frontend assets from frontend-builder
COPY --from=frontend-builder /app/ui/dist ./ui/dist/
COPY --from=frontend-builder /app/docs/out ./docs/out/

# Fetch model catalog (embedded at compile time via include_str!)
RUN mkdir -p data && curl -sSL https://models.dev/api.json -o data/models-dev-catalog.json

# Touch main.rs to invalidate the dummy build
RUN touch src/main.rs

# Build the actual application
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/hadrian/target \
    cargo build --release && \
    cp target/release/hadrian /usr/src/hadrian/hadrian-bin

# Runtime stage
FROM debian:trixie-slim

# Install runtime dependencies
# Includes SAML libraries for XML signature verification
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    libxml2 \
    libxslt1.1 \
    libxmlsec1 \
    libxmlsec1-openssl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 hadrian

# Create app directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /usr/src/hadrian/hadrian-bin /app/hadrian

# Copy migrations
COPY --from=builder /usr/src/hadrian/migrations_sqlx /app/migrations_sqlx

# Create data directory for SQLite (will be overwritten by volume mount)
RUN mkdir -p /app/data

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

CMD ["/app/hadrian", "--config", "/app/config/hadrian.toml"]
