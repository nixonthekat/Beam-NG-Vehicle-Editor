mod app;
mod backup;
mod config;
mod engine;
mod error;
mod gui;
mod json_util;
mod mod_scanner;
mod scan_util;
mod scanner;
mod settings;
mod state;
mod parts;
mod thumbnail;
mod vehicle_source;

use app::BeamNgVehicleEditor;

fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 733.0])
            .with_min_inner_size([1024.0, 733.0])
            .with_title("BeamNG Vehicle Editor"),
        ..Default::default()
    };

    eframe::run_native(
        "BeamNG Vehicle Editor",
        options,
        Box::new(|cc| Ok(Box::new(BeamNgVehicleEditor::new(cc)))),
    )
}
