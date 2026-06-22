#!/bin/bash
# setup-build-env.sh - Fix build environment for CBC-Chain on Ubuntu with modern toolchains
# Run this ONCE before building. It installs required system packages and Rust toolchain.

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  CBC-Chain Build Environment Setup     ${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# ── 1. System dependencies ─────────────────────────────────────────────────────
echo -e "${YELLOW}[1/4] Installing system dependencies...${NC}"

# Remove broken LLVM apt source (references non-existent 'resolute' Ubuntu codename)
BROKEN_LLVM_SRC="/etc/apt/sources.list.d/archive_uri-https_apt_llvm_org_resolute_-resolute.list"
if [ -f "$BROKEN_LLVM_SRC" ]; then
    echo -e "${YELLOW}Removing stale LLVM apt source (broken 'resolute' codename)...${NC}"
    sudo rm -f "$BROKEN_LLVM_SRC"
fi

sudo apt-get update -qq
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
echo -e "${GREEN}✓ System dependencies installed${NC}"

# ── 2. Rust toolchain ──────────────────────────────────────────────────────────
echo -e "${YELLOW}[2/4] Setting up Rust toolchain...${NC}"

# Install rustup if not present
if ! command -v rustup &>/dev/null; then
    echo "Installing rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    source "$HOME/.cargo/env"
fi

# Install pinned toolchain from rust-toolchain.toml (1.85)
rustup show active-toolchain || true
rustup toolchain install 1.85 \
    --target wasm32-unknown-unknown \
    --component rust-src \
    --component rustfmt \
    --component clippy

# Add wasm target to the active toolchain too
rustup target add wasm32-unknown-unknown
rustup component add rust-src

echo -e "${GREEN}✓ Rust toolchain ready: $(rustc --version)${NC}"

# ── 3. Verify the CXXFLAGS fix is in .cargo/config.toml ───────────────────────
echo -e "${YELLOW}[3/4] Verifying build configuration...${NC}"
CONFIG=".cargo/config.toml"
if ! grep -q "CXXFLAGS" "$CONFIG" 2>/dev/null; then
    echo -e "${RED}ERROR: CXXFLAGS fix not found in $CONFIG${NC}"
    echo "Please ensure .cargo/config.toml contains:"
    echo '[env]'
    echo 'CXXFLAGS = "-include cstdint"'
    exit 1
fi
echo -e "${GREEN}✓ CXXFLAGS fix present in $CONFIG${NC}"

# ── 4. Clean stale build artifacts ────────────────────────────────────────────
echo -e "${YELLOW}[4/4] Cleaning stale rocksdb build artifacts...${NC}"
if [ -d "target/release/build" ]; then
    find target/release/build -name "librocksdb-sys-*" -type d -exec rm -rf {} + 2>/dev/null || true
    echo -e "${GREEN}✓ Stale librocksdb-sys artifacts cleaned${NC}"
fi

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Setup complete! Ready to build.       ${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo -e "${YELLOW}Now run:${NC}"
echo "  ./scripts/build-node-local.sh"
echo ""
echo -e "${BLUE}What was fixed:${NC}"
echo "  • librocksdb-sys v0.11.0+8.1.1 uses uint64_t/uint32_t in headers"
echo "    without including <cstdint>. gcc-15 and clang-18 no longer expose"
echo "    these types via transitive includes from <string>, <cassert>, etc."
echo "  • Fix: CXXFLAGS=\"-include cstdint\" forces the include in all C++ units"
echo "  • Rust pinned to 1.85 (matching Dockerfile) via rust-toolchain.toml"
