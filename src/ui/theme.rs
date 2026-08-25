//! Тема egui.

use eframe::egui::{self, Color32, FontId};

pub fn apply_theme(ctx: &egui::Context) {
    apply_theme_scale(ctx, 1.0);
}

pub fn apply_theme_scale(ctx: &egui::Context, scale: f32) {
    let scale = scale.clamp(1.0, 1.6);
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        FontId::proportional(20.0 * scale),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::proportional(22.0 * scale),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::proportional(32.0 * scale),
    );
    style.spacing.item_spacing.y = 6.0 * scale;
    style.spacing.button_padding = egui::vec2(10.0 * scale, 6.0 * scale);
    ctx.set_style(style);

    let mut visuals = egui::Visuals::light();
    visuals.panel_fill = Color32::from_rgb(245, 248, 250);
    visuals.window_fill = Color32::from_rgb(245, 248, 250);
    ctx.set_visuals(visuals);
}
