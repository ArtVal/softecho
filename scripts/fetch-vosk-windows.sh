#!/usr/bin/env bash
# Скачать vosk-win64 в native/vosk/ для ASR под Windows (все .dll из архива).
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

if ! find "$TMP" -name 'libvosk.dll' | grep -q .; then
  echo "В архиве нет libvosk.dll" >&2
  exit 1
fi

echo "==> Копирую все .dll из vosk-win64 (MinGW runtime + libvosk)"
while IFS= read -r -d '' dll; do
  cp -v "$dll" "$NATIVE_DIR/$(basename "$dll")"
done < <(find "$TMP" -type f -name '*.dll' -print0)

LIB="$(find "$TMP" -name 'libvosk.lib' | head -n1)"
if [[ -n "$LIB" ]]; then
  cp -v "$LIB" "$NATIVE_DIR/libvosk.lib"
fi

echo "Готово: $NATIVE_DIR/"
ls -la "$NATIVE_DIR"/*.dll "$NATIVE_DIR"/libvosk.lib 2>/dev/null || ls -la "$NATIVE_DIR"
