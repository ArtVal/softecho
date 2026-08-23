#!/usr/bin/env bash
# Скачать vosk-win64 (libvosk.dll + .lib) в native/vosk/ для сборки ASR под Windows.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
NATIVE_DIR="$ROOT/native/vosk"
VER="${VOSK_VERSION:-0.3.45}"
URL="https://github.com/alphacep/vosk-api/releases/download/v${VER}/vosk-win64-${VER}.zip"

mkdir -p "$NATIVE_DIR"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "==> Скачиваю $URL"
curl -L --fail --retry 3 -o "$TMP/vosk-win.zip" "$URL"
unzip -o "$TMP/vosk-win.zip" -d "$TMP"

# Архив: vosk-win64-.../libvosk.dll, libvosk.lib, ...
DLL="$(find "$TMP" -name 'libvosk.dll' | head -n1)"
LIB="$(find "$TMP" -name 'libvosk.lib' | head -n1)"
if [[ -z "$DLL" ]]; then
  echo "В архиве нет libvosk.dll" >&2
  exit 1
fi
cp -v "$DLL" "$NATIVE_DIR/libvosk.dll"
if [[ -n "$LIB" ]]; then
  cp -v "$LIB" "$NATIVE_DIR/libvosk.lib"
fi

echo "Готово: $NATIVE_DIR/libvosk.dll"
ls -la "$NATIVE_DIR"/libvosk.*
