#!/usr/bin/env bash
# Скачать libvosk.so (Linux x86_64) в native/vosk/ для сборки ASR.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NATIVE_DIR="$ROOT/native/vosk"
VER="${VOSK_VERSION:-0.3.45}"
URL="https://github.com/alphacep/vosk-api/releases/download/v${VER}/vosk-linux-x86_64-${VER}.zip"

mkdir -p "$NATIVE_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "==> Скачиваю $URL"
curl -L --fail --retry 3 -o "$TMP/vosk.zip" "$URL"
unzip -o "$TMP/vosk.zip" -d "$TMP"

SO="$(find "$TMP" -name 'libvosk.so' | head -n1)"
if [[ -z "$SO" ]]; then
  echo "В архиве нет libvosk.so" >&2
  exit 1
fi
cp -v "$SO" "$NATIVE_DIR/libvosk.so"

echo "Готово: $NATIVE_DIR/libvosk.so"
ls -la "$NATIVE_DIR/libvosk.so"
