#!/bin/bash
# Build Flatpak package for Saga Scribe
# Usage: ./build-flatpak.sh [--skip-tauri-build]
#
# Prerequisites:
#   - flatpak-builder installed
#   - org.gnome.Platform//49 and org.gnome.Sdk//49 installed:
#     flatpak install flathub org.gnome.Platform//49 org.gnome.Sdk//49

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$PROJECT_ROOT/flatpak-build"
REPO_DIR="$PROJECT_ROOT/flatpak-repo"

cd "$PROJECT_ROOT"

SKIP_TAURI_BUILD=false
for arg in "$@"; do
    case $arg in
    --skip-tauri-build)
        SKIP_TAURI_BUILD=true
        shift
        ;;
    esac
done

echo "=== Saga Scribe Flatpak Build ==="
echo "Project root: $PROJECT_ROOT"
echo "Build directory: $BUILD_DIR"

# Step 1: Build Tauri application (release mode)
if [ "$SKIP_TAURI_BUILD" = false ]; then
    echo ""
    echo "=== Step 1: Building Tauri application ==="
    bun run tauri:build
else
    echo ""
    echo "=== Step 1: Skipping Tauri build (--skip-tauri-build) ==="
fi

# Verify binary exists
if [ ! -f "src-tauri/target/release/saga-scribe" ]; then
    echo "Error: Tauri binary not found at src-tauri/target/release/saga-scribe"
    echo "Run without --skip-tauri-build to build first"
    exit 1
fi

# Step 2: Prepare Flatpak build directory
echo ""
echo "=== Step 2: Preparing Flatpak build directory ==="
rm -rf "$BUILD_DIR"
"$SCRIPT_DIR/prepare-flatpak.sh" "src-tauri/target/release" "$BUILD_DIR"

# Step 3: Copy manifest to build directory
echo ""
echo "=== Step 3: Setting up manifest ==="
cp "$SCRIPT_DIR/app.sagascribe.Sagascribe.yml" "$BUILD_DIR/"

# Step 4: Build Flatpak
echo ""
echo "=== Step 4: Building Flatpak ==="
cd "$BUILD_DIR"

flatpak-builder \
    --force-clean \
    --repo="$REPO_DIR" \
    --install-deps-from=flathub \
    build-dir \
    app.sagascribe.Sagascribe.yml

echo ""
echo "=== Build complete ==="
echo "Flatpak repository created at: $REPO_DIR"
echo ""
echo "To install locally:"
echo "  flatpak --user remote-add --no-gpgsign saga-scribe-local $REPO_DIR"
echo "  flatpak --user install saga-scribe-local app.sagascribe.Sagascribe"
echo ""
echo "To create a distributable bundle:"
echo "  flatpak build-bundle $REPO_DIR saga-scribe.flatpak app.sagascribe.Sagascribe"
flatpak build-bundle $REPO_DIR $REPO_DIR/app.sagascribe.Sagascribe.flatpak app.sagascribe.Sagascribe
