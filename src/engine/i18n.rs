//! Язык приложения: UI + привязка к наборам и модели Vosk.

use serde::{Deserialize, Serialize};

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
pub fn stage_label(lang: AppLanguage, stage: crate::engine::ExerciseStage) -> &'static str {
    use crate::engine::ExerciseStage::*;
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
        (AppLanguage::Ru, "start") => "Начать занятие",
        (AppLanguage::En, "start") => "Start practice",
        (AppLanguage::Ru, "diagnosis") => "Экспресс-диагностика",
        (AppLanguage::En, "diagnosis") => "Quick check",
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

        // Settings
        (AppLanguage::Ru, "language") => "Язык",
        (AppLanguage::En, "language") => "Language",
        (AppLanguage::Ru, "language_hint") => "Интерфейс, набор и модель голоса. Смена языка переключит набор по умолчанию.",
        (AppLanguage::En, "language_hint") => "UI, exercise pack, and voice model. Changing language switches to the default pack.",
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
        (AppLanguage::Ru, "weak_hint") => "Слабые места возвращаются в занятии; «Не повторять» — пропуск до конца урока.",
        (AppLanguage::En, "weak_hint") => "Weak items return in practice; “Don't repeat” skips them for this lesson.",
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

        // Misc
        (AppLanguage::Ru, "pack_pick_title") => "Набор упражнений",
        (AppLanguage::En, "pack_pick_title") => "Exercise pack",
        (AppLanguage::Ru, "level_pick_title") => "Уровень",
        (AppLanguage::En, "level_pick_title") => "Level",
        (AppLanguage::Ru, "mine") => "мой",
        (AppLanguage::En, "mine") => "mine",

        _ => "???",
    }
}
