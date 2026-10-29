#!/bin/bash
# Build the Saga Scribe Flatpak locally, the way CI does: the frontend is
# built here with Bun, the app itself inside the GNOME SDK.
#
# Usage: ./flatpak/build-flatpak.sh
#
# Prerequisites: flatpak-builder, and the Flathub remote. The GNOME 51
# runtime/SDK and the Rust and LLVM SDK extensions (branch 26.08) are
# installed from Flathub as needed.

set -e

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
