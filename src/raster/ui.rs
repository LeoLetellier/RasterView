use crate::{
    raster::{BandMetadata, RasterHandler},
    viewers::coords::Bbox,
};
use egui::{RichText, Ui};

impl RasterHandler {
    pub(crate) fn ui_dataset(&self, ui: &mut Ui) {
        let metadata = &self.raster_metadata;
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
            ui.horizontal(|ui| {
                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.small("Driver");
                        ui.label(
                            RichText::new(driver)
                                .strong()
                                .color(egui::Color32::LIGHT_BLUE),
                        );
                    });
                });

                ui.group(|ui| {
                    ui.vertical(|ui| {
                        ui.small("Bands");
                        ui.label(
                            RichText::new(band_nb.to_string())
                                .strong()
                                .color(egui::Color32::LIGHT_BLUE),
                        );
                    });
                });
            });

            ui.separator();

            // Size section with better layout
            ui.collapsing("Size", |ui| {
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

            if let Some(gt) = geotransform {
                ui.collapsing("Geotransform", |ui| {
                    grid_2col(
                        ui,
                        &[
                            ("X Offset (UL)", &gt.offsets().x.to_string()),
                            ("X Resolution", &gt.resolutions().x.to_string()),
                            ("X Rotation", &gt.rotations().x.to_string()),
                            ("Y Offset (UL)", &gt.offsets().y.to_string()),
                            ("Y Rotation", &gt.rotations().y.to_string()),
                            ("Y Resolution", &gt.resolutions().y.to_string()),
                        ],
                    );
                });
            }

            if let Some(bb) = bbox {
                ui.collapsing("Bounding Box", |ui| {
                    grid_2col(
                        ui,
                        &[
                            ("X Min", &bb.xmin().to_string()),
                            ("X Max", &bb.xmax().to_string()),
                            ("Y Min", &bb.ymin().to_string()),
                            ("Y Max", &bb.ymax().to_string()),
                        ],
                    );
                });
            }
        });
    }

    pub(crate) fn ui_bands(&self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.heading("Bands");
            ui.separator();

            for (idx, band) in self.raster_metadata.bands.iter().enumerate() {
                ui.collapsing(format!("Band {} ({})", idx + 1, &band.dtype), |ui| {
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

// impl RasterHandler {
//     pub(crate) fn ui_dataset(&self, ui: &mut Ui) {
//         let metadata = &self.raster_metadata;
//         let driver = &metadata.driver;
//         let size = metadata.size;
//         let band_nb = metadata.band_nb;
//         let projection = &metadata.projection;
//         let geotransform = &metadata.geotransform;
//         let bbox = metadata.bbox;

//         prop_section(ui, None, &[["Driver".to_string(), driver.clone()]]);
//         prop_section(
//             ui,
//             Some("Size"),
//             &[
//                 ["x".to_string(), size.0.to_string()],
//                 ["y".to_string(), size.1.to_string()],
//             ],
//         );
//         prop_section(ui, None, &[["Band nb".to_string(), band_nb.to_string()]]);
//         prop_section(ui, None, &[["Projection".to_string(), projection.clone()]]);
//         if let Some(gt) = geotransform {
//             prop_section(
//                 ui,
//                 Some("Geotransform"),
//                 &[
//                     ["x ul".to_string(), gt.offsets().x.to_string()],
//                     ["x res".to_string(), gt.resolutions().x.to_string()],
//                     ["x rot".to_string(), gt.rotations().x.to_string()],
//                     ["y ul".to_string(), gt.offsets().y.to_string()],
//                     ["x rot".to_string(), gt.rotations().y.to_string()],
//                     ["x res".to_string(), gt.resolutions().y.to_string()],
//                 ],
//             );
//         }
//         if let Some(bb) = bbox {
//             prop_section(
//                 ui,
//                 Some("BBox"),
//                 &[
//                     ["xmin".to_string(), bb.xmin().to_string()],
//                     ["xmax".to_string(), bb.xmax().to_string()],
//                     ["ymin".to_string(), bb.ymin().to_string()],
//                     ["ymax".to_string(), bb.ymax().to_string()],
//                 ],
//             );
//         }
//     }

//     pub(crate) fn ui_bands(&self, ui: &mut Ui) {
//         self.raster_metadata.bands.iter().for_each(|b| {
//             self.ui_band(&b, ui);
//         })
//     }

//     fn ui_band(&self, band: &BandMetadata, ui: &mut Ui) {
//         let dtype = &band.dtype;
//         let unit = &band.unit;
//         let overviews_nb = band.overview_nb;
//         let overviews = &band.overviews;
//         let ndv = band.ndv;
//         let scale = band.scale;
//         let offset = band.offset;

//         let mut props = vec![["dtype".to_string(), dtype.clone()]];
//         if !unit.is_empty() {
//             props.push(["unit".to_string(), unit.clone()]);
//         }
//         if let Some(v) = ndv {
//             props.push(["ndv".to_string(), v.to_string()]);
//         }
//         if let Some(v) = scale {
//             props.push(["scale".to_string(), v.to_string()]);
//         }
//         if let Some(v) = offset {
//             props.push(["offset".to_string(), v.to_string()]);
//         }
//         prop_section(ui, Some("Data"), &props);
//         for ovr in overviews {
//             prop_section(
//                 ui,
//                 Some(&format!("overview {}", ovr[0])),
//                 &[
//                     ["x_size".to_string(), ovr[1].to_string()],
//                     ["y_size".to_string(), ovr[2].to_string()],
//                 ],
//             );
//         }
//     }
// }

// pub(crate) fn prop_ui(ui: &mut Ui, value: &str) {
//     if ui.button(RichText::new(value).monospace()).clicked() {
//         ui.ctx().copy_text(value.to_string());
//     }
// }

// pub(crate) fn prop_section(ui: &mut Ui, section_name: Option<&str>, props: &[[String; 2]]) {
//     if let Some(n) = section_name {
//         ui.label(n);
//     }
//     for prop in props {
//         ui.horizontal(|ui| {
//             ui.label(&prop[0]);
//             prop_ui(ui, &prop[1]);
//         });
//     }
// }
