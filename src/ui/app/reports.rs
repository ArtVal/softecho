//! Экраны прогресса, карты произнесения и итога диагностики.

use crate::engine::{Command, ExerciseStage, SpeechRating};
use crate::engine::exercise::speech_map_stage_summaries;
use crate::engine::i18n::{rating_label, stage_label};
use crate::ui::widgets::big_button;
use eframe::egui::{self, Color32, FontId, RichText};

use super::UiApp;

impl UiApp {
    pub(super) fn ui_diagnosis_result(&mut self, ui: &mut egui::Ui, level: ExerciseStage) {
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
            if big_button(ui, t.t("export_report"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::ExportProgressReport);
            }
            if let Some(note) = self.engine.report_export_note() {
                ui.add_space(10.0);
                let color = if note.starts_with(t.t("export_failed")) {
                    Color32::from_rgb(160, 60, 40)
                } else {
                    Color32::from_rgb(30, 120, 60)
                };
                ui.label(
                    RichText::new(note)
                        .font(FontId::proportional(15.0))
                        .color(color),
                );
            }
            ui.add_space(12.0);
            if big_button(ui, t.t("choose_other"), Color32::from_rgb(90, 100, 120)).clicked() {
                self.engine.handle(Command::OpenLevelPick);
            }
        });
    }


    pub(super) fn ui_speech_map(&mut self, ui: &mut egui::Ui) {
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


    pub(super) fn ui_progress_report(&mut self, ui: &mut egui::Ui) {
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
                    let pct = (100u32)
                        .checked_mul(s.correct)
                        .and_then(|n| n.checked_div(s.total))
                        .unwrap_or(0);
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
                ui.ctx().copy_text(report.clone());
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("export_report"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::ExportProgressReport);
            }
            if let Some(note) = self.engine.report_export_note() {
                ui.add_space(10.0);
                let failed = note.starts_with(t.t("export_failed"));
                let color = if failed {
                    Color32::from_rgb(160, 60, 40)
                } else {
                    Color32::from_rgb(30, 120, 60)
                };
                ui.label(
                    RichText::new(note)
                        .font(FontId::proportional(15.0))
                        .color(color),
                );
            }
            ui.add_space(20.0);
        });
    }

}
