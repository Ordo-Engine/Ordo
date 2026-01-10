#!/bin/bash
# Build script for ordo-wasm
# Compiles Rust to WebAssembly and generates TypeScript bindings

set -e

echo "Building ordo-wasm..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "Error: wasm-pack is not installed"
    echo "Install it with: cargo install wasm-pack"
    exit 1
fi

# Build for web target
wasm-pack build \
    --target web \
    --out-dir ../../ordo-editor/packages/wasm/dist \
    --out-name ordo_wasm \
    --release

echo "Build complete! Output in ordo-editor/packages/wasm/dist"
echo ""
echo "Files generated:"
ls -lh ../../ordo-editor/packages/wasm/dist

