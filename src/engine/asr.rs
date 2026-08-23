//! Опциональное распознавание речи (feature = "asr").
//! Без feature — заглушка: голос недоступен, UI остаётся на самопроверке.
//!
//! Движок (feature asr) устроен как unix-пайп: данные текут стадиями.
//!   mic | downsample | pipe | vosk → events

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ListenOutcome {
    pub text: String,
}

/// Параметры сессии записи.
#[derive(Clone)]
#[allow(dead_code)] // поля читает только vosk_impl при feature = "asr"
pub struct ListenConfig {
    /// Несколько фраз подряд, пока не Стоп / не истечёт лимит.
    pub continuous: bool,
    pub max_duration: Duration,
    /// Сигнал остановки с UI (длинный диктофон).
    pub stop: Option<Arc<AtomicBool>>,
    /// Живой текст текущей фразы (без очереди в канале — без роста памяти).
    pub live_partial: Option<Arc<Mutex<String>>>,
}

impl Default for ListenConfig {
    fn default() -> Self {
        Self {
            continuous: false,
            max_duration: Duration::from_secs(60),
            stop: None,
            live_partial: None,
        }
    }
}

#[allow(dead_code)] // вызывается из vosk_impl при feature = "asr"
impl ListenConfig {
    pub fn single_utterance(
        live_partial: Arc<Mutex<String>>,
        stop: Option<Arc<AtomicBool>>,
    ) -> Self {
        Self {
            continuous: false,
            max_duration: Duration::from_secs(60),
            stop,
            live_partial: Some(live_partial),
        }
    }

    pub fn long_dictaphone(stop: Arc<AtomicBool>, live_partial: Arc<Mutex<String>>) -> Self {
        Self {
            continuous: true,
            max_duration: Duration::from_secs(3 * 60 * 60),
            stop: Some(stop),
            live_partial: Some(live_partial),
        }
    }

    fn should_stop(&self) -> bool {
        self.stop
            .as_ref()
            .is_some_and(|s| s.load(Ordering::Relaxed))
    }

    fn set_live(&self, text: &str) {
        if let Some(live) = &self.live_partial {
            if let Ok(mut g) = live.lock() {
                if g.as_str() != text {
                    g.clear();
                    g.push_str(text);
                    if g.len() > 8_000 {
                        let keep = g.chars().rev().take(4_000).collect::<String>();
                        let keep: String = keep.chars().rev().collect();
                        *g = keep;
                    }
                }
            }
        }
    }
}

/// События: только редкие (фраза готова / конец). Partial — через `live_partial`.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum ListenEvent {
    /// Законченная фраза (длинный диктофон).
    Utterance(String),
    /// Vosk отстаёт: микрофон на паузе, разгребаем буфер — покажите «подождите».
    PleaseWait,
    /// Буфер обработан — можно снова говорить.
    ReadyAgain,
    Done(Result<ListenOutcome, String>),
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AsrStatus {
    Disabled,
    ModelMissing,
    Ready,
    Error(String),
}

pub trait SpeechRecognizer: Send {
    fn status(&self) -> AsrStatus;
    fn listen_stream(
        &mut self,
        grammar_words: &[String],
        events: Sender<ListenEvent>,
        config: ListenConfig,
    );
}

#[cfg(not(feature = "asr"))]
pub struct NullRecognizer;

#[cfg(not(feature = "asr"))]
impl SpeechRecognizer for NullRecognizer {
    fn status(&self) -> AsrStatus {
        AsrStatus::Disabled
    }

    fn listen_stream(
        &mut self,
        _grammar_words: &[String],
        events: Sender<ListenEvent>,
        _config: ListenConfig,
    ) {
        let _ = events.send(ListenEvent::Done(Err(
            "Распознавание не собрано (нужен --features asr)".into(),
        )));
    }
}

pub fn create_recognizer(model_dir: Option<&std::path::Path>) -> Box<dyn SpeechRecognizer> {
    #[cfg(feature = "asr")]
    {
        vosk_impl::create(model_dir)
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
    use crate::engine::audio_pipe::{
        catch_up_start_frames, chunk_rms, downsample_i16, pipe_capacity_frames, AudioPipe,
        FRAME_SAMPLES, TARGET_HZ,
    };
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use std::path::Path;
    use std::time::{Duration, Instant};
    use vosk::{DecodingState, Model, Recognizer};

    const PIPE_FRAMES: usize = pipe_capacity_frames();
    const CATCH_UP_START: usize = catch_up_start_frames();

    const SILENCE_AFTER_SPEECH: Duration = Duration::from_millis(1200);
    /// Короткая пауза для одного слога/слова — не держим кнопку «Сказать» вечно.
    const SILENCE_SINGLE_UTTERANCE: Duration = Duration::from_millis(850);
    const NO_SPEECH_TIMEOUT: Duration = Duration::from_secs(12);
    const SPEECH_RMS: f32 = 700.0;

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
        fn listen_stream(
            &mut self,
            _: &[String],
            events: Sender<ListenEvent>,
            _: ListenConfig,
        ) {
            let _ = events.send(ListenEvent::Done(Err(
                "Модель Vosk не найдена. См. README.".into(),
            )));
        }
    }

    struct ErrorRecognizer(String);
    impl SpeechRecognizer for ErrorRecognizer {
        fn status(&self) -> AsrStatus {
            AsrStatus::Error(self.0.clone())
        }
        fn listen_stream(
            &mut self,
            _: &[String],
            events: Sender<ListenEvent>,
            _: ListenConfig,
        ) {
            let _ = events.send(ListenEvent::Done(Err(self.0.clone())));
        }
    }

    impl SpeechRecognizer for VoskRecognizer {
        fn status(&self) -> AsrStatus {
            AsrStatus::Ready
        }

        fn listen_stream(
            &mut self,
            grammar_words: &[String],
            events: Sender<ListenEvent>,
            config: ListenConfig,
        ) {
            let result = run_pipeline(&self.model, grammar_words, &events, &config);
            config.set_live("");
            let _ = events.send(ListenEvent::Done(result));
        }
    }

    /// mic | downsample | pipe(120s) | vosk → events
    fn run_pipeline(
        model: &Model,
        grammar_words: &[String],
        events: &Sender<ListenEvent>,
        config: &ListenConfig,
    ) -> Result<ListenOutcome, String> {
        let pipe = AudioPipe::new(PIPE_FRAMES);
        let mic = open_mic(Arc::clone(&pipe))?;
        let outcome = vosk_stage(model, grammar_words, &pipe, events, config);
        pipe.set_pause(false);
        drop(mic);
        outcome
    }

    // ─── stage: mic | downsample ───────────────────────────────────────────

    struct MicSource {
        _stream: cpal::Stream,
    }

    /// Собирает PCM в кадры FRAME_SAMPLES и пишет в pipe.
    struct FrameWriter {
        pending: Mutex<Vec<i16>>,
        pipe: Arc<AudioPipe>,
        from_hz: u32,
    }

    impl FrameWriter {
        fn write_mono(&self, mono: &[i16]) {
            if self.pipe.is_paused() {
                // Пауза: не копить pending — иначе после resume всплеск.
                if let Ok(mut buf) = self.pending.lock() {
                    buf.clear();
                }
                return;
            }
            let down = downsample_i16(mono, self.from_hz, TARGET_HZ);
            if down.is_empty() {
                return;
            }
            let Ok(mut buf) = self.pending.lock() else {
                return;
            };
            buf.extend_from_slice(&down);
            while buf.len() >= FRAME_SAMPLES {
                let frame: Vec<i16> = buf.drain(..FRAME_SAMPLES).collect();
                self.pipe.send_frame(frame);
            }
        }
    }

    fn open_mic(pipe: Arc<AudioPipe>) -> Result<MicSource, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| "Микрофон не найден".to_string())?;
        let config_audio = device
            .default_input_config()
            .map_err(|e| format!("Настройка микрофона: {e}"))?;

        let sample_rate = config_audio.sample_rate().0;
        let channels = config_audio.channels() as usize;
        let err_fn = |err| eprintln!("Ошибка микрофона: {err}");

        let writer = Arc::new(FrameWriter {
            pending: Mutex::new(Vec::with_capacity(FRAME_SAMPLES * 2)),
            pipe,
            from_hz: sample_rate,
        });

        let stream = match config_audio.sample_format() {
            cpal::SampleFormat::F32 => {
                let w = Arc::clone(&writer);
                device.build_input_stream(
                    &config_audio.clone().into(),
                    move |data: &[f32], _| {
                        let mono: Vec<i16> = data
                            .chunks(channels)
                            .map(|frame| {
                                let s = frame.first().copied().unwrap_or(0.0);
                                (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
                            })
                            .collect();
                        w.write_mono(&mono);
                    },
                    err_fn,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let w = Arc::clone(&writer);
                device.build_input_stream(
                    &config_audio.clone().into(),
                    move |data: &[i16], _| {
                        let mono: Vec<i16> = data
                            .chunks(channels)
                            .map(|frame| *frame.first().unwrap_or(&0))
                            .collect();
                        w.write_mono(&mono);
                    },
                    err_fn,
                    None,
                )
            }
            other => {
                return Err(format!("Формат микрофона не поддерживается: {other:?}"));
            }
        }
        .map_err(|e| format!("Поток микрофона: {e}"))?;

        stream.play().map_err(|e| format!("Старт записи: {e}"))?;
        Ok(MicSource { _stream: stream })
    }

    // ─── stage: vosk ───────────────────────────────────────────────────────

    fn vosk_stage(
        model: &Model,
        grammar_words: &[String],
        pipe: &AudioPipe,
        events: &Sender<ListenEvent>,
        config: &ListenConfig,
    ) -> Result<ListenOutcome, String> {
        let mut recognizer = make_recognizer(model, TARGET_HZ, grammar_words)?;
        recognizer.set_words(true);
        recognizer.set_partial_words(true);

        let session_start = Instant::now();
        let silence_after = if config.continuous {
            SILENCE_AFTER_SPEECH
        } else {
            SILENCE_SINGLE_UTTERANCE
        };
        let mut last_single = String::new();
        let mut got_any = false;
        let mut utter_start = Instant::now();
        let mut heard_speech = false;
        let mut last_loud = Instant::now();
        let mut last_partial = String::new();
        let mut catching_up = false;

        // Закрыть фразу. prefer_result — после DecodingState::Finalized (result()).
        // Иначе — final_result() по тишине/Стоп. Запасной путь — last_partial.
        let commit_utterance =
            |recognizer: &mut Recognizer,
             events: &Sender<ListenEvent>,
             config: &ListenConfig,
             last_partial: &mut String,
             got_any: &mut bool,
             last_single: &mut String,
             prefer_result: bool|
             -> bool {
                let mut text = if prefer_result {
                    recognizer
                        .result()
                        .single()
                        .map(|s| s.text.to_string())
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                if text.trim().is_empty() {
                    text = finalize_phrase(recognizer).unwrap_or_default();
                }
                if text.trim().is_empty() {
                    text = last_partial.trim().to_string();
                }
                last_partial.clear();
                config.set_live("");
                let text = text.trim().to_string();
                if text.is_empty() {
                    return false;
                }
                *got_any = true;
                if config.continuous {
                    let _ = events.send(ListenEvent::Utterance(text));
                    false
                } else {
                    *last_single = text;
                    true
                }
            };

        loop {
            if config.should_stop() || session_start.elapsed() >= config.max_duration {
                break;
            }

            // Буфер почти полный → пауза mic, просьба подождать, разгрести очередь.
            if !catching_up && pipe.len() >= CATCH_UP_START {
                catching_up = true;
                pipe.set_pause(true);
                let _ = events.send(ListenEvent::PleaseWait);
            }

            let samples = if catching_up {
                match pipe.try_recv() {
                    Some(s) => Some(s),
                    None => {
                        catching_up = false;
                        pipe.set_pause(false);
                        let _ = events.send(ListenEvent::ReadyAgain);
                        utter_start = Instant::now();
                        None
                    }
                }
            } else {
                pipe.recv_timeout(Duration::from_millis(20))
            };

            let Some(samples) = samples else {
                if catching_up {
                    continue;
                }
                if heard_speech && last_loud.elapsed() >= silence_after {
                    let stop = commit_utterance(
                        &mut recognizer,
                        events,
                        config,
                        &mut last_partial,
                        &mut got_any,
                        &mut last_single,
                        false,
                    );
                    heard_speech = false;
                    utter_start = Instant::now();
                    last_loud = Instant::now();
                    if stop {
                        break;
                    }
                } else {
                    let wait_limit = if config.continuous && got_any {
                        Duration::from_secs(90)
                    } else {
                        NO_SPEECH_TIMEOUT
                    };
                    if !heard_speech && utter_start.elapsed() >= wait_limit {
                        if config.continuous {
                            if config.should_stop() {
                                break;
                            }
                            utter_start = Instant::now();
                        } else {
                            return Err("Не услышала речь. Попробуйте ближе к микрофону.".into());
                        }
                    }
                }
                continue;
            };

            let loud = chunk_rms(&samples) >= SPEECH_RMS;
            if loud {
                heard_speech = true;
                // При стабильном partial фоновый шум не должен сбрасывать таймер паузы.
                if last_partial.is_empty() {
                    last_loud = Instant::now();
                }
            }

            match recognizer.accept_waveform(&samples) {
                Ok(DecodingState::Finalized) if heard_speech || !last_partial.is_empty() => {
                    let stop = commit_utterance(
                        &mut recognizer,
                        events,
                        config,
                        &mut last_partial,
                        &mut got_any,
                        &mut last_single,
                        true,
                    );
                    heard_speech = false;
                    utter_start = Instant::now();
                    last_loud = Instant::now();
                    if stop {
                        break;
                    }
                }
                Ok(DecodingState::Running) => {
                    let partial = recognizer.partial_result().partial.trim().to_string();
                    if !partial.is_empty() {
                        heard_speech = true;
                        // Не трогаем last_loud при том же partial — иначе пауза никогда не сработает.
                        if partial != last_partial {
                            last_loud = Instant::now();
                            last_partial.clear();
                            last_partial.push_str(&partial);
                            config.set_live(&partial);
                        }
                    }
                }
                Ok(DecodingState::Finalized) | Ok(DecodingState::Failed) | Err(_) => {}
            }

            if heard_speech && last_loud.elapsed() >= silence_after {
                let stop = commit_utterance(
                    &mut recognizer,
                    events,
                    config,
                    &mut last_partial,
                    &mut got_any,
                    &mut last_single,
                    false,
                );
                heard_speech = false;
                utter_start = Instant::now();
                last_loud = Instant::now();
                if stop {
                    break;
                }
            }
        }

        if catching_up {
            pipe.set_pause(false);
            let _ = events.send(ListenEvent::ReadyAgain);
        }

        if heard_speech || !last_partial.is_empty() {
            for frame in pipe.drain() {
                let _ = recognizer.accept_waveform(&frame);
            }
            let _ = commit_utterance(
                &mut recognizer,
                events,
                config,
                &mut last_partial,
                &mut got_any,
                &mut last_single,
                false,
            );
        }

        config.set_live("");

        if config.continuous {
            Ok(ListenOutcome {
                text: String::new(),
            })
        } else if last_single.is_empty() {
            if config.should_stop() {
                Ok(ListenOutcome {
                    text: String::new(),
                })
            } else {
                Err("Ничего не записала".into())
            }
        } else {
            Ok(ListenOutcome { text: last_single })
        }
    }

    fn finalize_phrase(recognizer: &mut Recognizer) -> Option<String> {
        let text = recognizer
            .final_result()
            .single()
            .map(|s| s.text.to_string())
            .unwrap_or_default()
            .trim()
            .to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }

    // ─── helpers ───────────────────────────────────────────────────────────

    fn make_recognizer(
        model: &Model,
        sample_rate: u32,
        grammar_words: &[String],
    ) -> Result<Recognizer, String> {
        if grammar_words.is_empty() {
            Recognizer::new(model, sample_rate as f32)
                .ok_or_else(|| "Не удалось создать распознаватель".to_string())
        } else {
            let phrase = grammar_words.join(" ");
            let phrases = [phrase.as_str(), "[unk]"];
            Recognizer::new_with_grammar(model, sample_rate as f32, &phrases)
                .ok_or_else(|| "Не удалось создать распознаватель с грамматикой".to_string())
        }
    }

}
