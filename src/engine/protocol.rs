//! Протокол движок ↔ UI (позже ↔ сеть).
//! Команды идут от клиента; состояние читается с движка после `tick`/`handle`.

use super::exercise::{CheckResult, UserAnswer};
use std::time::Duration;

/// Экран навигации (общее для UI и движка).
#[derive(Debug, Clone)]
pub enum Screen {
    Home,
    Exercise,
    Feedback {
        result: CheckResult,
        heard: Option<String>,
        expected: Option<String>,
    },
    Dictaphone,
    Settings,
    Result {
        correct: u32,
        total: u32,
    },
}

/// Команды клиента → движок (сетевой API почти 1:1).
#[derive(Debug, Clone)]
#[allow(dead_code)] // часть команд — запас под UI/сеть
pub enum Command {
    GoHome,
    StartSession,
    OpenDictaphone,
    OpenSettings,
    LeaveSettings,
    StartModelDownload,
    AgainSession,
    AdvanceAfterFeedback,
    Submit(UserAnswer),
    ListenExercise,
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
