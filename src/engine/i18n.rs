//! Язык приложения: UI + привязка к наборам и модели Vosk.

use serde::{Deserialize, Serialize};

use super::exercise::{ExerciseStage, SpeechRating};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppLanguage {
    #[default]
    Ru,
    En,
}

impl AppLanguage {
    pub const ALL: [AppLanguage; 2] = [AppLanguage::Ru, AppLanguage::En];

    pub fn code(self) -> &'static str {
        match self {
            Self::Ru => "ru",
            Self::En => "en",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Ru => "Русский",
            Self::En => "English",
        }
    }

    pub fn default_pack_id(self) -> &'static str {
        match self {
            Self::Ru => "starter",
            Self::En => "starter_en",
        }
    }

    pub fn vosk_model_dir_name(self) -> &'static str {
        match self {
            Self::Ru => "vosk-model-small-ru-0.22",
            Self::En => "vosk-model-small-en-us-0.15",
        }
    }

    pub fn vosk_model_url(self) -> &'static str {
        match self {
            Self::Ru => {
                "https://huggingface.co/rhasspy/vosk-models/resolve/main/ru/vosk-model-small-ru-0.22.zip"
            }
            Self::En => {
                "https://huggingface.co/rhasspy/vosk-models/resolve/main/en/vosk-model-small-en-us-0.15.zip"
            }
        }
    }

    pub fn vosk_model_size_hint(self) -> &'static str {
        match self {
            Self::Ru => "~45 MB",
            Self::En => "~40 MB",
        }
    }
}

/// Подписи ступеней (уровень / карта).
pub fn stage_label(lang: AppLanguage, stage: ExerciseStage) -> &'static str {
    use ExerciseStage::*;
    match (lang, stage) {
        (AppLanguage::Ru, Sound) => "Звуки",
        (AppLanguage::Ru, Syllable) => "Слоги",
        (AppLanguage::Ru, Word) => "Слова",
        (AppLanguage::Ru, Phrase) => "Фразы",
        (AppLanguage::Ru, Twister) => "Скороговорки",
        (AppLanguage::En, Sound) => "Sounds",
        (AppLanguage::En, Syllable) => "Syllables",
        (AppLanguage::En, Word) => "Words",
        (AppLanguage::En, Phrase) => "Phrases",
        (AppLanguage::En, Twister) => "Tongue twisters",
    }
}

pub fn rating_label(lang: AppLanguage, rating: SpeechRating) -> &'static str {
    match (lang, rating) {
        (AppLanguage::Ru, SpeechRating::Unknown) => "ещё не пробовали",
        (AppLanguage::Ru, SpeechRating::Good) => "получается",
        (AppLanguage::Ru, SpeechRating::Almost) => "почти",
        (AppLanguage::Ru, SpeechRating::Weak) => "нужна практика",
        (AppLanguage::En, SpeechRating::Unknown) => "not tried yet",
        (AppLanguage::En, SpeechRating::Good) => "going well",
        (AppLanguage::En, SpeechRating::Almost) => "almost",
        (AppLanguage::En, SpeechRating::Weak) => "needs practice",
    }
}

/// Короткие строки UI. Ключи стабильные; тексты — по языку.
pub struct UiText {
    pub lang: AppLanguage,
}

impl UiText {
    pub fn new(lang: AppLanguage) -> Self {
        Self { lang }
    }

    pub fn t(&self, key: &str) -> &'static str {
        tr(self.lang, key)
    }
}

pub fn tr(lang: AppLanguage, key: &str) -> &'static str {
    match (lang, key) {
        // Home
        (AppLanguage::Ru, "tagline") => "Восстановление речи · занятия дома",
        (AppLanguage::En, "tagline") => "Speech recovery · practice at home",
        (AppLanguage::Ru, "pack") => "Набор",
        (AppLanguage::En, "pack") => "Pack",
        (AppLanguage::Ru, "level") => "Уровень",
        (AppLanguage::En, "level") => "Level",
        (AppLanguage::Ru, "level_none") => "не выбран",
        (AppLanguage::En, "level_none") => "not set",
        (AppLanguage::Ru, "sessions") => "Занятий",
        (AppLanguage::En, "sessions") => "Sessions",
        (AppLanguage::Ru, "sessions_total") => "Всего занятий",
        (AppLanguage::En, "sessions_total") => "Total sessions",
        (AppLanguage::Ru, "start") => "Начать занятие",
        (AppLanguage::En, "start") => "Start practice",
        (AppLanguage::Ru, "diagnosis") => "Экспресс-диагностика",
        (AppLanguage::En, "diagnosis") => "Quick check",
        (AppLanguage::Ru, "diagnosis_again") => "Повторная диагностика",
        (AppLanguage::En, "diagnosis_again") => "Repeat check",
        (AppLanguage::Ru, "warmup") => "Разминка",
        (AppLanguage::En, "warmup") => "Warm-up",
        (AppLanguage::Ru, "progress") => "Прогресс",
        (AppLanguage::En, "progress") => "Progress",
        (AppLanguage::Ru, "dictaphone") => "Диктофон",
        (AppLanguage::En, "dictaphone") => "Dictaphone",
        (AppLanguage::Ru, "dictaphone_need_asr") => "Диктофон — в сборке с голосом (Vosk)",
        (AppLanguage::En, "dictaphone_need_asr") => "Dictaphone needs the voice build (Vosk)",
        (AppLanguage::Ru, "settings") => "Настройки",
        (AppLanguage::En, "settings") => "Settings",
        (AppLanguage::Ru, "back") => "Назад",
        (AppLanguage::En, "back") => "Back",
        (AppLanguage::Ru, "menu") => "Меню",
        (AppLanguage::En, "menu") => "Menu",
        (AppLanguage::Ru, "choose_other") => "Выбрать другой",
        (AppLanguage::En, "choose_other") => "Choose another",

        // Common actions
        (AppLanguage::Ru, "say") => "Сказать",
        (AppLanguage::En, "say") => "Speak",
        (AppLanguage::Ru, "done") => "Готово",
        (AppLanguage::En, "done") => "Done",
        (AppLanguage::Ru, "listen") => "Послушать",
        (AppLanguage::En, "listen") => "Play back",
        (AppLanguage::Ru, "stop_listen") => "Стоп прослушивания",
        (AppLanguage::En, "stop_listen") => "Stop playback",
        (AppLanguage::Ru, "ok_self") => "Получилось",
        (AppLanguage::En, "ok_self") => "I did it",
        (AppLanguage::Ru, "fail_self") => "Не получилось",
        (AppLanguage::En, "fail_self") => "Not yet",
        (AppLanguage::Ru, "or_self") => "Или отметьте сами:",
        (AppLanguage::En, "or_self") => "Or mark yourself:",
        (AppLanguage::Ru, "next") => "Дальше",
        (AppLanguage::En, "next") => "Next",
        (AppLanguage::Ru, "save") => "Сохранить",
        (AppLanguage::En, "save") => "Save",
        (AppLanguage::Ru, "add") => "Добавить",
        (AppLanguage::En, "add") => "Add",
        (AppLanguage::Ru, "reset") => "Сбросить",
        (AppLanguage::En, "reset") => "Reset",
        (AppLanguage::Ru, "check") => "Проверить",
        (AppLanguage::En, "check") => "Check",
        (AppLanguage::Ru, "stop") => "Стоп",
        (AppLanguage::En, "stop") => "Stop",
        (AppLanguage::Ru, "clear") => "Очистить",
        (AppLanguage::En, "clear") => "Clear",
        (AppLanguage::Ru, "again") => "Ещё раз",
        (AppLanguage::En, "again") => "Again",
        (AppLanguage::Ru, "empty") => "Пусто",
        (AppLanguage::En, "empty") => "Empty",
        (AppLanguage::Ru, "of") => "из",
        (AppLanguage::En, "of") => "of",

        // Settings
        (AppLanguage::Ru, "language") => "Язык",
        (AppLanguage::En, "language") => "Language",
        (AppLanguage::Ru, "language_hint") => {
            "Интерфейс, набор и модель голоса. Смена языка переключит набор по умолчанию."
        }
        (AppLanguage::En, "language_hint") => {
            "UI, exercise pack, and voice model. Changing language switches to the default pack."
        }
        (AppLanguage::Ru, "pack_and_level") => "Набор и уровень",
        (AppLanguage::En, "pack_and_level") => "Pack and level",
        (AppLanguage::Ru, "change_pack") => "Сменить набор",
        (AppLanguage::En, "change_pack") => "Change pack",
        (AppLanguage::Ru, "choose_level") => "Выбрать уровень",
        (AppLanguage::En, "choose_level") => "Choose level",
        (AppLanguage::Ru, "speech_map") => "Карта произнесения",
        (AppLanguage::En, "speech_map") => "Speech map",
        (AppLanguage::Ru, "pack_editor") => "Редактор набора",
        (AppLanguage::En, "pack_editor") => "Pack editor",
        (AppLanguage::Ru, "voice") => "Голос",
        (AppLanguage::En, "voice") => "Voice",
        (AppLanguage::Ru, "voice_ready") => "Голос: готов (Vosk)",
        (AppLanguage::En, "voice_ready") => "Voice: ready (Vosk)",
        (AppLanguage::Ru, "voice_missing") => "Голос: модель не найдена",
        (AppLanguage::En, "voice_missing") => "Voice: model not found",
        (AppLanguage::Ru, "voice_disabled") => "Голос: выключен в этой сборке",
        (AppLanguage::En, "voice_disabled") => "Voice: disabled in this build",
        (AppLanguage::Ru, "download_model") => "Скачать модель",
        (AppLanguage::En, "download_model") => "Download model",
        (AppLanguage::Ru, "download_model_hint") => "Скачать модель Vosk. Нужен интернет один раз.",
        (AppLanguage::En, "download_model_hint") => "Download the Vosk model. Internet needed once.",
        (AppLanguage::Ru, "model_ready") => "Модель уже на месте — перезапуск не нужен.",
        (AppLanguage::En, "model_ready") => "Model is ready — no restart needed.",
        (AppLanguage::Ru, "retry") => "Повторить",
        (AppLanguage::En, "retry") => "Retry",
        (AppLanguage::Ru, "to_menu") => "< В меню",
        (AppLanguage::En, "to_menu") => "< Menu",
        (AppLanguage::Ru, "data") => "Данные",
        (AppLanguage::En, "data") => "Data",
        (AppLanguage::Ru, "correct_count") => "верно",
        (AppLanguage::En, "correct_count") => "correct",
        (AppLanguage::Ru, "weak_hint") => {
            "Слабые места возвращаются в занятии; «Не повторять» — пропуск до конца урока."
        }
        (AppLanguage::En, "weak_hint") => {
            "Weak items return in practice; “Don't repeat” skips them for this lesson."
        }
        (AppLanguage::Ru, "progress_err") => "Прогресс",
        (AppLanguage::En, "progress_err") => "Progress",

        // Feedback
        (AppLanguage::Ru, "correct") => "Верно",
        (AppLanguage::En, "correct") => "Correct",
        (AppLanguage::Ru, "incorrect") => "Неверно",
        (AppLanguage::En, "incorrect") => "Incorrect",
        (AppLanguage::Ru, "heard") => "Услышала",
        (AppLanguage::En, "heard") => "I heard",
        (AppLanguage::Ru, "skip_repeat") => "Не повторять",
        (AppLanguage::En, "skip_repeat") => "Don't repeat",
        (AppLanguage::Ru, "expected") => "Нужно было",
        (AppLanguage::En, "expected") => "Expected",
        (AppLanguage::Ru, "asr_none") => "Модель ничего не распознала (или была самопроверка)",
        (AppLanguage::En, "asr_none") => "Nothing recognized (or you checked yourself)",
        (AppLanguage::Ru, "requeue_done") => {
            "В этом занятии больше не вернём — лимит или «не повторять»."
        }
        (AppLanguage::En, "requeue_done") => {
            "Won't return in this lesson — limit reached or “don't repeat”."
        }
        (AppLanguage::Ru, "hint_ok") => "Похоже верно — отметьте сами",
        (AppLanguage::En, "hint_ok") => "Looks right — mark yourself",
        (AppLanguage::Ru, "hint_bad") => "Похоже иначе — отметьте сами",
        (AppLanguage::En, "hint_bad") => "Looks different — mark yourself",

        // Misc / pickers
        (AppLanguage::Ru, "pack_pick_title") => "Набор упражнений",
        (AppLanguage::En, "pack_pick_title") => "Exercise pack",
        (AppLanguage::Ru, "level_pick_title") => "Уровень",
        (AppLanguage::En, "level_pick_title") => "Level",
        (AppLanguage::Ru, "mine") => "мой",
        (AppLanguage::En, "mine") => "mine",

        // Speech map
        (AppLanguage::Ru, "speech_map_hint") => {
            "Получается · почти · нужна практика — по результатам занятий и диагностики.\nСлабые места идут первыми в следующем занятии."
        }
        (AppLanguage::En, "speech_map_hint") => {
            "Going well · almost · needs practice — from lessons and checks.\nWeak items come first in the next practice."
        }
        (AppLanguage::Ru, "speech_map_empty") => "В этом наборе пока нет заданий.",
        (AppLanguage::En, "speech_map_empty") => "This pack has no exercises yet.",
        (AppLanguage::Ru, "rating_weak") => "Нужна практика",
        (AppLanguage::En, "rating_weak") => "Needs practice",
        (AppLanguage::Ru, "rating_almost") => "почти",
        (AppLanguage::En, "rating_almost") => "almost",
        (AppLanguage::Ru, "rating_good") => "получается",
        (AppLanguage::En, "rating_good") => "going well",
        (AppLanguage::Ru, "rating_unknown") => "ещё нет",
        (AppLanguage::En, "rating_unknown") => "not yet",
        (AppLanguage::Ru, "weak_list") => "Слабые",
        (AppLanguage::En, "weak_list") => "Weak",

        // Pack editor
        (AppLanguage::Ru, "editor_now") => "Сейчас",
        (AppLanguage::En, "editor_now") => "Current",
        (AppLanguage::Ru, "editor_readonly") => {
            "Встроенные наборы только для чтения. Сделайте копию — она сохранится на этом компьютере."
        }
        (AppLanguage::En, "editor_readonly") => {
            "Built-in packs are read-only. Make a copy — it is saved on this computer."
        }
        (AppLanguage::Ru, "editor_clone") => "Сделать копию и править",
        (AppLanguage::En, "editor_clone") => "Clone and edit",
        (AppLanguage::Ru, "editor_file") => "Файл",
        (AppLanguage::En, "editor_file") => "File",
        (AppLanguage::Ru, "editor_active_n") => "активно",
        (AppLanguage::En, "editor_active_n") => "active",
        (AppLanguage::Ru, "editor_off_n") => "выкл.",
        (AppLanguage::En, "editor_off_n") => "off",
        (AppLanguage::Ru, "editor_active") => "Активные",
        (AppLanguage::En, "editor_active") => "Active",
        (AppLanguage::Ru, "editor_disabled") => "Отключённые",
        (AppLanguage::En, "editor_disabled") => "Disabled",
        (AppLanguage::Ru, "editor_off") => "Выкл",
        (AppLanguage::En, "editor_off") => "Off",
        (AppLanguage::Ru, "editor_on") => "Вкл",
        (AppLanguage::En, "editor_on") => "On",
        (AppLanguage::Ru, "editor_add_read") => "Добавить «прочитать вслух»",
        (AppLanguage::En, "editor_add_read") => "Add “read aloud”",
        (AppLanguage::Ru, "editor_prompt") => "Подсказка",
        (AppLanguage::En, "editor_prompt") => "Prompt",
        (AppLanguage::Ru, "editor_text") => "Текст",
        (AppLanguage::En, "editor_text") => "Text",
        (AppLanguage::Ru, "editor_prompt_default") => "Скажите",
        (AppLanguage::En, "editor_prompt_default") => "Say",

        // Progress
        (AppLanguage::Ru, "trend") => "Тренд занятий",
        (AppLanguage::En, "trend") => "Session trend",
        (AppLanguage::Ru, "trend_empty") => "Пока нет завершённых занятий — пройдите урок.",
        (AppLanguage::En, "trend_empty") => "No finished sessions yet — complete a lesson.",
        (AppLanguage::Ru, "trend_recent") => "Последние",
        (AppLanguage::En, "trend_recent") => "Last",
        (AppLanguage::Ru, "trend_pct") => "% верных",
        (AppLanguage::En, "trend_pct") => "% correct",
        (AppLanguage::Ru, "map_by_stage") => "Карта по ступеням",
        (AppLanguage::En, "map_by_stage") => "Map by stage",
        (AppLanguage::Ru, "map_empty") => "В наборе пока нет целей для карты.",
        (AppLanguage::En, "map_empty") => "No map targets in this pack yet.",
        (AppLanguage::Ru, "copy_report") => "Скопировать отчёт",
        (AppLanguage::En, "copy_report") => "Copy report",

        // Warmup
        (AppLanguage::Ru, "warmup_hint") => {
            "Перед занятием: губы, язык, выдох. Это не проверка — только подготовка."
        }
        (AppLanguage::En, "warmup_hint") => {
            "Before practice: lips, tongue, breath. Not a test — just preparation."
        }
        (AppLanguage::Ru, "warmup_video") => "Видео снаружи (откроется в браузере)",
        (AppLanguage::En, "warmup_video") => "External videos (opens in browser)",
        (AppLanguage::Ru, "warmup_video_note") => {
            "Чужие ролики — смотреть можно, в приложение не вшиваем."
        }
        (AppLanguage::En, "warmup_video_note") => {
            "Third-party videos — watch outside; we do not embed them."
        }
        (AppLanguage::Ru, "warmup_odk_hint") => "После схем удобно набор «Артикуляция: па-та-ка».",
        (AppLanguage::En, "warmup_odk_hint") => {
            "After the schemas, the Russian “pa-ta-ka” pack is a good motor warm-up."
        }
        (AppLanguage::Ru, "warmup_odk_btn") => "Открыть па-та-ка",
        (AppLanguage::En, "warmup_odk_btn") => "Open pa-ta-ka",
        (AppLanguage::Ru, "warmup_lips") => "Губы",
        (AppLanguage::En, "warmup_lips") => "Lips",
        (AppLanguage::Ru, "warmup_lips_how") => {
            "Сначала широко «улыбка», потом губы в «трубочку». По 3–5 раз, без спешки."
        }
        (AppLanguage::En, "warmup_lips_how") => {
            "Wide smile, then lips in a “tube”. 3–5 times, no rush."
        }
        (AppLanguage::Ru, "warmup_tongue") => "Язык",
        (AppLanguage::En, "warmup_tongue") => "Tongue",
        (AppLanguage::Ru, "warmup_tongue_how") => {
            "Кончик языка вверх к нёбу, потом влево и вправо. Рот приоткрыт, без напряжения шеи."
        }
        (AppLanguage::En, "warmup_tongue_how") => {
            "Tongue tip up to the palate, then left and right. Mouth slightly open, neck relaxed."
        }
        (AppLanguage::Ru, "warmup_breath") => "Выдох",
        (AppLanguage::En, "warmup_breath") => "Breath",
        (AppLanguage::Ru, "warmup_breath_how") => {
            "Спокойный вдох носом, долгий выдох ртом со звуком «с-с-с» или «ф-ф-ф». 3 раза."
        }
        (AppLanguage::En, "warmup_breath_how") => {
            "Calm nose inhale, long mouth exhale with “s-s-s” or “f-f-f”. 3 times."
        }
        (AppLanguage::Ru, "warmup_link1") => "Викторова — сайт (артикуляционная гимнастика)",
        (AppLanguage::En, "warmup_link1") => "Viktorova — articulation exercises (site, Russian)",
        (AppLanguage::Ru, "warmup_link2") => "Сергеева — урок при афазии Брока (Rutube)",
        (AppLanguage::En, "warmup_link2") => "Sergeeva — Broca aphasia lesson (Rutube, Russian)",
        (AppLanguage::Ru, "warmup_link3") => "ГКБ №52 — упражнения при афазии (Rutube)",
        (AppLanguage::En, "warmup_link3") => "Hospital №52 — aphasia exercises (Rutube, Russian)",
        (AppLanguage::Ru, "warmup_link4") => "Начальные фонетические упражнения (~36 мин)",
        (AppLanguage::En, "warmup_link4") => "Basic phonetic exercises (~36 min, Russian)",

        // Diagnosis result
        (AppLanguage::Ru, "diag_ready") => "Диагностика готова",
        (AppLanguage::En, "diag_ready") => "Check complete",
        (AppLanguage::Ru, "diag_saved") => {
            "Сохранён. Занятие пойдёт с этой ступени.\nКарта произнесения тоже обновлена."
        }
        (AppLanguage::En, "diag_saved") => {
            "Saved. Practice will start from this stage.\nThe speech map was updated too."
        },

        // Exercise
        (AppLanguage::Ru, "mode_diagnosis") => "Диагностика",
        (AppLanguage::En, "mode_diagnosis") => "Check",
        (AppLanguage::Ru, "mode_practice") => "Занятие",
        (AppLanguage::En, "mode_practice") => "Practice",
        (AppLanguage::Ru, "practice_repeat") => "Повтор — нужна практика",
        (AppLanguage::En, "practice_repeat") => "Repeat — needs practice",
        (AppLanguage::Ru, "tap_words") => "Нажимайте слова по порядку",
        (AppLanguage::En, "tap_words") => "Tap the words in order",
        (AppLanguage::Ru, "twister_tip") => {
            "Сначала медленно по словам → трудные места отдельно → целиком медленно → чуть быстрее.\n«Готово» или самопроверка — принять попытку."
        }
        (AppLanguage::En, "twister_tip") => {
            "Slowly word by word → hard spots alone → whole phrase slowly → a bit faster.\n“Done” or self-check accepts the try."
        }
        (AppLanguage::Ru, "please_wait_asr") => {
            "Подождите: распознаю накопленный звук. Говорить пока не нужно."
        }
        (AppLanguage::En, "please_wait_asr") => {
            "Please wait: recognizing buffered audio. No need to speak yet."
        }
        (AppLanguage::Ru, "speaking") => "Говорите… остановлюсь после паузы",
        (AppLanguage::En, "speaking") => "Speak… I'll stop after a pause",

        // Dictaphone
        (AppLanguage::Ru, "dict_title") => "Долгий диктофон",
        (AppLanguage::En, "dict_title") => "Long dictaphone",
        (AppLanguage::Ru, "dict_hint") => {
            "Говорите сколько нужно — текст копится в .txt. Стоп — когда закончите."
        }
        (AppLanguage::En, "dict_hint") => {
            "Speak as long as you need — text builds in a .txt file. Stop when finished."
        }
        (AppLanguage::Ru, "dict_recording") => "Идёт запись… (Стоп внизу)",
        (AppLanguage::En, "dict_recording") => "Recording… (Stop below)",
        (AppLanguage::Ru, "dict_wait") => "Подождите: распознаю звук. Говорить пока не нужно.",
        (AppLanguage::En, "dict_wait") => "Please wait: recognizing audio. No need to speak yet.",
        (AppLanguage::Ru, "dict_record") => "Запись",
        (AppLanguage::En, "dict_record") => "Record",
        (AppLanguage::Ru, "dict_save_txt") => "Сохранить txt",
        (AppLanguage::En, "dict_save_txt") => "Save txt",
        (AppLanguage::Ru, "dict_text") => "Текст:",
        (AppLanguage::En, "dict_text") => "Text:",
        (AppLanguage::Ru, "dict_text_tail") => "Текст (хвост; полный — в .txt):",
        (AppLanguage::En, "dict_text_tail") => "Text (tail; full file in .txt):",

        // Result
        (AppLanguage::Ru, "result_done") => "Занятие закончено",
        (AppLanguage::En, "result_done") => "Practice finished",
        (AppLanguage::Ru, "result_score") => "Верно",
        (AppLanguage::En, "result_score") => "Correct",
        (AppLanguage::Ru, "result_plan") => "Заданий в плане",
        (AppLanguage::En, "result_plan") => "Items in plan",
        (AppLanguage::Ru, "result_with_repeats") => "с повторами слабых",
        (AppLanguage::En, "result_with_repeats") => "with weak repeats",
        (AppLanguage::Ru, "save_failed") => "Не удалось сохранить прогресс",
        (AppLanguage::En, "save_failed") => "Could not save progress",

        // Report
        (AppLanguage::Ru, "report_title") => "SoftEcho — отчёт о прогрессе",
        (AppLanguage::En, "report_title") => "SoftEcho — progress report",
        (AppLanguage::Ru, "report_history") => "История занятий (старые → новые):",
        (AppLanguage::En, "report_history") => "Session history (old → new):",
        (AppLanguage::Ru, "report_trend") => "Тренд (последние",
        (AppLanguage::En, "report_trend") => "Trend (last",
        (AppLanguage::Ru, "report_disclaimer") => "Не заменяет занятие с логопедом.",
        (AppLanguage::En, "report_disclaimer") => "Does not replace work with a speech therapist.",
        (AppLanguage::Ru, "weak_places") => "Слабые места",
        (AppLanguage::En, "weak_places") => "Weak spots",

        (AppLanguage::Ru, "download_fetching") => "Скачиваю модель",
        (AppLanguage::En, "download_fetching") => "Downloading model",
        (AppLanguage::Ru, "download_unpack") => "Распаковка…",
        (AppLanguage::En, "download_unpack") => "Extracting…",
        (AppLanguage::Ru, "download_prepare") => "Подготовка…",
        (AppLanguage::En, "download_prepare") => "Preparing…",
        (AppLanguage::Ru, "model_installed") => "Модель уже установлена.",
        (AppLanguage::En, "model_installed") => "Model is already installed.",
        (AppLanguage::Ru, "err_mkdir") => "Не удалось создать каталог",
        (AppLanguage::En, "err_mkdir") => "Could not create directory",
        (AppLanguage::Ru, "err_download") => "Не удалось скачать",
        (AppLanguage::En, "err_download") => "Download failed",
        (AppLanguage::Ru, "err_read") => "Ошибка чтения",
        (AppLanguage::En, "err_read") => "Read error",
        (AppLanguage::Ru, "err_write") => "Ошибка записи",
        (AppLanguage::En, "err_write") => "Write error",
        (AppLanguage::Ru, "err_write_file") => "Не удалось записать файл",
        (AppLanguage::En, "err_write_file") => "Could not write file",
        (AppLanguage::Ru, "err_clear_dir") => "Не удалось очистить каталог",
        (AppLanguage::En, "err_clear_dir") => "Could not clear directory",
        (AppLanguage::Ru, "err_zip_missing") => "В архиве нет папки",
        (AppLanguage::En, "err_zip_missing") => "Archive is missing folder",
        (AppLanguage::Ru, "err_zip_open") => "Не открыть архив",
        (AppLanguage::En, "err_zip_open") => "Could not open archive",
        (AppLanguage::Ru, "err_zip_bad") => "Повреждённый архив",
        (AppLanguage::En, "err_zip_bad") => "Corrupt archive",

        _ => "???",
    }
}
