//! Состояние и логика тренажёра (серверная часть).
//! UI / будущий клиент общаются только через Command + геттеры + tick.

use super::asr::{
    create_recognizer, AsrStatus, ListenConfig, ListenEvent, SpeechRecognizer,
};
use super::data::{
    append_dictaphone_text, load_progress, load_starter_pack, new_dictaphone_path,
    save_dictaphone_text, save_progress, user_data_dir, vosk_model_dir,
};
use super::exercise::{
    check_answer, speech_matches, CheckResult, Exercise, ExercisePack, Progress, UserAnswer,
};
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
    /// Текст по мере распознавания.
    pub live_text: String,
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
    /// Vosk разгребает буфер — показать «подождите».
    please_wait: bool,
    dictaphone: DictaphoneState,
    model_download: ModelDownloadState,
    model_download_rx: Option<Receiver<DownloadMsg>>,
    model_download_note: Option<String>,
}

impl Engine {
    pub fn new() -> Self {
        Self::create(vosk_model_dir())
    }

    fn create(model: Option<std::path::PathBuf>) -> Self {
        let (pack, load_error) = match load_starter_pack() {
            Ok(p) => (p, None),
            Err(e) => (
                ExercisePack {
                    title: "Пусто".into(),
                    exercises: vec![],
                },
                Some(e),
            ),
        };

        let recognizer = Arc::new(Mutex::new(create_recognizer(model.as_deref())));

        Self {
            screen: Screen::Home,
            pack,
            progress: load_progress(),
            session: None,
            load_error,
            save_error: None,
            recognizer,
            listen_rx: None,
            listen_target: None,
            listen_purpose: None,
            listen_live: Arc::new(Mutex::new(String::new())),
            please_wait: false,
            dictaphone: DictaphoneState::default(),
            model_download: ModelDownloadState::default(),
            model_download_rx: None,
            model_download_note: None,
        }
    }

    /// Движок без загрузки модели Vosk (юнит-тесты логики).
    #[cfg(test)]
    fn new_logic_only() -> Self {
        Self::create(None)
    }

    fn reload_recognizer(&mut self) {
        self.abort_listen();
        let model = vosk_model_dir();
        if let Ok(mut r) = self.recognizer.lock() {
            *r = create_recognizer(model.as_deref());
        }
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

        let (tx, rx) = mpsc::channel();
        self.model_download_rx = Some(rx);
        self.model_download = ModelDownloadState::Working {
            label: "Подготовка…".into(),
            percent: None,
        };
        self.model_download_note = None;
        spawn_model_download(dest, tx);
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
        let mut exercises = self.pack.exercises.clone();
        exercises.shuffle(&mut rand::rng());
        if exercises.is_empty() {
            return;
        }
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
        };
        session.prepare_current();
        self.session = Some(session);
        self.screen = Screen::Exercise;
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

        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.listening = false;
        let Some(ex) = session.exercises.get(session.index).cloned() else {
            return;
        };
        let result = check_answer(&ex, &answer);
        if result == CheckResult::Correct {
            session.correct += 1;
        }
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
        self.screen = Screen::Feedback {
            result,
            heard,
            expected,
        };
    }

    fn advance_after_feedback(&mut self) {
        self.listen_rx = None;
        self.listen_target = None;
        self.listen_purpose = None;
        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.index += 1;
        if session.index >= session.exercises.len() {
            let correct = session.correct;
            let total = session.exercises.len() as u32;
            self.progress.record_session(correct, total);
            self.persist_progress();
            self.session = None;
            self.screen = Screen::Result { correct, total };
        } else {
            session.prepare_current();
            self.screen = Screen::Exercise;
        }
    }

    fn try_listen(&mut self) {
        if self.listen_rx.is_some() {
            return;
        }

        let target = self
            .current_exercise()
            .and_then(|e| e.target_text())
            .map(|s| s.to_string());
        let Some(target) = target else {
            return;
        };

        let grammar: Vec<String> = target
            .split_whitespace()
            .map(|w| w.to_lowercase())
            .collect();

        if let Some(session) = self.session.as_mut() {
            session.listening = true;
            session.listen_error = None;
            session.live_text.clear();
        }

        let live = Arc::clone(&self.listen_live);
        if let Ok(mut g) = live.lock() {
            g.clear();
        }
        self.please_wait = false;
        self.spawn_listen(
            grammar,
            Some(target),
            ListenPurpose::Exercise,
            ListenConfig::single_utterance(live),
        );
    }

    fn try_listen_dictaphone(&mut self) {
        if self.listen_rx.is_some() {
            return;
        }
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
        self.dictaphone.live_text.clear();
        self.dictaphone.transcript.clear();
        self.dictaphone.error = None;
        self.dictaphone.save_path = None;
        self.dictaphone.save_note = None;
        if let Ok(mut g) = self.dictaphone.live_partial.lock() {
            g.clear();
        }
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
                }
                ListenEvent::Done(outcome) => {
                    self.listen_rx = None;
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
                                    if self.dictaphone.transcript.is_empty()
                                        && !heard.text.is_empty()
                                    {
                                        self.append_dictaphone_phrase(&heard.text);
                                    }
                                    self.dictaphone.live_text.clear();
                                    if self.dictaphone.save_path.is_some()
                                        && !self.dictaphone.transcript.is_empty()
                                    {
                                        self.save_dictaphone_now();
                                    }
                                }
                                Err(e) => {
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
                                    if let Some(session) = self.session.as_mut() {
                                        session.live_text = heard.text.clone();
                                    }
                                    let matched = speech_matches(&target, &heard.text);
                                    self.submit(UserAnswer::ReadDone {
                                        matched,
                                        heard: Some(heard.text),
                                    });
                                }
                                Err(e) => {
                                    if let Some(session) = self.session.as_mut() {
                                        session.listen_error = Some(e);
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
                self.screen = Screen::Home;
            }
            Command::StartSession => {
                self.abort_listen();
                self.start_session();
            }
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
            Command::AdvanceAfterFeedback => self.advance_after_feedback(),
            Command::Submit(answer) => self.submit(answer),
            Command::ListenExercise => self.try_listen(),
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

    pub fn dictaphone(&self) -> &DictaphoneState {
        &self.dictaphone
    }

    pub fn session(&self) -> Option<&SessionState> {
        self.session.as_ref()
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
    fn start_session_opens_exercise() {
        let mut eng = Engine::new_logic_only();
        assert!(matches!(eng.screen(), Screen::Home));
        eng.handle(Command::StartSession);
        assert!(matches!(eng.screen(), Screen::Exercise));
        assert!(eng.session().is_some());
        let s = eng.session().unwrap();
        assert!(!s.exercises.is_empty());
        assert_eq!(s.index, 0);
    }

    #[test]
    fn choose_word_flow_to_feedback() {
        let mut eng = Engine::new_logic_only();
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
        eng.handle(Command::StartSession);
        eng.handle(Command::GoHome);
        assert!(matches!(eng.screen(), Screen::Home));
        assert!(eng.session().is_none());
        assert!(!eng.please_wait());
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

