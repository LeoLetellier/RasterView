use super::{LeftPanel, panel_button};
use crate::RasterView;
use crate::icon;
use crate::widgets::buttons::IconButton;
use crate::widgets::buttons::Side;
use egui::{Color32, RichText, Ui};

impl RasterView {
    pub(crate) fn ui_bottom_panel(&mut self, ui: &mut Ui) {
        egui::Grid::new("bottom grid")
            .num_columns(3)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let is_param_open = self.ui_parameters(ui);
                    let response = ui
                        .add(
                            IconButton::new(icon::regular::GEAR, is_param_open)
                                .fill(icon::fill::GEAR)
                                .size(24.0)
                                .side(Side::Bottom)
                                .accent(Color32::from_rgb(30, 144, 255)),
                        )
                        .on_hover_text("Parameters");

                    if response.clicked() {
                        self.app_state.settings_panel.is_open = true;
                    }

                    ui.separator();

                    panel_button(
                        &mut self.left_panel_open,
                        &mut self.left_panel,
                        LeftPanel::Metadata,
                        ui,
                        "Toggle metadata panel",
                    );
                    panel_button(
                        &mut self.left_panel_open,
                        &mut self.left_panel,
                        LeftPanel::Palette,
                        ui,
                        "Toggle palette panel",
                    );
                });

                ui.horizontal_centered(|_| {});

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if cfg!(debug_assertions) {
                        egui::warn_if_debug_build(ui);
                        if self.app_state.settings.show_frame_rate {
                            let ms = ui.ctx().input(|i| i.unstable_dt * 1000.0);
                            ui.label(format!("{ms:.1} ms"));
                        }
                    }

                    if let Some(view) = &self.viewer
                        && let Some(px_pos) = view.state.last_cursor_pos
                    {
                        // Get pixel integers, so floor the value
                        let x_pos = px_pos.x.floor();
                        let y_pos = px_pos.y.floor();

                        if let Some(gt) = view.raster_handler.get_pixel_geotransform() {
                            let geo_pos = gt.pixel_to_geo(x_pos, y_pos);
                            ui.label(format!(" | geo: ({:.3},{:.3})", geo_pos.0, geo_pos.1));
                        }
                        ui.label(format!("px: ({:.0},{:.0})", x_pos, y_pos));
                        // grid-four, globe-simple
                    }
                });
            });
    }
}
