use crate::raster::RasterHandler;
use anyhow::Result;
use egui::{RichText, Ui};
use gdal::raster::RasterBand;

impl RasterHandler {
    fn ui_dataset(&self, ui: &mut Ui) {
        let dataset = &self.0;
        let driver = dataset.driver().short_name();
        let size = dataset.raster_size();
        let band_nb = dataset.raster_count();
        let projection = dataset.projection();
        let geotransform = dataset.geo_transform().ok();
        let bbox = geotransform.map(|gt| {
            [
                gt[0],
                gt[0] + (size.0 as f64) * gt[1],
                gt[3] + (size.1 as f64) * gt[5],
                gt[3],
            ]
        });

        prop_section(ui, None, &[["Driver".to_string(), driver.clone()]]);
        prop_section(
            ui,
            Some("Size"),
            &[
                ["x".to_string(), size.0.to_string()],
                ["y".to_string(), size.1.to_string()],
            ],
        );
        prop_section(ui, None, &[["Band nb".to_string(), band_nb.to_string()]]);
        prop_section(ui, None, &[["Projection".to_string(), projection.clone()]]);
        if let Some(gt) = geotransform {
            prop_section(
                ui,
                Some("Geotransform"),
                &[
                    ["x ul".to_string(), gt[0].to_string()],
                    ["x res".to_string(), gt[1].to_string()],
                    ["x rot".to_string(), gt[2].to_string()],
                    ["y ul".to_string(), gt[3].to_string()],
                    ["x rot".to_string(), gt[4].to_string()],
                    ["x res".to_string(), gt[5].to_string()],
                ],
            );
        }
        if let Some(bb) = bbox {
            prop_section(
                ui,
                Some("BBox"),
                &[
                    ["xmin".to_string(), bb[0].to_string()],
                    ["xmax".to_string(), bb[1].to_string()],
                    ["ymin".to_string(), bb[2].to_string()],
                    ["ymax".to_string(), bb[3].to_string()],
                ],
            );
        }
    }

    fn ui_bands(&self, ui: &mut Ui) -> Result<()> {
        self.0.rasterbands().try_for_each(|b| {
            self.ui_band(&b?, ui);
            Ok(())
        })
    }

    fn ui_band(&self, band: &RasterBand, ui: &mut Ui) {
        let dtype = band.band_type().name();
        let unit = band.unit();
        let unit = if unit.is_empty() { None } else { Some(unit) };
        let overviews_nb = band.overview_count().unwrap_or(0) as usize;
        let mut overviews = vec![];
        for k in 0..overviews_nb {
            if let Ok(o) = band.overview(k) {
                let s = o.size();
                overviews.push([k, s.0, s.1]);
            }
        }
        let ndv = band.no_data_value();
        let scale = band.scale();
        let offset = band.offset();

        let mut props = vec![["dtype".to_string(), dtype.clone()]];
        if let Some(v) = unit {
            props.push(["unit".to_string(), v.clone()]);
        }
        if let Some(v) = ndv {
            props.push(["ndv".to_string(), v.to_string()]);
        }
        if let Some(v) = scale {
            props.push(["scale".to_string(), v.to_string()]);
        }
        if let Some(v) = offset {
            props.push(["offset".to_string(), v.to_string()]);
        }
        prop_section(ui, Some("Data"), &props);
        for ovr in overviews {
            prop_section(
                ui,
                Some(&format!("overview {}", ovr[0])),
                &[
                    ["x_size".to_string(), ovr[1].to_string()],
                    ["y_size".to_string(), ovr[2].to_string()],
                ],
            );
        }
    }
}

pub fn prop_ui(ui: &mut Ui, value: &str) {
    if ui.button(RichText::new(value).monospace()).clicked() {
        ui.ctx().copy_text(value.to_string());
    }
}

pub fn prop_section(ui: &mut Ui, section_name: Option<&str>, props: &[[String; 2]]) {
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
