//! Кнопки и мелкие UI-хелперы.

use eframe::egui::{self, Color32, FontId, Sense, Vec2};

pub fn str_byte_tail(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut idx = s.len() - max_bytes;
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    &s[idx..]
}

pub fn big_button(ui: &mut egui::Ui, label: &str, fill: Color32) -> egui::Response {
    big_button_enabled(ui, label, fill, true)
}

/// Крупная кнопка; при `enabled == false` серая и без клика.
pub fn big_button_enabled(
    ui: &mut egui::Ui,
    label: &str,
    fill: Color32,
    enabled: bool,
) -> egui::Response {
    let scale = ui
        .ctx()
        .data(|d| d.get_temp::<f32>(egui::Id::new("softecho_ui_scale")))
        .unwrap_or(1.0);
    big_button_scaled(ui, label, fill, scale, enabled)
}

pub fn big_button_scaled(
    ui: &mut egui::Ui,
    label: &str,
    fill: Color32,
    scale: f32,
    enabled: bool,
) -> egui::Response {
    let scale = scale.clamp(1.0, 1.6);
    let width = (ui.available_width().clamp(200.0, 280.0) * scale.min(1.25)).min(ui.available_width());
    let narrow = ui.available_width() < 520.0;
    let height = if narrow { 52.0 } else { 56.0 } * scale;
    let font_size = if narrow { 20.0 } else { 24.0 } * scale;
    let fill = if enabled {
        fill
    } else {
        Color32::from_rgb(150, 155, 160)
    };
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let desired = Vec2::new(width, height);
    let (rect, response) = ui.allocate_exact_size(desired, sense);
    if ui.is_rect_visible(rect) {
        let bg = if enabled && response.hovered() {
            lighten(fill, 20)
        } else {
            fill
        };
        ui.painter().rect_filled(rect, 8.0, bg);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(font_size),
            Color32::WHITE,
        );
    }
    response
}

/// Компактная кнопка «назад в меню» для верхней панели.
pub fn back_to_menu_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(
            egui::RichText::new(label)
                .font(FontId::proportional(18.0))
                .color(Color32::from_rgb(40, 55, 75)),
        )
        .min_size(Vec2::new(120.0, 36.0)),
    )
}

/// Кнопки внизу экрана: всегда по центру; на узком — столбиком, на широком — в ряд.
pub fn footer_buttons(ui: &mut egui::Ui, add_buttons: impl FnOnce(&mut egui::Ui)) {
    let narrow = ui.available_width() < 640.0;
    ui.vertical_centered(|ui| {
        if narrow {
            add_buttons(ui);
        } else {
            // horizontal на всю ширину без center «уезжает» влево — оборачиваем.
            ui.horizontal(|ui| add_buttons(ui));
        }
    });
}

/// Прокрутка экрана целиком — на низком окне (Windows + панель задач) кнопки не уезжают «за край».
pub fn screen_scroll(ui: &mut egui::Ui, id: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            add_contents(ui);
            // Запас снизу: панель задач / жесткая навигация.
            ui.add_space(24.0);
        });
}

fn lighten(c: Color32, amount: u8) -> Color32 {
    Color32::from_rgb(
        c.r().saturating_add(amount),
        c.g().saturating_add(amount),
        c.b().saturating_add(amount),
    )
}
