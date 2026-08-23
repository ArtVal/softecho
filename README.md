# SoftEcho

**SoftEcho** (`softecho`) — домашний десктоп-тренажёр речи после инсульта и афазии.  
Работает **офлайн**: упражнения, проверка произнесённого, прогресс на вашем компьютере.

> Не заменяет занятия с логопедом. Для домашней практики между визитами.

## Для кого

- человек восстанавливает речь дома;
- родственник помогает короткими занятиями;
- слабый ПК, крупные кнопки, без облака после первой настройки.

## Платформы

| ОС | Сборка | Голос (Vosk) |
|----|--------|--------------|
| **Windows** 10/11 x86_64 | да | да |
| **Linux** x86_64 | да | да |
| **macOS** Apple Silicon | да | да (libvosk 0.3.42) |

Дискретная видеокарта не нужна.

## Скачать готовый бинарник

Сборки лежат в **GitHub Actions** (не на странице Releases):

1. [Actions](https://github.com/ArtVal/softecho/actions) → workflow **windows-portable** / **linux-portable** / **macos-portable**
2. Успешный run → внизу **Artifacts**
3. Скачать архив **text** (без голоса) или **asr** (с `libvosk`)

| Артефакт | Содержимое |
|----------|------------|
| `…-text` | только упражнения, без микрофона |
| `…-asr` | распознавание речи; **модель языка (~45 МБ) отдельно** |

После распаковки asr-сборки: **Настройки → Скачать модель** (нужен интернет один раз) или положите папку `vosk-model-small-ru-0.22` рядом с exe / в каталог данных (см. ниже).

## Быстрый старт (разработка)

```bash
# Только текст
cargo run --release

# С голосом (Linux, один раз)
./scripts/setup-asr.sh          # ALSA + libvosk + модель в assets/
cargo run --release --features asr
```

Бинарник: `target/release/softecho` (Windows: `softecho.exe`).

### Зависимости для сборки

- **Linux:** Wayland/X11, для ASR — `alsa-lib-devel`
- **Windows:** [MSVC Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) + Rust
- **macOS:** Xcode Command Line Tools + Rust

## Первый запуск (asr-сборка)

1. Запустите `softecho`.
2. На главном экране: **Настройки**.
3. **Скачать модель** (~45 МБ, Hugging Face → каталог данных).
4. После загрузки голос включается **без перезапуска** («Голос: готов»).

Альтернатива вручную:

- модель [vosk-model-small-ru-0.22](https://alphacephei.com/vosk/models);
- каталоги поиска: рядом с exe → `assets/vosk/` → данные приложения.

**Данные приложения** (прогресс, диктофон, скачанная модель):

| ОС | Путь |
|----|------|
| Linux | `~/.local/share/softecho/` |
| Windows | `%APPDATA%\SoftEcho\SoftEcho\` |
| macOS | `~/Library/Application Support/SoftEcho/SoftEcho/` |

Прогресс **не** хранится в git — только локально у пользователя.

## Что умеет

- **Занятие:** выбор слова, сборка фразы, «прочитать вслух»
- **Голос:** кнопка «Сказать», сверка с эталоном, мягкий допуск ошибок ASR
- **Диктофон:** длинная запись, текст в `.txt`, буфер 120 с + «подождите»
- **Настройки:** скачивание модели Vosk из приложения
- **Прогресс:** число занятий и верных ответов между запусками

## Голос (Vosk) — для сборки

Нативные библиотеки (не в git):

| ОС | Скрипт | Файл |
|----|--------|------|
| Linux | `./scripts/fetch-vosk-linux.sh` | `native/vosk/libvosk.so` |
| Windows | `./scripts/fetch-vosk-windows.sh` | `native/vosk/libvosk.dll` |
| macOS | `./scripts/fetch-vosk-macos.sh` | `native/vosk/libvosk.dylib` (0.3.42) |

Полная подготовка на Fedora: `./scripts/setup-asr.sh`.

Без `--features asr` или без модели — текстовый режим и самопроверка.

## Portable-сборки

| Платформа | CI workflow | Локально |
|-----------|-------------|----------|
| Windows | `windows-portable` | `./scripts/package-windows-portable.sh` |
| Linux | `linux-portable` | `./scripts/package-linux-portable.sh` |
| macOS | `macos-portable` | `./scripts/package-macos-portable.sh` |

С голосом: `fetch-vosk-*.sh`, затем `package-*-portable.sh --asr`.  
Модель в zip: `INCLUDE_MODEL=1 ./scripts/package-*-portable.sh --asr`.

Артефакты в `dist/`:

- `softecho-windows-x86_64-{text,asr}.zip`
- `softecho-linux-x86_64-{text,asr}.tar.gz`
- `softecho-macos-aarch64-{text,asr}.tar.gz`

## Архитектура

```
src/main.rs
src/engine/           — логика (будущий «сервер»)
  protocol.rs         — Command / Screen
  runtime.rs          — Engine::handle / tick
  vosk_download.rs    — загрузка модели из UI
  asr.rs, audio_pipe.rs, data.rs, exercise.rs
src/ui/               — egui-клиент
```

UI шлёт только `Command`; состояние читает через геттеры и `tick`.

План развития: [ROADMAP.md](ROADMAP.md).

## Проверки

```bash
cargo test
cargo test --features asr
cargo clippy -- -D warnings
cargo clippy --features asr -- -D warnings
```

## Лицензия

MIT (код). Модели и бинарники Vosk — [лицензия Alphacephei](https://alphacephei.com/vosk/).
