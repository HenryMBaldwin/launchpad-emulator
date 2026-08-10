# Rust base stage with tooling
FROM lukemathwalker/cargo-chef:latest-rust-1.95.0-bookworm AS rust-base

# Install precompiled sccache
ARG TARGETARCH
ARG SCCACHE_VERSION=0.7.7

RUN case "$TARGETARCH" in \
      "amd64") ARCH="x86_64" ;; \
      "arm64") ARCH="aarch64" ;; \
      *) echo "Unsupported architecture: $TARGETARCH" && exit 1 ;; \
    esac && \
    curl -L "https://github.com/mozilla/sccache/releases/download/v${SCCACHE_VERSION}/sccache-v${SCCACHE_VERSION}-${ARCH}-unknown-linux-musl.tar.gz" | \
    tar xz -C /usr/local/bin --strip-components=1 "sccache-v${SCCACHE_VERSION}-${ARCH}-unknown-linux-musl/sccache" && \
    chmod +x /usr/local/bin/sccache

ENV RUSTC_WRAPPER=sccache \
    SCCACHE_DIR=/sccache \
    CARGO_INCREMENTAL=0

# Planner stage
FROM rust-base AS rust-planner
WORKDIR /app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Builder stage
FROM rust-base AS rust-builder
WORKDIR /app

ARG SERVICE
RUN if [ -z "$SERVICE" ]; then \
      echo "ERROR: SERVICE build argument is required"; \
      echo "Usage: docker build --build-arg SERVICE=<service-name> ..."; \
      exit 1; \
    fi

COPY --from=rust-planner /app/recipe.json recipe.json

# Build dependencies (cached layer)
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo chef cook --release --bin "$SERVICE" --recipe-path recipe.json

COPY . .

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked \
    cargo build --release --bin "$SERVICE"

# Runtime stage
FROM debian:bookworm-slim AS runtime
ARG SERVICE
RUN if [ -z "$SERVICE" ]; then \
      echo "ERROR: SERVICE build argument is required in runtime stage"; \
      exit 1; \
    fi

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=rust-builder /app/target/release/${SERVICE} /usr/local/bin/service

ENTRYPOINT ["/usr/local/bin/service"]
