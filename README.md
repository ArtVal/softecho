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

## Portable Windows

Сборка без установщика (папка + zip):

| Как | Команда / действие |
|-----|-------------------|
| **GitHub Actions** | Actions → workflow **windows-portable** → Run workflow → artifact |
| **На Windows** | `./scripts/package-windows-portable.sh` |
| **С голосом** | `./scripts/fetch-vosk-windows.sh` затем `./scripts/package-windows-portable.sh --asr` |
| **Модель в zip** | `INCLUDE_MODEL=1 ./scripts/package-windows-portable.sh --asr` |

Артефакты: `dist/softecho-windows-x86_64-text.zip` и `…-asr.zip`.

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
3. Нативная библиотека: [vosk-linux-x86_64](https://github.com/alphacep/vosk-api/releases/tag/v0.3.45) → `native/vosk/libvosk.so` (Windows: `./scripts/fetch-vosk-windows.sh`).

Без модели или без `--features asr` — текстовый режим.

## Лицензия

MIT (код). Модели Vosk — по лицензии Alphacephei.
