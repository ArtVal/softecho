#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Точка входа: UI над движком.
//! `engine` не знает про egui — граница для будущего клиент-сервера.
//! Release под Windows — GUI без чёрной консоли. Debug (`cargo run`) консоль оставляет.

mod engine;
mod ui;
mod version;

pub use version::APP_VERSION;

use ui::UiApp;

fn main() -> eframe::Result<()> {
    #[cfg(feature = "asr")]
    engine::vosk_runtime::prepare();

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../assets/softecho.png"))
        .expect("иконка assets/softecho.png");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([480.0, 420.0])
            .with_title(format!("SoftEcho {}", crate::APP_VERSION))
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "SoftEcho",
        options,
        Box::new(|cc| Ok(Box::new(UiApp::new(cc)))),
    )
}
