use super::{LeftPanel, panel_button};
use crate::RasterView;
use crate::icon;
use crate::widgets::icon_button::{IconButton, Indicator, Side};
use crate::widgets::value_bar::ValueBar;
use crate::widgets::value_bar::ValueItem;
use egui::{Color32, Ui};

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
                                .indicator(Indicator::Border)
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
                            ui.add(
                                ValueBar::new()
                                    .name("frame")
                                    .item(ValueItem::new(ms).unit("ms").precision(1).chars(5)),
                            );
                        }
                        ui.separator();
                    }

                    if let Some(view) = &self.viewer {
                        let gt = view.raster_handler.get_pixel_geotransform();
                        let cur = view.state.last_cursor_pos; // Option<PlotPoint>, None when outside the plot

                        // Pixel indices (floored)
                        let px = cur.map(|p| (p.x.floor(), p.y.floor()));

                        // Geo position at the pixel center, None if not georeferenced or cursor outside
                        let geo = cur
                            .zip(gt)
                            .map(|(p, gt)| gt.pixel_to_geo(p.x.floor() + 0.5, p.y.floor() + 0.5));

                        ui.horizontal(|ui| {
                            ui.add(
                                ValueBar::new()
                                    .name("px")
                                    .item(ValueItem::new(px.map(|p| p.0)).int())
                                    .item(ValueItem::new(px.map(|p| p.1)).int()),
                            );

                            // Decided from metadata, not from cursor presence, so the layout stays stable.
                            if gt.is_some() {
                                ui.separator();
                                ui.add(
                                    ValueBar::new()
                                        .name("geo")
                                        .item(ValueItem::new(geo.map(|g| g.0)).label("E"))
                                        .item(ValueItem::new(geo.map(|g| g.1)).label("N")),
                                );
                            }
                        });
                    }
                });
            });
    }
}
