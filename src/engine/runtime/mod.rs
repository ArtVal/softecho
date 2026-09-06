//! Состояние и логика тренажёра (серверная часть).
//! UI / будущий клиент общаются только через Command + геттеры + tick.

mod pack_editor;
mod listen;

pub use pack_editor::PackEditorState;

use listen::ListenPurpose;

use super::asr::{
    create_recognizer, AsrStatus, ListenEvent, SpeechRecognizer,
};
use super::data::{
    list_packs_for, load_active_pack, load_pack, load_progress, new_report_path,
    pack_matches_language, save_progress, save_report_text, user_data_dir, vosk_model_dir,
    DEFAULT_PACK_ID, PackCatalogEntry,
};
use super::exercise::{
    build_diagnosis_set, check_answer, infer_level, order_session_for_level_with_map,
    twister_unlocked, pack_speech_entries, CheckResult, Exercise, ExercisePack, ExerciseStage,
    Progress, SpeechMapEntry, UserAnswer,
};
use super::i18n::{AppLanguage, UiText};
use super::playback::play_pcm_16k;
use std::collections::{HashMap, HashSet};
use super::protocol::{Command, ModelDownloadState, Screen, TickResult};
use super::vosk_download::{spawn_model_download, DownloadMsg};

use rand::seq::SliceRandom;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;


pub struct DictaphoneState {
    pub listening: bool,
    /// Весь накопленный текст длинной записи (может быть очень большим).
    pub transcript: String,
    /// Текущая фраза (потоком) — копия из `live_partial`.
    pub live_text: String,
    /// Общий буфер с ASR-потоком (без очереди Partial).
    live_partial: Arc<Mutex<String>>,
    pub error: Option<String>,
    stop: Option<Arc<AtomicBool>>,
    /// Файл сессии на диске (дописывается по фразам).
    save_path: Option<PathBuf>,
    pub save_note: Option<String>,
}

impl Default for DictaphoneState {
    fn default() -> Self {
        Self {
            listening: false,
            transcript: String::new(),
            live_text: String::new(),
            live_partial: Arc::new(Mutex::new(String::new())),
            error: None,
            stop: None,
            save_path: None,
            save_note: None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionKind {
    Practice,
    Diagnosis,
}

/// Сколько раз за занятие можно вернуть одно и то же задание в очередь.
const MAX_REQUEUE_PER_KEY: u32 = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
struct PendingAdvance {
    result: CheckResult,
}

pub struct SessionState {
    pub exercises: Vec<Exercise>,
    pub index: usize,
    pub correct: u32,
    /// Перемешанные варианты для «выбор слова».
    pub choice_options: Vec<String>,
    /// Для «собрать фразу»: доступные и выбранные слова.
    pub pool: Vec<String>,
    pub picked: Vec<String>,
    pub listening: bool,
    pub listen_error: Option<String>,
    /// Текст по мере распознавания / после записи (подсказка ASR).
    pub live_text: String,
    /// Мягкая оценка ASR после «Сказать» (не зачёт — ждём самопроверку).
    pub asr_hint_ok: Option<bool>,
    kind: SessionKind,
    /// Итоги по заданиям диагностики (ступень, верно?).
    outcomes: Vec<(ExerciseStage, bool)>,
    /// Горячий приоритет повтора в этом занятии (растёт с каждой неудачей).
    session_boost: HashMap<String, u32>,
    /// Ключи, которые пользователь попросил не возвращать в этой сессии.
    skip_repeat: HashSet<String>,
    /// Сколько раз уже вернули задание в очередь за сессию.
    requeue_count: HashMap<String, u32>,
    /// Длина очереди при старте (без последующих возвратов).
    initial_exercise_count: u32,
    /// Результат последнего ответа — обрабатывается при «Дальше».
    pending_advance: Option<PendingAdvance>,
}

pub struct Engine {
    screen: Screen,
    pack: ExercisePack,
    progress: Progress,
    session: Option<SessionState>,
    load_error: Option<String>,
    save_error: Option<String>,
    recognizer: Arc<Mutex<Box<dyn SpeechRecognizer>>>,
    /// Фоновый поток распознавания (частичный текст + финал).
    listen_rx: Option<Receiver<ListenEvent>>,
    /// JoinHandle listen-воркера: не стартовать второй, пока старый жив (после abort без join).
    listen_join: Option<thread::JoinHandle<()>>,
    listen_target: Option<String>,
    listen_purpose: Option<ListenPurpose>,
    /// Живой partial текущей записи (ASR пишет сюда, UI читает).
    listen_live: Arc<Mutex<String>>,
    /// Сигнал «Готово» на экране упражнения.
    exercise_listen_stop: Option<Arc<AtomicBool>>,
    /// Vosk разгребает буфер — показать «подождите».
    please_wait: bool,
    /// Последняя запись (упражнение или диктофон) для «Послушать».
    last_clip: Vec<i16>,
    playback_stop: Option<Arc<AtomicBool>>,
    playback_busy: Arc<AtomicBool>,
    /// Ошибка из потока воспроизведения (заполняет `play_pcm_16k`).
    playback_last_error: Arc<Mutex<Option<String>>>,
    /// Текст для UI после неудачного «Послушать».
    playback_error: Option<String>,
    /// После Stop по PlayLastClip — снова запустить, когда поток освободится.
    playback_pending_replay: bool,
    dictaphone: DictaphoneState,
    model_download: ModelDownloadState,
    model_download_rx: Option<Receiver<DownloadMsg>>,
    /// Отмена фонового скачивания (смена языка / новый старт).
    model_download_cancel: Option<Arc<AtomicBool>>,
    /// Поток скачивания: join перед новым стартом, чтобы unpack не писал после cancel.
    model_download_join: Option<std::thread::JoinHandle<()>>,
    model_download_note: Option<String>,
    pack_editor: Option<PackEditorState>,
    /// Кэш статуса ASR, если mutex занят listen'ом.
    asr_status_cache: AsrStatus,
    /// Результат последнего экспорта отчёта (путь или ошибка).
    report_export_note: Option<String>,
    /// abort_listen оставил живой listen-воркер — reload mutex отложен до tick.
    pending_recognizer_reload: bool,
}

impl Engine {
    pub fn new() -> Self {
        let (progress, warn) = load_progress();
        let model = vosk_model_dir(progress.language);
        Self::create(progress, warn, model)
    }

    fn create(
        mut progress: Progress,
        progress_warn: Option<String>,
        model: Option<std::path::PathBuf>,
    ) -> Self {
        let fallback = progress.language.default_pack_id();
        if let Some(id) = progress.pack_id.clone() {
            if !pack_matches_language(&id, progress.language) {
                progress.set_pack(fallback);
            }
        }
        let (pack, pack_error) = match load_active_pack(&progress) {
            Ok(p) => (p, None),
            Err(e) => match load_pack(fallback).or_else(|_| load_pack(DEFAULT_PACK_ID)) {
                Ok(p) => (p, Some(e)),
                Err(e2) => (
                    ExercisePack {
                        title: "Пусто".into(),
                        exercises: vec![],
                    },
                    Some(format!("{e}; {e2}")),
                ),
            },
        };
        let load_error = progress_warn.or(pack_error);

        let recognizer = Arc::new(Mutex::new(create_recognizer(model.as_deref())));
        let asr_status_cache = recognizer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .status();

        Self {
            screen: Screen::Home,
            pack,
            progress,
            session: None,
            load_error,
            save_error: None,
            recognizer,
            listen_rx: None,
            listen_join: None,
            listen_target: None,
            listen_purpose: None,
            listen_live: Arc::new(Mutex::new(String::new())),
            exercise_listen_stop: None,
            please_wait: false,
            last_clip: Vec::new(),
            playback_stop: None,
            playback_busy: Arc::new(AtomicBool::new(false)),
            playback_last_error: Arc::new(Mutex::new(None)),
            playback_error: None,
            playback_pending_replay: false,
            dictaphone: DictaphoneState::default(),
            model_download: ModelDownloadState::default(),
            model_download_rx: None,
            model_download_cancel: None,
            model_download_join: None,
            model_download_note: None,
            pack_editor: None,
            asr_status_cache,
            report_export_note: None,
            pending_recognizer_reload: false,
        }
    }

    /// Движок без загрузки модели Vosk (юнит-тесты логики).
    #[cfg(test)]
    fn new_logic_only() -> Self {
        Self::create(Progress::default(), None, None)
    }

    fn reload_recognizer(&mut self) {
        self.abort_listen();
        if self.listen_worker_busy() {
            // Воркер ещё держит Mutex — не блокируемся; доделаем в tick.
            self.pending_recognizer_reload = true;
            return;
        }
        self.apply_recognizer_reload();
    }

    fn apply_recognizer_reload(&mut self) {
        self.reap_finished_listen_worker();
        let model = vosk_model_dir(self.progress.language);
        let mut r = self.recognizer.lock().unwrap_or_else(|e| e.into_inner());
        *r = create_recognizer(model.as_deref());
        self.asr_status_cache = r.status();
        self.pending_recognizer_reload = false;
    }

    fn cancel_model_download(&mut self) {
        if let Some(cancel) = &self.model_download_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        // JoinHandle оставляем: start_model_download дождётся конца unpack.
        self.model_download_rx = None;
        self.model_download = ModelDownloadState::Idle;
        self.model_download_note = None;
    }

    /// Дождаться завершения воркера скачивания (после cancel или перед новым стартом).
    fn join_model_download_worker(&mut self) {
        if let Some(cancel) = &self.model_download_cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.model_download_join.take() {
            let _ = handle.join();
        }
        self.model_download_cancel = None;
        self.model_download_rx = None;
    }

    fn set_language(&mut self, language: AppLanguage) {
        if self.progress.language == language {
            return;
        }
        self.abort_listen();
        self.session = None;
        // Остановить поток прошлой модели: иначе tmp/модель дерутся со сменой языка.
        self.cancel_model_download();
        self.join_model_download_worker();
        self.progress.set_language(language);
        let fallback = language.default_pack_id();
        let current = self.pack_id().to_string();
        if !pack_matches_language(&current, language) {
            match load_pack(fallback) {
                Ok(pack) => {
                    self.pack = pack;
                    self.progress.set_pack(fallback);
                    self.load_error = None;
                }
                Err(e) => self.load_error = Some(e),
            }
        }
        self.reload_recognizer();
        self.persist_progress();
    }

    fn start_model_download(&mut self) {
        if self.model_download_rx.is_some() {
            return;
        }
        // Предыдущий cancel мог оставить живой unpack — не стартуем параллельно.
        self.join_model_download_worker();
        if matches!(self.asr_status(), AsrStatus::Disabled) {
            return;
        }
        if matches!(self.asr_status(), AsrStatus::Ready) {
            self.model_download_note =
                Some(super::i18n::tr(self.progress.language, "model_installed").into());
            return;
        }

        let dest = match user_data_dir() {
            Ok(p) => p,
            Err(e) => {
                self.model_download = ModelDownloadState::Failed(e);
                return;
            }
        };

        let language = self.progress.language;
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        self.model_download_rx = Some(rx);
        self.model_download_cancel = Some(Arc::clone(&cancel));
        self.model_download = ModelDownloadState::Working {
            label: super::i18n::tr(language, "download_prepare").into(),
            percent: None,
        };
        self.model_download_note = None;
        self.model_download_join = Some(spawn_model_download(dest, language, tx, cancel));
    }

    fn poll_model_download(&mut self, tick: &mut TickResult) {
        let Some(rx) = self.model_download_rx.as_ref() else {
            return;
        };
        let mut messages = Vec::new();
        while let Ok(msg) = rx.try_recv() {
            messages.push(msg);
        }
        for msg in messages {
            match msg {
                DownloadMsg::Phase(label) => {
                    let percent = match &self.model_download {
                        ModelDownloadState::Working { percent, .. } => *percent,
                        _ => None,
                    };
                    self.model_download = ModelDownloadState::Working { label, percent };
                    tick.want_repaint = true;
                }
                DownloadMsg::Percent(p) => {
                    let label = match &self.model_download {
                        ModelDownloadState::Working { label, .. } => label.clone(),
                        _ => "Скачиваю…".into(),
                    };
                    self.model_download = ModelDownloadState::Working {
                        label,
                        percent: Some(p),
                    };
                    tick.want_repaint = true;
                }
                DownloadMsg::Done => {
                    self.model_download_rx = None;
                    self.model_download_cancel = None;
                    if let Some(handle) = self.model_download_join.take() {
                        let _ = handle.join();
                    }
                    self.reload_recognizer();
                    match self.asr_status() {
                        AsrStatus::Ready => {
                            self.model_download = ModelDownloadState::Succeeded;
                            self.model_download_note =
                                Some("Модель установлена. Голос готов.".into());
                        }
                        AsrStatus::ModelMissing => {
                            self.model_download = ModelDownloadState::Failed(
                                "Файлы скачаны, но модель не загрузилась. \
                                 Перезапустите приложение."
                                    .into(),
                            );
                            self.model_download_note = None;
                        }
                        AsrStatus::Error(e) => {
                            self.model_download = ModelDownloadState::Failed(format!(
                                "Модель на диске, но не открылась: {e}"
                            ));
                            self.model_download_note = None;
                        }
                        AsrStatus::Disabled => {
                            self.model_download = ModelDownloadState::Failed(
                                "Голос выключен в этой сборке.".into(),
                            );
                            self.model_download_note = None;
                        }
                    }
                    tick.want_repaint = true;
                }
                DownloadMsg::Err(e) => {
                    self.model_download_rx = None;
                    self.model_download_cancel = None;
                    if let Some(handle) = self.model_download_join.take() {
                        let _ = handle.join();
                    }
                    self.model_download = ModelDownloadState::Failed(e);
                    tick.want_repaint = true;
                }
            }
        }
    }

    fn persist_progress(&mut self) {
        // Юнит-тесты без with_temp_xdg_data_home не пишут в реальный progress.json.
        #[cfg(test)]
        if std::env::var_os("SOFTECHO_ALLOW_PROGRESS_WRITE").is_none() {
            return;
        }
        match save_progress(&self.progress) {
            Ok(()) => self.save_error = None,
            Err(e) => self.save_error = Some(e),
        }
    }

    fn start_session(&mut self) {
        if self.progress.level.is_none() && self.progress.simple_mode {
            // В упрощённом режиме без уровня начинаем со звуков — без экрана выбора.
            self.progress.set_level(ExerciseStage::Sound);
            self.persist_progress();
        }
        let Some(level) = self.progress.level else {
            self.screen = Screen::LevelPick;
            return;
        };
        let include_twister = twister_unlocked(
            self.progress.level,
            &self.pack,
            &self.progress.speech_map,
        );
        let exercises = order_session_for_level_with_map(
            self.pack.exercises.clone(),
            level,
            &self.progress.speech_map,
            include_twister,
        );
        if exercises.is_empty() {
            self.load_error = Some(
                "В этом наборе нет заданий с выбранного уровня. Выберите другой уровень или набор."
                    .into(),
            );
            self.screen = Screen::Home;
            return;
        }
        self.load_error = None;
        let mut session = SessionState {
            exercises,
            index: 0,
            correct: 0,
            choice_options: vec![],
            pool: vec![],
            picked: vec![],
            listening: false,
            listen_error: None,
            live_text: String::new(),
            asr_hint_ok: None,
            kind: SessionKind::Practice,
            outcomes: vec![],
            session_boost: HashMap::new(),
            skip_repeat: HashSet::new(),
            requeue_count: HashMap::new(),
            initial_exercise_count: 0,
            pending_advance: None,
        };
        session.initial_exercise_count = session.exercises.len() as u32;
        session.prepare_current();
        self.session = Some(session);
        self.screen = Screen::Exercise;
    }

    fn start_diagnosis(&mut self) {
        const PER_STAGE: usize = 2;
        let exercises = build_diagnosis_set(&self.pack.exercises, PER_STAGE);
        if exercises.is_empty() {
            self.load_error = Some(
                "В этом наборе нет заданий для диагностики. Выберите другой набор.".into(),
            );
            self.screen = Screen::Home;
            return;
        }
        self.load_error = None;
        let mut session = SessionState {
            exercises,
            index: 0,
            correct: 0,
            choice_options: vec![],
            pool: vec![],
            picked: vec![],
            listening: false,
            listen_error: None,
            live_text: String::new(),
            asr_hint_ok: None,
            kind: SessionKind::Diagnosis,
            outcomes: vec![],
            session_boost: HashMap::new(),
            skip_repeat: HashSet::new(),
            requeue_count: HashMap::new(),
            initial_exercise_count: 0,
            pending_advance: None,
        };
        session.initial_exercise_count = session.exercises.len() as u32;
        session.prepare_current();
        self.session = Some(session);
        self.screen = Screen::Exercise;
    }

    fn set_level(&mut self, level: ExerciseStage) {
        if level == ExerciseStage::Twister
            && !twister_unlocked(
                self.progress.level,
                &self.pack,
                &self.progress.speech_map,
            )
        {
            self.load_error = Some(
                "Скороговорки пока закрыты: нужен уровень «Фразы» или ≥70% «получается» на фразах набора."
                    .into(),
            );
            self.screen = Screen::LevelPick;
            return;
        }
        self.progress.set_level(level);
        self.persist_progress();
        self.session = None;
        self.screen = Screen::Home;
    }

    fn set_pack(&mut self, pack_id: &str) {
        match load_pack(pack_id) {
            Ok(pack) => {
                self.pack = pack;
                self.progress.set_pack(pack_id);
                self.persist_progress();
                self.load_error = None;
                self.session = None;
                self.screen = Screen::Home;
            }
            Err(e) => self.load_error = Some(e),
        }
    }

    pub fn current_exercise(&self) -> Option<&Exercise> {
        let s = self.session.as_ref()?;
        s.exercises.get(s.index)
    }

    fn submit(&mut self, answer: UserAnswer) {
        // Отменяем отложенный ASR, чтобы не зачесть ответ дважды.
        self.abort_listen();

        let heard_fallback = self
            .session
            .as_ref()
            .map(|s| s.live_text.trim().to_string())
            .filter(|t| !t.is_empty());
        let answer = match answer {
            UserAnswer::ReadDone {
                matched,
                heard: None,
            } => UserAnswer::ReadDone {
                matched,
                heard: heard_fallback,
            },
            other => other,
        };

        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.listening = false;
        session.asr_hint_ok = None;
        let Some(ex) = session.exercises.get(session.index).cloned() else {
            return;
        };
        let result = check_answer(&ex, &answer);
        if result == CheckResult::Correct {
            session.correct += 1;
            if let Some(key) = ex.map_key() {
                session.session_boost.remove(&key);
            }
        }
        if session.kind == SessionKind::Diagnosis {
            session
                .outcomes
                .push((ex.stage(), result == CheckResult::Correct));
        }
        session.pending_advance = Some(PendingAdvance { result });
        let heard = match &answer {
            UserAnswer::ReadDone { heard, .. } => heard.clone(),
            _ => None,
        };
        let expected = match &ex {
            Exercise::ReadAloud { text, .. } => Some(text.clone()),
            Exercise::ChooseWord { answer, .. } | Exercise::BuildPhrase { answer, .. } => {
                Some(answer.clone())
            }
        };
        self.progress
            .record_speech(&ex, result == CheckResult::Correct);
        self.persist_progress();
        self.screen = Screen::Feedback {
            result,
            heard,
            expected,
        };
    }

    fn advance_after_feedback(&mut self, skip_repeat: bool) {
        self.abort_listen();
        self.last_clip.clear();
        let Some(session) = self.session.as_mut() else {
            return;
        };
        let Some(pending) = session.pending_advance.take() else {
            session.index += 1;
            if session.index >= session.exercises.len() {
                self.finish_session();
            } else {
                session.prepare_current();
                self.screen = Screen::Exercise;
            }
            return;
        };
        let Some(ex) = session.exercises.get(session.index).cloned() else {
            return;
        };

        if session.kind == SessionKind::Practice && pending.result == CheckResult::Incorrect {
            if skip_repeat {
                if let Some(key) = ex.map_key() {
                    session.skip_repeat.insert(key);
                }
            } else {
                Self::maybe_requeue_failed(session, ex);
            }
        }

        session.index += 1;
        if session.index >= session.exercises.len() {
            self.finish_session();
        } else {
            session.prepare_current();
            self.screen = Screen::Exercise;
        }
    }

    fn finish_session(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        match session.kind {
            SessionKind::Diagnosis => {
                let level = infer_level(&session.outcomes);
                self.progress.set_level(level);
                self.persist_progress();
                self.session = None;
                self.screen = Screen::DiagnosisResult { level };
            }
            SessionKind::Practice => {
                let correct = session.correct;
                let total = session.exercises.len() as u32;
                let unique = session.initial_exercise_count;
                self.progress.record_session(correct, total);
                self.persist_progress();
                self.session = None;
                self.screen = Screen::Result {
                    correct,
                    total,
                    unique,
                };
            }
        }
    }

    /// Вернуть неудачное задание в хвост очереди — чем больше неудач, тем раньше.
    fn maybe_requeue_failed(session: &mut SessionState, ex: Exercise) {
        let Some(key) = ex.map_key() else {
            return;
        };
        if session.skip_repeat.contains(&key) {
            return;
        }
        let times = session.requeue_count.entry(key.clone()).or_insert(0);
        if *times >= MAX_REQUEUE_PER_KEY {
            return;
        }
        *times += 1;
        let boost = session.session_boost.entry(key).or_insert(0);
        *boost += 1;
        let boost = *boost;

        let mut insert_at = session.index + 1;
        while insert_at < session.exercises.len() {
            let other_boost = session.exercises[insert_at]
                .map_key()
                .and_then(|k| session.session_boost.get(&k).copied())
                .unwrap_or(0);
            if other_boost < boost {
                break;
            }
            insert_at += 1;
        }
        session.exercises.insert(insert_at, ex);
    }

    fn store_last_clip(&mut self, pcm: Vec<i16>) {
        if pcm.is_empty() {
            return;
        }
        self.last_clip = pcm;
    }

    fn play_last_clip(&mut self) {
        if self.last_clip.is_empty() {
            self.playback_error = Some("Нет записи для прослушивания.".into());
            return;
        }
        if self.playback_busy.load(Ordering::Relaxed) {
            // Остановить текущее и поставить повтор (UI шлёт Stop отдельно).
            if let Some(stop) = &self.playback_stop {
                stop.store(true, Ordering::Relaxed);
            }
            self.playback_stop = None;
            self.playback_pending_replay = true;
            return;
        }
        self.start_last_clip_playback();
    }

    fn start_last_clip_playback(&mut self) {
        if self.last_clip.is_empty() {
            self.playback_pending_replay = false;
            return;
        }
        self.playback_pending_replay = false;
        self.playback_error = None;
        if let Ok(mut g) = self.playback_last_error.lock() {
            *g = None;
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.playback_stop = Some(Arc::clone(&stop));
        play_pcm_16k(
            self.last_clip.clone(),
            stop,
            Arc::clone(&self.playback_busy),
            Arc::clone(&self.playback_last_error),
        );
    }

    fn stop_playback(&mut self) {
        self.playback_pending_replay = false;
        if let Some(stop) = &self.playback_stop {
            stop.store(true, Ordering::Relaxed);
        }
        self.playback_stop = None;
    }

    pub fn tick(&mut self) -> TickResult {
        let mut tick = TickResult::default();
        self.poll_model_download(&mut tick);
        if let Ok(mut g) = self.playback_last_error.lock() {
            if let Some(e) = g.take() {
                self.playback_error = Some(e);
                tick.want_repaint = true;
            }
        }
        if self.playback_pending_replay && !self.playback_busy.load(Ordering::Relaxed) {
            self.start_last_clip_playback();
            tick.want_repaint = true;
        }
        if matches!(self.model_download, ModelDownloadState::Working { .. }) {
            tick.want_repaint = true;
            tick.repaint_after.get_or_insert(Duration::from_millis(200));
        }
        if self.playback_busy.load(Ordering::Relaxed) || self.playback_pending_replay {
            tick.want_repaint = true;
            tick.repaint_after.get_or_insert(Duration::from_millis(100));
        }
        if self.pending_recognizer_reload {
            if !self.listen_worker_busy() {
                self.apply_recognizer_reload();
                tick.want_repaint = true;
            } else {
                tick.want_repaint = true;
                tick.repaint_after.get_or_insert(Duration::from_millis(50));
            }
        }
        self.poll_listen_events(&mut tick);
        tick
    }

    /// Единый выход на Home (GoHome и Leave*): flush listen, экранные подчистки, сброс сессии.
    fn leave_to_home(&mut self) {
        if matches!(self.screen, Screen::PackEditor) {
            self.leave_pack_editor(false);
            if matches!(self.screen, Screen::PackEditor) {
                return;
            }
        }
        // Сначала flush listen (PCM/хвост), потом очистка диктофона.
        self.abort_listen();
        if matches!(self.screen, Screen::Dictaphone) {
            self.clear_dictaphone_buffer();
        }
        if matches!(self.screen, Screen::Settings) {
            if !matches!(self.model_download, ModelDownloadState::Working { .. }) {
                self.model_download = ModelDownloadState::Idle;
            }
            self.model_download_note = None;
        }
        self.session = None;
        self.pack_editor = None;
        self.report_export_note = None;
        self.screen = Screen::Home;
    }

    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::GoHome
            | Command::LeavePackPick
            | Command::LeaveLevelPick
            | Command::LeaveSpeechMap
            | Command::LeaveProgress
            | Command::LeavePackEditor
            | Command::LeaveWarmup
            | Command::LeaveSettings
            | Command::LeaveDictaphone => self.leave_to_home(),
            Command::StartSession => {
                self.abort_listen();
                self.start_session();
            }
            Command::StartDiagnosis => {
                self.abort_listen();
                self.start_diagnosis();
            }
            Command::OpenPackPick => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::PackPick;
            }
            Command::SetPack(id) => {
                self.abort_listen();
                self.set_pack(&id);
            }
            Command::OpenLevelPick => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::LevelPick;
            }
            Command::OpenSpeechMap => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::SpeechMap;
            }
            Command::OpenProgress => {
                self.abort_listen();
                self.session = None;
                self.report_export_note = None;
                self.screen = Screen::ProgressReport;
            }
            Command::OpenPackEditor => self.open_pack_editor(),
            Command::DiscardPackEditor => self.leave_pack_editor(true),
            Command::ClonePackForEdit => self.clone_pack_for_edit(),
            Command::EditorDisable(i) => self.editor_disable(i),
            Command::EditorEnable(i) => self.editor_enable(i),
            Command::EditorAddReadAloud {
                prompt,
                text,
                stage,
            } => self.editor_add_read_aloud(prompt, text, stage),
            Command::EditorSave => self.editor_save(),
            Command::OpenWarmup => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::Warmup;
            }
            Command::SetLevel(level) => {
                self.abort_listen();
                self.set_level(level);
            }
            Command::SetLanguage(language) => self.set_language(language),
            Command::SetSimpleMode(on) => {
                self.progress.set_simple_mode(on);
                self.persist_progress();
            }
            Command::ExportProgressReport => self.export_progress_report(),
            Command::OpenDictaphone => {
                self.abort_listen();
                self.dictaphone = DictaphoneState::default();
                self.screen = Screen::Dictaphone;
            }
            Command::OpenSettings => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::Settings;
            }
            Command::StartModelDownload => self.start_model_download(),
            Command::AgainSession => {
                self.abort_listen();
                self.start_session();
            }
            Command::AdvanceAfterFeedback => self.advance_after_feedback(false),
            Command::SkipRepeatAndAdvance => self.advance_after_feedback(true),
            Command::Submit(answer) => self.submit(answer),
            Command::ListenExercise => self.try_listen(),
            Command::StopExerciseListen => self.stop_exercise_listen(),
            Command::PlayLastClip => self.play_last_clip(),
            Command::StopPlayback => self.stop_playback(),
            Command::ListenDictaphone => self.try_listen_dictaphone(),
            Command::StopDictaphone => self.stop_dictaphone(),
            Command::ClearDictaphone => self.clear_dictaphone_buffer(),
            Command::SaveDictaphone => self.save_dictaphone_now(),
            Command::PickPoolWord(i) => {
                if let Some(session) = self.session.as_mut() {
                    if i < session.pool.len() {
                        let w = session.pool.remove(i);
                        session.picked.push(w);
                    }
                }
            }
            Command::UndoPickedWord => {
                if let Some(session) = self.session.as_mut() {
                    if let Some(w) = session.picked.pop() {
                        session.pool.push(w);
                    }
                }
            }
            Command::ClearPickedWords => {
                if let Some(session) = self.session.as_mut() {
                    session.pool.append(&mut session.picked);
                }
            }
            Command::ResetBuildPhrase => {
                if let Some(session) = self.session.as_mut() {
                    session.prepare_current();
                }
            }
        }
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }

    pub fn pack(&self) -> &ExercisePack {
        &self.pack
    }

    pub fn pack_id(&self) -> &str {
        self.progress
            .pack_id
            .as_deref()
            .unwrap_or_else(|| self.progress.language.default_pack_id())
    }

    pub fn pack_catalog(&self) -> Vec<PackCatalogEntry> {
        list_packs_for(Some(self.progress.language))
    }

    pub fn language(&self) -> AppLanguage {
        self.progress.language
    }

    pub fn simple_mode(&self) -> bool {
        self.progress.simple_mode
    }

    pub fn ui_text(&self) -> UiText {
        UiText::new(self.progress.language)
    }

    pub fn pack_editor(&self) -> Option<&PackEditorState> {
        self.pack_editor.as_ref()
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn load_error(&self) -> Option<&str> {
        self.load_error.as_deref()
    }

    pub fn save_error(&self) -> Option<&str> {
        self.save_error.as_deref()
    }

    pub fn please_wait(&self) -> bool {
        self.please_wait
    }

    pub fn has_last_clip(&self) -> bool {
        !self.last_clip.is_empty()
    }

    pub fn playback_error(&self) -> Option<&str> {
        self.playback_error.as_deref()
    }

    pub fn is_playing_clip(&self) -> bool {
        self.playback_busy.load(Ordering::Relaxed)
    }

    pub fn dictaphone(&self) -> &DictaphoneState {
        &self.dictaphone
    }

    pub fn session(&self) -> Option<&SessionState> {
        self.session.as_ref()
    }

    pub fn session_is_diagnosis(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.kind == SessionKind::Diagnosis)
    }

    pub fn session_is_practice(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| s.kind == SessionKind::Practice)
    }

    /// Сколько ещё раз можно вернуть текущее задание в очередь (0 — лимит или «не повторять»).
    pub fn feedback_requeues_left(&self) -> Option<u32> {
        let session = self.session.as_ref()?;
        if session.kind != SessionKind::Practice {
            return None;
        }
        let ex = session.exercises.get(session.index)?;
        let key = ex.map_key()?;
        if session.skip_repeat.contains(&key) {
            return Some(0);
        }
        let used = session.requeue_count.get(&key).copied().unwrap_or(0);
        Some(MAX_REQUEUE_PER_KEY.saturating_sub(used))
    }

    /// Подсказка на экране упражнения: это повтор слабого места.
    pub fn current_exercise_is_practice_repeat(&self) -> bool {
        let Some(session) = self.session.as_ref() else {
            return false;
        };
        if session.kind != SessionKind::Practice {
            return false;
        }
        let Some(ex) = session.exercises.get(session.index) else {
            return false;
        };
        let Some(key) = ex.map_key() else {
            return false;
        };
        session
            .session_boost
            .get(&key)
            .is_some_and(|b| *b > 0)
    }

    pub fn level(&self) -> Option<ExerciseStage> {
        self.progress.level
    }

    pub fn twister_unlocked(&self) -> bool {
        twister_unlocked(
            self.progress.level,
            &self.pack,
            &self.progress.speech_map,
        )
    }

    pub fn speech_map_entries(&self) -> Vec<SpeechMapEntry> {
        pack_speech_entries(&self.pack, &self.progress.speech_map)
    }

    pub fn progress_report_text(&self) -> String {
        use super::exercise::format_progress_report;
        format_progress_report(
            &self.progress,
            &self.pack.title,
            &self.speech_map_entries(),
            self.progress.language,
        )
    }

    fn export_progress_report(&mut self) {
        let text = self.progress_report_text();
        match new_report_path().and_then(|path| {
            save_report_text(&path, &text)?;
            Ok(path)
        }) {
            Ok(path) => {
                let msg = format!(
                    "{}: {}",
                    super::i18n::tr(self.progress.language, "export_saved"),
                    path.display()
                );
                self.report_export_note = Some(msg);
            }
            Err(e) => {
                self.report_export_note = Some(format!(
                    "{}: {e}",
                    super::i18n::tr(self.progress.language, "export_failed")
                ));
            }
        }
    }

    pub fn report_export_note(&self) -> Option<&str> {
        self.report_export_note.as_deref()
    }

    pub fn asr_status(&self) -> AsrStatus {
        match self.recognizer.try_lock() {
            Ok(r) => r.status(),
            Err(std::sync::TryLockError::WouldBlock) => self.asr_status_cache.clone(),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                AsrStatus::Error("Сбой распознавателя. Перезапустите приложение.".into())
            }
        }
    }

    pub fn model_download(&self) -> &ModelDownloadState {
        &self.model_download
    }

    pub fn model_download_note(&self) -> Option<&str> {
        self.model_download_note.as_deref()
    }

    pub fn user_data_dir_display(&self) -> Option<String> {
        user_data_dir()
            .ok()
            .map(|p| p.display().to_string())
    }

    #[cfg(test)]
    pub(crate) fn test_download_rx(&mut self, rx: Receiver<DownloadMsg>) {
        self.model_download_rx = Some(rx);
    }

    /// Подставить канал listen (без реального ASR-воркера).
    #[cfg(test)]
    fn test_inject_listen(
        &mut self,
        rx: Receiver<ListenEvent>,
        purpose: ListenPurpose,
        target: Option<String>,
    ) {
        self.listen_rx = Some(rx);
        self.listen_purpose = Some(purpose);
        self.listen_target = target;
    }

    /// Имитация «join ещё жив»: поток спит, пока `release` не станет true.
    #[cfg(test)]
    fn test_inject_busy_listen_join(&mut self, release: Arc<AtomicBool>) {
        self.listen_join = Some(thread::spawn(move || {
            while !release.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(5));
            }
        }));
    }

    #[cfg(test)]
    fn test_set_dictaphone_save(&mut self, path: PathBuf, transcript: String) {
        self.dictaphone.save_path = Some(path);
        self.dictaphone.transcript = transcript;
    }

    #[cfg(test)]
    fn test_pending_recognizer_reload(&self) -> bool {
        self.pending_recognizer_reload
    }

    #[cfg(test)]
    fn test_set_pending_recognizer_reload(&mut self, pending: bool) {
        self.pending_recognizer_reload = pending;
    }

    #[cfg(test)]
    fn test_listen_worker_busy(&self) -> bool {
        self.listen_worker_busy()
    }

    #[cfg(test)]
    fn test_reload_recognizer(&mut self) {
        self.reload_recognizer();
    }

    #[cfg(test)]
    fn test_abort_listen(&mut self) {
        self.abort_listen();
    }

    #[cfg(test)]
    fn test_set_last_clip(&mut self, pcm: Vec<i16>) {
        self.last_clip = pcm;
    }

    #[cfg(test)]
    fn test_set_playback_busy(&self, busy: bool) {
        self.playback_busy.store(busy, Ordering::Relaxed);
    }

    #[cfg(test)]
    fn test_playback_busy(&self) -> bool {
        self.playback_busy.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    fn test_playback_pending_replay(&self) -> bool {
        self.playback_pending_replay
    }
}

impl SessionState {
    pub fn prepare_current(&mut self) {
        self.pool.clear();
        self.picked.clear();
        self.choice_options.clear();
        self.listen_error = None;
        self.listening = false;
        self.live_text.clear();
        self.asr_hint_ok = None;
        match self.exercises.get(self.index) {
            Some(Exercise::BuildPhrase { words, .. }) => {
                self.pool = words.clone();
                self.pool.shuffle(&mut rand::rng());
            }
            Some(Exercise::ChooseWord { options, .. }) => {
                self.choice_options = options.clone();
                self.choice_options.shuffle(&mut rand::rng());
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;
