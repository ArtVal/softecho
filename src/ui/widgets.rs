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
