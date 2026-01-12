#!/bin/sh
# Setup git hooks for development

set -e

echo "📦 Setting up git hooks..."

# Configure git to use .githooks directory
git config core.hooksPath .githooks

echo "✅ Git hooks configured!"
echo ""
echo "The following hooks are now active:"
echo "  - pre-commit: Auto-formats Rust code with 'cargo fmt'"
echo ""
echo "To disable hooks temporarily, use: git commit --no-verify"
