//! Клиентский UI (egui). Общается с движком только через Command / геттеры / tick.

use crate::engine::{
    AppLanguage, AsrStatus, CheckResult, Command, Engine, Exercise, ExerciseStage,
    ModelDownloadState, Screen, SpeechRating, UserAnswer,
};
use crate::engine::exercise::speech_map_stage_summaries;
use crate::engine::i18n::{rating_label, stage_label};
use crate::engine::warmup::{WARMUP_LINKS, WARMUP_SCHEMAS};
use crate::ui::theme::{apply_theme, apply_theme_scale};
use crate::ui::widgets::{back_to_menu_button, big_button, footer_buttons, screen_scroll, str_byte_tail};

use eframe::egui::{self, Color32, FontId, OpenUrl, RichText};

pub struct UiApp {
    engine: Engine,
    editor_prompt: String,
    editor_text: String,
    editor_stage: ExerciseStage,
}

impl UiApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        apply_theme(&cc.egui_ctx);
        let engine = Engine::new();
        let editor_prompt = engine.ui_text().t("editor_prompt_default").into();
        Self {
            engine,
            editor_prompt,
            editor_text: String::new(),
            editor_stage: ExerciseStage::Word,
        }
    }

    fn sync_editor_prompt_default(&mut self) {
        let def = self.engine.ui_text().t("editor_prompt_default");
        if self.editor_prompt == "Скажите" || self.editor_prompt == "Say" || self.editor_prompt.is_empty() {
            self.editor_prompt = def.into();
        }
    }
}

impl eframe::App for UiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let scale = if self.engine.simple_mode() { 1.35 } else { 1.0 };
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("softecho_ui_scale"), scale));
        apply_theme_scale(ctx, scale);

        let tick = self.engine.tick();
        if tick.want_repaint {
            ctx.request_repaint();
        } else if let Some(after) = tick.repaint_after {
            ctx.request_repaint_after(after);
        }

        if !matches!(self.engine.screen(), Screen::Home) {
            egui::TopBottomPanel::top("menu_back_bar")
                .resizable(false)
                .show_separator_line(true)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        let menu = self.engine.ui_text().t("to_menu");
                        if back_to_menu_button(ui, menu).clicked() {
                            self.engine.handle(Command::GoHome);
                        }
                    });
                    ui.add_space(4.0);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(8.0);
            match self.engine.screen().clone() {
                Screen::Home => screen_scroll(ui, "home", |ui| self.ui_home(ui)),
                Screen::PackPick => screen_scroll(ui, "pack", |ui| self.ui_pack_pick(ui)),
                Screen::LevelPick => screen_scroll(ui, "level", |ui| self.ui_level_pick(ui)),
                Screen::Exercise => screen_scroll(ui, "ex", |ui| self.ui_exercise(ui)),
                Screen::Feedback {
                    result,
                    heard,
                    expected,
                } => screen_scroll(ui, "fb", |ui| self.ui_feedback(ui, result, heard, expected)),
                Screen::DiagnosisResult { level } => {
                    screen_scroll(ui, "diag_ok", |ui| self.ui_diagnosis_result(ui, level))
                }
                Screen::SpeechMap => {
                    screen_scroll(ui, "speech_map", |ui| self.ui_speech_map(ui))
                }
                Screen::ProgressReport => {
                    screen_scroll(ui, "progress", |ui| self.ui_progress_report(ui))
                }
                Screen::PackEditor => {
                    screen_scroll(ui, "pack_ed", |ui| self.ui_pack_editor(ui))
                }
                Screen::Warmup => screen_scroll(ui, "warmup", |ui| self.ui_warmup(ui)),
                Screen::Dictaphone => self.ui_dictaphone(ui),
                Screen::Settings => screen_scroll(ui, "settings", |ui| self.ui_settings(ui)),
                Screen::Result { correct, total, unique } => {
                    screen_scroll(ui, "result", |ui| self.ui_result(ui, correct, total, unique))
                }
            }
        });
    }
}

impl UiApp {
    fn ui_home(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        ui.vertical_centered(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new("SoftEcho")
                    .font(FontId::proportional(42.0))
                    .strong()
                    .color(Color32::from_rgb(20, 40, 60)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("tagline"))
                    .font(FontId::proportional(22.0))
                    .color(Color32::from_rgb(60, 80, 100)),
            );
            ui.add_space(20.0);
            ui.label(
                RichText::new(format!("{}: {}", t.t("pack"), self.engine.pack().title))
                    .font(FontId::proportional(20.0)),
            );
            ui.add_space(6.0);
            let level_text = match self.engine.level() {
                Some(l) => format!("{}: {}", t.t("level"), stage_label(lang, l)),
                None => format!("{}: {}", t.t("level"), t.t("level_none")),
            };
            ui.label(
                RichText::new(level_text)
                    .font(FontId::proportional(18.0))
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(4.0);
            ui.label(
                RichText::new(format!(
                    "{}: {} · {} {}/{}",
                    t.t("sessions"),
                    self.engine.progress().sessions_completed,
                    t.t("correct_count"),
                    self.engine.progress().total_correct,
                    self.engine.progress().total_answered
                ))
                .font(FontId::proportional(16.0))
                .color(Color32::DARK_GRAY),
            );

            if let Some(err) = self.engine.load_error() {
                ui.add_space(12.0);
                ui.colored_label(Color32::RED, err);
            }
            if let Some(err) = self.engine.save_error() {
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("{}: {err}", t.t("progress_err")),
                );
            }

            ui.add_space(32.0);
            if big_button(ui, t.t("start"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(12.0);
            if !self.engine.simple_mode() {
                if big_button(ui, t.t("diagnosis"), Color32::from_rgb(40, 130, 90)).clicked() {
                    self.engine.handle(Command::StartDiagnosis);
                }
                ui.add_space(12.0);
            }
            if big_button(ui, t.t("warmup"), Color32::from_rgb(70, 120, 100)).clicked() {
                self.engine.handle(Command::OpenWarmup);
            }
            if !self.engine.simple_mode() {
                ui.add_space(12.0);
                if big_button(ui, t.t("progress"), Color32::from_rgb(100, 80, 150)).clicked() {
                    self.engine.handle(Command::OpenProgress);
                }
                ui.add_space(12.0);
                if matches!(self.engine.asr_status(), AsrStatus::Ready) {
                    if big_button(ui, t.t("dictaphone"), Color32::from_rgb(140, 60, 100)).clicked() {
                        self.engine.handle(Command::OpenDictaphone);
                    }
                } else {
                    ui.label(
                        RichText::new(t.t("dictaphone_need_asr"))
                            .font(FontId::proportional(16.0))
                            .color(Color32::DARK_GRAY),
                    );
                }
            }
            ui.add_space(24.0);
            if big_button(ui, t.t("settings"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenSettings);
            }
        });
    }

    fn ui_pack_pick(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let current = self.engine.pack_id().to_string();
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            ui.label(
                RichText::new(t.t("pack_pick_title"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("language_hint"))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(24.0);
            for entry in self.engine.pack_catalog() {
                let selected = entry.id == current;
                let fill = if selected {
                    Color32::from_rgb(40, 130, 90)
                } else if entry.editable {
                    Color32::from_rgb(70, 110, 140)
                } else {
                    Color32::from_rgb(40, 110, 180)
                };
                let label = if entry.editable {
                    format!("{} · {}", entry.title, t.t("mine"))
                } else {
                    entry.title.clone()
                };
                if big_button(ui, &label, fill).clicked() {
                    self.engine.handle(Command::SetPack(entry.id));
                }
                ui.add_space(10.0);
            }
            ui.add_space(12.0);
            if big_button(ui, t.t("pack_editor"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenPackEditor);
            }
            ui.add_space(12.0);
        });
    }

    fn ui_level_pick(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        ui.vertical_centered(|ui| {
            ui.add_space(28.0);
            ui.label(
                RichText::new(t.t("level_pick_title"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("language_hint"))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(24.0);
            for level in ExerciseStage::ALL {
                if level == ExerciseStage::Twister && !self.engine.twister_unlocked() {
                    ui.add_space(12.0);
                    continue;
                }
                if big_button(ui, stage_label(lang, level), Color32::from_rgb(40, 110, 180)).clicked()
                {
                    self.engine.handle(Command::SetLevel(level));
                }
                ui.add_space(12.0);
            }
            ui.add_space(12.0);
            if big_button(ui, t.t("diagnosis"), Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::StartDiagnosis);
            }
        });
    }


    fn ui_speech_map(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new(t.t("speech_map"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{}: {}", t.t("pack"), self.engine.pack().title))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("speech_map_hint"))
                    .font(FontId::proportional(16.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(20.0);

            let entries = self.engine.speech_map_entries();
            let weak_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Weak)
                .count();
            let almost_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Almost)
                .count();
            let good_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Good)
                .count();
            let unknown_n = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Unknown)
                .count();
            if !entries.is_empty() {
                ui.label(
                    RichText::new(format!(
                        "{}: {weak_n} · {}: {almost_n} · {}: {good_n} · {}: {unknown_n}",
                        t.t("rating_weak"),
                        t.t("rating_almost"),
                        t.t("rating_good"),
                        t.t("rating_unknown"),
                    ))
                    .font(FontId::proportional(16.0))
                    .color(Color32::from_rgb(60, 80, 100)),
                );
                ui.add_space(12.0);
            }
            if entries.is_empty() {
                ui.label(
                    RichText::new(t.t("speech_map_empty"))
                        .font(FontId::proportional(18.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                let mut current_stage: Option<ExerciseStage> = None;
                for entry in &entries {
                    if current_stage != Some(entry.stage) {
                        current_stage = Some(entry.stage);
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new(stage_label(lang, entry.stage))
                                .font(FontId::proportional(22.0))
                                .strong()
                                .color(Color32::from_rgb(40, 70, 100)),
                        );
                        ui.add_space(8.0);
                    }
                    let color = match entry.rating {
                        SpeechRating::Good => Color32::from_rgb(40, 130, 70),
                        SpeechRating::Almost => Color32::from_rgb(180, 130, 30),
                        SpeechRating::Weak => Color32::from_rgb(180, 60, 50),
                        SpeechRating::Unknown => Color32::from_rgb(120, 120, 130),
                    };
                    let detail = if entry.attempts > 0 {
                        format!(
                            "{} — {}/{}",
                            rating_label(lang, entry.rating),
                            entry.correct,
                            entry.attempts
                        )
                    } else {
                        rating_label(lang, entry.rating).to_string()
                    };
                    ui.label(
                        RichText::new(format!("{} · {}", entry.label, detail))
                            .font(FontId::proportional(18.0))
                            .color(color),
                    );
                    ui.add_space(4.0);
                }
            }
        });
    }


    fn ui_pack_editor(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        ui.vertical_centered(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(t.t("pack_editor"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);

            if let Some(err) = self.engine.load_error() {
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                ui.add_space(12.0);
            }

            if self.engine.pack_editor().is_none() {
                ui.label(
                    RichText::new(format!("{}: {}", t.t("editor_now"), self.engine.pack().title))
                        .font(FontId::proportional(20.0)),
                );
                ui.add_space(12.0);
                ui.label(
                    RichText::new(t.t("editor_readonly"))
                        .font(FontId::proportional(17.0))
                        .color(Color32::DARK_GRAY),
                );
                ui.add_space(20.0);
                if big_button(ui, t.t("editor_clone"), Color32::from_rgb(40, 130, 90)).clicked() {
                    self.engine.handle(Command::ClonePackForEdit);
                }
                ui.add_space(8.0);
                if big_button(ui, t.t("back"), Color32::from_rgb(90, 100, 120)).clicked() {
                    self.engine.handle(Command::LeavePackEditor);
                }
                return;
            }

            let (pack_id, title, active_n, disabled_n, err, note) = {
                let ed = self.engine.pack_editor().unwrap();
                (
                    ed.pack_id.clone(),
                    ed.draft.title.clone(),
                    ed.draft.exercises.len(),
                    ed.draft.disabled.len(),
                    ed.error.clone(),
                    ed.note.clone(),
                )
            };

            ui.label(
                RichText::new(format!("{title}"))
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.label(
                RichText::new(format!(
                    "{}: {pack_id}.json · {} {active_n}, {} {disabled_n}",
                    t.t("editor_file"),
                    t.t("editor_active_n"),
                    t.t("editor_off_n"),
                ))
                .font(FontId::proportional(15.0))
                .color(Color32::DARK_GRAY),
            );
            if let Some(err) = err {
                ui.add_space(8.0);
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
            }
            if let Some(note) = note {
                ui.add_space(6.0);
                ui.label(
                    RichText::new(note)
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                );
            }

            ui.add_space(16.0);
            ui.label(
                RichText::new(t.t("editor_active"))
                    .font(FontId::proportional(22.0))
                    .strong(),
            );
            ui.add_space(8.0);
            let active_labels: Vec<(usize, String)> = self
                .engine
                .pack_editor()
                .map(|ed| {
                    ed.draft
                        .exercises
                        .iter()
                        .enumerate()
                        .map(|(i, ex)| {
                            let label = ex
                                .map_label()
                                .or_else(|| ex.target_text().map(|s| s.to_string()))
                                .unwrap_or_else(|| format!("#{}", i + 1));
                            (i, format!("{} · {}", stage_label(lang, ex.stage()), label))
                        })
                        .collect()
                })
                .unwrap_or_default();
            for (i, label) in active_labels {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&label)
                            .font(FontId::proportional(16.0))
                            .color(Color32::from_rgb(20, 40, 60)),
                    );
                    if ui.button(t.t("editor_off")).clicked() {
                        self.engine.handle(Command::EditorDisable(i));
                    }
                });
                ui.add_space(4.0);
            }

            ui.add_space(12.0);
            ui.label(
                RichText::new(t.t("editor_disabled"))
                    .font(FontId::proportional(22.0))
                    .strong(),
            );
            ui.add_space(8.0);
            let disabled_labels: Vec<(usize, String)> = self
                .engine
                .pack_editor()
                .map(|ed| {
                    ed.draft
                        .disabled
                        .iter()
                        .enumerate()
                        .map(|(i, ex)| {
                            let label = ex
                                .map_label()
                                .or_else(|| ex.target_text().map(|s| s.to_string()))
                                .unwrap_or_else(|| format!("#{}", i + 1));
                            (i, format!("{} · {}", stage_label(lang, ex.stage()), label))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if disabled_labels.is_empty() {
                ui.label(
                    RichText::new(t.t("empty"))
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                for (i, label) in disabled_labels {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(&label)
                                .font(FontId::proportional(16.0))
                                .color(Color32::DARK_GRAY),
                        );
                        if ui.button(t.t("editor_on")).clicked() {
                            self.engine.handle(Command::EditorEnable(i));
                        }
                    });
                    ui.add_space(4.0);
                }
            }

            ui.add_space(20.0);
            ui.label(
                RichText::new(t.t("editor_add_read"))
                    .font(FontId::proportional(22.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(t.t("editor_prompt")).font(FontId::proportional(15.0)));
            ui.add(
                egui::TextEdit::singleline(&mut self.editor_prompt)
                    .font(FontId::proportional(18.0))
                    .desired_width(280.0),
            );
            ui.add_space(6.0);
            ui.label(RichText::new(t.t("editor_text")).font(FontId::proportional(15.0)));
            ui.add(
                egui::TextEdit::singleline(&mut self.editor_text)
                    .font(FontId::proportional(18.0))
                    .desired_width(280.0),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                for st in [
                    ExerciseStage::Sound,
                    ExerciseStage::Syllable,
                    ExerciseStage::Word,
                    ExerciseStage::Phrase,
                    ExerciseStage::Twister,
                ] {
                    let selected = self.editor_stage == st;
                    if ui
                        .selectable_label(selected, stage_label(lang, st))
                        .clicked()
                    {
                        self.editor_stage = st;
                    }
                }
            });
            ui.add_space(10.0);
            if big_button(ui, t.t("add"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::EditorAddReadAloud {
                    prompt: self.editor_prompt.clone(),
                    text: self.editor_text.clone(),
                    stage: self.editor_stage,
                });
                self.editor_text.clear();
            }

            ui.add_space(20.0);
            if big_button(ui, t.t("save"), Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::EditorSave);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("back"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::LeavePackEditor);
            }
            ui.add_space(24.0);
        });
    }


    fn ui_progress_report(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        let progress = self.engine.progress().clone();
        let pack_title = self.engine.pack().title.clone();
        let entries = self.engine.speech_map_entries();
        let summaries = speech_map_stage_summaries(&entries);
        let report = self.engine.progress_report_text();

        ui.vertical_centered(|ui| {
            ui.add_space(16.0);
            ui.label(
                RichText::new(t.t("progress"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{}: {pack_title}", t.t("pack")))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            let level_text = match progress.level {
                Some(l) => stage_label(lang, l).to_string(),
                None => t.t("level_none").into(),
            };
            ui.label(
                RichText::new(format!("{}: {level_text}", t.t("level")))
                    .font(FontId::proportional(20.0))
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!(
                    "{}: {} · {} {}/{}",
                    t.t("sessions_total"),
                    progress.sessions_completed,
                    t.t("correct_count"),
                    progress.total_correct,
                    progress.total_answered
                ))
                .font(FontId::proportional(18.0)),
            );

            ui.add_space(20.0);
            ui.label(
                RichText::new(t.t("trend"))
                    .font(FontId::proportional(24.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            if progress.session_history.is_empty() {
                ui.label(
                    RichText::new(t.t("trend_empty"))
                        .font(FontId::proportional(17.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                if let Some(acc) = progress.recent_accuracy() {
                    ui.label(
                        RichText::new(format!(
                            "{} {}: {:.0}{}",
                            t.t("trend_recent"),
                            progress.session_history.len(),
                            acc * 100.0,
                            t.t("trend_pct")
                        ))
                        .font(FontId::proportional(18.0)),
                    );
                    ui.add_space(8.0);
                }
                for (i, s) in progress.session_history.iter().enumerate() {
                    let pct = if s.total == 0 {
                        0
                    } else {
                        (100 * s.correct) / s.total
                    };
                    let bar_n = (pct / 10).min(10) as usize;
                    let bar = "█".repeat(bar_n) + &"░".repeat(10 - bar_n);
                    ui.label(
                        RichText::new(format!(
                            "{}. {}/{} ({}%)  {bar}",
                            i + 1,
                            s.correct,
                            s.total,
                            pct
                        ))
                        .font(FontId::monospace(16.0))
                        .color(Color32::from_rgb(20, 40, 60)),
                    );
                    ui.add_space(2.0);
                }
            }

            ui.add_space(20.0);
            ui.label(
                RichText::new(t.t("map_by_stage"))
                    .font(FontId::proportional(24.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            if summaries.is_empty() {
                ui.label(
                    RichText::new(t.t("map_empty"))
                        .font(FontId::proportional(17.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                for s in &summaries {
                    ui.label(
                        RichText::new(format!(
                            "{}: {} {}, {} {}, {} {}, {} {}",
                            stage_label(lang, s.stage),
                            t.t("rating_good"),
                            s.good,
                            t.t("rating_almost"),
                            s.almost,
                            t.t("rating_weak"),
                            s.weak,
                            t.t("rating_unknown"),
                            s.unknown
                        ))
                        .font(FontId::proportional(17.0))
                        .color(Color32::from_rgb(30, 50, 70)),
                    );
                    ui.add_space(4.0);
                }
            }

            let weak: Vec<_> = entries
                .iter()
                .filter(|e| e.rating == SpeechRating::Weak)
                .map(|e| e.label.as_str())
                .collect();
            if !weak.is_empty() {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("{}: {}", t.t("weak_list"), weak.join(", ")))
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(140, 70, 50)),
                );
            }

            ui.add_space(24.0);
            if big_button(ui, t.t("diagnosis_again"), Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::StartDiagnosis);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("speech_map"), Color32::from_rgb(100, 80, 150)).clicked() {
                self.engine.handle(Command::OpenSpeechMap);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("copy_report"), Color32::from_rgb(60, 100, 140)).clicked() {
                ui.ctx().copy_text(report);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("back"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::LeaveProgress);
            }
            ui.add_space(20.0);
        });
    }


    fn ui_warmup(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        ui.vertical_centered(|ui| {
            ui.add_space(20.0);
            ui.label(
                RichText::new(t.t("warmup"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("warmup_hint"))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(20.0);

            let schemas = [
                (t.t("warmup_lips"), WARMUP_SCHEMAS[0].diagram, t.t("warmup_lips_how")),
                (t.t("warmup_tongue"), WARMUP_SCHEMAS[1].diagram, t.t("warmup_tongue_how")),
                (t.t("warmup_breath"), WARMUP_SCHEMAS[2].diagram, t.t("warmup_breath_how")),
            ];
            for (title, diagram, how) in schemas {
                ui.label(
                    RichText::new(title)
                        .font(FontId::proportional(26.0))
                        .strong()
                        .color(Color32::from_rgb(40, 70, 100)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(diagram)
                        .font(FontId::monospace(18.0))
                        .color(Color32::from_rgb(20, 40, 60)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(how)
                        .font(FontId::proportional(18.0))
                        .color(Color32::from_rgb(50, 70, 90)),
                );
                ui.add_space(20.0);
            }

            ui.label(
                RichText::new(t.t("warmup_video"))
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(t.t("warmup_video_note"))
                    .font(FontId::proportional(15.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);

            let links = [
                (t.t("warmup_link1"), WARMUP_LINKS[0].url),
                (t.t("warmup_link2"), WARMUP_LINKS[1].url),
                (t.t("warmup_link3"), WARMUP_LINKS[2].url),
                (t.t("warmup_link4"), WARMUP_LINKS[3].url),
            ];
            for (label, url) in links {
                if big_button(ui, label, Color32::from_rgb(60, 100, 140)).clicked() {
                    ui.ctx().open_url(OpenUrl::new_tab(url));
                }
                ui.add_space(8.0);
            }

            ui.add_space(16.0);
            if self.engine.language() == AppLanguage::Ru {
                ui.label(
                    RichText::new(t.t("warmup_odk_hint"))
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                );
                ui.add_space(12.0);
                if big_button(ui, t.t("warmup_odk_btn"), Color32::from_rgb(40, 130, 90)).clicked() {
                    self.engine.handle(Command::SetPack("odk".into()));
                }
                ui.add_space(8.0);
            }
            if big_button(ui, t.t("start"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("back"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::LeaveWarmup);
            }
            ui.add_space(24.0);
        });
    }


    fn ui_diagnosis_result(&mut self, ui: &mut egui::Ui, level: ExerciseStage) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.label(
                RichText::new(t.t("diag_ready"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!("{}: {}", t.t("level"), stage_label(lang, level)))
                    .font(FontId::proportional(32.0))
                    .strong()
                    .color(Color32::from_rgb(30, 100, 70)),
            );
            ui.add_space(12.0);
            ui.label(
                RichText::new(t.t("diag_saved"))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(36.0);
            if big_button(ui, t.t("start"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(12.0);
            if big_button(ui, t.t("speech_map"), Color32::from_rgb(100, 80, 150)).clicked() {
                self.engine.handle(Command::OpenSpeechMap);
            }
            ui.add_space(12.0);
            if big_button(ui, t.t("choose_other"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenLevelPick);
            }
        });
    }

    fn ui_settings(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);
            ui.label(
                RichText::new(t.t("settings"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);

            ui.label(
                RichText::new(t.t("language"))
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("language_hint"))
                    .font(FontId::proportional(15.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                for option in AppLanguage::ALL {
                    let selected = option == lang;
                    let fill = if selected {
                        Color32::from_rgb(40, 130, 90)
                    } else {
                        Color32::from_rgb(90, 100, 120)
                    };
                    if big_button(ui, option.label(), fill).clicked() {
                        self.engine.handle(Command::SetLanguage(option));
                        self.sync_editor_prompt_default();
                    }
                    ui.add_space(8.0);
                }
            });

            ui.add_space(24.0);
            ui.label(
                RichText::new(t.t("simple_mode"))
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("simple_mode_hint"))
                    .font(FontId::proportional(15.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);
            {
                let on = self.engine.simple_mode();
                let label = if on {
                    format!("{} · {}", t.t("simple_mode"), t.t("simple_on"))
                } else {
                    format!("{} · {}", t.t("simple_mode"), t.t("simple_off"))
                };
                let fill = if on {
                    Color32::from_rgb(40, 130, 90)
                } else {
                    Color32::from_rgb(90, 100, 120)
                };
                if big_button(ui, &label, fill).clicked() {
                    self.engine.handle(Command::SetSimpleMode(!on));
                }
            }

            ui.add_space(24.0);
            ui.label(
                RichText::new(t.t("pack_and_level"))
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("{}: {}", t.t("pack"), self.engine.pack().title))
                    .font(FontId::proportional(18.0)),
            );
            let level_text = match self.engine.level() {
                Some(l) => format!("{}: {}", t.t("level"), stage_label(lang, l)),
                None => format!("{}: {}", t.t("level"), t.t("level_none")),
            };
            ui.label(
                RichText::new(level_text)
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);
            if big_button(ui, t.t("change_pack"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenPackPick);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("choose_level"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenLevelPick);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("speech_map"), Color32::from_rgb(100, 80, 150)).clicked() {
                self.engine.handle(Command::OpenSpeechMap);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("warmup"), Color32::from_rgb(70, 120, 100)).clicked() {
                self.engine.handle(Command::OpenWarmup);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("progress"), Color32::from_rgb(100, 80, 150)).clicked() {
                self.engine.handle(Command::OpenProgress);
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("pack_editor"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenPackEditor);
            }
            ui.add_space(8.0);
            ui.label(
                RichText::new(t.t("weak_hint"))
                    .font(FontId::proportional(15.0))
                    .color(Color32::DARK_GRAY),
            );

            ui.add_space(24.0);
            ui.label(
                RichText::new(t.t("voice"))
                    .font(FontId::proportional(22.0))
                    .strong()
                    .color(Color32::from_rgb(40, 70, 100)),
            );
            ui.add_space(8.0);

            match self.engine.asr_status() {
                AsrStatus::Ready => ui.label(
                    RichText::new(t.t("voice_ready"))
                        .font(FontId::proportional(20.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                ),
                AsrStatus::ModelMissing => ui.label(
                    RichText::new(t.t("voice_missing"))
                        .font(FontId::proportional(20.0))
                        .color(Color32::from_rgb(150, 90, 30)),
                ),
                AsrStatus::Disabled => ui.label(
                    RichText::new(t.t("voice_disabled"))
                        .font(FontId::proportional(20.0))
                        .color(Color32::DARK_GRAY),
                ),
                AsrStatus::Error(e) => ui.label(
                    RichText::new(format!("{}: {e}", t.t("voice")))
                        .font(FontId::proportional(20.0))
                        .color(Color32::from_rgb(160, 60, 40)),
                ),
            };

            if let Some(dir) = self.engine.user_data_dir_display() {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!("{}: {dir}", t.t("data")))
                        .font(FontId::proportional(16.0))
                        .color(Color32::DARK_GRAY),
                );
            }

            ui.add_space(20.0);
            match self.engine.model_download() {
                ModelDownloadState::Idle | ModelDownloadState::Succeeded => {
                    let can_download = !matches!(
                        self.engine.asr_status(),
                        AsrStatus::Disabled | AsrStatus::Ready
                    );
                    if can_download {
                        ui.label(
                            RichText::new(format!(
                                "{} ({})",
                                t.t("download_model_hint"),
                                lang.vosk_model_size_hint()
                            ))
                            .font(FontId::proportional(18.0))
                            .color(Color32::DARK_GRAY),
                        );
                        ui.add_space(12.0);
                        if big_button(ui, t.t("download_model"), Color32::from_rgb(40, 110, 180))
                            .clicked()
                        {
                            self.engine.handle(Command::StartModelDownload);
                        }
                    } else if matches!(self.engine.asr_status(), AsrStatus::Ready) {
                        ui.label(
                            RichText::new(t.t("model_ready"))
                                .font(FontId::proportional(18.0))
                                .color(Color32::DARK_GRAY),
                        );
                    }
                }
                ModelDownloadState::Working { label, percent } => {
                    ui.label(
                        RichText::new(label)
                            .font(FontId::proportional(20.0))
                            .strong(),
                    );
                    if let Some(p) = percent {
                        ui.add_space(8.0);
                        ui.add(
                            egui::ProgressBar::new(f32::from(*p) / 100.0)
                                .text(format!("{p}%"))
                                .desired_width(320.0),
                        );
                    } else {
                        ui.add_space(8.0);
                        ui.spinner();
                    }
                }
                ModelDownloadState::Failed(err) => {
                    ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                    ui.add_space(12.0);
                    if big_button(ui, t.t("retry"), Color32::from_rgb(40, 110, 180)).clicked() {
                        self.engine.handle(Command::StartModelDownload);
                    }
                }
            }

            if let Some(note) = self.engine.model_download_note() {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(note)
                        .font(FontId::proportional(18.0))
                        .color(Color32::from_rgb(30, 120, 60)),
                );
            }

            if let Some(err) = self.engine.save_error() {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("{}: {err}", t.t("progress_err")),
                );
            }
        });
    }

    fn ui_exercise(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.engine.session() else {
            return;
        };
        let total = session.exercises.len();
        let idx = session.index;
        let stage_name = session
            .exercises
            .get(idx)
            .map(|e| stage_label(self.engine.language(), e.stage()))
            .unwrap_or("");
        let t = self.engine.ui_text();
        let mode = if self.engine.session_is_diagnosis() {
            t.t("mode_diagnosis")
        } else {
            t.t("mode_practice")
        };
        let progress_label = if stage_name.is_empty() {
            format!("{mode} · {} {} {}", idx + 1, t.t("of"), total)
        } else {
            format!("{mode} · {stage_name} · {} {} {}", idx + 1, t.t("of"), total)
        };

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(progress_label)
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            if self.engine.current_exercise_is_practice_repeat() {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(self.engine.ui_text().t("practice_repeat"))
                        .font(FontId::proportional(16.0))
                        .color(Color32::from_rgb(180, 100, 40)),
                );
            }
            ui.add_space(16.0);
        });

        let exercise = match self.engine.current_exercise().cloned() {
            Some(e) => e,
            None => return,
        };

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(exercise.prompt())
                    .font(FontId::proportional(28.0))
                    .strong(),
            );
            if self.engine.simple_mode() {
                if let Some(cue) = exercise.speech_cue() {
                    ui.add_space(10.0);
                    ui.label(
                        RichText::new(format!(
                            "{} «{}»",
                            self.engine.ui_text().t("cue_prefix"),
                            cue
                        ))
                        .font(FontId::proportional(24.0))
                        .color(Color32::from_rgb(40, 100, 70)),
                    );
                }
            }
            ui.add_space(24.0);
        });

        match exercise {
            Exercise::ChooseWord { .. } => {
                let options = self
                    .engine
                    .session()
                    .map(|s| s.choice_options.clone())
                    .unwrap_or_default();
                ui.vertical_centered(|ui| {
                    for opt in &options {
                        if big_button(ui, opt, Color32::from_rgb(50, 90, 130)).clicked() {
                            self.engine.handle(Command::Submit(UserAnswer::Choice(opt.clone())));
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
                let is_twister = matches!(
                    self.engine.current_exercise().map(|e| e.stage()),
                    Some(ExerciseStage::Twister)
                );
                self.ui_read_aloud(ui, &text, is_twister);
            }
        }
    }

    fn ui_build_phrase(&mut self, ui: &mut egui::Ui) {
        let Some(session) = self.engine.session() else {
            return;
        };
        let picked_label = if session.picked.is_empty() {
            self.engine.ui_text().t("tap_words").to_string()
        } else {
            session.picked.join(" ")
        };
        let pool = session.pool.clone();
        let can_check = !session.picked.is_empty() && session.pool.is_empty();

        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(picked_label)
                    .font(FontId::proportional(26.0))
                    .color(Color32::from_rgb(20, 60, 40)),
            );
            ui.add_space(20.0);
        });

        ui.horizontal_wrapped(|ui| {
            ui.add_space(ui.available_width() * 0.1);
            let mut clicked: Option<usize> = None;
            for (i, w) in pool.iter().enumerate() {
                if big_button(ui, w, Color32::from_rgb(70, 100, 140)).clicked() {
                    clicked = Some(i);
                }
            }
            if let Some(i) = clicked {
                self.engine.handle(Command::PickPoolWord(i));
            }
        });

        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            ui.horizontal(|ui| {
                if big_button(ui, self.engine.ui_text().t("reset"), Color32::from_rgb(120, 90, 70)).clicked() {
                    self.engine.handle(Command::ResetBuildPhrase);
                }
                ui.add_space(12.0);
                if can_check && big_button(ui, self.engine.ui_text().t("check"), Color32::from_rgb(40, 130, 90)).clicked()
                {
                    let parts = self
                        .engine
                        .session()
                        .map(|s| s.picked.clone())
                        .unwrap_or_default();
                    self.engine.handle(Command::Submit(UserAnswer::Phrase(parts)));
                }
            });
        });
    }

    fn ui_read_aloud(&mut self, ui: &mut egui::Ui, text: &str, is_twister: bool) {
        ui.vertical_centered(|ui| {
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(40.0))
                    .strong()
                    .color(Color32::from_rgb(15, 35, 55)),
            );
            if is_twister {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(self.engine.ui_text().t("twister_tip"))
                        .font(FontId::proportional(17.0))
                        .color(Color32::from_rgb(80, 90, 100)),
                );
            }
            ui.add_space(28.0);

            let asr_ready = matches!(self.engine.asr_status(), AsrStatus::Ready);
            let listening = self
                .engine
                .session()
                .map(|s| s.listening)
                .unwrap_or(false);

            if asr_ready {
                if listening {
                    if self.engine.please_wait() {
                        ui.label(
                            RichText::new(self.engine.ui_text().t("please_wait_asr"))
                                .font(FontId::proportional(22.0))
                                .strong()
                                .color(Color32::from_rgb(150, 90, 30)),
                        );
                    } else {
                        ui.label(
                            RichText::new(self.engine.ui_text().t("speaking"))
                                .font(FontId::proportional(22.0))
                                .color(Color32::from_rgb(140, 60, 100)),
                        );
                    }
                    let live = self
                        .engine
                        .session()
                        .map(|s| s.live_text.as_str())
                        .unwrap_or("");
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(if live.is_empty() {
                            "…"
                        } else {
                            live
                        })
                        .font(FontId::proportional(32.0))
                        .strong()
                        .color(Color32::from_rgb(20, 40, 60)),
                    );
                    ui.add_space(16.0);
                    if big_button(ui, self.engine.ui_text().t("done"), Color32::from_rgb(40, 130, 90))
                        .clicked()
                    {
                        self.engine.handle(Command::StopExerciseListen);
                    }
                } else if big_button(
                    ui,
                    self.engine.ui_text().t("say"),
                    Color32::from_rgb(140, 60, 100),
                )
                .clicked()
                {
                    self.engine.handle(Command::ListenExercise);
                }
                ui.add_space(12.0);
            }

            if let Some(err) = self.engine.session().and_then(|s| s.listen_error.clone()) {
                ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                ui.add_space(8.0);
            }

            if !listening {
                let heard = self
                    .engine
                    .session()
                    .map(|s| s.live_text.as_str())
                    .unwrap_or("")
                    .to_string();
                let hint = self.engine.session().and_then(|s| s.asr_hint_ok);
                if !heard.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "{}: {heard}",
                            self.engine.ui_text().t("heard")
                        ))
                        .font(FontId::proportional(22.0))
                        .color(Color32::from_rgb(20, 40, 60)),
                    );
                    if let Some(ok) = hint {
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new(if ok {
                                self.engine.ui_text().t("hint_ok")
                            } else {
                                self.engine.ui_text().t("hint_bad")
                            })
                            .font(FontId::proportional(18.0))
                            .color(Color32::DARK_GRAY),
                        );
                    }
                    ui.add_space(12.0);
                }

                if self.engine.has_last_clip() {
                    let playing = self.engine.is_playing_clip();
                    let t = self.engine.ui_text();
                    let label = if playing {
                        t.t("stop_listen")
                    } else {
                        t.t("listen")
                    };
                    let color = if playing {
                        Color32::from_rgb(120, 90, 70)
                    } else {
                        Color32::from_rgb(60, 100, 140)
                    };
                    if big_button(ui, label, color).clicked() {
                        if playing {
                            self.engine.handle(Command::StopPlayback);
                        } else {
                            self.engine.handle(Command::PlayLastClip);
                        }
                    }
                    ui.add_space(12.0);
                }
            }

            ui.label(
                RichText::new(self.engine.ui_text().t("or_self"))
                    .font(FontId::proportional(18.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(8.0);
            if listening {
                ui.label(
                    RichText::new(self.engine.ui_text().t("done"))
                        .font(FontId::proportional(18.0))
                        .color(Color32::DARK_GRAY),
                );
            } else {
                ui.horizontal(|ui| {
                    if big_button(
                        ui,
                        self.engine.ui_text().t("ok_self"),
                        Color32::from_rgb(40, 130, 90),
                    )
                    .clicked()
                    {
                        self.engine.handle(Command::Submit(UserAnswer::ReadDone {
                            matched: true,
                            heard: None,
                        }));
                    }
                    ui.add_space(12.0);
                    if big_button(
                        ui,
                        self.engine.ui_text().t("fail_self"),
                        Color32::from_rgb(150, 70, 60),
                    )
                    .clicked()
                    {
                        self.engine.handle(Command::Submit(UserAnswer::ReadDone {
                            matched: false,
                            heard: None,
                        }));
                    }
                });
            }
        });
    }

    fn ui_feedback(
        &mut self,
        ui: &mut egui::Ui,
        result: CheckResult,
        heard: Option<String>,
        expected: Option<String>,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            let t = self.engine.ui_text();
            let (text, color) = match result {
                CheckResult::Correct => (t.t("correct"), Color32::from_rgb(30, 130, 70)),
                CheckResult::Incorrect => (t.t("incorrect"), Color32::from_rgb(160, 50, 40)),
            };
            ui.label(
                RichText::new(text)
                    .font(FontId::proportional(48.0))
                    .strong()
                    .color(color),
            );

            if result == CheckResult::Incorrect {
                if let Some(h) = &heard {
                    ui.add_space(20.0);
                    ui.label(
                        RichText::new(format!("{}:", self.engine.ui_text().t("heard")))
                            .font(FontId::proportional(22.0))
                            .color(Color32::DARK_GRAY),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(h)
                            .font(FontId::proportional(36.0))
                            .strong()
                            .color(Color32::from_rgb(20, 40, 60)),
                    );
                } else {
                    ui.add_space(12.0);
                    ui.label(
                        RichText::new(self.engine.ui_text().t("asr_none"))
                            .font(FontId::proportional(18.0))
                            .color(Color32::DARK_GRAY),
                    );
                }
                if let Some(exp) = &expected {
                    ui.add_space(16.0);
                    ui.label(
                        RichText::new(format!("{}: {exp}", self.engine.ui_text().t("expected")))
                            .font(FontId::proportional(22.0))
                            .color(Color32::from_rgb(80, 90, 100)),
                    );
                }
            } else if let Some(h) = &heard {
                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!(
                        "{}: {h}",
                        self.engine.ui_text().t("heard")
                    ))
                    .font(FontId::proportional(22.0))
                    .color(Color32::DARK_GRAY),
                );
            }

            if self.engine.has_last_clip() {
                ui.add_space(20.0);
                let playing = self.engine.is_playing_clip();
                let t = self.engine.ui_text();
                let label = if playing {
                    t.t("stop_listen")
                } else {
                    t.t("listen")
                };
                let color = if playing {
                    Color32::from_rgb(120, 90, 70)
                } else {
                    Color32::from_rgb(60, 100, 140)
                };
                if big_button(ui, label, color).clicked() {
                    if playing {
                        self.engine.handle(Command::StopPlayback);
                    } else {
                        self.engine.handle(Command::PlayLastClip);
                    }
                }
            }

            ui.add_space(40.0);
            if result == CheckResult::Incorrect && self.engine.session_is_practice() {
                if let Some(left) = self.engine.feedback_requeues_left() {
                    let hint = if left == 0 {
                        self.engine.ui_text().t("requeue_done").into()
                    } else if self.engine.language() == AppLanguage::En {
                        format!(
                            "We'll bring it back later — more errors come sooner. Up to {left} more times this lesson."
                        )
                    } else {
                        format!(
                            "Вернём позже — чем чаще ошибка, тем раньше. Ещё до {left} раз в этом занятии."
                        )
                    };
                    ui.label(
                        RichText::new(hint)
                            .font(FontId::proportional(16.0))
                            .color(Color32::DARK_GRAY),
                    );
                }
                ui.add_space(16.0);
                if big_button(ui, self.engine.ui_text().t("next"), Color32::from_rgb(40, 110, 180))
                    .clicked()
                {
                    self.engine.handle(Command::AdvanceAfterFeedback);
                }
                ui.add_space(12.0);
                if big_button(
                    ui,
                    self.engine.ui_text().t("skip_repeat"),
                    Color32::from_rgb(90, 100, 120),
                )
                .clicked()
                {
                    self.engine.handle(Command::SkipRepeatAndAdvance);
                }
            } else if big_button(ui, self.engine.ui_text().t("next"), Color32::from_rgb(40, 110, 180))
                .clicked()
            {
                self.engine.handle(Command::AdvanceAfterFeedback);
            }
        });
    }

    fn ui_dictaphone(&mut self, ui: &mut egui::Ui) {
        let t = self.engine.ui_text();
        // Нижняя полоса всегда на виду (Windows: не прячется за панель задач).
        egui::TopBottomPanel::bottom("dictaphone_footer")
            .resizable(false)
            .show_separator_line(true)
            .show_inside(ui, |ui| {
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    if self.engine.dictaphone().listening {
                        if big_button(ui, t.t("stop"), Color32::from_rgb(150, 70, 60)).clicked() {
                            self.engine.handle(Command::StopDictaphone);
                        }
                    } else {
                        footer_buttons(ui, |ui| {
                            let has_text = !self.engine.dictaphone().transcript.is_empty();
                            if has_text
                                && big_button(ui, t.t("dict_save_txt"), Color32::from_rgb(40, 110, 90))
                                    .clicked()
                            {
                                self.engine.handle(Command::SaveDictaphone);
                            }
                            if ui.available_width() >= 640.0 {
                                ui.add_space(12.0);
                            } else {
                                ui.add_space(8.0);
                            }
                            let can_clear = has_text
                                || !self.engine.dictaphone().live_text.is_empty()
                                || self.engine.dictaphone().error.is_some()
                                || self.engine.dictaphone().save_note.is_some();
                            if can_clear
                                && big_button(ui, t.t("clear"), Color32::from_rgb(120, 90, 70))
                                    .clicked()
                            {
                                self.engine.handle(Command::ClearDictaphone);
                            }
                            if ui.available_width() >= 640.0 {
                                ui.add_space(12.0);
                            } else {
                                ui.add_space(8.0);
                            }
                            if big_button(ui, t.t("back"), Color32::from_rgb(90, 100, 110)).clicked() {
                                self.engine.handle(Command::LeaveDictaphone);
                            }
                        });
                    }
                });
                ui.add_space(12.0);
            });

        egui::ScrollArea::vertical()
            .id_salt("dictaphone_body")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let compact = ui.available_width() < 520.0 || ui.available_height() < 520.0;
                let title_size = if compact { 28.0 } else { 36.0 };
                let body_size = if compact { 16.0 } else { 18.0 };
                let text_size = if compact { 20.0 } else { 24.0 };
                ui.vertical_centered(|ui| {
                    ui.add_space(if compact { 8.0 } else { 16.0 });
                    ui.label(
                        RichText::new(t.t("dict_title"))
                            .font(FontId::proportional(title_size))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(t.t("dict_hint"))
                        .font(FontId::proportional(body_size))
                        .color(Color32::DARK_GRAY),
                    );
                    ui.add_space(if compact { 12.0 } else { 20.0 });

                    if self.engine.dictaphone().listening {
                        ui.label(
                            RichText::new(t.t("dict_recording"))
                                .font(FontId::proportional(body_size + 2.0))
                                .color(Color32::from_rgb(140, 60, 100)),
                        );
                        if self.engine.please_wait() {
                            ui.add_space(12.0);
                            ui.label(
                                RichText::new(t.t("dict_wait"))
                                .font(FontId::proportional(body_size + 2.0))
                                .strong()
                                .color(Color32::from_rgb(150, 90, 30)),
                            );
                        }
                    } else if big_button(ui, t.t("dict_record"), Color32::from_rgb(140, 60, 100)).clicked() {
                        self.engine.handle(Command::ListenDictaphone);
                    }

                    if !self.engine.dictaphone().listening && self.engine.has_last_clip() {
                        ui.add_space(12.0);
                        let playing = self.engine.is_playing_clip();
                        let label = if playing {
                            t.t("stop_listen")
                        } else {
                            t.t("listen")
                        };
                        let color = if playing {
                            Color32::from_rgb(120, 90, 70)
                        } else {
                            Color32::from_rgb(60, 100, 140)
                        };
                        if big_button(ui, label, color).clicked() {
                            if playing {
                                self.engine.handle(Command::StopPlayback);
                            } else {
                                self.engine.handle(Command::PlayLastClip);
                            }
                        }
                    }

                    if let Some(err) = &self.engine.dictaphone().error {
                        ui.add_space(12.0);
                        ui.colored_label(Color32::from_rgb(160, 60, 40), err);
                    }
                    if let Some(note) = &self.engine.dictaphone().save_note {
                        ui.add_space(8.0);
                        ui.label(
                            RichText::new(note)
                                .font(FontId::proportional(16.0))
                                .color(Color32::from_rgb(50, 90, 70)),
                        );
                    }

                    const UI_BYTES: usize = 12_000;
                    let tr = self.engine.dictaphone().transcript.as_str();
                    let live = self.engine.dictaphone().live_text.as_str();
                    let truncated = tr.len() > UI_BYTES;
                    let tr_tail = str_byte_tail(tr, UI_BYTES);

                    if !tr.is_empty() || !live.is_empty() || self.engine.dictaphone().listening {
                        ui.add_space(24.0);
                        ui.label(
                            RichText::new(if truncated {
                                t.t("dict_text_tail")
                            } else {
                                t.t("dict_text")
                            })
                            .font(FontId::proportional(20.0))
                            .color(Color32::DARK_GRAY),
                        );
                        ui.add_space(8.0);
                        let text_h = if compact {
                            (ui.available_height() * 0.35).clamp(72.0, 160.0)
                        } else {
                            (ui.available_height() * 0.45).clamp(96.0, 280.0)
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("dictaphone_text")
                            .max_height(text_h)
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                if truncated {
                                    ui.label(
                                        RichText::new("…")
                                            .font(FontId::proportional(text_size))
                                            .color(Color32::DARK_GRAY),
                                    );
                                }
                                if !tr_tail.is_empty() {
                                    ui.label(
                                        RichText::new(tr_tail)
                                            .font(FontId::proportional(text_size))
                                            .strong()
                                            .color(Color32::from_rgb(20, 40, 60)),
                                    );
                                }
                                if !live.is_empty() {
                                    ui.label(
                                        RichText::new(live)
                                            .font(FontId::proportional(text_size))
                                            .color(Color32::from_rgb(80, 50, 100)),
                                    );
                                } else if tr_tail.is_empty() {
                                    ui.label(
                                        RichText::new("…")
                                            .font(FontId::proportional(text_size))
                                            .strong()
                                            .color(Color32::from_rgb(20, 40, 60)),
                                    );
                                }
                            });
                    }
                    ui.add_space(16.0);
                });
            });
    }

    fn ui_result(&mut self, ui: &mut egui::Ui, correct: u32, total: u32, unique: u32) {
        let t = self.engine.ui_text();
        ui.vertical_centered(|ui| {
            ui.add_space(50.0);
            ui.label(
                RichText::new(t.t("result_done"))
                    .font(FontId::proportional(36.0))
                    .strong(),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new(format!(
                    "{}: {correct} {} {total}",
                    t.t("result_score"),
                    t.t("of")
                ))
                .font(FontId::proportional(28.0)),
            );
            if total > unique {
                ui.add_space(8.0);
                ui.label(
                    RichText::new(format!(
                        "{}: {unique} · {}: {total}",
                        t.t("result_plan"),
                        t.t("result_with_repeats")
                    ))
                    .font(FontId::proportional(16.0))
                    .color(Color32::DARK_GRAY),
                );
            }
            if let Some(err) = self.engine.save_error() {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("{}: {err}", t.t("save_failed")),
                );
            }
            ui.add_space(40.0);
            if big_button(ui, t.t("again"), Color32::from_rgb(40, 130, 90)).clicked() {
                self.engine.handle(Command::AgainSession);
            }
            ui.add_space(12.0);
            if big_button(ui, t.t("speech_map"), Color32::from_rgb(100, 80, 150)).clicked() {
                self.engine.handle(Command::OpenSpeechMap);
            }
        });
    }
}
