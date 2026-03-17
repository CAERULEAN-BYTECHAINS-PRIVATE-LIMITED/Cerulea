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

WORKDIR /build

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo

# Copy all crate sources
COPY cbc-node cbc-node
COPY cbc-runtime cbc-runtime
COPY cbc-pallets cbc-pallets
COPY tools tools

# Build release binary
RUN cargo build --release -p cbc-node

# Stage 2: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cbc-node /usr/local/bin/cbc-node

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
