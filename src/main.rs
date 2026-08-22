mod asr;
mod data;
mod exercise;

use asr::{create_recognizer, AsrStatus, ListenOutcome, SpeechRecognizer};
use data::{load_progress, load_starter_pack, save_progress, vosk_model_dir};
use exercise::{
    check_answer, speech_matches, CheckResult, Exercise, ExercisePack, Progress, UserAnswer,
};

use eframe::egui::{self, Color32, FontId, RichText, Sense, Vec2};
use rand::seq::SliceRandom;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Речевой тренажёр"),
        ..Default::default()
    };

    eframe::run_native(
        "Речевой тренажёр",
        options,
        Box::new(|cc| Ok(Box::new(TrainerApp::new(cc)))),
    )
}

#[derive(Clone)]
enum Screen {
    Home,
    Exercise,
    Feedback {
        result: CheckResult,
        detail: Option<String>,
    },
    Result {
        correct: u32,
        total: u32,
    },
}

struct SessionState {
    exercises: Vec<Exercise>,
    index: usize,
    correct: u32,
    /// Перемешанные варианты для «выбор слова».
    choice_options: Vec<String>,
    /// Для «собрать фразу»: доступные и выбранные слова.
    pool: Vec<String>,
    picked: Vec<String>,
    listening: bool,
    listen_error: Option<String>,
}

struct TrainerApp {
    screen: Screen,
    pack: ExercisePack,
    progress: Progress,
    session: Option<SessionState>,
    load_error: Option<String>,
    save_error: Option<String>,
    recognizer: Arc<Mutex<Box<dyn SpeechRecognizer>>>,
    /// Фоновый результат «Сказать» (не блокирует UI).
    listen_rx: Option<Receiver<Result<ListenOutcome, String>>>,
    listen_target: Option<String>,
}

impl TrainerApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);

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

        let model = vosk_model_dir();
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
        }
    }

    fn asr_status(&self) -> AsrStatus {
        match self.recognizer.try_lock() {
            Ok(r) => r.status(),
            // Пока идёт запись, мьютекс занят — не блокируем UI.
            Err(_) => AsrStatus::Ready,
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
        };
        session.prepare_current();
        self.session = Some(session);
        self.screen = Screen::Exercise;
    }

    fn current_exercise(&self) -> Option<&Exercise> {
        let s = self.session.as_ref()?;
        s.exercises.get(s.index)
    }

    fn submit(&mut self, answer: UserAnswer) {
        // Отменяем отложенный ASR, чтобы не зачесть ответ дважды.
        self.listen_rx = None;
        self.listen_target = None;

        let Some(session) = self.session.as_mut() else {
            return;
        };
        session.listening = false;
        let Some(ex) = session.exercises.get(session.index) else {
            return;
        };
        let result = check_answer(ex, &answer);
        if result == CheckResult::Correct {
            session.correct += 1;
        }
        let detail = match &answer {
            UserAnswer::ReadDone {
                heard: Some(h), ..
            } => Some(format!("Распознано: {h}")),
            _ => None,
        };
        self.screen = Screen::Feedback { result, detail };
    }

    fn advance_after_feedback(&mut self) {
        self.listen_rx = None;
        self.listen_target = None;
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
        }

        let (tx, rx) = mpsc::channel();
        let recognizer = Arc::clone(&self.recognizer);
        self.listen_rx = Some(rx);
        self.listen_target = Some(target);

        thread::spawn(move || {
            let outcome = match recognizer.lock() {
                Ok(mut r) => r.listen_once(&grammar),
                Err(_) => Err("Распознаватель недоступен".into()),
            };
            let _ = tx.send(outcome);
        });
    }

    fn poll_listen(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.listen_rx.as_ref() else {
            return;
        };

        match rx.try_recv() {
            Ok(outcome) => {
                self.listen_rx = None;
                let target = self.listen_target.take().unwrap_or_default();
                // Уже ушли с упражнения / ответили вручную — поздний ASR игнорируем.
                let still_on_exercise = matches!(self.screen, Screen::Exercise)
                    && self.session.as_ref().is_some_and(|s| s.listening);
                if let Some(session) = self.session.as_mut() {
                    session.listening = false;
                }
                if !still_on_exercise {
                    return;
                }
                match outcome {
                    Ok(heard) => {
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
            Err(mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.listen_rx = None;
                self.listen_target = None;
                if let Some(session) = self.session.as_mut() {
                    session.listening = false;
                    session.listen_error = Some("Сбой записи голоса".into());
                }
            }
        }
    }
}

impl SessionState {
    fn prepare_current(&mut self) {
        self.pool.clear();
        self.picked.clear();
        self.choice_options.clear();
        self.listen_error = None;
        self.listening = false;
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

impl eframe::App for TrainerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_listen(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(12.0);
            match self.screen.clone() {
                Screen::Home => self.ui_home(ui),
                Screen::Exercise => self.ui_exercise(ui),
                Screen::Feedback { result, detail } => self.ui_feedback(ui, result, detail),
                Screen::Result { correct, total } => self.ui_result(ui, correct, total),
            }
        });
    }
}

impl TrainerApp {
    fn ui_home(&mut self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new("Речевой тренажёр")
                    .font(FontId::proportional(42.0))
                    .strong()
                    .color(Color32::from_rgb(20, 40, 60)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new("Восстановление речи · занятия дома")
                    .font(FontId::proportional(22.0))
                    .color(Color32::from_rgb(60, 80, 100)),
            );
            ui.add_space(24.0);
            ui.label(
                RichText::new(format!("Набор: {}", self.pack.title))
                    .font(FontId::proportional(20.0)),
            );
            ui.label(
                RichText::new(format!(
                    "Пройдено занятий: {} · верных ответов: {}/{}",
                    self.progress.sessions_completed,
                    self.progress.total_correct,
                    self.progress.total_answered
                ))
                .font(FontId::proportional(18.0))
                .color(Color32::DARK_GRAY),
            );

            ui.add_space(12.0);
            match self.asr_status() {
                AsrStatus::Ready => ui.label(
                    RichText::new("Голос: готов (Vosk)")
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                ),
                AsrStatus::ModelMissing => ui.label(
                    RichText::new("Голос: модель не найдена — доступна самопроверка")
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                ),
                AsrStatus::Disabled => ui.label(
                    RichText::new("Голос: выключен в сборке (текстовый режим)")
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                ),
                AsrStatus::Error(e) => ui.label(
                    RichText::new(format!("Голос: {e}"))
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(160, 60, 40)),
                ),
            };

            if let Some(err) = &self.load_error {
                ui.colored_label(Color32::RED, err);
            }
            if let Some(err) = &self.save_error {
                ui.colored_label(Color32::from_rgb(160, 60, 40), format!("Прогресс: {err}"));
            }

            ui.add_space(36.0);
            if big_button(ui, "Начать занятие", Color32::from_rgb(40, 110, 180)).clicked() {
                self.start_session();
            }
        });
    }

    fn ui_exercise(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let total = session.exercises.len();
        let idx = session.index;
        let progress_label = format!("Упражнение {} из {}", idx + 1, total);

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(progress_label)
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(16.0);
        });

        let exercise = match self.current_exercise().cloned() {
            Some(e) => e,
            None => return,
        };

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(exercise.prompt())
                    .font(FontId::proportional(28.0))
                    .strong(),
            );
            ui.add_space(24.0);
        });

        match exercise {
            Exercise::ChooseWord { .. } => {
                let options = self
                    .session
                    .as_ref()
                    .map(|s| s.choice_options.clone())
                    .unwrap_or_default();
                ui.vertical_centered(|ui| {
                    for opt in &options {
                        if big_button(ui, opt, Color32::from_rgb(50, 90, 130)).clicked() {
                            self.submit(UserAnswer::Choice(opt.clone()));
                            return;
                        }
                        ui.add_space(10.0);
                    }
                });
            }
            Exercise::BuildPhrase { .. } => {
                self.ui_build_phrase(ui);
            }
            Exercise::ReadAloud { text, .. } => {
                self.ui_read_aloud(ui, &text);
            }
        }
    }

    fn ui_build_phrase(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.session.as_mut() else {
            return;
        };

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(if session.picked.is_empty() {
                    "Нажимайте слова по порядку".into()
                } else {
                    session.picked.join(" ")
                })
                .font(FontId::proportional(26.0))
                .color(Color32::from_rgb(20, 60, 40)),
            );
            ui.add_space(20.0);
        });

        ui.horizontal_wrapped(|ui| {
            ui.add_space(ui.available_width() * 0.1);
            let mut clicked: Option<usize> = None;
            for (i, w) in session.pool.iter().enumerate() {
                if big_button(ui, w, Color32::from_rgb(70, 100, 140)).clicked() {
                    clicked = Some(i);
                }
            }
            if let Some(i) = clicked {
                let w = session.pool.remove(i);
                session.picked.push(w);
            }
        });

        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                if big_button(ui, "Сбросить", Color32::from_rgb(120, 90, 70)).clicked() {
                    if let Some(s) = self.session.as_mut() {
                        s.prepare_current();
                    }
                }
                ui.add_space(12.0);
                let can_check = self
                    .session
                    .as_ref()
                    .map(|s| !s.picked.is_empty() && s.pool.is_empty())
                    .unwrap_or(false);
                if can_check && big_button(ui, "Проверить", Color32::from_rgb(40, 130, 90)).clicked()
                {
                    let parts = self.session.as_ref().map(|s| s.picked.clone()).unwrap_or_default();
                    self.submit(UserAnswer::Phrase(parts));
                }
            });
        });
    }

    fn ui_read_aloud(&mut self, ui: &mut egui::Ui, text: &str) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(40.0))
                    .strong()
                    .color(Color32::from_rgb(15, 35, 55)),
            );
            ui.add_space(28.0);

            let asr_ready = matches!(self.asr_status(), AsrStatus::Ready);
            let listening = self
                .session
                .as_ref()
                .map(|s| s.listening)
                .unwrap_or(false);

            if asr_ready {
                if listening {
                    ui.label(
                        RichText::new("Слушаю…")
                            .font(FontId::proportional(24.0))
                            .color(Color32::from_rgb(140, 60, 100)),
                    );
                } else if big_button(ui, "Сказать", Color32::from_rgb(140, 60, 100)).clicked() {
                    self.try_listen();
                }
                ui.add_space(12.0);
            }

            if let Some(err) = self.session.as_ref().and_then(|s| s.listen_error.clone()) {
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                ui.add_space(8.0);
            }

            ui.label(
                RichText::new("Или отметьте сами:")
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(8.0);
            if listening {
                ui.label(
                    RichText::new("Подождите окончания записи…")
                        .font(FontId::proportional(18.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                ui.horizontal(|ui| {
                    if big_button(ui, "Получилось", Color32::from_rgb(40, 130, 90)).clicked() {
                        self.submit(UserAnswer::ReadDone {
                            matched: true,
                            heard: None,
                        });
                    }
                    ui.add_space(12.0);
                    if big_button(ui, "Не получилось", Color32::from_rgb(150, 70, 60)).clicked() {
                        self.submit(UserAnswer::ReadDone {
                            matched: false,
                            heard: None,
                        });
                    }
                });
            }
        });
    }

    fn ui_feedback(&mut self, ui: &mut egui::Ui, result: CheckResult, detail: Option<String>) {
        ui.vertical_centered(|ui| {
            ui.add_space(60.0);
            let (text, color) = match result {
                CheckResult::Correct => ("Верно", Color32::from_rgb(30, 130, 70)),
                CheckResult::Incorrect => ("Неверно", Color32::from_rgb(160, 50, 40)),
            };
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(48.0))
                    .strong()
                    .color(color),
            );
            if let Some(d) = detail {
                ui.add_space(12.0);
                ui.label(RichText::new(d).font(FontId::proportional(22.0)));
            }
            ui.add_space(40.0);
            if big_button(ui, "Дальше", Color32::from_rgb(40, 110, 180)).clicked() {
                self.advance_after_feedback();
            }
        });
    }

    fn ui_result(&mut self, ui: &mut egui::Ui, correct: u32, total: u32) {
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.label(
                RichText::new("Занятие закончено")
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("Верно: {correct} из {total}"))
                    .font(FontId::proportional(28.0)),
            );
            if let Some(err) = &self.save_error {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("Не удалось сохранить прогресс: {err}"),
                );
            }
            ui.add_space(40.0);
            if big_button(ui, "На главный экран", Color32::from_rgb(40, 110, 180)).clicked() {
                self.screen = Screen::Home;
            }
            ui.add_space(12.0);
            if big_button(ui, "Ещё раз", Color32::from_rgb(40, 130, 90)).clicked() {
                self.start_session();
            }
        });
    }
}

fn big_button(ui: &mut egui::Ui, label: &str, fill: Color32) -> egui::Response {
    let text = RichText::new(label)
        .font(FontId::proportional(24.0))
        .color(Color32::WHITE);
    let desired = Vec2::new(280.0, 56.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    if ui.is_rect_visible(rect) {
        let bg = if response.hovered() {
            lighten(fill, 20)
        } else {
            fill
        };
        ui.painter()
            .rect_filled(rect, 8.0, bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text.text(),
            FontId::proportional(24.0),
            Color32::WHITE,
        );
    }
    response
}

fn lighten(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(amount),
        c.g().saturating_add(amount),
        c.b().saturating_add(amount),
    )
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::proportional(20.0),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::proportional(22.0),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::proportional(32.0),
    );
    ctx.set_style(style);

    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Color32::from_rgb(245, 248, 250);
    visuals.window_fill = Color32::from_rgb(245, 248, 250);
    ctx.set_visuals(visuals);
}
