//! Прослушивание ASR и диктофон (состояние канала + команды Engine).

use super::super::asr::{ListenConfig, ListenEvent};
use super::super::data::{
    append_dictaphone_text, new_dictaphone_path, save_dictaphone_text,
};
use super::super::exercise::speech_matches;
use super::super::protocol::{Screen, TickResult};
use super::Engine;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ListenPurpose {
    Exercise,
    Dictaphone,
}

impl Engine {
    pub(super) fn try_listen(&mut self) {
        if self.listen_rx.is_some() {
            return;
        }
        self.reap_finished_listen_worker();
        if self.listen_worker_busy() {
            if let Some(session) = self.session.as_mut() {
                session.listen_error =
                    Some("Подождите, предыдущая запись ещё останавливается".into());
            }
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

    pub(super) fn stop_exercise_listen(&mut self) {
        if let Some(stop) = &self.exercise_listen_stop {
            stop.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn try_listen_dictaphone(&mut self) {
        if self.listen_rx.is_some() {
            return;
        }
        self.reap_finished_listen_worker();
        if self.listen_worker_busy() {
            self.dictaphone.error =
                Some("Подождите, предыдущая запись ещё останавливается".into());
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

    pub(super) fn stop_dictaphone(&mut self) {
        if let Some(stop) = &self.dictaphone.stop {
            stop.store(true, Ordering::Relaxed);
        }
    }

    pub(super) fn clear_dictaphone_buffer(&mut self) {
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

    pub(super) fn save_dictaphone_now(&mut self) {
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

    pub(super) fn append_dictaphone_phrase(&mut self, phrase: &str) {
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

    pub(super) fn exercise_heard_text(&self, heard: &str) -> String {
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

    pub(super) fn flush_dictaphone_live_tail(&mut self) {
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

    pub(super) fn listen_worker_busy(&self) -> bool {
        self.listen_join
            .as_ref()
            .is_some_and(|h| !h.is_finished())
    }

    /// Снять уже завершённый listen-воркер (после Done или быстрый abort).
    pub(super) fn reap_finished_listen_worker(&mut self) {
        let Some(handle) = self.listen_join.take() else {
            return;
        };
        if handle.is_finished() {
            let _ = handle.join();
        } else {
            self.listen_join = Some(handle);
        }
    }

    fn spawn_listen(
        &mut self,
        grammar: Vec<String>,
        target: Option<String>,
        purpose: ListenPurpose,
        config: ListenConfig,
    ) {
        debug_assert!(
            !self.listen_worker_busy(),
            "spawn_listen при живом воркере — сначала gate в try_listen*"
        );
        let (tx, rx) = mpsc::channel();
        let recognizer = Arc::clone(&self.recognizer);
        self.listen_rx = Some(rx);
        self.listen_target = target;
        self.listen_purpose = Some(purpose);

        self.listen_join = Some(thread::spawn(move || match recognizer.lock() {
            Ok(mut r) => r.listen_stream(&grammar, tx, config),
            Err(_) => {
                let _ = tx.send(ListenEvent::Done(Err(
                    "Распознаватель недоступен".into(),
                )));
            }
        }));
    }

    pub(super) fn sync_live_text(&mut self) {
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

    /// Неблокирующий poll listen-канала для `tick` (не путать с drain_on_abort).
    pub(super) fn poll_listen_events(&mut self, tick: &mut TickResult) {
        if self.listen_rx.is_some() {
            self.sync_live_text();
        }

        let Some(rx) = self.listen_rx.as_ref() else {
            return;
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
            return;
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
                    // Только диктофон (continuous). Иначе latent-запись в чужой transcript.
                    if matches!(self.listen_purpose, Some(ListenPurpose::Dictaphone)) {
                        self.append_dictaphone_phrase(&phrase);
                        self.dictaphone.live_text.clear();
                        if let Ok(mut g) = self.listen_live.lock() {
                            g.clear();
                        }
                        if let Ok(mut g) = self.dictaphone.live_partial.lock() {
                            g.clear();
                        }
                    }
                }
                ListenEvent::Done(outcome) => {
                    self.listen_rx = None;
                    if let Some(handle) = self.listen_join.take() {
                        let _ = handle.join();
                    }
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
                                    // Сначала save: save_dictaphone_now при Ok сбрасывает error.
                                    if self.dictaphone.save_path.is_some()
                                        && !self.dictaphone.transcript.is_empty()
                                    {
                                        self.save_dictaphone_now();
                                    }
                                    self.dictaphone.error = Some(e);
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
    }

    pub(crate) fn abort_listen(&mut self) {
        self.stop_playback();
        if let Some(stop) = &self.exercise_listen_stop {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(stop) = &self.dictaphone.stop {
            stop.store(true, Ordering::Relaxed);
        }
        // Спасти уже пришедшие Done/Utterance до сброса канала.
        if let Some(rx) = self.listen_rx.take() {
            self.drain_listen_rx_on_abort(rx);
        }
        self.exercise_listen_stop = None;
        self.listen_target = None;
        self.listen_purpose = None;
        self.please_wait = false;
        self.dictaphone.stop = None;
        self.dictaphone.listening = false;
        if let Some(session) = self.session.as_mut() {
            session.listening = false;
        }
    }

    /// Неблокирующий drain: сохранить PCM / хвост диктофона из очереди.
    fn drain_listen_rx_on_abort(&mut self, rx: Receiver<ListenEvent>) {
        let purpose = self.listen_purpose;
        while let Ok(ev) = rx.try_recv() {
            match ev {
                ListenEvent::Utterance(phrase)
                    if matches!(purpose, Some(ListenPurpose::Dictaphone)) =>
                {
                    self.append_dictaphone_phrase(&phrase);
                }
                ListenEvent::Done(Ok(heard)) => {
                    self.store_last_clip(heard.pcm);
                    if matches!(purpose, Some(ListenPurpose::Dictaphone)) {
                        self.flush_dictaphone_live_tail();
                        if self.dictaphone.transcript.is_empty() && !heard.text.is_empty() {
                            self.append_dictaphone_phrase(&heard.text);
                        }
                    } else if matches!(purpose, Some(ListenPurpose::Exercise)) {
                        let text = self.exercise_heard_text(&heard.text);
                        if let Some(session) = self.session.as_mut() {
                            session.live_text = text;
                        }
                    }
                }
                ListenEvent::Done(Err(e)) => {
                    if matches!(purpose, Some(ListenPurpose::Dictaphone)) {
                        self.flush_dictaphone_live_tail();
                        self.dictaphone.error = Some(e);
                    } else if matches!(purpose, Some(ListenPurpose::Exercise)) {
                        if let Some(session) = self.session.as_mut() {
                            session.listen_error = Some(e);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

