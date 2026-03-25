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
COPY cbc-node/Cargo.toml                       cbc-node/Cargo.toml
COPY cbc-node/build.rs                         cbc-node/build.rs
COPY cbc-node/src/cbc-consensus/Cargo.toml     cbc-node/src/cbc-consensus/Cargo.toml
COPY cbc-runtime/Cargo.toml                    cbc-runtime/Cargo.toml
COPY cbc-runtime/build.rs                      cbc-runtime/build.rs
COPY cbc-pallets/pallet-cbc-poi/Cargo.toml     cbc-pallets/pallet-cbc-poi/Cargo.toml
COPY cbc-pallets/pallet-cbc-pos/Cargo.toml     cbc-pallets/pallet-cbc-pos/Cargo.toml
COPY cbc-pallets/pallet-cbc-dcf/Cargo.toml     cbc-pallets/pallet-cbc-dcf/Cargo.toml
COPY cbc-pallets/pallet-cbc-dvf/Cargo.toml     cbc-pallets/pallet-cbc-dvf/Cargo.toml
COPY cbc-pallets/pallet-todo/Cargo.toml        cbc-pallets/pallet-todo/Cargo.toml

# tools has bins at the root (no src/ dir) — copy real source, it's tiny
COPY tools/ tools/

# ── Stub out lib crates so `cargo fetch` can resolve the dep graph ────────────
# (tools is already real; cbc-node main.rs is stubbed separately)
RUN set -e; \
    for crate in \
        cbc-node/src/cbc-consensus \
        cbc-runtime \
        cbc-pallets/pallet-cbc-poi \
        cbc-pallets/pallet-cbc-pos \
        cbc-pallets/pallet-cbc-dcf \
        cbc-pallets/pallet-cbc-dvf \
        cbc-pallets/pallet-todo \
    ; do \
        mkdir -p "$crate/src" && printf '// stub\n' > "$crate/src/lib.rs"; \
    done; \
    mkdir -p cbc-node/src \
    && printf 'fn main(){}\n' > cbc-node/src/main.rs \
    && printf '// stub\n'    > cbc-node/src/lib.rs

RUN cargo fetch --locked

###############################################################################
# Stage 2: Build the real binary
# Inherits the warm dep cache from stage 1 — only recompiles changed crates.
###############################################################################
FROM deps AS builder

COPY cbc-node       cbc-node
COPY cbc-runtime    cbc-runtime
COPY cbc-pallets    cbc-pallets
COPY tools          tools

RUN cargo build --release --locked -p cbc-node

###############################################################################
# Stage 3: Minimal runtime image (~100 MB vs ~2 GB builder)
###############################################################################
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    libssl3 ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/cbc-node /usr/local/bin/cbc-node
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

RUN chmod +x /usr/local/bin/cbc-node /usr/local/bin/docker-entrypoint.sh

# P2P | RPC | Prometheus
EXPOSE 30333 9944 9615

VOLUME ["/data"]
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
