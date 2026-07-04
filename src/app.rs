use crate::raster::RasterHandler;
// use crate::texture_thread::TextureWorker;
use crate::viewers::Viewer;
use anyhow::{Result, bail};
use std::path::PathBuf;

pub struct RasterView {
    raster_path: Option<PathBuf>,
    viewer: Viewer,
    // texture_worker: TextureWorker,
    left_panel_collapsed: bool,
}

impl RasterView {
    pub fn new(ctx: egui::Context) -> Self {
        // let texture_worker = TextureWorker::new(ctx);
        Self {
            raster_path: Default::default(),
            viewer: Default::default(),
            // texture_worker,
            left_panel_collapsed: false,
        }
    }

    fn update_datasets(&mut self) -> Result<()> {
        if let Some(path) = &self.raster_path {
            let raster_handler = RasterHandler::new(path)?;
            // self.view_mode = Some(ViewMode::Panchromatic(
            //     raster_handler,
            //     PanchromaticParams::default(),
            //     None,
            // ));
            return Ok(());
        }
        bail!("no dataset to update");
    }

    // /// Delete the current view in case load a file non supported
    // /// In that case should reset the view
    // fn clear_datasets(&mut self) {
    //     self.view_mode = None;
    // }
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
                let left_panel_symbol = if self.left_panel_collapsed {
                    "⏶"
                } else {
                    "⏷"
                };
                if ui.button(left_panel_symbol).clicked() {
                    self.left_panel_collapsed = !self.left_panel_collapsed;
                }

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
                    // self.clear_datasets();
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

        egui::Panel::bottom("bottom panel").show_inside(ui, |ui| {
            if cfg!(debug_assertions) {
                let ms = ui.ctx().input(|i| i.unstable_dt * 1000.0);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    egui::warn_if_debug_build(ui);
                    ui.label(format!("{ms:.1} ms"));
                });
            }
        });

        let is_open = !self.left_panel_collapsed;
        egui::Panel::left("left panel")
            .resizable(is_open)
            .size_range(if is_open {
                100.0..=ui.ctx().content_rect().width() * 0.33
            } else {
                0.0..=0.0
            })
            .show_animated_inside(ui, is_open, |ui| {
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

                {
                    if let Some(raster) = self.viewer.raster_handler.as_mut() {
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

        // egui::Panel::right("right panel").show_inside(ui, |_ui| {});

        let viewer = &mut self.viewer;
        egui::CentralPanel::default().show_inside(ui, |ui| {
            viewer.ui(ui);
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
