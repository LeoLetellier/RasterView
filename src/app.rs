use anyhow::Result;
use egui_phosphor as icon;
use std::path::{Path, PathBuf};

use crate::panels::{LeftPanel, RightPanel};
use crate::viewers::Viewer;

/// The structure containing the whole rview app
///
/// # Example
///
/// ```rs
/// let ctx = cc.egui_ctx.clone();
/// let app = RasterView::new(ctx);
/// ```
pub(crate) struct RasterView {
    pub(crate) raster_path: Option<PathBuf>,
    pub(crate) viewer: Option<Viewer>,
    pub(crate) left_panel_open: bool,
    pub(crate) left_panel: LeftPanel,
    pub(crate) right_panel_open: bool,
    pub(crate) right_panel: RightPanel,
}

impl RasterView {
    /// Create the app structure
    ///
    /// Need the egui context to register custom icons from phosphoricons
    pub(crate) fn new(ctx: egui::Context) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        icon::add_to_fonts(&mut fonts, icon::Variant::Regular);
        icon::add_to_fonts(&mut fonts, icon::Variant::Fill);
        ctx.set_fonts(fonts);

        Self {
            raster_path: Default::default(),
            viewer: Default::default(),
            left_panel_open: true,
            left_panel: LeftPanel::Metadata,
            right_panel_open: true,
            right_panel: RightPanel::Palette,
        }
    }

    pub(crate) fn update_path(&mut self, new_path: &Path, ctx: egui::Context) -> Result<()> {
        if let Some(path) = &self.raster_path {
            // Check if we really got new raster
            if (path == new_path) {
                // Nothing to do, early return
                return Ok(());
            } else {
                self.viewer = Some(Viewer::with_raster(new_path, ctx)?);
            }
        } else {
            // First raster to initialize
            self.viewer = Some(Viewer::with_raster(new_path, ctx)?);
        }

        self.raster_path = Some(new_path.into());
        Ok(())
    }

    pub(crate) fn update_path_force(&mut self, new_path: &Path, ctx: egui::Context) -> Result<()> {
        if let Some(path) = &self.raster_path {
            self.viewer = Some(Viewer::with_raster(new_path, ctx)?);
        } else {
            // First raster to initialize
            self.viewer = Some(Viewer::with_raster(new_path, ctx)?);
        }

        self.raster_path = Some(new_path.into());
        Ok(())
    }
}

impl eframe::App for RasterView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Drag n Drop
        ui.ctx().input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(path) = &dropped.path {
                    let _ = self.update_path(&path.to_path_buf(), ui.ctx().clone());
                }
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

        if self.raster_path.is_some() {
            // Show the viewer when a raster is loaded

            // Show left panel is toggled
            let mut is_open = self.left_panel_open;
            egui::Panel::left("left panel")
                .max_size(ui.ctx().content_rect().width() * 0.33)
                .show_collapsible(ui, &mut is_open, |ui| {
                    self.ui_left_panel(ui);
                });

            // Show right panel if toggled
            let mut is_open = self.right_panel_open;
            egui::Panel::right("right panel")
                .max_size(ui.ctx().content_rect().width() * 0.33)
                .show_collapsible(ui, &mut is_open, |ui| {
                    self.ui_right_panel(ui);
                });

            // Lastly show the view at the center
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(view) = &mut self.viewer {
                    view.ui(ui);
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

                    if ui.add(button).clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_file() {
                            let _ = self.update_path(path.as_path(), ui.ctx().clone());
                        }
                    }

                    ui.style_mut().visuals = old_visuals;
                },
            );
        }
    }
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

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "GeistRegular".to_owned());

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
