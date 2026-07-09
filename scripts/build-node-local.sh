#!/bin/bash

# Build CBC Node locally to avoid Docker build issues
# This script builds the node on your local machine first

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}Building CBC Node Locally${NC}"
echo "=================================================="

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}ERROR: Rust/Cargo not found. Please install Rust first.${NC}"
    echo "Visit: https://rustup.rs/"
    exit 1
fi

# Check if required targets are installed
echo -e "${YELLOW}Checking Rust setup...${NC}"
if ! rustup target list --installed | grep -q "wasm32-unknown-unknown"; then
    echo -e "${YELLOW}Installing WASM target...${NC}"
    rustup target add wasm32-unknown-unknown
fi

if ! rustup component list --installed | grep -q "rust-src"; then
    echo -e "${YELLOW}Installing Rust source component...${NC}"
    rustup component add rust-src
fi

# Check system resources
AVAILABLE_MEMORY=$(free -h | awk 'NR==2{print $7}' 2>/dev/null || echo "Unknown")
AVAILABLE_SPACE=$(df -h . | awk 'NR==2{print $4}')
echo "Available memory: $AVAILABLE_MEMORY"
echo "Available disk space: $AVAILABLE_SPACE"

# Check if binary already exists
if [ -f "target/release/cerulea-node" ]; then
    echo -e "${GREEN}SUCCESS: Found existing binary: target/release/cerulea-node${NC}"
    BINARY_SIZE=$(ls -lh target/release/cerulea-node | awk '{print $5}')
    echo "Binary size: $BINARY_SIZE"
    
    read -p "Do you want to rebuild? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo -e "${BLUE}Using existing binary.${NC}"
        exit 0
    fi
fi

# Build the project
echo -e "${YELLOW}Building CBC Node (this may take 20-30 minutes on first build)...${NC}"
echo -e "${BLUE}Tip: Subsequent builds will be much faster due to incremental compilation.${NC}"

# Set environment variables for better performance
export CARGO_INCREMENTAL=1
export RUST_BACKTRACE=1

# Show progress
echo -e "${YELLOW}Building dependencies and CBC components...${NC}"

# Build with progress indicator
if cargo build --release --bin cerulea-node; then
    echo -e "\n${GREEN}SUCCESS: CBC Node built successfully!${NC}"
    echo "Binary location: target/release/cerulea-node"
    
    # Show binary size
    BINARY_SIZE=$(ls -lh target/release/cerulea-node | awk '{print $5}')
    echo "Binary size: $BINARY_SIZE"
    
    # Test the binary
    echo -e "\n${YELLOW}Testing the binary...${NC}"
    if ./target/release/cerulea-node --version; then
        echo -e "${GREEN}SUCCESS: Binary works correctly!${NC}"
    else
        echo -e "${RED}WARNING: Binary test failed, but continuing...${NC}"
    fi
    
    echo -e "\n${GREEN}Build completed successfully!${NC}"
    echo "=================================================="
    echo -e "${YELLOW}Next steps:${NC}"
    echo "1. Run the node: ./target/release/cerulea-node --dev"
    echo "2. Access RPC endpoints at: http://localhost:9944"
    
else
    echo -e "\n${RED}ERROR: Build failed!${NC}"
    echo -e "${YELLOW}Troubleshooting tips:${NC}"
    echo "1. Ensure you have enough memory (recommended: 8GB+)"
    echo "2. Check disk space (recommended: 10GB+ free)"
    echo "3. Try: cargo clean && ./scripts/build-node-local.sh"
    echo "4. Check Rust version: rustc --version"
    exit 1
fi