#!/bin/bash
# sync-version.sh - Synchronize version across all packages
#
# Usage: ./scripts/sync-version.sh 0.4.0
#
# Updates:
#   - Rust: workspace Cargo.toml + all crate Cargo.toml files
#   - NPM:  ordo-editor packages (core/vue/react/wasm) + apps/studio
#   - TypeScript VERSION constants
#   - ordo-web: package.json + i18n badge strings
#   - VitePress docs: version badge in config

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.4.0"
    exit 1
fi

VERSION="$1"
ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WEB_DIR="$(cd "$ROOT_DIR/../ordo-web" 2>/dev/null && pwd)" || WEB_DIR=""

echo "Syncing version to $VERSION..."
echo ""

# ── Rust Cargo.toml files ─────────────────────────────────────────────────────

echo "Updating Cargo.toml files..."

# Workspace root: version + ordo-derive dep reference
sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$ROOT_DIR/Cargo.toml"
sed -i.bak "s/ordo-derive = { version = \"[^\"]*\"/ordo-derive = { version = \"$VERSION\"/" "$ROOT_DIR/Cargo.toml"

# Individual crates
for crate in ordo-core ordo-server ordo-proto ordo-wasm ordo-derive ordo-platform; do
    TOML="$ROOT_DIR/crates/$crate/Cargo.toml"
    if [ -f "$TOML" ]; then
        sed -i.bak "s/^version = \"[^\"]*\"/version = \"$VERSION\"/" "$TOML"
        echo "  Updated crates/$crate/Cargo.toml"
    fi
done

# Internal crate dep references in ordo-server
sed -i.bak "s/ordo-core = { version = \"[^\"]*\"/ordo-core = { version = \"$VERSION\"/" \
    "$ROOT_DIR/crates/ordo-server/Cargo.toml"
sed -i.bak "s/ordo-proto = { version = \"[^\"]*\"/ordo-proto = { version = \"$VERSION\"/" \
    "$ROOT_DIR/crates/ordo-server/Cargo.toml"

# Clean up Rust backup files
find "$ROOT_DIR/crates" "$ROOT_DIR" -maxdepth 1 -name "*.bak" -delete

# ── NPM package.json files ────────────────────────────────────────────────────

echo ""
echo "Updating package.json files..."

NPM_TARGETS=(
    "$ROOT_DIR/ordo-editor/packages/core"
    "$ROOT_DIR/ordo-editor/packages/vue"
    "$ROOT_DIR/ordo-editor/packages/react"
    "$ROOT_DIR/ordo-editor/packages/wasm"
    "$ROOT_DIR/ordo-editor/apps/studio"
    "$ROOT_DIR/ordo-editor"
)

for pkg in "${NPM_TARGETS[@]}"; do
    if [ -f "$pkg/package.json" ]; then
        node -e "
            const fs = require('fs');
            const p = JSON.parse(fs.readFileSync('$pkg/package.json', 'utf8'));
            p.version = '$VERSION';
            fs.writeFileSync('$pkg/package.json', JSON.stringify(p, null, 2) + '\n');
        "
        echo "  Updated $pkg/package.json"
    fi
done

# ── TypeScript VERSION constants ──────────────────────────────────────────────

echo ""
echo "Updating TypeScript VERSION constants..."

for ts_file in \
    "$ROOT_DIR/ordo-editor/packages/core/src/index.ts" \
    "$ROOT_DIR/ordo-editor/packages/vue/src/index.ts"; do
    if [ -f "$ts_file" ]; then
        sed -i.bak "s/VERSION = '[^']*'/VERSION = '$VERSION'/" "$ts_file"
        sed -i.bak "s/VERSION = \"[^\"]*\"/VERSION = \"$VERSION\"/" "$ts_file"
        echo "  Updated $ts_file"
    fi
done

find "$ROOT_DIR/ordo-editor" -name "*.bak" -delete

# ── ordo-web ──────────────────────────────────────────────────────────────────

if [ -n "$WEB_DIR" ] && [ -d "$WEB_DIR" ]; then
    echo ""
    echo "Updating ordo-web..."

    # package.json
    if [ -f "$WEB_DIR/package.json" ]; then
        node -e "
            const fs = require('fs');
            const p = JSON.parse(fs.readFileSync('$WEB_DIR/package.json', 'utf8'));
            p.version = '$VERSION';
            fs.writeFileSync('$WEB_DIR/package.json', JSON.stringify(p, null, 2) + '\n');
        "
        echo "  Updated ordo-web/package.json"
    fi

    # i18n badge strings: "v0.x.y Released" / "v0.x.y 已发布" / "v0.x.y 已發布"
    find "$WEB_DIR/src/i18n" -name "*.json" 2>/dev/null | while read f; do
        sed -i.bak "s/v[0-9]\+\.[0-9]\+\.[0-9]\+ Released/v$VERSION Released/" "$f"
        sed -i.bak "s/v[0-9]\+\.[0-9]\+\.[0-9]\+ 已发布/v$VERSION 已发布/" "$f"
        sed -i.bak "s/v[0-9]\+\.[0-9]\+\.[0-9]\+ 已發布/v$VERSION 已發布/" "$f"
        echo "  Updated $f"
    done
    find "$WEB_DIR/src" -name "*.bak" -delete
else
    echo ""
    echo "  ordo-web not found at $WEB_DIR, skipping"
fi

# ── VitePress docs ────────────────────────────────────────────────────────────

DOCS_CONFIG="$ROOT_DIR/ordo-editor/apps/docs/.vitepress/config.mts"
if [ -f "$DOCS_CONFIG" ]; then
    echo ""
    echo "Updating VitePress docs..."
    sed -i.bak "s/v[0-9]\+\.[0-9]\+\.[0-9]\+/v$VERSION/g" "$DOCS_CONFIG"
    find "$ROOT_DIR/ordo-editor/apps/docs/.vitepress" -name "*.bak" -delete
    echo "  Updated $DOCS_CONFIG"
fi

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "✓ Version synced to $VERSION"
echo ""
echo "Next steps:"
echo "  1. Review changes : git diff"
echo "  2. Commit         : git commit -am 'chore: bump version to $VERSION'"
echo "  3. Tag            : git tag v$VERSION"
echo "  4. Push           : git push && git push --tags"
