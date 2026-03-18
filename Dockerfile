# Stage 1: Build
FROM rust:1.85-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    clang \
    libclang-dev \
    llvm \
    protobuf-compiler \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add the WASM target for Substrate runtime compilation
RUN rustup target add wasm32-unknown-unknown
RUN rustup component add rust-src

# Set cargo cache directories for persistent caching across builds
ENV CARGO_HOME=/cargo
ENV CARGO_TARGET_DIR=/target

WORKDIR /build

# Copy workspace manifests first for layer caching (dependencies)
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo

# Pre-fetch and cache all dependencies before copying source code
# This ensures dependency downloads are cached separately from code changes
RUN cargo fetch --locked || true

# Copy all crate sources
COPY cbc-node cbc-node
COPY cbc-runtime cbc-runtime
COPY cbc-pallets cbc-pallets
COPY tools tools

# Build release binary
# --locked ensures we use the exact Cargo.lock versions
RUN cargo build --release --locked -p cbc-node

# Stage 2: Runtime (minimal image)
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy only the final binary from builder (no build artifacts, keeps image small)
COPY --from=builder /target/release/cbc-node /usr/local/bin/cbc-node
RUN chmod +x /usr/local/bin/cbc-node

# Copy the entrypoint script
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh

# P2P port
EXPOSE 30333
# RPC port
EXPOSE 9944
# Prometheus metrics
EXPOSE 9615

VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
