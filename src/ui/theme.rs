//! Тема egui.

use eframe::egui::{self, Color32, FontId};

pub fn apply_theme(ctx: &egui::Context) {
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
