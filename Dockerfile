###############################################################################
# Stage 1: Dependency cache
# Only re-runs when Cargo.toml / Cargo.lock change — not on source edits.
###############################################################################
FROM rust:1.85-bookworm AS deps

RUN apt-get update && apt-get install -y --no-install-recommends \
    clang libclang-dev llvm protobuf-compiler pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown \
 && rustup component add rust-src

WORKDIR /build

# ── Workspace manifests (cached until Cargo.toml / Cargo.lock change) ────────
COPY Cargo.toml Cargo.lock ./
COPY .cargo .cargo

# Per-crate manifests
COPY cerulea-node/Cargo.toml                       cerulea-node/Cargo.toml
COPY cerulea-node/build.rs                         cerulea-node/build.rs
COPY cerulea-node/src/cerulea-consensus/Cargo.toml     cerulea-node/src/cerulea-consensus/Cargo.toml
COPY cerulea-runtime/Cargo.toml                    cerulea-runtime/Cargo.toml
COPY cerulea-runtime/build.rs                      cerulea-runtime/build.rs
COPY cerulea-pallets/pallet-cerulea-poi/Cargo.toml     cerulea-pallets/pallet-cerulea-poi/Cargo.toml
COPY cerulea-pallets/pallet-cerulea-pos/Cargo.toml     cerulea-pallets/pallet-cerulea-pos/Cargo.toml
COPY cerulea-pallets/pallet-cerulea-dcf/Cargo.toml     cerulea-pallets/pallet-cerulea-dcf/Cargo.toml
COPY cerulea-pallets/pallet-cerulea-dvf/Cargo.toml     cerulea-pallets/pallet-cerulea-dvf/Cargo.toml
COPY cerulea-pallets/pallet-todo/Cargo.toml        cerulea-pallets/pallet-todo/Cargo.toml

# tools has bins at the root (no src/ dir) — copy real source, it's tiny
COPY tools/ tools/

# ── Stub out lib crates so `cargo fetch` can resolve the dep graph ────────────
# (tools is already real; cerulea-node main.rs is stubbed separately)
RUN set -e; \
    for crate in \
        cerulea-node/src/cerulea-consensus \
        cerulea-runtime \
        cerulea-pallets/pallet-cerulea-poi \
        cerulea-pallets/pallet-cerulea-pos \
        cerulea-pallets/pallet-cerulea-dcf \
        cerulea-pallets/pallet-cerulea-dvf \
        cerulea-pallets/pallet-todo \
    ; do \
        mkdir -p "$crate/src" && printf '// stub\n' > "$crate/src/lib.rs"; \
    done; \
    mkdir -p cerulea-node/src \
    && printf 'fn main(){}\n' > cerulea-node/src/main.rs \
    && printf '// stub\n'    > cerulea-node/src/lib.rs

RUN cargo fetch --locked

###############################################################################
# Stage 2: Build the real binary
# Inherits the warm dep cache from stage 1 — only recompiles changed crates.
###############################################################################
FROM deps AS builder

COPY cerulea-node       cerulea-node
COPY cerulea-runtime    cerulea-runtime
COPY cerulea-pallets    cerulea-pallets
COPY tools          tools

RUN cargo build --release --locked -p cerulea-node

###############################################################################
# Stage 3: Minimal runtime image (~100 MB vs ~2 GB builder)
###############################################################################
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cerulea-node /usr/local/bin/cerulea-node
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

# Alice's fixed network key — baked in so her peer-id is deterministic across
# restarts and deployments.  Bob and Charlie auto-generate their own keys.
# If keys/alice/secret_ed25519 doesn't exist the entrypoint generates one at
# first boot (useful for local dev without the keys/ directory).
COPY keys/ /etc/cbc/keys/
RUN if [ -f /etc/cbc/keys/alice/secret_ed25519 ]; then \
        mkdir -p /etc/cbc && \
        cp /etc/cbc/keys/alice/secret_ed25519 /etc/cbc/alice_network_key && \
        chmod 600 /etc/cbc/alice_network_key; \
    fi

RUN chmod +x /usr/local/bin/cerulea-node /usr/local/bin/docker-entrypoint.sh

# P2P | RPC | Prometheus
EXPOSE 30333 9944 9615

VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
