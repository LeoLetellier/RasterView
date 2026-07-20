use crate::RasterView;
use egui::Ui;

impl RasterView {
    pub(crate) fn ui_metadata_panel(&mut self, ui: &mut Ui) {
        ui.heading("Raster Information");
        ui.separator();

        egui::Grid::new("raster_info_grid")
            .num_columns(2)
            .spacing([8.0, 2.0])
            .show(ui, |ui| {
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

        {
            if let Some(viewer) = &self.viewer {
                egui::ScrollArea::both().show(ui, |ui| {
                    viewer.raster_handler.ui_dataset(ui);
                    viewer.raster_handler.ui_bands(ui);
                });
            }
        }
    }
}
