//! Опциональное распознавание речи (feature = "asr").
//! Без feature — заглушка: голос недоступен, UI остаётся на самопроверке.

#[derive(Debug, Clone)]
pub struct ListenOutcome {
    pub text: String,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // варианты используются при feature = "asr" и в UI
pub enum AsrStatus {
    Disabled,
    ModelMissing,
    Ready,
    Error(String),
}

pub trait SpeechRecognizer: Send {
    fn status(&self) -> AsrStatus;
    /// Короткая запись и распознавание. `grammar_words` — узкий словарь (если поддерживается).
    fn listen_once(&mut self, grammar_words: &[String]) -> Result<ListenOutcome, String>;
}

/// Заглушка без feature `asr`.
pub struct NullRecognizer;

impl SpeechRecognizer for NullRecognizer {
    fn status(&self) -> AsrStatus {
        AsrStatus::Disabled
    }

    fn listen_once(&mut self, _grammar_words: &[String]) -> Result<ListenOutcome, String> {
        Err("Распознавание не собрано (нужен --features asr)".into())
    }
}

pub fn create_recognizer(model_dir: Option<&std::path::Path>) -> Box<dyn SpeechRecognizer> {
    #[cfg(feature = "asr")]
    {
        return vosk_impl::create(model_dir);
    }
    #[cfg(not(feature = "asr"))]
    {
        let _ = model_dir;
        Box::new(NullRecognizer)
    }
}

#[cfg(feature = "asr")]
mod vosk_impl {
    use super::*;
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};
    use vosk::{Model, Recognizer};

    pub struct VoskRecognizer {
        model: Model,
    }

    pub fn create(model_dir: Option<&Path>) -> Box<dyn SpeechRecognizer> {
        let Some(dir) = model_dir else {
            return Box::new(MissingModel);
        };
        match Model::new(dir.to_string_lossy().as_ref()) {
            Some(model) => Box::new(VoskRecognizer { model }),
            None => Box::new(ErrorRecognizer(format!(
                "Не удалось загрузить модель Vosk из {}",
                dir.display()
            ))),
        }
    }

    struct MissingModel;
    impl SpeechRecognizer for MissingModel {
        fn status(&self) -> AsrStatus {
            AsrStatus::ModelMissing
        }
        fn listen_once(&mut self, _: &[String]) -> Result<ListenOutcome, String> {
            Err("Модель Vosk не найдена. См. README.".into())
        }
    }

    struct ErrorRecognizer(String);
    impl SpeechRecognizer for ErrorRecognizer {
        fn status(&self) -> AsrStatus {
            AsrStatus::Error(self.0.clone())
        }
        fn listen_once(&mut self, _: &[String]) -> Result<ListenOutcome, String> {
            Err(self.0.clone())
        }
    }

    impl SpeechRecognizer for VoskRecognizer {
        fn status(&self) -> AsrStatus {
            AsrStatus::Ready
        }

        fn listen_once(&mut self, grammar_words: &[String]) -> Result<ListenOutcome, String> {
            let host = cpal::default_host();
            let device = host
                .default_input_device()
                .ok_or_else(|| "Микрофон не найден".to_string())?;
            let config = device
                .default_input_config()
                .map_err(|e| format!("Настройка микрофона: {e}"))?;

            let sample_rate = config.sample_rate().0;
            let mut recognizer = if grammar_words.is_empty() {
                Recognizer::new(&self.model, sample_rate as f32)
                    .ok_or_else(|| "Не удалось создать распознаватель".to_string())?
            } else {
                let phrase = grammar_words.join(" ");
                let phrases = [phrase.as_str(), "[unk]"];
                Recognizer::new_with_grammar(&self.model, sample_rate as f32, &phrases)
                    .ok_or_else(|| "Не удалось создать распознаватель с грамматикой".to_string())?
            };

            let buffer: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
            let buffer_cb = Arc::clone(&buffer);
            let channels = config.channels() as usize;
            let err_fn = |err| eprintln!("Ошибка микрофона: {err}");

            let stream = match config.sample_format() {
                cpal::SampleFormat::F32 => device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        let mut b = buffer_cb.lock().unwrap();
                        for frame in data.chunks(channels) {
                            let s = frame.first().copied().unwrap_or(0.0);
                            b.push((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16);
                        }
                    },
                    err_fn,
                    None,
                ),
                cpal::SampleFormat::I16 => device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        let mut b = buffer_cb.lock().unwrap();
                        for frame in data.chunks(channels) {
                            b.push(*frame.first().unwrap_or(&0));
                        }
                    },
                    err_fn,
                    None,
                ),
                other => {
                    return Err(format!("Формат микрофона не поддерживается: {other:?}"));
                }
            }
            .map_err(|e| format!("Поток микрофона: {e}"))?;

            stream.play().map_err(|e| format!("Старт записи: {e}"))?;

            let deadline = Instant::now() + Duration::from_secs(4);
            while Instant::now() < deadline {
                let chunk = {
                    let mut b = buffer.lock().unwrap();
                    if b.is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut *b))
                    }
                };
                if let Some(samples) = chunk {
                    let _ = recognizer.accept_waveform(&samples);
                }
                std::thread::sleep(Duration::from_millis(50));
            }

            drop(stream);

            // Дочитываем остаток
            {
                let mut b = buffer.lock().unwrap();
                if !b.is_empty() {
                    let _ = recognizer.accept_waveform(&b);
                    b.clear();
                }
            }

            let result = recognizer.final_result();
            let text = result
                .single()
                .map(|s| s.text.to_string())
                .unwrap_or_default()
                .trim()
                .to_string();

            if text.is_empty() {
                return Err("Не расслышал, попробуйте ещё раз".into());
            }

            Ok(ListenOutcome { text })
        }
    }
}
