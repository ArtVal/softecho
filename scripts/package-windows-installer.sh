#!/usr/bin/env bash
# Собрать Windows Setup.exe (Inno Setup) из уже готовой portable-папки.
#
#   ./scripts/package-windows-portable.sh --asr   # сначала
#   ./scripts/package-windows-installer.sh --asr
#
# Нужен iscc: Inno Setup 6 (Windows) или choco install innosetup.
# На CI: workflow windows-portable ставит Inno через Chocolatey.
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

NAME_SUFFIX="text"
if [[ "$WITH_ASR" -eq 1 ]]; then
  NAME_SUFFIX="asr"
fi

PORTABLE="$ROOT/dist/softecho-windows-x86_64-${NAME_SUFFIX}"
ISS="$ROOT/scripts/windows/softecho.iss"
OUT_DIR="$ROOT/dist"

if [[ ! -d "$PORTABLE" ]]; then
  echo "Нет portable-папки: $PORTABLE" >&2
  echo "Сначала: ./scripts/package-windows-portable.sh${WITH_ASR:+ --asr}" >&2
  exit 1
fi
if [[ ! -f "$PORTABLE/softecho.exe" ]]; then
  echo "В $PORTABLE нет softecho.exe" >&2
  exit 1
fi
if [[ ! -f "$ISS" ]]; then
  echo "Нет $ISS" >&2
  exit 1
fi

VERSION="$(./scripts/version.sh get)"
if [[ -z "$VERSION" ]]; then
  VERSION="0.0.0"
fi

find_iscc() {
  if command -v iscc >/dev/null 2>&1; then
    command -v iscc
    return
  fi
  if command -v ISCC.exe >/dev/null 2>&1; then
    command -v ISCC.exe
    return
  fi
  local candidates=(
    "/c/Program Files (x86)/Inno Setup 6/ISCC.exe"
    "/c/Program Files/Inno Setup 6/ISCC.exe"
    "C:/Program Files (x86)/Inno Setup 6/ISCC.exe"
    "C:/Program Files/Inno Setup 6/ISCC.exe"
  )
  local p
  for p in "${candidates[@]}"; do
    if [[ -x "$p" ]] || [[ -f "$p" ]]; then
      echo "$p"
      return
    fi
  done
  return 1
}

ISCC="$(find_iscc)" || {
  echo "Не найден ISCC (Inno Setup 6)." >&2
  echo "Установите: https://jrsoftware.org/isinfo.php  или  choco install innosetup" >&2
  exit 1
}

# Inno на Windows ждёт Windows-пути в /D SourceDir=...
to_win_path() {
  if command -v cygpath >/dev/null 2>&1; then
    cygpath -w "$1"
  else
    # GitHub Actions bash / MSYS: /d/a/... → D:\a\...
    local p="$1"
    if [[ "$p" =~ ^/([a-zA-Z])/(.*)$ ]]; then
      echo "${BASH_REMATCH[1]^}:\\${BASH_REMATCH[2]//\//\\}"
    else
      echo "$p" | sed 's|/|\\|g'
    fi
  fi
}

SRC_WIN="$(to_win_path "$PORTABLE")"
OUT_WIN="$(to_win_path "$OUT_DIR")"
ISS_WIN="$(to_win_path "$ISS")"

echo "==> Inno Setup: SoftEcho $VERSION ($NAME_SUFFIX)"
echo "    source: $SRC_WIN"
echo "    iscc:   $ISCC"

mkdir -p "$OUT_DIR"
# iscc пишет рядом с OutputBaseFilename в OutputDir
"$ISCC" \
  "//DMyAppVersion=$VERSION" \
  "//DNameSuffix=$NAME_SUFFIX" \
  "//DSourceDir=$SRC_WIN" \
  "//DOutputDir=$OUT_WIN" \
  "$ISS_WIN"

SETUP="$OUT_DIR/softecho-windows-x86_64-setup-${NAME_SUFFIX}.exe"
if [[ ! -f "$SETUP" ]]; then
  echo "Ожидали $SETUP — файл не создан" >&2
  ls -la "$OUT_DIR" >&2 || true
  exit 1
fi

echo
echo "Готово: $SETUP"
ls -lh "$SETUP"
