mod app;
mod decoder;
mod encoder;
mod io;
mod protocol;

use app::SpiraVisionApp;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SpiraVision-10 Protocol",
        options,
        Box::new(|_cc| Ok(Box::new(SpiraVisionApp::default()))),
    )
}
