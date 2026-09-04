//! Экран разминки (схемы + внешние ссылки).

use crate::engine::{AppLanguage, Command};
use crate::engine::warmup::{WARMUP_LINKS, WARMUP_SCHEMAS};
use crate::ui::widgets::big_button;
use eframe::egui::{self, Color32, FontId, OpenUrl, RichText};

use super::UiApp;

impl UiApp {
    pub(super) fn ui_warmup(&mut self, ui: &mut egui::Ui) {
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
            ui.label(
                RichText::new(t.t("warmup_odk_hint"))
                    .font(FontId::proportional(16.0))
                    .color(Color32::DARK_GRAY),
            );
            ui.add_space(12.0);
            if big_button(ui, t.t("warmup_odk_btn"), Color32::from_rgb(40, 130, 90)).clicked() {
                let odk_id = if self.engine.language() == AppLanguage::En {
                    "odk_en"
                } else {
                    "odk"
                };
                self.engine.handle(Command::SetPack(odk_id.into()));
            }
            ui.add_space(8.0);
            if big_button(ui, t.t("start"), Color32::from_rgb(40, 110, 180)).clicked() {
                self.engine.handle(Command::StartSession);
            }
            ui.add_space(24.0);
        });
    }
}
