#!/usr/bin/env bash
# Скачать libvosk.dylib (macOS universal2) в native/vosk/ для сборки ASR.
# Официальных бинарников Vosk 0.3.45 под macOS нет — берём 0.3.42 (universal2).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NATIVE_DIR="$ROOT/native/vosk"
VER="${VOSK_MACOS_VERSION:-0.3.42}"
URL="https://github.com/alphacep/vosk-api/releases/download/v${VER}/vosk-osx-${VER}.zip"

mkdir -p "$NATIVE_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "==> Скачиваю $URL"
curl -L --fail --retry 3 -o "$TMP/vosk.zip" "$URL"
unzip -o "$TMP/vosk.zip" -d "$TMP"

DYLIB="$(find "$TMP" -name 'libvosk.dylib' | head -n1)"
if [[ -z "$DYLIB" ]]; then
  echo "В архиве нет libvosk.dylib" >&2
  exit 1
fi
cp -v "$DYLIB" "$NATIVE_DIR/libvosk.dylib"

echo "Готово: $NATIVE_DIR/libvosk.dylib (Vosk $VER, universal2)"
ls -la "$NATIVE_DIR/libvosk.dylib"
