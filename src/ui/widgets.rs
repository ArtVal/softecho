//! Кнопки и мелкие UI-хелперы.

use eframe::egui::{self, Color32, FontId, RichText, Sense, Vec2};

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
    let width = ui.available_width().clamp(200.0, 280.0);
    let height = if ui.available_width() < 520.0 { 52.0 } else { 56.0 };
    let font_size = if ui.available_width() < 520.0 { 20.0 } else { 24.0 };
    let text = RichText::new(label)
        .font(FontId::proportional(font_size))
        .color(Color32::WHITE);
    let desired = Vec2::new(width, height);
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
            FontId::proportional(font_size),
            Color32::WHITE,
        );
    }
    response
}

/// Кнопки в нижней панели: на узком экране — столбиком.
pub fn footer_buttons(ui: &mut egui::Ui, add_buttons: impl FnOnce(&mut egui::Ui)) {
    if ui.available_width() < 640.0 {
        ui.vertical_centered(|ui| add_buttons(ui));
    } else {
        ui.horizontal(|ui| {
            ui.with_layout(
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| add_buttons(ui),
            );
        });
    }
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
