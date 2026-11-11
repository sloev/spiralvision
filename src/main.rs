#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use spiralvision::app::{apply_theme, SpiralVisionApp};

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 620.0])
            .with_title("SPIRALVISION-10"),
        ..Default::default()
    };
    eframe::run_native(
        "SPIRALVISION-10",
        options,
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            let app = match std::env::args().nth(1) {
                Some(path) => SpiralVisionApp::with_video(&path),
                None => SpiralVisionApp::default(),
            };
            Ok(Box::new(app))
        }),
    )
}
