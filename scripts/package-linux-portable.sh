#!/usr/bin/env bash
# Собрать portable-папку под Linux (x86_64).
#
#   ./scripts/package-linux-portable.sh           # текст
#   ./scripts/package-linux-portable.sh --asr     # с голосом
#   INCLUDE_MODEL=1 ./scripts/package-linux-portable.sh --asr
#   SKIP_BUILD=1 ./scripts/package-linux-portable.sh --asr
#
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

WITH_ASR=0
for arg in "$@"; do
  case "$arg" in
    --asr) WITH_ASR=1 ;;
    -h|--help)
      sed -n '2,12p' "$0"
      exit 0
      ;;
  esac
done

FEATURES=()
NAME_SUFFIX="text"
if [[ "$WITH_ASR" -eq 1 ]]; then
  FEATURES=(--features asr)
  NAME_SUFFIX="asr"
  if [[ ! -f "$ROOT/native/vosk/libvosk.so" ]]; then
    echo "==> Нет native/vosk/libvosk.so — качаю"
    "$ROOT/scripts/fetch-vosk-linux.sh"
  fi
fi

TARGET="${CARGO_BUILD_TARGET:-}"
if [[ -n "$TARGET" ]]; then
  BIN_DIR="$ROOT/target/$TARGET/release"
else
  BIN_DIR="$ROOT/target/release"
fi
BIN="$BIN_DIR/softecho"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "==> Сборка release (asr=$WITH_ASR, target=${TARGET:-host})"
  if [[ -n "$TARGET" ]]; then
    cargo build --release "${FEATURES[@]}" --target "$TARGET"
  else
    cargo build --release "${FEATURES[@]}"
  fi
fi

if [[ ! -f "$BIN" ]]; then
  echo "Нет $BIN" >&2
  exit 1
fi

OUT="$ROOT/dist/softecho-linux-x86_64-${NAME_SUFFIX}"
rm -rf "$OUT"
mkdir -p "$OUT"
cp -v "$BIN" "$OUT/"

if [[ "$WITH_ASR" -eq 1 ]]; then
  if [[ -f "$BIN_DIR/libvosk.so" ]]; then
    cp -v "$BIN_DIR/libvosk.so" "$OUT/"
  elif [[ -f "$ROOT/native/vosk/libvosk.so" ]]; then
    cp -v "$ROOT/native/vosk/libvosk.so" "$OUT/"
  fi
fi

if [[ "${INCLUDE_MODEL:-0}" == "1" ]]; then
  MODEL_SRC="$ROOT/assets/vosk/vosk-model-small-ru-0.22"
  if [[ -d "$MODEL_SRC" ]]; then
    echo "==> Копирую модель"
    mkdir -p "$OUT/vosk-model-small-ru-0.22"
    cp -a "$MODEL_SRC"/. "$OUT/vosk-model-small-ru-0.22/"
  fi
fi

cat > "$OUT/ЧТОБЫ_ЗАПУСТИТЬ.txt" << 'EOF'
SoftEcho — portable (Linux x86_64)

1. Распакуйте архив целиком.
2. Запустите ./softecho из этой папки (chmod +x softecho при необходимости).

Голос (ASR):
- Рядом с бинарником: libvosk.so
- Модель: Настройки → «Скачать модель» (интернет один раз)
  или папка vosk-model-small-ru-0.22 рядом с softecho
  https://alphacephei.com/vosk/models (vosk-model-small-ru-0.22)
  или ~/.local/share/softecho/vosk-model-small-ru-0.22

Без модели/libvosk.so — текстовый режим. Установщик не нужен.
EOF

mkdir -p "$ROOT/dist"
ARCHIVE="$ROOT/dist/softecho-linux-x86_64-${NAME_SUFFIX}.tar.gz"
rm -f "$ARCHIVE"
tar -czf "$ARCHIVE" -C "$ROOT/dist" "$(basename "$OUT")"

echo
echo "Готово: $OUT/"
echo "        $ARCHIVE"
ls -lh "$BIN" "$ARCHIVE" 2>/dev/null || true
