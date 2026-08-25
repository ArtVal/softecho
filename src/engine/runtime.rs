//! Состояние и логика тренажёра (серверная часть).
//! UI / будущий клиент общаются только через Command + геттеры + tick.

use super::asr::{
    create_recognizer, AsrStatus, ListenConfig, ListenEvent, SpeechRecognizer,
};
use super::data::{
    append_dictaphone_text, clone_pack_to_user, is_user_pack, list_packs_for, load_active_pack,
    load_editable_pack, load_pack, load_progress, new_dictaphone_path, pack_matches_language,
    save_dictaphone_text, save_progress, save_user_pack, user_data_dir, vosk_model_dir,
    DEFAULT_PACK_ID, EditablePack, PackCatalogEntry,
};
use super::exercise::{
    build_diagnosis_set, check_answer, infer_level, order_session_for_level_with_map, speech_matches,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ListenPurpose {
    Exercise,
    Dictaphone,
}

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
    dictaphone: DictaphoneState,
    model_download: ModelDownloadState,
    model_download_rx: Option<Receiver<DownloadMsg>>,
    model_download_note: Option<String>,
    pack_editor: Option<PackEditorState>,
}

pub struct PackEditorState {
    pub pack_id: String,
    pub draft: EditablePack,
    pub error: Option<String>,
    pub note: Option<String>,
}

impl Engine {
    pub fn new() -> Self {
        let language = load_progress().language;
        Self::create(vosk_model_dir(language))
    }

    fn create(model: Option<std::path::PathBuf>) -> Self {
        let mut progress = load_progress();
        let fallback = progress.language.default_pack_id();
        if let Some(id) = progress.pack_id.clone() {
            if !pack_matches_language(&id, progress.language) {
                progress.set_pack(fallback);
            }
        }
        let (pack, load_error) = match load_active_pack(&progress) {
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

        let recognizer = Arc::new(Mutex::new(create_recognizer(model.as_deref())));

        Self {
            screen: Screen::Home,
            pack,
            progress,
            session: None,
            load_error,
            save_error: None,
            recognizer,
            listen_rx: None,
            listen_target: None,
            listen_purpose: None,
            listen_live: Arc::new(Mutex::new(String::new())),
            exercise_listen_stop: None,
            please_wait: false,
            last_clip: Vec::new(),
            playback_stop: None,
            playback_busy: Arc::new(AtomicBool::new(false)),
            dictaphone: DictaphoneState::default(),
            model_download: ModelDownloadState::default(),
            model_download_rx: None,
            model_download_note: None,
            pack_editor: None,
        }
    }

    /// Движок без загрузки модели Vosk (юнит-тесты логики).
    #[cfg(test)]
    fn new_logic_only() -> Self {
        Self::create(None)
    }

    fn reload_recognizer(&mut self) {
        self.abort_listen();
        let model = vosk_model_dir(self.progress.language);
        if let Ok(mut r) = self.recognizer.lock() {
            *r = create_recognizer(model.as_deref());
        }
    }

    fn set_language(&mut self, language: AppLanguage) {
        if self.progress.language == language {
            return;
        }
        self.abort_listen();
        self.session = None;
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
        self.model_download = ModelDownloadState::Idle;
        self.model_download_note = None;
        self.persist_progress();
    }

    fn start_model_download(&mut self) {
        if self.model_download_rx.is_some() {
            return;
        }
        if matches!(self.asr_status(), AsrStatus::Disabled) {
            return;
        }
        if matches!(self.asr_status(), AsrStatus::Ready) {
            self.model_download_note = Some("Модель уже установлена.".into());
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
        self.model_download_rx = Some(rx);
        self.model_download = ModelDownloadState::Working {
            label: "Подготовка…".into(),
            percent: None,
        };
        self.model_download_note = None;
        spawn_model_download(dest, language, tx);
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
                    self.model_download = ModelDownloadState::Failed(e);
                    tick.want_repaint = true;
                }
            }
        }
    }

    fn persist_progress(&mut self) {
        match save_progress(&self.progress) {
            Ok(()) => self.save_error = None,
            Err(e) => self.save_error = Some(e),
        }
    }

    fn start_session(&mut self) {
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

    fn open_pack_editor(&mut self) {
        self.abort_listen();
        self.session = None;
        let id = self.pack_id().to_string();
        if !is_user_pack(&id) {
            self.load_error = Some(
                "Встроенный набор нельзя менять. Нажмите «Сделать копию» — правка будет в ваших данных."
                    .into(),
            );
            self.screen = Screen::PackEditor;
            self.pack_editor = None;
            return;
        }
        match load_editable_pack(&id) {
            Ok(draft) => {
                self.load_error = None;
                self.pack_editor = Some(PackEditorState {
                    pack_id: id,
                    draft,
                    error: None,
                    note: None,
                });
                self.screen = Screen::PackEditor;
            }
            Err(e) => {
                self.load_error = Some(e);
                self.pack_editor = None;
                self.screen = Screen::Home;
            }
        }
    }

    fn clone_pack_for_edit(&mut self) {
        self.abort_listen();
        self.session = None;
        let source = self.pack_id().to_string();
        match clone_pack_to_user(&source, "") {
            Ok((id, draft)) => {
                self.pack = draft.to_active_pack();
                self.progress.set_pack(&id);
                self.persist_progress();
                self.load_error = None;
                self.pack_editor = Some(PackEditorState {
                    pack_id: id,
                    draft,
                    error: None,
                    note: Some("Копия сохранена. Можно править и сохранять.".into()),
                });
                self.screen = Screen::PackEditor;
            }
            Err(e) => {
                self.load_error = Some(e);
            }
        }
    }

    fn editor_disable(&mut self, index: usize) {
        let Some(ed) = self.pack_editor.as_mut() else {
            return;
        };
        if index >= ed.draft.exercises.len() {
            return;
        }
        if ed.draft.exercises.len() == 1 {
            ed.error = Some("Нельзя отключить последнее активное задание.".into());
            return;
        }
        let ex = ed.draft.exercises.remove(index);
        ed.draft.disabled.push(ex);
        ed.error = None;
        ed.note = None;
    }

    fn editor_enable(&mut self, index: usize) {
        let Some(ed) = self.pack_editor.as_mut() else {
            return;
        };
        if index >= ed.draft.disabled.len() {
            return;
        }
        let ex = ed.draft.disabled.remove(index);
        ed.draft.exercises.push(ex);
        ed.error = None;
        ed.note = None;
    }

    fn editor_add_read_aloud(&mut self, prompt: String, text: String, stage: ExerciseStage) {
        let Some(ed) = self.pack_editor.as_mut() else {
            return;
        };
        let prompt = if prompt.trim().is_empty() {
            "Скажите".into()
        } else {
            prompt.trim().to_string()
        };
        let text = text.trim().to_string();
        if text.is_empty() {
            ed.error = Some("Введите текст задания.".into());
            return;
        }
        ed.draft.exercises.push(Exercise::ReadAloud {
            stage: Some(stage),
            prompt,
            text,
            speak: None,
        });
        ed.error = None;
        ed.note = Some("Задание добавлено — нажмите «Сохранить».".into());
    }

    fn editor_save(&mut self) {
        let Some(ed) = self.pack_editor.as_ref() else {
            return;
        };
        let id = ed.pack_id.clone();
        let draft = ed.draft.clone();
        match save_user_pack(&id, &draft) {
            Ok(_) => {
                self.pack = draft.to_active_pack();
                if let Some(ed) = self.pack_editor.as_mut() {
                    ed.error = None;
                    ed.note = Some("Сохранено.".into());
                    ed.draft = draft;
                }
                self.load_error = None;
            }
            Err(e) => {
                if let Some(ed) = self.pack_editor.as_mut() {
                    ed.error = Some(e);
                    ed.note = None;
                }
            }
        }
    }

    pub fn current_exercise(&self) -> Option<&Exercise> {
        let s = self.session.as_ref()?;
        s.exercises.get(s.index)
    }

    fn submit(&mut self, answer: UserAnswer) {
        // Отменяем отложенный ASR, чтобы не зачесть ответ дважды.
        self.listen_rx = None;
        self.listen_target = None;
        self.listen_purpose = None;
        self.please_wait = false;
        self.stop_playback();

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
        self.listen_rx = None;
        self.listen_target = None;
        self.listen_purpose = None;
        self.stop_playback();
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

    fn try_listen(&mut self) {
        if self.listen_rx.is_some() {
            return;
        }
        self.stop_playback();
        self.last_clip.clear();

        let target = self
            .current_exercise()
            .and_then(|e| e.target_text())
            .map(|s| s.to_string());
        let Some(target) = target else {
            return;
        };

        let mut grammar: Vec<String> = Vec::new();
        for w in target.split_whitespace() {
            let w = w.to_lowercase();
            if !grammar.iter().any(|g| g == &w) {
                grammar.push(w);
            }
        }

        if let Some(session) = self.session.as_mut() {
            session.listening = true;
            session.listen_error = None;
            session.live_text.clear();
            session.asr_hint_ok = None;
        }

        let stop = Arc::new(AtomicBool::new(false));
        self.exercise_listen_stop = Some(Arc::clone(&stop));

        let live = Arc::clone(&self.listen_live);
        if let Ok(mut g) = live.lock() {
            g.clear();
        }
        self.please_wait = false;
        self.spawn_listen(
            grammar,
            Some(target),
            ListenPurpose::Exercise,
            ListenConfig::single_utterance(live, Some(stop)),
        );
    }

    fn stop_exercise_listen(&mut self) {
        if let Some(stop) = &self.exercise_listen_stop {
            stop.store(true, Ordering::Relaxed);
        }
    }

    fn try_listen_dictaphone(&mut self) {
        if self.listen_rx.is_some() {
            return;
        }
        self.stop_playback();
        let stop = Arc::new(AtomicBool::new(false));
        self.dictaphone.listening = true;
        self.dictaphone.error = None;
        self.dictaphone.save_note = None;
        self.dictaphone.live_text.clear();
        self.please_wait = false;
        if let Ok(mut g) = self.dictaphone.live_partial.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.listen_live.lock() {
            g.clear();
        }
        // Файл сессии: новый, если ещё нет (после Очистить / первый старт).
        if self.dictaphone.save_path.is_none() {
            match new_dictaphone_path() {
                Ok(path) => {
                    self.dictaphone.save_note =
                        Some(format!("Пишу в файл: {}", path.display()));
                    self.dictaphone.save_path = Some(path);
                }
                Err(e) => {
                    self.dictaphone.error = Some(e);
                    self.dictaphone.listening = false;
                    return;
                }
            }
        }
        // transcript не очищаем — можно дописать; Очистить сбрасывает всё.
        self.dictaphone.stop = Some(Arc::clone(&stop));
        let live = Arc::clone(&self.dictaphone.live_partial);
        self.listen_live = Arc::clone(&live);
        self.spawn_listen(
            Vec::new(),
            None,
            ListenPurpose::Dictaphone,
            ListenConfig::long_dictaphone(stop, live),
        );
    }

    fn stop_dictaphone(&mut self) {
        if let Some(stop) = &self.dictaphone.stop {
            stop.store(true, Ordering::Relaxed);
        }
    }

    fn clear_dictaphone_buffer(&mut self) {
        self.stop_playback();
        self.last_clip.clear();
        self.dictaphone.live_text.clear();
        self.dictaphone.transcript.clear();
        self.dictaphone.error = None;
        self.dictaphone.save_path = None;
        self.dictaphone.save_note = None;
        if let Ok(mut g) = self.dictaphone.live_partial.lock() {
            g.clear();
        }
    }

    fn store_last_clip(&mut self, pcm: Vec<i16>) {
        if pcm.is_empty() {
            return;
        }
        self.last_clip = pcm;
    }

    fn play_last_clip(&mut self) {
        if self.last_clip.is_empty() {
            return;
        }
        if self.playback_busy.load(Ordering::Relaxed) {
            self.stop_playback();
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        self.playback_stop = Some(Arc::clone(&stop));
        play_pcm_16k(
            self.last_clip.clone(),
            stop,
            Arc::clone(&self.playback_busy),
        );
    }

    fn stop_playback(&mut self) {
        if let Some(stop) = &self.playback_stop {
            stop.store(true, Ordering::Relaxed);
        }
        self.playback_stop = None;
    }

    fn save_dictaphone_now(&mut self) {
        let text = self.dictaphone.transcript.clone();
        if text.is_empty() {
            self.dictaphone.save_note = Some("Нечего сохранять — текста ещё нет.".into());
            return;
        }
        let path = match &self.dictaphone.save_path {
            Some(p) => p.clone(),
            None => match new_dictaphone_path() {
                Ok(p) => {
                    self.dictaphone.save_path = Some(p.clone());
                    p
                }
                Err(e) => {
                    self.dictaphone.error = Some(e);
                    return;
                }
            },
        };
        match save_dictaphone_text(&path, &text) {
            Ok(()) => {
                self.dictaphone.save_note =
                    Some(format!("Сохранено: {}", path.display()));
                self.dictaphone.error = None;
            }
            Err(e) => self.dictaphone.error = Some(e),
        }
    }

    fn append_dictaphone_phrase(&mut self, phrase: &str) {
        if phrase.is_empty() {
            return;
        }
        if !self.dictaphone.transcript.is_empty() {
            self.dictaphone.transcript.push('\n');
        }
        self.dictaphone.transcript.push_str(phrase);
        if let Some(path) = &self.dictaphone.save_path {
            let chunk = if path
                .metadata()
                .map(|m| m.len() > 0)
                .unwrap_or(false)
            {
                format!("\n{phrase}")
            } else {
                phrase.to_string()
            };
            if let Err(e) = append_dictaphone_text(path, &chunk) {
                self.dictaphone.error = Some(e);
            }
        }
    }

    /// Хвост из live UI / listen_live, если Utterance не успел уйти в transcript.
    fn dictaphone_live_tail(&self) -> String {
        let from_ui = self.dictaphone.live_text.trim().to_string();
        if !from_ui.is_empty() {
            from_ui
        } else {
            self.listen_live
                .lock()
                .map(|g| g.trim().to_string())
                .unwrap_or_default()
        }
    }

    fn exercise_heard_text(&self, heard: &str) -> String {
        let trimmed = heard.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
        let from_ui = self
            .session()
            .map(|s| s.live_text.trim().to_string())
            .unwrap_or_default();
        if !from_ui.is_empty() {
            return from_ui;
        }
        self.listen_live
            .lock()
            .map(|g| g.trim().to_string())
            .unwrap_or_default()
    }

    fn flush_dictaphone_live_tail(&mut self) {
        let live_tail = self.dictaphone_live_tail();
        if !live_tail.is_empty() {
            let already = self
                .dictaphone
                .transcript
                .lines()
                .any(|l| l.trim() == live_tail);
            if !already {
                self.append_dictaphone_phrase(&live_tail);
            }
        }
        self.dictaphone.live_text.clear();
        if let Ok(mut g) = self.listen_live.lock() {
            g.clear();
        }
        if let Ok(mut g) = self.dictaphone.live_partial.lock() {
            g.clear();
        }
    }

    fn spawn_listen(
        &mut self,
        grammar: Vec<String>,
        target: Option<String>,
        purpose: ListenPurpose,
        config: ListenConfig,
    ) {
        let (tx, rx) = mpsc::channel();
        let recognizer = Arc::clone(&self.recognizer);
        self.listen_rx = Some(rx);
        self.listen_target = target;
        self.listen_purpose = Some(purpose);

        thread::spawn(move || match recognizer.lock() {
            Ok(mut r) => r.listen_stream(&grammar, tx, config),
            Err(_) => {
                let _ = tx.send(ListenEvent::Done(Err(
                    "Распознаватель недоступен".into(),
                )));
            }
        });
    }

    fn sync_live_text(&mut self) {
        let Ok(g) = self.listen_live.try_lock() else {
            return;
        };
        match self.listen_purpose {
            Some(ListenPurpose::Dictaphone) => {
                if self.dictaphone.live_text != *g {
                    self.dictaphone.live_text.clone_from(&g);
                }
            }
            Some(ListenPurpose::Exercise) => {
                if let Some(session) = self.session.as_mut() {
                    if session.live_text != *g {
                        session.live_text.clone_from(&g);
                    }
                }
            }
            None => {}
        }
    }

    pub fn tick(&mut self) -> TickResult {
        let mut tick = TickResult::default();
        self.poll_model_download(&mut tick);
        if matches!(self.model_download, ModelDownloadState::Working { .. }) {
            tick.want_repaint = true;
            tick.repaint_after.get_or_insert(Duration::from_millis(200));
        }
        if self.playback_busy.load(Ordering::Relaxed) {
            tick.want_repaint = true;
            tick.repaint_after.get_or_insert(Duration::from_millis(100));
        }
        if self.listen_rx.is_some() {
            self.sync_live_text();
        }

        let Some(rx) = self.listen_rx.as_ref() else {
            return tick;
        };

        let mut events = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(ev) => events.push(ev),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    events.push(ListenEvent::Done(Err("Сбой записи голоса".into())));
                    break;
                }
            }
        }

        if events.is_empty() {
            if self.listen_rx.is_some() {
                tick.repaint_after = Some(Duration::from_millis(50));
            }
            return tick;
        }

        tick.want_repaint = true;

        for event in events {
            match event {
                ListenEvent::PleaseWait => {
                    self.please_wait = true;
                }
                ListenEvent::ReadyAgain => {
                    self.please_wait = false;
                }
                ListenEvent::Utterance(phrase) => {
                    self.append_dictaphone_phrase(&phrase);
                    self.dictaphone.live_text.clear();
                    if let Ok(mut g) = self.listen_live.lock() {
                        g.clear();
                    }
                    if let Ok(mut g) = self.dictaphone.live_partial.lock() {
                        g.clear();
                    }
                }
                ListenEvent::Done(outcome) => {
                    self.listen_rx = None;
                    self.exercise_listen_stop = None;
                    self.please_wait = false;
                    let target = self.listen_target.take().unwrap_or_default();
                    let purpose = self.listen_purpose.take();

                    match purpose {
                        Some(ListenPurpose::Dictaphone) => {
                            self.dictaphone.listening = false;
                            self.dictaphone.stop = None;
                            if !matches!(self.screen, Screen::Dictaphone) {
                                continue;
                            }
                            match outcome {
                                Ok(heard) => {
                                    self.store_last_clip(heard.pcm);
                                    self.flush_dictaphone_live_tail();
                                    if self.dictaphone.transcript.is_empty()
                                        && !heard.text.is_empty()
                                    {
                                        self.append_dictaphone_phrase(&heard.text);
                                    }
                                    if self.dictaphone.save_path.is_some()
                                        && !self.dictaphone.transcript.is_empty()
                                    {
                                        self.save_dictaphone_now();
                                    }
                                }
                                Err(e) => {
                                    self.flush_dictaphone_live_tail();
                                    if self.dictaphone.transcript.is_empty() {
                                        self.dictaphone.error = Some(e);
                                    } else {
                                        self.save_dictaphone_now();
                                    }
                                }
                            }
                        }
                        Some(ListenPurpose::Exercise) => {
                            let still_on_exercise = matches!(self.screen, Screen::Exercise)
                                && self.session.as_ref().is_some_and(|s| s.listening);
                            if let Some(session) = self.session.as_mut() {
                                session.listening = false;
                            }
                            if !still_on_exercise {
                                continue;
                            }
                            match outcome {
                                Ok(heard) => {
                                    self.store_last_clip(heard.pcm);
                                    let text = self.exercise_heard_text(&heard.text);
                                    let matched = if text.is_empty() {
                                        None
                                    } else {
                                        Some(speech_matches(&target, &text))
                                    };
                                    if let Some(session) = self.session.as_mut() {
                                        session.live_text = text;
                                        session.asr_hint_ok = matched;
                                        session.listen_error = None;
                                    }
                                    // ASR — подсказка; зачёт только через «Получилось / Не получилось».
                                }
                                Err(e) => {
                                    if let Some(session) = self.session.as_mut() {
                                        session.listen_error = Some(e);
                                        session.asr_hint_ok = None;
                                    }
                                }
                            }
                        }
                        None => {}
                    }
                }
            }
        }
        tick
    }

    fn abort_listen(&mut self) {
        self.stop_playback();
        if let Some(stop) = &self.exercise_listen_stop {
            stop.store(true, Ordering::Relaxed);
        }
        self.exercise_listen_stop = None;
        self.listen_rx = None;
        self.listen_target = None;
        self.listen_purpose = None;
        self.please_wait = false;
        if let Some(stop) = &self.dictaphone.stop {
            stop.store(true, Ordering::Relaxed);
        }
        self.dictaphone.stop = None;
        self.dictaphone.listening = false;
        if let Some(session) = self.session.as_mut() {
            session.listening = false;
        }
    }

    pub fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::GoHome => {
                self.abort_listen();
                self.session = None;
                self.pack_editor = None;
                self.screen = Screen::Home;
            }
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
            Command::LeavePackPick => {
                self.screen = Screen::Home;
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
            Command::LeaveLevelPick => {
                self.screen = Screen::Home;
            }
            Command::OpenSpeechMap => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::SpeechMap;
            }
            Command::LeaveSpeechMap => {
                self.screen = Screen::Home;
            }
            Command::OpenProgress => {
                self.abort_listen();
                self.session = None;
                self.screen = Screen::ProgressReport;
            }
            Command::LeaveProgress => {
                self.screen = Screen::Home;
            }
            Command::OpenPackEditor => self.open_pack_editor(),
            Command::LeavePackEditor => {
                self.pack_editor = None;
                self.screen = Screen::Home;
            }
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
            Command::LeaveWarmup => {
                self.screen = Screen::Home;
            }
            Command::SetLevel(level) => {
                self.abort_listen();
                self.set_level(level);
            }
            Command::SetLanguage(language) => self.set_language(language),
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
            Command::LeaveSettings => {
                self.screen = Screen::Home;
                if !matches!(self.model_download, ModelDownloadState::Working { .. }) {
                    self.model_download = ModelDownloadState::Idle;
                }
                self.model_download_note = None;
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
            Command::LeaveDictaphone => {
                self.abort_listen();
                self.clear_dictaphone_buffer();
                self.screen = Screen::Home;
            }
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

    pub fn asr_status(&self) -> AsrStatus {
        match self.recognizer.try_lock() {
            Ok(r) => r.status(),
            Err(_) => AsrStatus::Ready,
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
mod tests {
    use super::*;
    use crate::engine::protocol::{ModelDownloadState, Screen};

    #[test]
    fn set_pack_manual() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::SetPack("daily".into()));
        assert_eq!(eng.pack().title, "Дом и быт");
        assert_eq!(eng.pack_id(), "daily");
        assert!(matches!(eng.screen(), Screen::Home));
    }

    #[test]
    fn start_session_without_level_opens_picker() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = None;
        eng.handle(Command::StartSession);
        assert!(matches!(eng.screen(), Screen::LevelPick));
        assert!(eng.session().is_none());
    }

    #[test]
    fn start_session_opens_exercise() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Syllable);
        assert!(matches!(eng.screen(), Screen::Home));
        eng.handle(Command::StartSession);
        assert!(matches!(eng.screen(), Screen::Exercise));
        assert!(eng.session().is_some());
        let s = eng.session().unwrap();
        assert!(!s.exercises.is_empty());
        assert_eq!(s.index, 0);
        let stages: Vec<_> = s.exercises.iter().map(Exercise::stage).collect();
        assert_eq!(stages.first(), Some(&crate::engine::ExerciseStage::Syllable));
        let mut seen_word = false;
        let mut seen_phrase = false;
        let mut seen_twister = false;
        for st in &stages {
            match st {
                crate::engine::ExerciseStage::Sound => {
                    panic!("звук отфильтрован уровнем «слоги»");
                }
                crate::engine::ExerciseStage::Syllable => {
                    assert!(!seen_word && !seen_phrase && !seen_twister);
                }
                crate::engine::ExerciseStage::Word => {
                    seen_word = true;
                    assert!(!seen_phrase && !seen_twister);
                }
                crate::engine::ExerciseStage::Phrase => {
                    seen_phrase = true;
                    assert!(!seen_twister);
                }
                crate::engine::ExerciseStage::Twister => seen_twister = true,
            }
        }
        assert!(seen_word && seen_phrase);
    }

    #[test]
    fn set_level_manual_and_filter_practice() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::SetLevel(ExerciseStage::Word));
        assert_eq!(eng.level(), Some(ExerciseStage::Word));
        assert!(matches!(eng.screen(), Screen::Home));
        eng.handle(Command::StartSession);
        let s = eng.session().unwrap();
        assert!(s.exercises.iter().all(|e| e.stage() >= ExerciseStage::Word));
        assert!(!s.exercises.iter().any(|e| e.stage() == ExerciseStage::Syllable));
    }

    #[test]
    fn diagnosis_sets_level_automatically() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::StartDiagnosis);
        assert!(eng.session_is_diagnosis());
        assert!(matches!(eng.screen(), Screen::Exercise));
        // Все ответы верные → уровень «Фразы».
        loop {
            let Some(ex) = eng.current_exercise().cloned() else {
                break;
            };
            match ex {
                Exercise::ChooseWord { answer, .. } => {
                    eng.handle(Command::Submit(UserAnswer::Choice(answer)));
                }
                Exercise::BuildPhrase { answer, .. } => {
                    let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
                    eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
                }
                Exercise::ReadAloud { .. } => {
                    eng.handle(Command::Submit(UserAnswer::ReadDone {
                        matched: true,
                        heard: None,
                    }));
                }
            }
            eng.handle(Command::AdvanceAfterFeedback);
            if matches!(eng.screen(), Screen::DiagnosisResult { .. }) {
                break;
            }
        }
        assert!(matches!(
            eng.screen(),
            Screen::DiagnosisResult {
                level: ExerciseStage::Phrase
            }
        ));
        assert_eq!(eng.level(), Some(ExerciseStage::Phrase));
        // Диагностика не считает обычное занятие.
        assert_eq!(eng.progress().sessions_completed, 0);
    }

    #[test]
    fn diagnosis_weak_syllables_sets_syllable_level() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::StartDiagnosis);
        loop {
            let Some(ex) = eng.current_exercise().cloned() else {
                break;
            };
            let ok = ex.stage() != ExerciseStage::Syllable;
            match ex {
                Exercise::ChooseWord { answer, .. } => {
                    if ok {
                        eng.handle(Command::Submit(UserAnswer::Choice(answer)));
                    } else {
                        eng.handle(Command::Submit(UserAnswer::Choice("__нет__".into())));
                    }
                }
                Exercise::BuildPhrase { answer, .. } => {
                    let parts: Vec<_> = if ok {
                        answer.split_whitespace().map(str::to_string).collect()
                    } else {
                        vec!["нет".into()]
                    };
                    eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
                }
                Exercise::ReadAloud { .. } => {
                    eng.handle(Command::Submit(UserAnswer::ReadDone {
                        matched: ok,
                        heard: None,
                    }));
                }
            }
            eng.handle(Command::AdvanceAfterFeedback);
            if matches!(eng.screen(), Screen::DiagnosisResult { .. }) {
                break;
            }
        }
        assert_eq!(eng.level(), Some(ExerciseStage::Syllable));
    }

    #[test]
    fn diagnosis_weak_sounds_sets_sound_level() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::StartDiagnosis);
        loop {
            let Some(ex) = eng.current_exercise().cloned() else {
                break;
            };
            let ok = ex.stage() != ExerciseStage::Sound;
            match ex {
                Exercise::ChooseWord { answer, .. } => {
                    if ok {
                        eng.handle(Command::Submit(UserAnswer::Choice(answer)));
                    } else {
                        eng.handle(Command::Submit(UserAnswer::Choice("__нет__".into())));
                    }
                }
                Exercise::BuildPhrase { answer, .. } => {
                    let parts: Vec<_> = if ok {
                        answer.split_whitespace().map(str::to_string).collect()
                    } else {
                        vec!["нет".into()]
                    };
                    eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
                }
                Exercise::ReadAloud { .. } => {
                    eng.handle(Command::Submit(UserAnswer::ReadDone {
                        matched: ok,
                        heard: None,
                    }));
                }
            }
            eng.handle(Command::AdvanceAfterFeedback);
            if matches!(eng.screen(), Screen::DiagnosisResult { .. }) {
                break;
            }
        }
        assert_eq!(eng.level(), Some(ExerciseStage::Sound));
    }

    #[test]
    fn choose_word_flow_to_feedback() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Syllable);
        eng.handle(Command::StartSession);
        // Дойти до ChooseWord, если первый другой — листаем через неверный ответ нельзя без feedback.
        // Берём упражнение из сессии и сабмитим подходящий тип.
        let ex = eng.current_exercise().cloned().unwrap();
        match ex {
            Exercise::ChooseWord { answer, .. } => {
                eng.handle(Command::Submit(UserAnswer::Choice(answer)));
            }
            Exercise::BuildPhrase { answer, .. } => {
                let parts: Vec<String> = answer.split_whitespace().map(str::to_string).collect();
                eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
            }
            Exercise::ReadAloud { .. } => {
                eng.handle(Command::Submit(UserAnswer::ReadDone {
                    matched: true,
                    heard: None,
                }));
            }
        }
        assert!(matches!(eng.screen(), Screen::Feedback { .. }));
        eng.handle(Command::AdvanceAfterFeedback);
        // Либо следующее упражнение, либо результат (если одно).
        assert!(matches!(
            eng.screen(),
            Screen::Exercise | Screen::Result { .. }
        ));
    }

    #[test]
    fn pick_pool_word_and_undo() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Syllable);
        eng.handle(Command::StartSession);
        // Найти BuildPhrase
        let mut found = false;
        for _ in 0..eng.session().unwrap().exercises.len() {
            if matches!(eng.current_exercise(), Some(Exercise::BuildPhrase { .. })) {
                found = true;
                break;
            }
            // форсируем переход: сдаём текущее
            let ex = eng.current_exercise().cloned().unwrap();
            match ex {
                Exercise::ChooseWord { answer, .. } => {
                    eng.handle(Command::Submit(UserAnswer::Choice(answer)));
                }
                Exercise::BuildPhrase { answer, .. } => {
                    let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
                    eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
                }
                Exercise::ReadAloud { .. } => {
                    eng.handle(Command::Submit(UserAnswer::ReadDone {
                        matched: true,
                        heard: None,
                    }));
                }
            }
            eng.handle(Command::AdvanceAfterFeedback);
            if matches!(eng.screen(), Screen::Result { .. }) {
                break;
            }
        }
        if !found {
            return; // набор без BuildPhrase — пропускаем
        }
        let pool_len = eng.session().unwrap().pool.len();
        assert!(pool_len > 0);
        eng.handle(Command::PickPoolWord(0));
        assert_eq!(eng.session().unwrap().pool.len(), pool_len - 1);
        assert_eq!(eng.session().unwrap().picked.len(), 1);
        eng.handle(Command::UndoPickedWord);
        assert_eq!(eng.session().unwrap().picked.len(), 0);
        assert_eq!(eng.session().unwrap().pool.len(), pool_len);
    }

    #[test]
    fn go_home_clears_session() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Syllable);
        eng.handle(Command::StartSession);
        eng.handle(Command::GoHome);
        assert!(matches!(eng.screen(), Screen::Home));
        assert!(eng.session().is_none());
        assert!(!eng.please_wait());
    }

    #[test]
    fn open_speech_map_and_leave() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenSpeechMap);
        assert!(matches!(eng.screen(), Screen::SpeechMap));
        assert!(!eng.speech_map_entries().is_empty());
        eng.handle(Command::LeaveSpeechMap);
        assert!(matches!(eng.screen(), Screen::Home));
    }

    #[test]
    fn open_warmup_and_leave() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenWarmup);
        assert!(matches!(eng.screen(), Screen::Warmup));
        eng.handle(Command::LeaveWarmup);
        assert!(matches!(eng.screen(), Screen::Home));
    }

    #[test]
    fn open_progress_and_leave() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenProgress);
        assert!(matches!(eng.screen(), Screen::ProgressReport));
        let text = eng.progress_report_text();
        assert!(text.contains("SoftEcho"));
        eng.handle(Command::LeaveProgress);
        assert!(matches!(eng.screen(), Screen::Home));
    }

    #[test]
    fn open_pack_editor_builtin_prompts_clone() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenPackEditor);
        assert!(matches!(eng.screen(), Screen::PackEditor));
        assert!(eng.pack_editor().is_none());
        eng.handle(Command::LeavePackEditor);
        assert!(matches!(eng.screen(), Screen::Home));
    }

    fn tiny_test_pack() -> ExercisePack {
        ExercisePack {
            title: "test".into(),
            exercises: vec![
                Exercise::ChooseWord {
                    stage: Some(ExerciseStage::Word),
                    prompt: "q".into(),
                    options: vec!["дом".into(), "чай".into()],
                    answer: "дом".into(),
                },
                Exercise::ChooseWord {
                    stage: Some(ExerciseStage::Word),
                    prompt: "q2".into(),
                    options: vec!["чай".into(), "стол".into()],
                    answer: "чай".into(),
                },
            ],
        }
    }

    fn wrong_answer_for(ex: &Exercise) -> UserAnswer {
        match ex {
            Exercise::ChooseWord { options, answer, .. } => {
                let wrong = options
                    .iter()
                    .find(|o| *o != answer)
                    .cloned()
                    .unwrap_or_else(|| "__нет__".into());
                UserAnswer::Choice(wrong)
            }
            Exercise::BuildPhrase { answer, .. } => {
                let mut parts: Vec<String> = answer.split_whitespace().map(str::to_string).collect();
                if parts.len() >= 2 {
                    parts.swap(0, 1);
                }
                UserAnswer::Phrase(parts)
            }
            Exercise::ReadAloud { .. } => UserAnswer::ReadDone {
                matched: false,
                heard: None,
            },
        }
    }

    #[test]
    fn incorrect_practice_requeues_on_advance() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Word);
        eng.pack = tiny_test_pack();
        eng.handle(Command::StartSession);
        let len = eng.session().unwrap().exercises.len();
        let ex = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex)));
        eng.handle(Command::AdvanceAfterFeedback);
        assert_eq!(eng.session().unwrap().exercises.len(), len + 1);
        assert!(matches!(eng.screen(), Screen::Exercise));
    }

    #[test]
    fn skip_repeat_does_not_requeue() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Word);
        eng.pack = tiny_test_pack();
        eng.handle(Command::StartSession);
        let len = eng.session().unwrap().exercises.len();
        let ex = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex)));
        eng.handle(Command::SkipRepeatAndAdvance);
        assert_eq!(eng.session().unwrap().exercises.len(), len);
    }

    #[test]
    fn requeue_capped_per_key() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Word);
        eng.pack = ExercisePack {
            title: "one".into(),
            exercises: vec![Exercise::ChooseWord {
                stage: Some(ExerciseStage::Word),
                prompt: "q".into(),
                options: vec!["дом".into(), "чай".into()],
                answer: "дом".into(),
            }],
        };
        eng.handle(Command::StartSession);
        let mut max_len = 1;
        for _ in 0..6 {
            if !matches!(eng.screen(), Screen::Exercise) {
                break;
            }
            let ex = eng.current_exercise().cloned().unwrap();
            eng.handle(Command::Submit(wrong_answer_for(&ex)));
            eng.handle(Command::AdvanceAfterFeedback);
            if let Some(s) = eng.session() {
                max_len = max_len.max(s.exercises.len());
            }
        }
        assert_eq!(max_len, 4);
    }

    #[test]
    fn skip_repeat_blocks_later_requeue_same_key() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Word);
        eng.pack = ExercisePack {
            title: "same-key".into(),
            exercises: vec![
                Exercise::ChooseWord {
                    stage: Some(ExerciseStage::Word),
                    prompt: "q1".into(),
                    options: vec!["дом".into(), "чай".into()],
                    answer: "дом".into(),
                },
                Exercise::ChooseWord {
                    stage: Some(ExerciseStage::Word),
                    prompt: "q2".into(),
                    options: vec!["дом".into(), "стол".into()],
                    answer: "дом".into(),
                },
            ],
        };
        eng.handle(Command::StartSession);
        let len = eng.session().unwrap().exercises.len();
        let ex = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex)));
        eng.handle(Command::SkipRepeatAndAdvance);
        assert_eq!(eng.session().unwrap().exercises.len(), len);
        assert!(matches!(eng.screen(), Screen::Exercise));
        let ex2 = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex2)));
        assert_eq!(eng.feedback_requeues_left(), Some(0));
        let len_before = eng.session().unwrap().exercises.len();
        eng.handle(Command::AdvanceAfterFeedback);
        if let Some(s) = eng.session() {
            assert_eq!(s.exercises.len(), len_before);
        } else {
            assert_eq!(len_before, len);
            assert!(matches!(eng.screen(), Screen::Result { .. }));
        }
    }

    #[test]
    fn higher_session_boost_comes_earlier() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Word);
        // «дом» слабее в карте — стартует первым (без случайного порядка).
        eng.progress.speech_map.record("дом", false);
        eng.progress.speech_map.record("дом", false);
        eng.pack = ExercisePack {
            title: "two".into(),
            exercises: vec![
                Exercise::ChooseWord {
                    stage: Some(ExerciseStage::Word),
                    prompt: "a".into(),
                    options: vec!["дом".into(), "чай".into()],
                    answer: "дом".into(),
                },
                Exercise::ChooseWord {
                    stage: Some(ExerciseStage::Word),
                    prompt: "b".into(),
                    options: vec!["чай".into(), "стол".into()],
                    answer: "чай".into(),
                },
            ],
        };
        eng.handle(Command::StartSession);
        let ex0 = eng.current_exercise().cloned().unwrap();
        match &ex0 {
            Exercise::ChooseWord { answer, .. } => assert_eq!(answer, "дом"),
            _ => panic!("ожидали «дом» первым"),
        }
        eng.handle(Command::Submit(wrong_answer_for(&ex0)));
        eng.handle(Command::AdvanceAfterFeedback);
        let ex1 = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex1)));
        eng.handle(Command::AdvanceAfterFeedback);
        let ex2 = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex2)));
        eng.handle(Command::AdvanceAfterFeedback);
        let next = eng.current_exercise().cloned().unwrap();
        match next {
            Exercise::ChooseWord { answer, .. } => assert_eq!(answer, "дом"),
            _ => panic!("ожидали повтор «дом»"),
        }
    }

    #[test]
    fn diagnosis_does_not_requeue() {
        let mut eng = Engine::new_logic_only();
        eng.pack = tiny_test_pack();
        eng.handle(Command::StartDiagnosis);
        let len = eng.session().unwrap().exercises.len();
        let ex = eng.current_exercise().cloned().unwrap();
        eng.handle(Command::Submit(wrong_answer_for(&ex)));
        eng.handle(Command::AdvanceAfterFeedback);
        assert_eq!(eng.session().unwrap().exercises.len(), len);
    }

    #[test]
    fn submit_updates_speech_map() {
        let mut eng = Engine::new_logic_only();
        eng.progress.level = Some(ExerciseStage::Syllable);
        eng.progress.speech_map = Default::default();
        eng.handle(Command::StartSession);
        let ex = eng.current_exercise().cloned().unwrap();
        let key = ex.map_key().expect("ключ");
        match ex {
            Exercise::ChooseWord { answer, .. } => {
                eng.handle(Command::Submit(UserAnswer::Choice(answer)));
            }
            Exercise::BuildPhrase { answer, .. } => {
                let parts: Vec<_> = answer.split_whitespace().map(str::to_string).collect();
                eng.handle(Command::Submit(UserAnswer::Phrase(parts)));
            }
            Exercise::ReadAloud { .. } => {
                eng.handle(Command::Submit(UserAnswer::ReadDone {
                    matched: true,
                    heard: None,
                }));
            }
        }
        let stat = eng.progress().speech_map.items.get(&key).unwrap();
        assert_eq!(stat.attempts, 1);
        assert_eq!(stat.correct, 1);
    }

    #[test]
    fn dictaphone_open_and_leave() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenDictaphone);
        assert!(matches!(eng.screen(), Screen::Dictaphone));
        eng.handle(Command::LeaveDictaphone);
        assert!(matches!(eng.screen(), Screen::Home));
    }

    #[test]
    fn settings_screen_from_home() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::OpenSettings);
        assert!(matches!(eng.screen(), Screen::Settings));
        eng.handle(Command::LeaveSettings);
        assert!(matches!(eng.screen(), Screen::Home));
        assert!(eng.model_download_note().is_none());
    }

    #[test]
    fn set_language_switches_default_pack() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::SetPack("daily".into()));
        assert_eq!(eng.pack_id(), "daily");
        eng.handle(Command::OpenSettings);
        eng.handle(Command::SetLanguage(AppLanguage::En));
        assert_eq!(eng.language(), AppLanguage::En);
        assert_eq!(eng.pack_id(), "starter_en");
        assert!(eng.pack().title.contains("Sounds") || eng.pack().title.contains("syllables"));
        assert!(matches!(eng.screen(), Screen::Settings));
        let ids: Vec<_> = eng.pack_catalog().into_iter().map(|e| e.id).collect();
        assert!(ids.contains(&"starter_en".into()));
        assert!(!ids.contains(&"daily".into()));
        eng.handle(Command::SetLanguage(AppLanguage::Ru));
        assert_eq!(eng.language(), AppLanguage::Ru);
        assert_eq!(eng.pack_id(), "starter");
    }

    #[test]
    #[cfg(not(feature = "asr"))]
    fn start_model_download_noop_without_asr() {
        let mut eng = Engine::new_logic_only();
        eng.handle(Command::StartModelDownload);
        assert!(matches!(eng.model_download(), ModelDownloadState::Idle));
    }

    #[test]
    #[cfg(feature = "asr")]
    fn start_model_download_begins_when_model_missing() {
        let tmp = std::env::temp_dir().join(format!("softecho-data-{}", std::process::id()));
        std::env::set_var("XDG_DATA_HOME", &tmp);
        let mut eng = Engine::new_logic_only();
        assert!(matches!(eng.asr_status(), AsrStatus::ModelMissing));
        eng.handle(Command::StartModelDownload);
        assert!(matches!(
            eng.model_download(),
            ModelDownloadState::Working { .. }
        ));
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn poll_download_done_updates_state_after_reload() {
        let (tx, rx) = mpsc::channel();
        tx.send(DownloadMsg::Done).unwrap();
        drop(tx);
        let mut eng = Engine::new_logic_only();
        eng.test_download_rx(rx);
        let tick = eng.tick();
        assert!(tick.want_repaint);
        match eng.asr_status() {
            AsrStatus::Ready => {
                assert!(matches!(
                    eng.model_download(),
                    ModelDownloadState::Succeeded
                ));
                assert_eq!(
                    eng.model_download_note(),
                    Some("Модель установлена. Голос готов.")
                );
            }
            _ => {
                assert!(matches!(
                    eng.model_download(),
                    ModelDownloadState::Failed(_)
                ));
                assert!(eng.model_download_note().is_none());
            }
        }
    }

    #[test]
    fn poll_download_err_sets_failed() {
        let (tx, rx) = mpsc::channel();
        tx.send(DownloadMsg::Err("сеть".into())).unwrap();
        drop(tx);
        let mut eng = Engine::new_logic_only();
        eng.test_download_rx(rx);
        eng.tick();
        assert!(matches!(
            eng.model_download(),
            ModelDownloadState::Failed(e) if e == "сеть"
        ));
    }

    #[test]
    fn poll_download_percent_updates_progress() {
        let (tx, rx) = mpsc::channel();
        tx.send(DownloadMsg::Phase("Скачиваю…".into())).unwrap();
        tx.send(DownloadMsg::Percent(42)).unwrap();
        drop(tx);
        let mut eng = Engine::new_logic_only();
        eng.test_download_rx(rx);
        eng.tick();
        assert!(matches!(
            eng.model_download(),
            ModelDownloadState::Working {
                label,
                percent: Some(42)
            } if label == "Скачиваю…"
        ));
    }

    #[test]
    fn tick_repaints_while_download_working() {
        let mut eng = Engine::new_logic_only();
        eng.model_download = ModelDownloadState::Working {
            label: "Скачиваю…".into(),
            percent: Some(10),
        };
        let tick = eng.tick();
        assert!(tick.want_repaint);
        assert_eq!(tick.repaint_after, Some(Duration::from_millis(200)));
    }

    #[test]
    fn tick_idle_is_quiet() {
        let mut eng = Engine::new_logic_only();
        let t = eng.tick();
        assert!(!t.want_repaint);
        assert!(t.repaint_after.is_none());
    }
}

