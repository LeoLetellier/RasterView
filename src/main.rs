#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod panels;
mod raster;
mod viewers;

pub(crate) use app::RasterView;
pub(crate) use egui_phosphor as icon;
pub(crate) use viewers::Viewer;

fn main() -> eframe::Result {
    const SHOW_ALL_TRACING: bool = false;

    let default_level = if cfg!(debug_assertions) {
        if SHOW_ALL_TRACING {
            "debug"
        } else {
            "raster_view=debug,warn"
        }
    } else if SHOW_ALL_TRACING {
        "info"
    } else {
        "raster_view=info,warn"
    };

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();

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
        }),
    )
}
