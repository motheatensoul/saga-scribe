#!/bin/env bash
# script for building the flatpak locally, needs flatpak with the flathub remote (should be enabled by default on most distros) and flatpak-builder.
# flathub is needed to grab the gnome runtime+sdk and llvm sdk, if you can get them from somewhere else that would be fine too

set -e

# absolute script dir rather than /bin/
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/flatpak-build"
REPO_DIR="$PROJECT_ROOT/flatpak-repo"

cd "$PROJECT_ROOT"

echo "=== Building frontend ==="
bun install --frozen-lockfile
bun run build

echo ""
echo "=== Building Flatpak ==="
flatpak-builder \
    --user \
    --force-clean \
    --install-deps-from=flathub \
    --state-dir="$BUILD_DIR/state" \
    --repo="$REPO_DIR" \
    "$BUILD_DIR/app" \
    "$SCRIPT_DIR/app.sagascribe.Sagascribe.yml"

flatpak build-bundle "$REPO_DIR" "$REPO_DIR/app.sagascribe.Sagascribe.flatpak" app.sagascribe.Sagascribe

echo ""
echo "=== Build complete ==="
echo "Bundle: $REPO_DIR/app.sagascribe.Sagascribe.flatpak"
echo "Install: flatpak install --user $REPO_DIR/app.sagascribe.Sagascribe.flatpak"
