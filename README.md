# Речевой тренажёр (stroke_trainer)

Десктоп-приложение на Rust для восстановления речевых функций после инсульта.

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

Сборка под текущую ОС:

```bash
cargo build --release
```

Бинарник: `target/release/stroke_trainer` (на Windows — `stroke_trainer.exe`).

### Зависимости для сборки UI

- **Linux:** пакеты для окна/Wayland/X11 (на Fedora часто уже есть; иначе `libxkbcommon-devel` и связанные). Дискретная GPU не нужна.
- **Windows:** [MSVC Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) + Rust (`rustup-init.exe`, toolchain `x86_64-pc-windows-msvc`).
- **macOS:** Xcode Command Line Tools + Rust (`rustup`).

Один и тот же код собирается на всех трёх ОС: `cargo build --release`.

## Что умеет сейчас

- Выбор слова по вопросу
- Сборка фразы из слов
- «Прочитать вслух» с самопроверкой («Получилось» / «Не получилось»)
- Крупный шрифт, простой экран
- Локальный прогресс занятий

## Фаза 2 — голос (Vosk, опционально)

Офлайн-распознавание на CPU (маленькая русская модель). Собирается отдельно:

```bash
cargo run --release --features asr
```

1. Скачайте модель [vosk-model-small-ru-0.22](https://alphacephei.com/vosk/models) (~45 МБ).
2. Распакуйте в одно из мест:
   - `assets/vosk/vosk-model-small-ru-0.22/`
   - или каталог данных приложения (`…/stroke_trainer/vosk-model-small-ru-0.22`)
3. На упражнении «Прочитать вслух» появится кнопка **Сказать**.

Без модели или без `--features asr` приложение работает в текстовом режиме и не падает.

На **Windows / Linux / macOS** для микрофона нужны системные права на запись звука; crate `vosk` подтянет нативный `libvosk` при сборке с feature `asr`.

Дополнительно для `--features asr`:

| ОС | Пакеты / заметки |
|----|------------------|
| **Linux (Fedora)** | `alsa-lib-devel` (для `cpal`) |
| **Linux (Debian/Ubuntu)** | `libasound2-dev` |
| **Windows** | WASAPI через `cpal`, отдельный ALSA не нужен |
| **macOS** | CoreAudio через `cpal` |

## Лицензия

MIT (код). Модели Vosk — по лицензии Alphacephei (смотрите на сайте моделей).
