use anyhow::Result;
use eframe::egui;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::panels::CommandPanel;
use crate::panels::parameters::{Settings, SettingsPanel};
use crate::panels::vrt_form::VrtRawPanel;
use crate::raster::RasterHandler;
use crate::raster::xml_vrt::VrtParameters;
use crate::viewers::Viewer;
use crate::widgets::install_phosphor;

use tracing_subscriber::{EnvFilter, Registry, prelude::*, reload};

type TracingHandle = reload::Handle<EnvFilter, Registry>;

const SETTINGS_KEY: &str = "settings";

/// Application Handler
pub(crate) struct RasterView {
    /// Access to the current raster
    pub(crate) raster_handler: Option<RasterHandler>,
    /// Access to the viewer
    pub(crate) viewer: Viewer,
    /// All possible panels and their state
    pub(crate) panels: Panels,
    /// Global app state
    pub(crate) app_state: AppState,
    /// Global app settings
    pub(crate) settings: Settings,
}

pub(crate) struct Panels {
    /// Left panels for metadata and view config
    pub(crate) command_panel: CommandPanel,
    /// Popup for loading raw rasters
    pub(crate) vrt_raw_panel: VrtRawPanel,
    /// Popup for all settings
    pub(crate) settings_panel: SettingsPanel,
}

pub(crate) struct AppState {
    pub(crate) raster_path: Option<PathBuf>,
    pub(crate) pyramid_promise: Option<poll_promise::Promise<Result<()>>>,
    pub(crate) vrt_params: VrtParameters,
    pub(crate) tracing_handle: TracingHandle,
}

static TRACING_HANDLE: OnceLock<TracingHandle> = OnceLock::new();

pub(crate) fn init_logging_once(verbose: bool) -> TracingHandle {
    TRACING_HANDLE.get_or_init(|| init_logging(verbose)).clone()
}

impl Default for AppState {
    fn default() -> Self {
        let tracing_handle = init_logging_once(false);
        Self {
            raster_path: None,
            pyramid_promise: Default::default(),
            vrt_params: Default::default(),
            tracing_handle,
        }
    }
}

// Handle for tracing logging
fn build_filter(verbose: bool) -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(if verbose {
            "rview=debug,warn"
        } else {
            "rview=info,warn"
        })
    })
}

fn init_logging(verbose: bool) -> TracingHandle {
    let (filter, handle) = reload::Layer::new(build_filter(verbose));
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .init();
    handle
}

fn apply_log_level(handle: &TracingHandle, verbose: bool) {
    let _ = handle.reload(build_filter(verbose));
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("pyramid_promise", &self.pyramid_promise.is_some())
            .finish()
    }
}

impl RasterView {
    /// Create the app structure
    ///
    /// Need the egui context to register custom icons from phosphoricons
    pub(crate) fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let panels = Panels {
            command_panel: CommandPanel::default(),
            vrt_raw_panel: VrtRawPanel::default(),
            settings_panel: SettingsPanel::default(),
        };
        let settings = cc
            .storage
            .and_then(|s| eframe::get_value::<Settings>(s, SETTINGS_KEY))
            .unwrap_or_default();

        Self {
            raster_handler: None,
            viewer: Viewer::default(),
            panels: panels,
            app_state: AppState::default(),
            settings,
        }
    }

    pub(crate) fn update_path(&mut self, new_path: &Path, ctx: egui::Context) -> Result<()> {
        if let Some(path) = &self.app_state.raster_path {
            // Check if we really got new raster
            if path == new_path {
                // Nothing to do, early return
                tracing::info!("Asking update path > file already loaded");
                return Ok(());
            } else {
                tracing::info!("Asking update path > change loaded file");
            }
        } else {
            // First raster to initialize
            tracing::info!("Asking update path > first loading file");
        }

        // TODO cache size param instead of hardcoded
        let raster_handler = RasterHandler::new(new_path, ctx, 256 * 1024 * 1024);

        self.raster_handler = raster_handler.ok();

        if self.raster_handler.is_none() {
            self.panels.vrt_raw_panel.is_open = true;
            self.panels.vrt_raw_panel.path_buf = new_path.to_string_lossy().into_owned();
        } else {
            self.app_state.raster_path = Some(new_path.into());
        }

        Ok(())
    }

    pub(crate) fn update_path_force(&mut self, new_path: &Path, ctx: egui::Context) -> Result<()> {
        self.raster_handler = Some(RasterHandler::new(new_path, ctx, 256 * 1024 * 1024)?);
        self.app_state.raster_path = Some(new_path.into());
        Ok(())
    }
}

impl eframe::App for RasterView {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.settings);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Vrt form
        if self.panels.vrt_raw_panel.is_open {
            let raw_res = self
                .panels
                .vrt_raw_panel
                .ui(ui, &mut self.app_state.vrt_params);
            if let Some((rh, buf)) = raw_res {
                self.raster_handler = Some(rh);
                self.app_state.raster_path = Some(buf);
            }
        }
        // Parameters
        self.panels.settings_panel.ui(ui, &mut self.settings);

        if self.settings.show_theme_panel && cfg!(debug_assertions) {
            ui.ctx().show_viewport_immediate(
                egui::ViewportId::from_hash_of("style_editor"),
                egui::ViewportBuilder::default()
                    .with_title("Style editor")
                    .with_inner_size([420.0, 720.0]),
                |ui, _class| {
                    // Fallback if the backend can't open real windows
                    // if class == egui::ViewportClass::EmbeddedWindow {
                    //     // draw an egui::Window here instead if you care
                    // }

                    egui::CentralPanel::default().show(ui, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.ctx().clone().style_ui(ui, egui::Theme::Dark);
                        });
                    });
                },
            );
        }

        // Drag n Drop
        ui.ctx().input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                let _ = self.update_path(dropped.path(), ui.ctx().clone());
            }
        });

        // Show top panel first for menus
        egui::Panel::top("top panel").show(ui, |ui| {
            self.ui_top_panel(ui);
        });

        // Then bottom panel for global contextual info
        egui::Panel::bottom("bottom panel").show(ui, |ui| {
            self.ui_bottom_panel(ui);
        });

        if self.raster_handler.is_some() {
            let mut is_open = self.panels.command_panel.is_open;
            egui::Panel::left("left panel")
                .min_size(250.0)
                .max_size(ui.ctx().content_rect().width() * 0.50)
                .show_collapsible(ui, &mut is_open, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        self.panels.command_panel.ui(
                            ui,
                            &mut self.viewer,
                            &mut self.raster_handler,
                        );
                    });
                });

            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(rh) = &mut self.raster_handler {
                    self.viewer.ui(ui, rh, &self.settings);
                }
            });
        } else {
            // If no raster loaded, show a big button to load one

            ui.with_layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| {
                    let old_visuals = ui.style().visuals.clone();
                    ui.style_mut().visuals.widgets.inactive.weak_bg_fill =
                        egui::Color32::TRANSPARENT;

                    let button = egui::Button::new("Open a raster file to begin...")
                        .min_size(egui::Vec2::new(360.0, 48.0));

                    if ui.add(button).clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_file()
                    {
                        let _ = self.update_path(path.as_path(), ui.ctx().clone());
                    }

                    ui.style_mut().visuals = old_visuals;
                },
            );
        }
    }
}

pub const REGULAR_FAMILY: &str = "phosphor-regular";
pub const FILL_FAMILY: &str = "phosphor-fill";

pub fn regular_family() -> egui::FontFamily {
    egui::FontFamily::Name(REGULAR_FAMILY.into())
}
pub fn fill_family() -> egui::FontFamily {
    egui::FontFamily::Name(FILL_FAMILY.into())
}

/// Change font family and size from egui default
///
/// Load a custom font from file `.ttf`
///
/// Must be used when creating the egui app, such as:
///
/// ```
/// eframe::run_native(
///     "RasterView",
///     native_options,
///     Box::new(|cc| {
///         crate::app::setup_custom_fonts(&cc.egui_ctx);
///         Ok(Box::new(app::RasterView::new(cc.egui_ctx.clone())))
///     }),
/// )
/// ```
pub(crate) fn setup_custom_fonts(ctx: &egui::Context) {
    // Font family
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "GeistRegular".to_owned(),
        egui::FontData::from_static(include_bytes!("../resources/fonts/Geist-Regular.ttf")).into(),
    );

    fonts.font_data.insert(
        "InterVariable".to_owned(),
        egui::FontData::from_static(include_bytes!("../resources/fonts/Inter-Variable.ttf")).into(),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "InterVariable".to_owned());

    // Explicitly register the Phosphor icons to avoid colliding with custom font
    install_phosphor(&mut fonts);

    ctx.set_fonts(fonts);

    // Font size
    use egui::FontFamily::Proportional;
    use egui::FontId;
    use egui::TextStyle::*;
    use std::collections::BTreeMap;

    let text_styles: BTreeMap<_, _> = [
        (Heading, FontId::new(22.0, Proportional)),
        (Name("Subheading".into()), FontId::new(16.0, Proportional)),
        (Body, FontId::new(14.0, Proportional)),
        (Monospace, FontId::new(13.0, egui::FontFamily::Monospace)),
        (Button, FontId::new(14.0, Proportional)),
        (Small, FontId::new(10.0, Proportional)),
    ]
    .into();

    ctx.all_styles_mut(move |style| style.text_styles = text_styles.clone());
}
