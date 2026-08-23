#!/usr/bin/env bash
# Собрать portable-папку под Windows (x86_64).
#
#   ./scripts/package-windows-portable.sh           # текст
#   ./scripts/package-windows-portable.sh --asr     # с голосом
#   INCLUDE_MODEL=1 ./scripts/package-windows-portable.sh --asr
#   SKIP_BUILD=1 ./scripts/package-windows-portable.sh --asr   # только упаковать
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

TARGET="${CARGO_BUILD_TARGET:-}"
HOST_OS="$(uname -s 2>/dev/null || echo unknown)"

pick_target() {
  if [[ -n "${CARGO_BUILD_TARGET:-}" ]]; then
    echo "$CARGO_BUILD_TARGET"
    return
  fi
  case "$HOST_OS" in
    MINGW*|MSYS*|CYGWIN*)
      echo ""
      ;;
    *)
      if rustup target list --installed 2>/dev/null | grep -qx 'x86_64-pc-windows-msvc'; then
        echo "x86_64-pc-windows-msvc"
      elif rustup target list --installed 2>/dev/null | grep -qx 'x86_64-pc-windows-gnu'; then
        echo "x86_64-pc-windows-gnu"
      else
        echo ""
      fi
      ;;
  esac
}

TARGET="$(pick_target)"
FEATURES=()
NAME_SUFFIX="text"
if [[ "$WITH_ASR" -eq 1 ]]; then
  FEATURES=(--features asr)
  NAME_SUFFIX="asr"
  if [[ ! -f "$ROOT/native/vosk/libvosk.dll" ]]; then
    echo "==> Нет native/vosk/libvosk.dll — качаю"
    "$ROOT/scripts/fetch-vosk-windows.sh"
  fi
fi

if [[ -n "$TARGET" ]]; then
  BIN_DIR="$ROOT/target/$TARGET/release"
else
  BIN_DIR="$ROOT/target/release"
fi
EXE="$BIN_DIR/softecho.exe"

if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
  echo "==> Сборка release (asr=$WITH_ASR, target=${TARGET:-host})"
  if [[ -n "$TARGET" ]]; then
    if [[ "$TARGET" == *windows-msvc* ]] && [[ "$HOST_OS" != MINGW* && "$HOST_OS" != MSYS* && "$HOST_OS" != CYGWIN* ]]; then
      if command -v cargo-xwin >/dev/null 2>&1; then
        cargo xwin build --release "${FEATURES[@]}" --target "$TARGET"
      else
        echo "Для $TARGET с Linux: cargo install cargo-xwin" >&2
        echo "Или GitHub Actions: .github/workflows/windows-portable.yml" >&2
        exit 1
      fi
    else
      cargo build --release "${FEATURES[@]}" --target "$TARGET"
    fi
  else
    case "$HOST_OS" in
      MINGW*|MSYS*|CYGWIN*)
        cargo build --release "${FEATURES[@]}"
        ;;
      Linux)
        echo "На Linux укажите target, например:" >&2
        echo "  rustup target add x86_64-pc-windows-gnu && sudo dnf install mingw64-gcc" >&2
        echo "  CARGO_BUILD_TARGET=x86_64-pc-windows-gnu $0 $*" >&2
        echo "Надёжнее: Actions → windows-portable" >&2
        exit 1
        ;;
      *)
        cargo build --release "${FEATURES[@]}"
        ;;
    esac
  fi
fi

if [[ ! -f "$EXE" ]]; then
  echo "Нет $EXE" >&2
  exit 1
fi

OUT="$ROOT/dist/softecho-windows-x86_64-${NAME_SUFFIX}"
rm -rf "$OUT"
mkdir -p "$OUT"
cp -v "$EXE" "$OUT/"

for dll in libvosk.dll vosk.dll; do
  if [[ -f "$BIN_DIR/$dll" ]]; then
    cp -v "$BIN_DIR/$dll" "$OUT/"
  elif [[ -f "$ROOT/native/vosk/libvosk.dll" && "$WITH_ASR" -eq 1 ]]; then
    cp -v "$ROOT/native/vosk/libvosk.dll" "$OUT/$dll"
  fi
done

if [[ "${INCLUDE_MODEL:-0}" == "1" ]]; then
  MODEL_SRC="$ROOT/assets/vosk/vosk-model-small-ru-0.22"
  if [[ -d "$MODEL_SRC" ]]; then
    echo "==> Копирую модель"
    mkdir -p "$OUT/vosk-model-small-ru-0.22"
    cp -a "$MODEL_SRC"/. "$OUT/vosk-model-small-ru-0.22/"
  fi
fi

cat > "$OUT/ЧТОБЫ_ЗАПУСТИТЬ.txt" << 'EOF'
SoftEcho — portable (Windows x86_64)

1. Распакуйте всю папку целиком (не только .exe).
2. Запустите softecho.exe

Голос (ASR):
- Рядом с exe: libvosk.dll (и vosk.dll, если есть).
- Модель: папка vosk-model-small-ru-0.22 рядом с exe
  https://alphacephei.com/vosk/models (vosk-model-small-ru-0.22)
  или %APPDATA%\SoftEcho\SoftEcho\vosk-model-small-ru-0.22

Без модели/DLL — текстовый режим. Установщик не нужен.
EOF

mkdir -p "$ROOT/dist"
ZIP="$ROOT/dist/softecho-windows-x86_64-${NAME_SUFFIX}.zip"
rm -f "$ZIP"
if command -v zip >/dev/null 2>&1; then
  ( cd "$ROOT/dist" && zip -r -q "$(basename "$ZIP")" "$(basename "$OUT")" )
elif command -v powershell.exe >/dev/null 2>&1; then
  powershell.exe -NoProfile -Command \
    "Compress-Archive -Path '$OUT' -DestinationPath '$ZIP' -Force"
else
  echo "Нет zip/powershell — папка готова: $OUT" >&2
  exit 0
fi

echo
echo "Готово: $OUT/"
echo "        $ZIP"
ls -lh "$EXE" "$ZIP" 2>/dev/null || true
