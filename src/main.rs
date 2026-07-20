#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod panels;
mod raster;
mod viewers;

pub(crate) use app::RasterView;

fn main() -> eframe::Result {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };

    eframe::run_native(
        "RasterView",
        native_options,
        Box::new(|cc| {
            crate::app::setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(app::RasterView::new(cc.egui_ctx.clone())))
        }), // Gets egui context reference
    )
}
