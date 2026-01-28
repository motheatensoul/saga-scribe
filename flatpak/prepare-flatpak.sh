#!/bin/bash
# Script to prepare the Flatpak build directory from Tauri build artifacts
# Usage: ./prepare-flatpak.sh <path-to-tauri-bundle> <output-dir>

set -e

BUNDLE_DIR="${1:-src-tauri/target/release}"
OUTPUT_DIR="${2:-flatpak-build}"

echo "Preparing Flatpak build from $BUNDLE_DIR to $OUTPUT_DIR"

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Copy the binary
if [ -f "$BUNDLE_DIR/saga-scribe" ]; then
    cp "$BUNDLE_DIR/saga-scribe" "$OUTPUT_DIR/"
    echo "Copied binary: saga-scribe"
else
    echo "Error: Binary not found at $BUNDLE_DIR/saga-scribe"
    exit 1
fi

# Copy bundled resources from the bundle directory
# Tauri places resources in the bundle based on tauri.conf.json
RESOURCE_DIR="$BUNDLE_DIR"

# Copy resource directories
for dir in entities normalizer dictionary schemas; do
    if [ -d "static/$dir" ]; then
        cp -r "static/$dir" "$OUTPUT_DIR/"
        echo "Copied resource directory: $dir"
    else
        echo "Warning: Resource directory static/$dir not found"
    fi
done

# Copy Flatpak metadata files
cp flatpak/app.sagascribe.Sagascribe.desktop "$OUTPUT_DIR/"
cp flatpak/app.sagascribe.Sagascribe.metainfo.xml "$OUTPUT_DIR/"
cp flatpak/saga-scribe-wrapper.sh "$OUTPUT_DIR/"

# Copy icons (resize if needed)
# The main icon is 512x512
cp src-tauri/icons/icon.png "$OUTPUT_DIR/icon-512.png"
cp src-tauri/icons/128x128@2x.png "$OUTPUT_DIR/icon-256.png"
cp src-tauri/icons/128x128.png "$OUTPUT_DIR/icon-128.png"

echo "Flatpak build directory prepared at $OUTPUT_DIR"
ls -la "$OUTPUT_DIR"
