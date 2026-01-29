mod app;
mod editor;
mod syntax;
mod ui;

use app::LuxApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("Lux Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "Lux Editor",
        options,
        Box::new(|cc| Ok(Box::new(LuxApp::new(cc)))),
    )
}
