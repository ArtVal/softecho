//! Движок тренажёра (без UI).
//!
//! Граница для будущего клиент–сервера:
//! - клиент шлёт [`protocol::Command`];
//! - сервер держит [`runtime::Engine`], вызывает `handle` / `tick`;
//! - UI читает состояние через геттеры (потом — снимок/сообщения по сети).

pub mod asr;
#[cfg(any(feature = "asr", test))]
pub mod audio_pipe;
pub mod data;
pub mod exercise;
pub mod i18n;
pub mod playback;
pub mod protocol;
pub mod runtime;
#[cfg(feature = "asr")]
pub mod vosk_runtime;
pub mod vosk_download;
pub mod warmup;

pub use asr::AsrStatus;
pub use exercise::{CheckResult, Exercise, ExerciseStage, SpeechRating, UserAnswer};
pub use i18n::AppLanguage;
pub use protocol::{Command, ModelDownloadState, Screen};
pub use runtime::Engine;
