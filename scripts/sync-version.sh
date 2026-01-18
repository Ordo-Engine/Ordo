#!/bin/bash
# sync-version.sh - Synchronize version across all packages
#
# Usage: ./scripts/sync-version.sh 0.2.0
#
# This script updates version numbers in:
# - Cargo.toml files (Rust crates)
# - package.json files (NPM packages)
# - VERSION constants in TypeScript

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.2.0"
    exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "Syncing version to $VERSION..."

# Update root Cargo.toml
echo "Updating Cargo.toml files..."
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/Cargo.toml"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-core/Cargo.toml"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-server/Cargo.toml"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-proto/Cargo.toml"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-wasm/Cargo.toml"
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-derive/Cargo.toml"

# Update internal crate dependencies
sed -i.bak "s/ordo-core = { version = \"[^\"]*\"/ordo-core = { version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-server/Cargo.toml"
sed -i.bak "s/ordo-proto = { version = \"[^\"]*\"/ordo-proto = { version = \"$VERSION\"/" "$ROOT_DIR/crates/ordo-server/Cargo.toml"

# Update NPM package.json files
echo "Updating package.json files..."
for pkg in "$ROOT_DIR/ordo-editor/packages/core" \
           "$ROOT_DIR/ordo-editor/packages/vue" \
           "$ROOT_DIR/ordo-editor/packages/react" \
           "$ROOT_DIR/ordo-editor/packages/wasm" \
           "$ROOT_DIR/ordo-editor"; do
    if [ -f "$pkg/package.json" ]; then
        # Use node to update JSON properly
        node -e "
            const fs = require('fs');
            const pkg = JSON.parse(fs.readFileSync('$pkg/package.json', 'utf8'));
            pkg.version = '$VERSION';
            fs.writeFileSync('$pkg/package.json', JSON.stringify(pkg, null, 2) + '\n');
        "
        echo "  Updated $pkg/package.json"
    fi
done

# Update VERSION constants in TypeScript
echo "Updating TypeScript VERSION constants..."
sed -i.bak "s/VERSION = '[^']*'/VERSION = '$VERSION'/" "$ROOT_DIR/ordo-editor/packages/core/src/index.ts"
sed -i.bak "s/VERSION = '[^']*'/VERSION = '$VERSION'/" "$ROOT_DIR/ordo-editor/packages/vue/src/index.ts"

# Clean up backup files
find "$ROOT_DIR" -name "*.bak" -delete

echo ""
echo "Version synced to $VERSION"
echo ""
echo "Files updated:"
echo "  - Cargo.toml (root and crates)"
echo "  - ordo-editor/package.json"
echo "  - ordo-editor/packages/*/package.json"
echo "  - TypeScript VERSION constants"
echo ""
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Commit: git commit -am 'chore: bump version to $VERSION'"
echo "  3. Tag: git tag v$VERSION"
echo "  4. Push: git push && git push --tags"
