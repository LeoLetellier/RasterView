use crate::{
    raster::{BandMetadata, RasterHandler},
    viewers::coords::Bbox,
};
use egui::{RichText, Ui};

// TODO struct containing all the displayed metadata to avoid FFI at each frame
impl RasterHandler {
    pub fn ui_dataset(&self, ui: &mut Ui) {
        let metadata = &self.raster_metadata;
        let driver = &metadata.driver;
        let size = metadata.size;
        let band_nb = metadata.band_nb;
        let projection = &metadata.projection;
        let geotransform = &metadata.geotransform;
        let bbox = metadata.bbox;

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
                    ["x ul".to_string(), gt.offsets().x.to_string()],
                    ["x res".to_string(), gt.resolutions().x.to_string()],
                    ["x rot".to_string(), gt.rotations().x.to_string()],
                    ["y ul".to_string(), gt.offsets().y.to_string()],
                    ["x rot".to_string(), gt.rotations().y.to_string()],
                    ["x res".to_string(), gt.resolutions().y.to_string()],
                ],
            );
        }
        if let Some(bb) = bbox {
            prop_section(
                ui,
                Some("BBox"),
                &[
                    ["xmin".to_string(), bb.xmin().to_string()],
                    ["xmax".to_string(), bb.xmax().to_string()],
                    ["ymin".to_string(), bb.ymin().to_string()],
                    ["ymax".to_string(), bb.ymax().to_string()],
                ],
            );
        }
    }

    pub fn ui_bands(&self, ui: &mut Ui) {
        self.raster_metadata.bands.iter().for_each(|b| {
            self.ui_band(&b, ui);
        })
    }

    fn ui_band(&self, band: &BandMetadata, ui: &mut Ui) {
        let dtype = &band.dtype;
        let unit = &band.unit;
        let overviews_nb = band.overview_nb;
        let overviews = &band.overviews;
        let ndv = band.ndv;
        let scale = band.scale;
        let offset = band.offset;

        let mut props = vec![["dtype".to_string(), dtype.clone()]];
        if !unit.is_empty() {
            props.push(["unit".to_string(), unit.clone()]);
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
