//! Экран редактора набора.

use crate::engine::{Command, ExerciseStage};
use crate::engine::i18n::stage_label;
use crate::ui::widgets::big_button;
use eframe::egui::{self, Color32, FontId, RichText};

use super::UiApp;

impl UiApp {
    pub(super) fn ui_pack_editor(&mut self, ui: &mut egui::Ui) {
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
                return;
            }

            let (pack_id, title, active_n, disabled_n, err, note) = {
                let Some(ed) = self.engine.pack_editor() else {
                    return;
                };
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
                RichText::new(title)
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
            if self.engine.pack_editor().is_some_and(|ed| ed.dirty) {
                ui.add_space(12.0);
                if big_button(ui, t.t("editor_discard"), Color32::from_rgb(120, 90, 70)).clicked()
                {
                    self.engine.handle(Command::DiscardPackEditor);
                }
            }
            ui.add_space(24.0);
        });
    }
}
