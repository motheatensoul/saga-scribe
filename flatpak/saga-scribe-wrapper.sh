#!/bin/bash
# Wrapper script for Saga Scribe in Flatpak
# Sets APPIMAGE env var so Tauri looks for resources next to the binary
export APPIMAGE=/app/bin/saga-scribe
exec /app/bin/saga-scribe.bin "$@"
