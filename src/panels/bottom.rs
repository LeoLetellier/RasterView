use super::{LeftPanel, RightPanel, panel_button};
use crate::RasterView;
use egui::Ui;

impl RasterView {
    pub(crate) fn ui_bottom_panel(&mut self, ui: &mut Ui) {
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

                            if let Some(gt) = view.raster_handler.get_pixel_geotransform() {
                                let geo_pos = gt.pixel_to_geo(x_pos, y_pos);
                                ui.label(format!(" | geo: ({:.3},{:.3})", geo_pos.0, geo_pos.1));
                            }
                            ui.label(format!("px: ({:.0},{:.0})", x_pos, y_pos));
                        }
                    }
                });
            });
    }
}
