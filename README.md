# SoftEcho (`softecho`)

Домашний десктоп-тренажёр речи после инсульта / афазии. Офлайн, с проверкой произнесения.

## Платформы

Кроссплатформенно через **egui / eframe**:

| ОС | Статус |
|----|--------|
| **Windows** 10/11 | поддерживается |
| **Linux** (X11 / Wayland) | поддерживается |
| **macOS** | поддерживается |

Дискретная видеокарта **не нужна** — хватает встроенной графики.

## Запуск (фаза 1 — текст)

```bash
cargo run --release
```

Бинарник: `target/release/softecho` (на Windows — `softecho.exe`).

### Зависимости для сборки UI

- **Linux:** пакеты для окна/Wayland/X11 (на Fedora часто уже есть).
- **Windows:** [MSVC Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) + Rust.
- **macOS:** Xcode Command Line Tools + Rust.

## Архитектура кода

```
src/main.rs          — вход
src/engine/          — движок (логика, ASR, данные) — будущий «сервер»
  protocol.rs        — Command / Screen (граница с клиентом)
  runtime.rs         — Engine::handle / tick
  audio_pipe.rs      — буфер 120 с перед ASR
  asr.rs, data.rs, exercise.rs
src/ui/              — egui-клиент (рисует, шлёт Command)
```

UI не содержит бизнес-логики: только `engine.handle(Command::…)` и чтение состояния.

План и условия (шина, сеть): см. [ROADMAP.md](ROADMAP.md).

Проверки:

```bash
cargo test
cargo test --features asr
cargo clippy --features asr -- -D warnings
```

## Portable-сборки

Сборка без установщика (папка + архив):

| Платформа | GitHub Actions | Локально |
|-----------|----------------|----------|
| **Windows** x86_64 | workflow **windows-portable** | `./scripts/package-windows-portable.sh` |
| **Linux** x86_64 | workflow **linux-portable** | `./scripts/package-linux-portable.sh` |
| **macOS** (Apple Silicon) | workflow **macos-portable** | `./scripts/package-macos-portable.sh` |

С голосом: сначала fetch Vosk (`fetch-vosk-*.sh`), затем `package-*-portable.sh --asr`.  
Модель в архив: `INCLUDE_MODEL=1 ./scripts/package-*-portable.sh --asr`.

Артефакты в `dist/`:

- Windows: `softecho-windows-x86_64-{text,asr}.zip`
- Linux: `softecho-linux-x86_64-{text,asr}.tar.gz`
- macOS: `softecho-macos-aarch64-{text,asr}.tar.gz`

На macOS ASR использует **libvosk 0.3.42** (universal2) — официальных бинарников 0.3.45 под macOS нет.

## Что умеет сейчас

- Выбор слова, сборка фразы, «прочитать вслух»
- Голос (Vosk): кнопка «Сказать», долгий диктофон, txt на диск
- Буфер ASR 120 с и «подождите», пока разгребается очередь
- Крупный UI, локальный прогресс

## Фаза 2 — голос (Vosk)

```bash
# Fedora: один раз
./scripts/setup-asr.sh

cargo run --release --features asr
```

Вручную:

1. Linux: `alsa-lib-devel` / `libasound2-dev`.
2. Модель [vosk-model-small-ru-0.22](https://alphacephei.com/vosk/models) в `assets/vosk/vosk-model-small-ru-0.22/` (или рядом с exe / в данных приложения).
3. Нативная библиотека:
   - Linux: `./scripts/fetch-vosk-linux.sh` → `native/vosk/libvosk.so`
   - Windows: `./scripts/fetch-vosk-windows.sh`
   - macOS: `./scripts/fetch-vosk-macos.sh` → `libvosk.dylib` (Vosk 0.3.42)

Без модели или без `--features asr` — текстовый режим.

## Лицензия

MIT (код). Модели Vosk — по лицензии Alphacephei.
