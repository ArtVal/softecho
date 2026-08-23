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

# Архив: vosk-win64-.../libvosk.dll + MinGW runtime (без них exe падает на чужой машине).
copy_from_zip() {
  local name="$1"
  local src
  src="$(find "$TMP" -name "$name" | head -n1)"
  if [[ -z "$src" ]]; then
    echo "В архиве нет $name" >&2
    return 1
  fi
  cp -v "$src" "$NATIVE_DIR/$name"
}

copy_from_zip libvosk.dll
copy_from_zip libwinpthread-1.dll
copy_from_zip libgcc_s_seh-1.dll
copy_from_zip 'libstdc++-6.dll'
LIB="$(find "$TMP" -name 'libvosk.lib' | head -n1)"
if [[ -n "$LIB" ]]; then
  cp -v "$LIB" "$NATIVE_DIR/libvosk.lib"
fi

echo "Готово: $NATIVE_DIR/ (libvosk + MinGW runtime)"
ls -la "$NATIVE_DIR"/*.dll "$NATIVE_DIR"/libvosk.lib 2>/dev/null || ls -la "$NATIVE_DIR"
