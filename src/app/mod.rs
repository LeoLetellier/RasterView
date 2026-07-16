// use crate::texture_thread::TextureWorker;
use crate::viewers::Viewer;
use anyhow::Result;
use egui::Align;
use egui::Color32;
use egui::InnerResponse;
use egui::Label;
use egui::Layout;
use egui::RichText;
use egui::Ui;
use egui::widget_text::WidgetText;
use egui_phosphor as icon;
use std::path::Path;
use std::path::PathBuf;

pub struct RasterView {
    raster_path: Option<PathBuf>,
    viewer: Option<Viewer>,
    // texture_worker: TextureWorker,
    left_panel_open: bool,
    left_panel: LeftPanel,
    right_panel_open: bool,
    right_panel: RightPanel,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum LeftPanel {
    Metadata,
}

trait Panel: PartialEq {
    fn symbol(&self) -> RichText;
    fn symbol_highlight(&self) -> RichText;
}

impl Panel for LeftPanel {
    fn symbol(&self) -> RichText {
        RichText::new(icon::regular::ARTICLE_MEDIUM)
    }

    fn symbol_highlight(&self) -> RichText {
        RichText::new(icon::fill::ARTICLE_MEDIUM).color(Color32::from_rgb(30, 144, 255))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RightPanel {
    Palette,
    Parameters,
}

impl Panel for RightPanel {
    fn symbol(&self) -> RichText {
        match &self {
            RightPanel::Palette => RichText::new(icon::regular::PAINT_BRUSH_HOUSEHOLD),
            RightPanel::Parameters => RichText::new(icon::regular::GEAR),
        }
    }

    fn symbol_highlight(&self) -> RichText {
        match &self {
            RightPanel::Palette => RichText::new(icon::fill::PAINT_BRUSH_HOUSEHOLD)
                .color(Color32::from_rgb(30, 144, 255)),
            RightPanel::Parameters => {
                RichText::new(icon::fill::GEAR).color(Color32::from_rgb(30, 144, 255))
            }
        }
    }
}

impl RasterView {
    pub fn new(ctx: egui::Context) -> Self {
        // let texture_worker = TextureWorker::new(ctx);

        let mut fonts = egui::FontDefinitions::default();
        icon::add_to_fonts(&mut fonts, icon::Variant::Regular);
        icon::add_to_fonts(&mut fonts, icon::Variant::Fill);
        ctx.set_fonts(fonts);

        Self {
            raster_path: Default::default(),
            viewer: Default::default(),
            // texture_worker,
            left_panel_open: true,
            left_panel: LeftPanel::Metadata,
            right_panel_open: true,
            right_panel: RightPanel::Palette,
        }
    }

    fn update_path(&mut self, new_path: &Path) -> Result<()> {
        if let Some(path) = &self.raster_path {
            // Check if we really got new raster
            if path != new_path {
                self.viewer = Some(Viewer::with_raster(new_path)?);
            } else {
                // Nothing to do, early return
                return Ok(());
            }
        } else {
            // First raster to initialize
            self.viewer = Some(Viewer::with_raster(new_path)?);
        }

        self.raster_path = Some(new_path.into());
        Ok(())
    }

    fn reset_viewer(&mut self) -> Result<()> {
        if let Some(path) = &self.raster_path {
            self.viewer = Some(Viewer::with_raster(&path.as_path())?);
        }
        Ok(())
    }

    // /// Delete the current view in case load a file non supported
    // /// In that case should reset the view
    // fn clear_datasets(&mut self) {
    //     self.view_mode = None;
    // }
    //
    fn ui_left_panel(&mut self, ui: &mut Ui) {
        match self.left_panel {
            LeftPanel::Metadata => {
                if let Some(path) = &self.raster_path.as_ref() {
                    ui.heading("Raster Information");
                    ui.separator();

                    egui::Grid::new("raster_info_grid")
                        .num_columns(2)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            ui.label("Path:");
                            if let Some(path) = &self.raster_path {
                                egui::ScrollArea::horizontal().id_salt("path scroll").show(
                                    ui,
                                    |ui| {
                                        if ui.monospace(path.display().to_string()).clicked() {
                                            ui.ctx().copy_text(path.display().to_string());
                                        }
                                    },
                                );
                            } else {
                                ui.monospace("None");
                            }
                            ui.end_row();

                            ui.label("File:");
                            if let Some(path) = &self.raster_path {
                                egui::ScrollArea::both()
                                    .id_salt("file scroll")
                                    .show(ui, |ui| {
                                        let name = path
                                            .file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Unknown");
                                        if ui.monospace(name).clicked() {
                                            ui.ctx().copy_text(name.to_string());
                                        }
                                    });
                            } else {
                                ui.monospace("None");
                            }
                            ui.end_row();
                        });
                } else {
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(Label::new("Open a raster file to begin ...").wrap());
                    });
                }

                {
                    if let Some(viewer) = &self.viewer {
                        if let Some(raster_handler) = &viewer.raster_handler {
                            egui::ScrollArea::both().show(ui, |ui| {
                                raster_handler.ui_dataset(ui);
                                raster_handler.ui_bands(ui);
                            });
                        }
                    }
                }
            }
            _ => (),
        }
    }

    fn ui_bottom_panel(&mut self, ui: &mut Ui) {
        egui::Grid::new("bottom grid")
            .num_columns(3)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    panel_button(
                        &mut self.left_panel_open,
                        &mut self.left_panel,
                        LeftPanel::Metadata,
                        ui,
                        "Toggle metadata panel",
                    );
                });

                ui.horizontal_centered(|ui| {});

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    panel_button(
                        &mut self.right_panel_open,
                        &mut self.right_panel,
                        RightPanel::Parameters,
                        ui,
                        "Toggle parameters panel",
                    );
                    panel_button(
                        &mut self.right_panel_open,
                        &mut self.right_panel,
                        RightPanel::Palette,
                        ui,
                        "Toggle palette panel",
                    );

                    if cfg!(debug_assertions) {
                        let ms = ui.ctx().input(|i| i.unstable_dt * 1000.0);
                        egui::warn_if_debug_build(ui);
                        ui.label(format!("{ms:.1} ms"));
                    }

                    if let Some(view) = &self.viewer {
                        if let Some(px_pos) = view.state.last_cursor_pos {
                            // Get pixel integers, so floor the value
                            let x_pos = px_pos.x.floor();
                            let y_pos = px_pos.y.floor();

                            if let Some(gt) = view
                                .raster_handler
                                .as_ref()
                                .map(|r| r.get_pixel_geotransform())
                                .flatten()
                            {
                                let geo_pos = gt.pixel_to_geo(x_pos, y_pos);
                                ui.label(format!(" | geo: ({:.3},{:.3})", geo_pos.0, geo_pos.1));
                            }
                            ui.label(format!("px: ({:.0},{:.0})", x_pos, y_pos));
                        }
                    }
                });
            });
    }

    fn ui_top_panel(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let button_file_name = if let Some(path) = &self.raster_path {
                format!(
                    "File: {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                )
            } else {
                "File".to_string()
            };
            if ui
                .button(button_file_name)
                .on_hover_text("Select a raster file")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let _ = self.update_path(path.as_path());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_switch(ui);
                if ui
                    .button("Refresh")
                    .on_hover_text("Refresh cache and metadata")
                    .clicked()
                {
                    if let Some(view) = &mut self.viewer {
                        let _ = view.refresh_cache();
                    }
                }
            });
        });
    }

    fn ui_right_panel(&mut self, ui: &mut Ui) {
        match &self.right_panel {
            RightPanel::Palette => {
                ui.with_layout(
                    Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| ui.add(Label::new("This is empty! For now...").wrap()),
                );
            }
            _ => (),
        }
    }
}

/// Create a button linked to a panel state, switching between panels or toggling the panel visibility
fn panel_button<P: Panel>(
    is_open: &mut bool,
    current_panel: &mut P,
    panel: P,
    ui: &mut Ui,
    on_hover: impl Into<WidgetText>,
) {
    let panel_selected = *current_panel == panel;
    let highlight = panel_selected & *is_open;
    let panel_symbol = if highlight {
        panel.symbol_highlight()
    } else {
        panel.symbol()
    };

    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = egui::vec2(0.0, 0.0);

        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.active.bg_fill = egui::Color32::TRANSPARENT;

        let btn = ui
            .add(
                egui::Button::new(panel_symbol)
                    .frame(false)
                    .min_size(egui::Vec2::ZERO),
            )
            .on_hover_text(on_hover);
        let btn = if highlight { btn.highlight() } else { btn };
        if btn.clicked() {
            if panel_selected {
                *is_open = !*is_open;
            } else {
                *current_panel = panel;
            }
        }
    });
}

impl eframe::App for RasterView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(path) = &dropped.path {
                    let _ = self.update_path(&path.to_path_buf());
                }
            }
        });

        egui::Panel::top("top panel").show(ui, |ui| {
            self.ui_top_panel(ui);
        });

        egui::Panel::bottom("bottom panel").show(ui, |ui| {
            self.ui_bottom_panel(ui);
        });

        let mut is_open = self.left_panel_open;
        egui::Panel::left("left panel")
            .max_size(ui.ctx().content_rect().width() * 0.33)
            .show_collapsible(ui, &mut is_open, |ui| {
                self.ui_left_panel(ui);
            });

        let mut is_open = self.right_panel_open;
        egui::Panel::right("right panel")
            .max_size(ui.ctx().content_rect().width() * 0.33)
            .show_collapsible(ui, &mut is_open, |ui| {
                self.ui_right_panel(ui);
            });

        egui::CentralPanel::default().show(ui, |ui| {
            if let Some(view) = &mut self.viewer {
                view.ui(ui);
            }
        });
    }
}

pub fn setup_custom_fonts(ctx: &egui::Context) {
    // Font family
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "GeistRegular".to_owned(),
        egui::FontData::from_static(include_bytes!("../../resources/fonts/Geist-Regular.ttf"))
            .into(),
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

fn display_section(
    title: String,
    main_section: bool,
    ui: &mut Ui,
    add_contents: impl FnOnce(&mut Ui),
) -> InnerResponse<()> {
    if main_section {
        ui.separator();
    }
    ui.heading(&title);
    egui::Grid::new(format!("grid_section_{}", &title))
        .num_columns(2)
        .show(ui, add_contents)
}
