# Сюда кладутся нативные библиотеки Vosk (в git не коммитятся):
#   libvosk.so      — Linux    (./scripts/fetch-vosk-linux.sh)
#   libvosk.dylib   — macOS    (./scripts/fetch-vosk-macos.sh, Vosk 0.3.42 universal2)
#   libvosk.dll, libvosk.lib — Windows (./scripts/fetch-vosk-windows.sh)
#   остальные .dll из того же zip — MinGW runtime (на целевой машине MinGW не нужен)
