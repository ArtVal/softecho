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
pub mod protocol;
pub mod runtime;

pub use asr::AsrStatus;
pub use exercise::{CheckResult, Exercise, UserAnswer};
pub use protocol::{Command, Screen};
pub use runtime::Engine;
