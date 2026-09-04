//! Экран настроек и блок скачивания модели.

use crate::engine::{AppLanguage, AsrStatus, Command, ModelDownloadState};
use crate::engine::i18n::stage_label;
use crate::ui::widgets::{big_button};
use eframe::egui::{self, Color32, FontId, RichText};

use super::UiApp;

impl UiApp {
    pub(super) fn ui_settings(&mut self, ui: &mut egui::Ui) {
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
            ui.vertical_centered(|ui| {
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
            if !self.engine.simple_mode() {
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
            } else {
                ui.add_space(8.0);
                if big_button(ui, t.t("warmup"), Color32::from_rgb(70, 120, 100)).clicked() {
                    self.engine.handle(Command::OpenWarmup);
                }
            }

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

            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{} {}", t.t("version"), crate::APP_VERSION))
                    .font(FontId::proportional(16.0))
                    .color(Color32::DARK_GRAY),
            );

            ui.add_space(20.0);
            self.ui_model_download(ui, true);

            if let Some(err) = self.engine.save_error() {
                ui.add_space(12.0);
                ui.colored_label(
                    Color32::from_rgb(160, 60, 40),
                    format!("{}: {err}", t.t("progress_err")),
                );
            }
        });
    }

    /// Скачивание модели Vosk (главная и настройки).
    pub(super) fn ui_model_download(&mut self, ui: &mut egui::Ui, show_ready: bool) {
        let t = self.engine.ui_text();
        let lang = self.engine.language();
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
                } else if show_ready && matches!(self.engine.asr_status(), AsrStatus::Ready) {
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
    }

}
