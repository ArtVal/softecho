//! Точка входа: UI над движком.
//! `engine` не знает про egui — граница для будущего клиент-сервера.

mod engine;
mod ui;

use ui::UiApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([640.0, 480.0])
            .with_title("Речевой тренажёр"),
        ..Default::default()
    };

    eframe::run_native(
        "Речевой тренажёр",
        options,
        Box::new(|cc| Ok(Box::new(UiApp::new(cc)))),
    )
}
