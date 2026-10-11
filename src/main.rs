#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod panels;
mod raster;
mod viewers;
mod widgets;

pub(crate) use app::RasterView;
pub(crate) use egui_phosphor as icon;
pub(crate) use viewers::Viewer;

fn main() -> eframe::Result {
    let _ = app::init_logging_once(false);

    if cfg!(debug_assertions) {
        println!("GDAL {}", gdal::version::VersionInfo::version_report());
        println!("drivers: {}", gdal::DriverManager::count());
        for i in 0..gdal::DriverManager::count() {
            if let Ok(d) = gdal::DriverManager::get_driver(i) {
                println!("  {}", d.short_name());
            }
        }
    }

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };
    eframe::run_native(
        "RasterView",
        native_options,
        Box::new(|cc| {
            crate::app::setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(app::RasterView::new(cc)))
        }),
    )
}
