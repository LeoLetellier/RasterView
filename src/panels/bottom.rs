use crate::RasterView;
use crate::panels::PanelType;
use crate::raster::CrsKind;
use crate::viewers::coords::Bbox;
use crate::widgets::value_bar::ValueBar;
use crate::widgets::value_bar::ValueItem;
use egui::Ui;

impl RasterView {
    pub(crate) fn ui_bottom_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Left side
            self.panels.command_panel.panel_button(
                ui,
                PanelType::Metadata,
                "Toggle metadata panel",
            );
            self.panels
                .command_panel
                .panel_button(ui, PanelType::Palette, "Toggle palette panel");

            // Right side: takes the remaining width, first added = rightmost
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if cfg!(debug_assertions) {
                    egui::warn_if_debug_build(ui);
                    ui.separator();
                }
                if self.settings.show_frame_rate {
                    let ms = ui.ctx().input(|i| i.unstable_dt * 1000.0);
                    ui.add(
                        ValueBar::new()
                            .name("frame")
                            .item(ValueItem::new(ms).unit("ms").precision(1).chars(4))
                            .shadowed(),
                    );
                    ui.separator();
                }

                if let Some(raster_handler) = &self.raster_handler {
                    let gt = raster_handler.get_pixel_geotransform();
                    let cur = self.viewer.state.last_cursor_pos;
                    let (rw, rh) = raster_handler.metadata().size;
                    let bbox = raster_handler.metadata().bbox;
                    let px = cur.map(|p| (p.x.floor(), p.y.floor()));
                    let geo = cur
                        .zip(gt)
                        .map(|(p, gt)| gt.pixel_to_geo(p.x.floor() + 0.5, p.y.floor() + 0.5));

                    if gt.is_some() {
                        let e_label = match raster_handler.raster_metadata.crs_kind {
                            CrsKind::Geographic => "Lon",
                            CrsKind::Projected => "E",
                            CrsKind::Unknown => "Y",
                        };
                        let n_label = match raster_handler.raster_metadata.crs_kind {
                            CrsKind::Geographic => "Lat",
                            CrsKind::Projected => "N",
                            CrsKind::Unknown => "X",
                        };
                        let mut e_item = ValueItem::new(geo.map(|g| g.0)).label(e_label);
                        let mut n_item = ValueItem::new(geo.map(|g| g.1)).label(n_label);
                        if let Some(b) = bbox {
                            e_item = e_item.bounds(b.xmin(), b.xmax());
                            n_item = n_item.bounds(b.ymin(), b.ymax());
                        }
                        let symbol = match raster_handler.raster_metadata.crs_kind {
                            CrsKind::Geographic => "°",
                            CrsKind::Projected => "m",
                            CrsKind::Unknown => "",
                        };
                        ui.add(
                            ValueBar::new()
                                .name("Geo")
                                .item(e_item.unit(symbol))
                                .item(n_item.unit(symbol))
                                .shadowed(),
                        );
                    }

                    ui.add(
                        ValueBar::new()
                            .item(
                                ValueItem::new(px.map(|p| p.0))
                                    .int()
                                    .bounds(0.0, rw as f64)
                                    .label("X")
                                    .unit("px"),
                            )
                            .item(
                                ValueItem::new(px.map(|p| p.1))
                                    .int()
                                    .bounds(0.0, rh as f64)
                                    .label("Y")
                                    .unit("px"),
                            )
                            .shadowed(),
                    );
                }
            });
        });
    }
}
