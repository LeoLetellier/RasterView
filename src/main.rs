#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod raster;
mod texture_thread;
mod view_mode;

fn main() -> eframe::Result {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };
    eframe::run_native(
        "RasterView",
        native_options,
        Box::new(|cc| Ok(Box::new(app::RasterView::new(cc.egui_ctx.clone())))), // Gets egui context reference
    )
}
