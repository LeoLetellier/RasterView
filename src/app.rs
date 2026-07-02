use crate::raster::RasterHandler;
use crate::texture_thread::TextureWorker;
use crate::viewers::{PanchromaticParams, ViewMode};
use anyhow::{Result, bail};
use std::path::PathBuf;

pub struct RasterView {
    raster_path: Option<PathBuf>,
    view_mode: Option<ViewMode>,
    texture_worker: TextureWorker,
}

impl RasterView {
    pub fn new(ctx: egui::Context) -> Self {
        let texture_worker = TextureWorker::new(ctx);
        Self {
            raster_path: Default::default(),
            view_mode: Default::default(),
            texture_worker,
        }
    }

    fn update_datasets(&mut self) -> Result<()> {
        if let Some(path) = &self.raster_path {
            let raster_handler = RasterHandler::new(path)?;
            self.view_mode = Some(ViewMode::Panchromatic(
                raster_handler,
                PanchromaticParams::default(),
                None,
            ));
            return Ok(());
        }
        bail!("no dataset to update");
    }

    /// Delete the current view in case load a file non supported
    /// In that case should reset the view
    fn clear_datasets(&mut self) {
        self.view_mode = None;
    }
}

impl eframe::App for RasterView {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.ctx().input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(path) = &dropped.path {
                    self.raster_path = Some(path.clone());
                    let _ = self.update_datasets();
                }
            }
        });

        egui::Panel::top("top panel").show_inside(ui, |ui| {
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
                    .on_hover_text("Open file...")
                    .clicked()
                {
                    self.raster_path = rfd::FileDialog::new().pick_file();
                    self.clear_datasets();
                    self.update_datasets();
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::widgets::global_theme_preference_switch(ui);
                    if ui
                        .button("Refresh")
                        .on_hover_text("refetch all data from file")
                        .clicked()
                    {
                        self.update_datasets();
                    }
                });
            });
        });

        egui::Panel::bottom("bottom panel").show_inside(ui, |_ui| {});

        egui::Panel::left("left panel")
            .size_range(100.0..=ui.ctx().content_rect().width() * 0.33)
            .show_inside(ui, |ui| {
                ui.heading("Raster Information");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Path:");
                    if let Some(path) = &self.raster_path {
                        egui::ScrollArea::horizontal()
                            .id_salt("path scroll")
                            .show(ui, |ui| {
                                if ui.monospace(path.display().to_string()).clicked() {
                                    ui.ctx().copy_text(path.display().to_string());
                                }
                            });
                    } else {
                        ui.monospace("None");
                    }
                });

                ui.horizontal(|ui| {
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
                });

                if let Some(view_mode) = &self.view_mode {
                    if let Some(raster) = view_mode.raster() {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.heading("Dataset");
                            ui.separator();
                            egui::ScrollArea::horizontal()
                                .id_salt("dataset scroll")
                                .show(ui, |ui| raster.dataset_properties.ui(ui));
                            for i in 0..raster.dataset.raster_count() {
                                ui.collapsing(format!("Band {}", i + 1), |ui| {
                                    raster.band_properties[i].ui(ui);
                                });
                            }
                        });
                    }
                }
            });

        egui::Panel::right("right panel").show_inside(ui, |_ui| {});

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(view) = &mut self.view_mode {
                view.ui(ui, &mut self.texture_worker);
            }
        });
    }
}

pub fn prop_ui(ui: &mut egui::Ui, value: &str) {
    if ui.button(egui::RichText::new(value).monospace()).clicked() {
        ui.ctx().copy_text(value.to_string());
    }
}

pub fn prop_section(ui: &mut egui::Ui, section_name: Option<&str>, props: &[[String; 2]]) {
    if let Some(n) = section_name {
        ui.label(n);
    }
    for prop in props {
        ui.horizontal(|ui| {
            ui.label(&prop[0]);
            prop_ui(ui, &prop[1]);
        });
    }
}
