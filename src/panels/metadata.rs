use crate::{RasterView, raster::BandMetadata, viewers::coords::Bbox};
use egui::{RichText, Ui};

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
                self.ui_dataset(ui);
                self.ui_bands(ui);
            }
        }
    }

    pub(crate) fn ui_dataset(&mut self, ui: &mut Ui) {
        let Some(view) = &self.viewer else {
            return;
        };
        let rh = &view.raster_handler;
        let metadata = rh.metadata();
        let driver = &metadata.driver;
        let size = metadata.size;
        let band_nb = metadata.band_nb;
        let projection = &metadata.projection;
        let geotransform = &metadata.geotransform;
        let bbox = metadata.bbox;

        ui.vertical(|ui| {
            ui.heading("Dataset");
            ui.separator();

            // Quick info cards
            let stats = [
                ("Driver", driver.to_string()),
                ("Bands", band_nb.to_string()),
                ("Width", size.0.to_string()),
                ("Height", size.1.to_string()),
            ];

            let card_width = 40.0;
            let available = ui.available_width();
            let cols =
                ((available / (card_width + 2.0 * ui.style().spacing.item_spacing.x)).floor()
                    as usize)
                    .max(1);

            egui::Grid::new("stats_grid")
                .num_columns(cols)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    for (i, (label, value)) in stats.iter().enumerate() {
                        ui.group(|ui| {
                            ui.set_width(card_width);
                            ui.vertical_centered(|ui| {
                                ui.small(*label);
                                ui.label(
                                    RichText::new(value)
                                        .strong()
                                        .color(egui::Color32::LIGHT_BLUE),
                                );
                            });
                        });
                        if (i + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });

            ui.separator();

            ui.collapsing("Description", |ui| {
                ui.label(&metadata.description);
            });

            // Size section with better layout
            ui.collapsing("Dimensions", |ui| {
                grid_2col(
                    ui,
                    &[
                        ("Width (X)", &size.0.to_string()),
                        ("Height (Y)", &size.1.to_string()),
                    ],
                );
            });

            ui.collapsing("Projection", |ui| {
                prop_ui(ui, projection);
            });

            let gt = geotransform;
            let offsets = gt.offsets();
            let resolutions = gt.resolutions();
            let rotations = gt.rotations();

            ui.collapsing("Geotransform", |ui| {
                egui::Grid::new("geotransform_grid")
                    .num_columns(3)
                    .spacing([16.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label("");
                        ui.label(RichText::new("X").weak());
                        ui.label(RichText::new("Y").weak());
                        ui.end_row();

                        geotransform_row(ui, "Offset", offsets.x, offsets.y);

                        geotransform_row(ui, "Resolution", resolutions.x, resolutions.y);

                        geotransform_row(ui, "Rotation", rotations.x, rotations.y);
                    });
            });

            if let Some(bb) = bbox {
                ui.collapsing("Bounding Box", |ui| {
                    grid_2col(
                        ui,
                        &[
                            ("X Min", format_decimal(bb.xmin()).as_str()),
                            ("X Max", format_decimal(bb.xmax()).as_str()),
                            ("Y Min", format_decimal(bb.ymin()).as_str()),
                            ("Y Max", format_decimal(bb.ymax()).as_str()),
                        ],
                    );
                });
            }
        });
    }

    pub(crate) fn ui_bands(&self, ui: &mut Ui) {
        let Some(view) = &self.viewer else {
            return;
        };
        let rh = &view.raster_handler;
        let metadata = rh.metadata();
        ui.vertical(|ui| {
            ui.heading("Bands");
            ui.separator();

            for (idx, band) in metadata.bands.iter().enumerate() {
                let band_label = if band.description.is_empty() {
                    format!("Band {}", idx + 1)
                } else {
                    format!("Band {}: {}", idx + 1, &band.description)
                };
                ui.collapsing(band_label, |ui| {
                    self.ui_band(band, ui);
                });
            }
        });
    }

    fn ui_band(&self, band: &BandMetadata, ui: &mut Ui) {
        let mut props = vec![["Data Type".to_string(), band.dtype.clone()]];

        if !band.unit.is_empty() {
            props.push(["Unit".to_string(), band.unit.clone()]);
        }
        if let Some(v) = band.ndv {
            props.push(["No Data Value".to_string(), v.to_string()]);
        }
        if let Some(v) = band.scale {
            props.push(["Scale".to_string(), v.to_string()]);
        }
        if let Some(v) = band.offset {
            props.push(["Offset".to_string(), v.to_string()]);
        }

        ui.vertical(|ui| {
            grid_2col(
                ui,
                &props
                    .iter()
                    .map(|p| (p[0].as_str(), p[1].as_str()))
                    .collect::<Vec<_>>(),
            );

            if !band.overviews.is_empty() {
                ui.separator();
                ui.label(RichText::new("Overviews").small().weak());

                for ovr in &band.overviews {
                    ui.horizontal(|ui| {
                        ui.small(format!("Level {}", ovr[0]));
                        ui.small(format!("{}×{}", ovr[1], ovr[2]));
                    });
                }
            }
        });
    }
}

pub(crate) fn prop_ui(ui: &mut Ui, value: &str) {
    if ui
        .button(RichText::new(value).monospace().size(10.0))
        .clicked()
    {
        ui.ctx().copy_text(value.to_string());
    }
}

pub(crate) fn grid_2col(ui: &mut Ui, props: &[(&str, &str)]) {
    ui.vertical(|ui| {
        for (label, value) in props {
            ui.horizontal(|ui| {
                ui.label(RichText::new(*label).weak());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.small_button(RichText::new(*value).monospace()).clicked() {
                        ui.ctx().copy_text(value.to_string());
                    }
                });
            });
        }
    });
}

fn geotransform_row(ui: &mut egui::Ui, label: &str, x: f64, y: f64) {
    ui.label(RichText::new(label).weak());

    copyable_value(ui, x);
    copyable_value(ui, y);

    ui.end_row();
}

fn copyable_value(ui: &mut egui::Ui, value: f64) {
    let text = format_decimal(value);

    if ui.small_button(RichText::new(&text).monospace()).clicked() {
        ui.ctx().copy_text(text);
    }
}

fn format_decimal(value: f64) -> String {
    // Keep at most six decimal places and remove unnecessary zeroes.
    let mut text = format!("{value:.6}");

    if text.contains('.') {
        text = text.trim_end_matches('0').trim_end_matches('.').to_owned();
    }

    text
}
