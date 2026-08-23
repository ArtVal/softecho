#!/usr/bin/env bash
# Подготовка микрофона (Vosk) для SoftEcho (softecho) на Fedora/RHEL-подобных.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MODEL_DIR="$ROOT/assets/vosk/vosk-model-small-ru-0.22"
NATIVE_DIR="$ROOT/native/vosk"
LIBVOSK="$NATIVE_DIR/libvosk.so"
URL_MODEL_HF="https://huggingface.co/rhasspy/vosk-models/resolve/main/ru/vosk-model-small-ru-0.22.zip"

echo "==> ALSA (нужен пароль sudo)"
if ! pkg-config --exists alsa 2>/dev/null; then
  sudo dnf install -y alsa-lib-devel
else
  echo "alsa уже есть"
fi

if [[ ! -d "$MODEL_DIR" ]]; then
  echo "==> Скачиваю языковую модель (~45 МБ)"
  mkdir -p "$ROOT/assets/vosk"
  ZIP="$ROOT/assets/vosk/vosk-model-small-ru-0.22.zip"
  curl -L --fail --retry 2 --connect-timeout 30 -o "$ZIP" "$URL_MODEL_HF"
  unzip -o "$ZIP" -d "$ROOT/assets/vosk"
  rm -f "$ZIP"
else
  echo "модель уже лежит в $MODEL_DIR"
fi

if [[ ! -f "$LIBVOSK" ]]; then
  echo "==> Скачиваю libvosk.so"
  "$ROOT/scripts/fetch-vosk-linux.sh"
else
  echo "libvosk.so уже есть"
fi

echo "==> Сборка с голосом"
cd "$ROOT"
cargo build --release --features asr

echo
echo "Готово. Запуск:"
echo "  cd \"$ROOT\" && cargo run --release --features asr"
echo "На главном экране: «Голос: готов (Vosk)» → в «Прочитать вслух» кнопка «Сказать»."
