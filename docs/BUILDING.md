# CBC-Chain — Build Guide (Linux)

> **Tested on:** Ubuntu 25.04 (Plucky) with gcc-15, clang-18, Rust 1.85.1  
> **Build time:** ~55 minutes (first build, single core) · ~5–10 min (incremental)

---

## Quick Start

```bash
# 1. Clone the repo
git clone https://github.com/CAERULEAN-BYTECHAINS-PRIVATE-LIMITED/CBC-Chain.git
cd CBC-Chain

# 2. Install system dependencies (one-time)
./scripts/setup-build-env.sh

# 3. Build the node
./scripts/build-node-local.sh
```

Binary output: `target/release/cbc-node`

---

## System Requirements

| Component | Minimum | Recommended |
|-----------|---------|-------------|
| RAM | 8 GB | 16 GB |
| Disk (free) | 20 GB | 40 GB |
| CPU cores | 2 | 8+ |
| OS | Ubuntu 22.04+ | Ubuntu 24.04 / 25.04 |

---

## Step-by-Step Manual Setup

If you prefer to run each step yourself instead of the setup script:

### 1 — System Packages

```bash
sudo apt-get update
sudo apt-get install -y \
    build-essential \
    clang \
    libclang-dev \
    llvm \
    protobuf-compiler \
    libssl-dev \
    pkg-config \
    libjemalloc-dev \
    libzstd-dev \
    liblz4-dev \
    zlib1g-dev \
    libbz2-dev \
    curl
```

### 2 — Rust Toolchain

```bash
# Install rustup (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"

# Install pinned toolchain (rust-toolchain.toml handles this automatically,
# but running it explicitly ensures components are present)
rustup toolchain install 1.85 \
    --target wasm32-unknown-unknown \
    --component rust-src \
    --component rustfmt \
    --component clippy

# Verify
rustc --version   # should show rustc 1.85.x
cargo --version
```

### 3 — Build

```bash
cargo build --release --bin cbc-node
```

---

## Known Issues & Fixes Already Applied

These issues were encountered during setup and are **already fixed** in the repo files.
You do **not** need to do anything — this section documents *why* the files are the way they are.

---

### ❌ Issue 1: `uint64_t` / `uint32_t` unknown type in librocksdb-sys

**Symptom:**
```
error: unknown type name 'uint64_t'
rocksdb/db/blob/blob_file_meta.h:28:7
error: unknown type name 'uint64_t'
rocksdb/include/rocksdb/trace_record.h:55:24
fatal error: too many errors emitted, stopping now
```

**Root Cause:**  
`librocksdb-sys v0.11.0+8.1.1` (RocksDB 8.1.1) ships C++ headers that use
`uint64_t` / `uint32_t` without `#include <cstdint>`, relying on those types
being pulled in transitively (e.g. through `<string>` or `<cassert>`).

`gcc-15` and `clang-18` — both shipped with Ubuntu 24.04+ — tightened their
libstdc++ headers and no longer expose these types transitively. Any C++ file
that doesn't explicitly include `<cstdint>` will fail.

**Fix applied in [`.cargo/config.toml`](.cargo/config.toml):**
```toml
[env]
CXXFLAGS = "-include cstdint"
```

This tells `cc-rs` (the Rust build helper that invokes the C++ compiler) to
prepend `-include cstdint` to every compilation unit, injecting the missing
type definitions globally.

---

### ❌ Issue 2: Broken LLVM apt repository

**Symptom:**
```
E: The repository 'https://apt.llvm.org/resolute llvm-toolchain-resolute-14 Release'
   does not have a Release file.
```

**Root Cause:**  
A stale apt source file was left behind from a previous LLVM 14 installation
attempt on an older Ubuntu. The codename `resolute` does not correspond to any
real Ubuntu release. The file is:
```
/etc/apt/sources.list.d/archive_uri-https_apt_llvm_org_resolute_-resolute.list
```

**Fix:**
```bash
sudo rm -f /etc/apt/sources.list.d/archive_uri-https_apt_llvm_org_resolute_-resolute.list
```

The setup script does this automatically. The system's built-in `clang-18`
from Ubuntu's main repos is all that's needed — no external LLVM PPA is required.

---

### ❌ Issue 3: Wrong Rust toolchain version

**Symptom:** Subtle compilation errors or behavior differences from the
Docker-based CI build, which uses `rust:1.85-bookworm`.

**Fix applied in [`rust-toolchain.toml`](rust-toolchain.toml):**
```toml
[toolchain]
channel = "1.85"
targets = ["wasm32-unknown-unknown", "x86_64-unknown-linux-gnu"]
components = ["rustfmt", "clippy", "rust-src"]
```

`rustup` reads this file automatically whenever you run any `cargo` or `rustc`
command inside the project directory, so the correct toolchain is always used
regardless of what your global default is.

---

### ⚠️ Warning: wasm32v1-none target (non-blocking)

**Symptom (build warning, not an error):**
```
warning: You are building WASM runtime using `wasm32-unknown-unknown` target,
although Rust >= 1.84 supports `wasm32v1-none` target!
```

**Explanation:**  
This is informational only — the build still succeeds and the binary is correct.
To silence it and use the newer WASM target:

```bash
rustup target add wasm32v1-none --toolchain 1.85-x86_64-unknown-linux-gnu
cargo clean
cargo build --release --bin cbc-node
```

> **Note:** `cargo clean` is required after adding the new target, as the WASM
> runtime blob must be rebuilt from scratch.

---

## Repo Files Changed During This Setup

| File | Change | Purpose |
|------|--------|---------|
| `.cargo/config.toml` | Added `[env] CXXFLAGS = "-include cstdint"` | Fix librocksdb-sys C++ build with gcc-15/clang-18 |
| `rust-toolchain.toml` | Created | Pin Rust to 1.85, matching Dockerfile |
| `scripts/setup-build-env.sh` | Created | One-shot setup script for new machines |

---

## Rebuilding After Code Changes

```bash
# Regular incremental rebuild (fast, ~1–5 min depending on changes)
cargo build --release --bin cbc-node

# Full clean rebuild (slow, use only when necessary)
cargo clean && cargo build --release --bin cbc-node

# Clean only the rocksdb C++ artifacts (useful if tweaking CXXFLAGS)
find target/release/build -name "librocksdb-sys-*" -type d -exec rm -rf {} + 2>/dev/null || true
cargo build --release --bin cbc-node
```

---

## Running the Node

```bash
# Development mode (single node, no peers needed)
./target/release/cbc-node --dev

# Check version
./target/release/cbc-node --version

# RPC endpoint (once running)
# WebSocket: ws://localhost:9944
# HTTP:      http://localhost:9944
```

---

## Docker Build (Alternative)

The `Dockerfile` in the repo root builds a production image using the same
toolchain and fixes described above:

```bash
docker build -t cbc-node:latest .
docker run --rm cbc-node:latest --version
```

---

## Troubleshooting Checklist

| Problem | Check |
|---------|-------|
| `uint64_t` errors in rocksdb | `.cargo/config.toml` has `CXXFLAGS = "-include cstdint"` |
| `apt-get update` fails | Remove stale LLVM source: `sudo rm /etc/apt/sources.list.d/archive_uri-https_apt_llvm_org_resolute_-resolute.list` |
| Wrong Rust version active | `rust-toolchain.toml` present in repo root? Run `rustup show` to verify |
| OOM / killed during build | Increase swap or reduce parallel jobs in `.cargo/config.toml` (`jobs = 1` is already set) |
| `protoc` not found | `sudo apt-get install protobuf-compiler` |
| `libssl` not found | `sudo apt-get install libssl-dev pkg-config` |
| WASM build fails | `rustup component add rust-src` and `rustup target add wasm32-unknown-unknown` |
