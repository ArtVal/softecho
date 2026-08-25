//! Протокол движок ↔ UI (позже ↔ сеть).
//! Команды идут от клиента; состояние читается с движка после `tick`/`handle`.

use super::exercise::{CheckResult, ExerciseStage, UserAnswer};
use std::time::Duration;

/// Экран навигации (общее для UI и движка).
#[derive(Debug, Clone)]
pub enum Screen {
    Home,
    /// Выбор набора упражнений.
    PackPick,
    /// Ручной выбор уровня (пропуск диагностики).
    LevelPick,
    Exercise,
    Feedback {
        result: CheckResult,
        heard: Option<String>,
        expected: Option<String>,
    },
    /// Итог экспресс-диагностики: уровень уже записан в прогресс.
    DiagnosisResult {
        level: ExerciseStage,
    },
    /// Карта произнесения по текущему набору.
    SpeechMap,
    /// Разминка: схемы + ссылки на внешние видео (не упражнение).
    Warmup,
    Dictaphone,
    Settings,
    Result {
        correct: u32,
        /// Все попытки, включая возвраты в очередь.
        total: u32,
        /// Заданий в исходном плане занятия (без повторов).
        unique: u32,
    },
}

/// Команды клиента → движок (сетевой API почти 1:1).
#[derive(Debug, Clone)]
#[allow(dead_code)] // часть команд — запас под UI/сеть
pub enum Command {
    GoHome,
    /// Занятие с текущего уровня; если уровня нет — экран выбора.
    StartSession,
    /// Короткий прогон звук→слог→слово→фраза, затем автоуровень.
    StartDiagnosis,
    OpenPackPick,
    LeavePackPick,
    /// Сменить встроенный набор упражнений.
    SetPack(String),
    OpenLevelPick,
    LeaveLevelPick,
    OpenSpeechMap,
    LeaveSpeechMap,
    /// Разминка перед занятием (схемы + внешние ссылки).
    OpenWarmup,
    LeaveWarmup,
    /// Ручная установка уровня (сохраняется локально).
    SetLevel(ExerciseStage),
    OpenDictaphone,
    OpenSettings,
    LeaveSettings,
    StartModelDownload,
    AgainSession,
    AdvanceAfterFeedback,
    /// Не возвращать это задание в очередь занятия (только практика).
    SkipRepeatAndAdvance,
    Submit(UserAnswer),
    ListenExercise,
    /// Закончить запись слога/слова (принять partial и проверить).
    StopExerciseListen,
    /// Прослушать последнюю запись (упражнение / диктофон).
    PlayLastClip,
    StopPlayback,
    ListenDictaphone,
    StopDictaphone,
    ClearDictaphone,
    SaveDictaphone,
    /// Выйти с экрана диктофона (очищает буфер).
    LeaveDictaphone,
    /// Собрать фразу: взять слово из пула по индексу.
    PickPoolWord(usize),
    UndoPickedWord,
    ClearPickedWords,
    /// Сбросить «собрать фразу» (перемешать пул заново).
    ResetBuildPhrase,
}

/// Состояние загрузки модели Vosk (экран «Настройки»).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ModelDownloadState {
    #[default]
    Idle,
    Working {
        label: String,
        percent: Option<u8>,
    },
    Succeeded,
    Failed(String),
}

/// Результат `Engine::tick` — подсказки для UI (локально) / таймеров (сеть).
#[derive(Debug, Clone, Default)]
pub struct TickResult {
    pub want_repaint: bool,
    pub repaint_after: Option<Duration>,
}
