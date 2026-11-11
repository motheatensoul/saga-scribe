#!/usr/bin/env bash
# Downloads the Menota catalogue (texts and charters) for the opt-in corpus
# tests, into tests/corpus/menota (not committed: the texts' licences differ).
#
#   scripts/fetch-menota-corpus.sh
#   cd src-tauri; MENOTA_CORPUS=../tests/corpus/menota cargo test --release corpus_ -- --nocapture
#
# The archives are what the catalogue's "Download all XML files" gives
# (https://clarino.uib.no/menota/catalogue).
set -euo pipefail

dest="$(cd "$(dirname "$0")/.." && pwd)/tests/corpus/menota"
mkdir -p "$dest"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

for corpus in menota menota-diploma; do
    echo "Downloading $corpus..."
    curl -fsSL --max-time 600 -o "$tmp/$corpus.zip" \
        "https://clarino.uib.no/korpuskel-api/download-zip?corpus=$corpus"
    rm -rf "${dest:?}/$corpus"
    mkdir -p "$dest/$corpus"
    unzip -q "$tmp/$corpus.zip" -d "$dest/$corpus"
    echo "  $(find "$dest/$corpus" -name '*.xml' | wc -l) files in $dest/$corpus"
done
