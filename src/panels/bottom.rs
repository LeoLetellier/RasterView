use crate::RasterView;
use crate::panels::CommandPanelType;
use crate::raster::CrsKind;
use crate::viewers::ActiveViewer;
use crate::viewers::coords::Bbox;
use crate::widgets::value_bar::ValueBar;
use crate::widgets::value_bar::ValueItem;
use eframe::egui::{self, Ui};

impl RasterView {
    pub(crate) fn ui_bottom_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            // Left side
            self.panels.command_panel.panel_button(
                ui,
                CommandPanelType::Metadata,
                "Toggle metadata panel",
            );
            self.panels.command_panel.panel_button(
                ui,
                CommandPanelType::Palette,
                "Toggle palette panel",
            );

            ui.separator();

            self.panels.slider_panel.panel_button(
                ui,
                super::SliderPanelType::BandSlider,
                "Toggle band slider",
            );
            self.panels.slider_panel.panel_button(
                ui,
                super::SliderPanelType::PointSeries,
                "Toggle point series",
            );

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
                    let (width, height) = raster_handler.raster_metadata.size;
                    let point: Option<(usize, usize)> = px.and_then(|(x, y)| {
                        if x < 0.0 || y < 0.0 || x >= width as f64 || y >= height as f64 {
                            None
                        } else {
                            Some((x as usize, y as usize))
                        }
                    });

                    let read = |band: usize| {
                        point.and_then(|p| {
                            self.raster_handler
                                .as_ref()
                                .map(|rh| rh.value_at_point(p, band))
                                .and_then(Result::ok)
                        })
                    };

                    if self.viewer.view_mode.active_viewer == ActiveViewer::Band {
                        let value = read(self.viewer.view_mode.panchro_band);
                        ui.add(
                            ValueBar::new()
                                .item(ValueItem::new(value).label("Value"))
                                .shadowed(),
                        );
                    } else if self.viewer.view_mode.active_viewer == ActiveViewer::Color {
                        let (r, g, b) = self.viewer.view_mode.rgb_bands;
                        ui.add(
                            ValueBar::new()
                                .item(ValueItem::new(read(r)).label("R"))
                                .item(ValueItem::new(read(g)).label("G"))
                                .item(ValueItem::new(read(b)).label("B"))
                                .shadowed(),
                        );
                    }
                }
            });
        });
    }
}
