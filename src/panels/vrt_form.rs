use eframe::egui::{self, Id, Ui};
use std::path::{Path, PathBuf};

use crate::raster::{GeoTransform, RasterHandler, xml_vrt::*};

#[derive(Default)]
pub(crate) struct VrtRawPanel {
    pub(crate) is_open: bool,
    pub(crate) path_buf: String,
    pub(crate) has_ndv: bool,
    pub(crate) ndv_buf: f64,
    pub(crate) has_crs: bool,
    pub(crate) crs_buf: String,
    pub(crate) has_geotransform: bool,
    pub(crate) geotransform_buf: GeoTransform,
}

impl VrtRawPanel {
    pub(crate) fn ui(
        &mut self,
        ui: &mut Ui,
        vrt_params: &mut VrtParameters,
    ) -> Option<(RasterHandler, PathBuf)> {
        let mut close_modal = false;
        let mut built: Option<VrtParameters> = None;
        let ctx = ui.ctx();

        vrt_params.array_path = PathBuf::from(self.path_buf.clone());

        egui::Modal::new(Id::unique("vrt_form")).show(ctx, |ui| {
            ui.heading("Loading raw file");
            ui.separator();
            ui.label("File could not be opened as GDAL raster.");
            ui.label("Trying to load the file as raw raster, providing dimensions.");
            ui.separator();

            egui::Grid::new("vrt_parameters_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label("Path:");
                    egui::ScrollArea::horizontal()
                        .id_salt("array_path_scroll")
                        .max_width(350.0)
                        .show(ui, |ui| {
                            ui.label(vrt_params.array_path.display().to_string());
                        });
                    ui.end_row();

                    // size
                    ui.label("Size (w × h)");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut vrt_params.size.0).range(0..=usize::MAX));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut vrt_params.size.1).range(0..=usize::MAX));
                    });
                    ui.end_row();

                    // band_nb
                    ui.label("Band count");
                    ui.add(egui::DragValue::new(&mut vrt_params.band_nb).range(1..=usize::MAX));
                    ui.end_row();

                    // image_offset
                    ui.label("Image offset (bytes)");
                    ui.add(egui::DragValue::new(&mut vrt_params.image_offset));
                    ui.end_row();

                    // pixel_offset
                    ui.label("Pixel offset (bytes)");
                    ui.add(egui::DragValue::new(&mut vrt_params.pixel_offset));
                    ui.end_row();

                    // interleave
                    ui.label("Interleave");
                    egui::ComboBox::from_id_salt("interleave")
                        .selected_text(format!("{:?}", vrt_params.interleave))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut vrt_params.interleave,
                                VrtInterleave::Bsq,
                                "BSQ",
                            );
                            ui.selectable_value(
                                &mut vrt_params.interleave,
                                VrtInterleave::Bip,
                                "BIP",
                            );
                            ui.selectable_value(
                                &mut vrt_params.interleave,
                                VrtInterleave::Bil,
                                "BIL",
                            );
                        });
                    ui.end_row();

                    // byte_order
                    ui.label("Byte order");
                    egui::ComboBox::from_id_salt("byte_order")
                        .selected_text(vrt_params.byte_order.as_gdal_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut vrt_params.byte_order,
                                VrtByteOrder::Lsb,
                                "LSB",
                            );
                            ui.selectable_value(
                                &mut vrt_params.byte_order,
                                VrtByteOrder::Msb,
                                "MSB",
                            );
                        });
                    ui.end_row();

                    // data_type
                    ui.label("Data type");
                    egui::ComboBox::from_id_salt("data_type")
                        .selected_text(vrt_params.data_type.as_gdal_str())
                        .show_ui(ui, |ui| {
                            for dt in [
                                VrtDataType::Byte,
                                VrtDataType::UInt16,
                                VrtDataType::Int16,
                                VrtDataType::UInt32,
                                VrtDataType::Int32,
                                VrtDataType::Float32,
                                VrtDataType::Float64,
                                VrtDataType::CFloat32,
                            ] {
                                let label = dt.as_gdal_str();
                                ui.selectable_value(&mut vrt_params.data_type, dt, label);
                            }
                        });
                    ui.end_row();

                    // ndv — optional
                    ui.label("No-data value");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.has_ndv, "");
                        ui.add_enabled(
                            self.has_ndv,
                            egui::DragValue::new(&mut self.ndv_buf).speed(0.1),
                        );
                    });
                    ui.end_row();

                    // crs — optional
                    ui.label("CRS");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.has_crs, "");
                        ui.add_enabled(self.has_crs, egui::TextEdit::singleline(&mut self.crs_buf));
                    });
                    ui.end_row();

                    // geotransform — optional
                    ui.label("Geotransform");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.has_geotransform, "");
                    });
                    ui.end_row();

                    ui.add_enabled_ui(self.has_geotransform, |ui| {
                        egui::Grid::new("geotransform_grid")
                            .num_columns(2)
                            .spacing([6.0, 4.0])
                            .show(ui, |ui| {
                                let fields = [
                                    (
                                        "Origin X",
                                        &mut self.geotransform_buf.x_off,
                                        "Origin Y",
                                        &mut self.geotransform_buf.y_off,
                                    ),
                                    (
                                        "Res X",
                                        &mut self.geotransform_buf.x_res,
                                        "Res Y",
                                        &mut self.geotransform_buf.y_res,
                                    ),
                                    (
                                        "Rot X",
                                        &mut self.geotransform_buf.x_rot,
                                        "Rot Y",
                                        &mut self.geotransform_buf.y_rot,
                                    ),
                                ];
                                for (label_a, val_a, label_b, val_b) in fields {
                                    ui.vertical(|ui| {
                                        ui.small(label_a);
                                        ui.add(egui::DragValue::new(val_a).speed(0.01));
                                    });
                                    ui.vertical(|ui| {
                                        ui.small(label_b);
                                        ui.add(egui::DragValue::new(val_b).speed(0.01));
                                    });
                                    ui.end_row();
                                }
                            });
                    });
                    ui.end_row();
                });

            ui.separator();

            let mut error: Option<String> = None;
            if vrt_params.size.0 == 0 || vrt_params.size.1 == 0 {
                error = Some("Width and height must be non-zero.".into());
            } else if vrt_params.band_nb == 0 {
                error = Some("Band count must be at least 1.".into());
            }
            // crs and geotransform are optional now, so no longer validated here.

            if let Some(err) = &error {
                ui.colored_label(egui::Color32::from_rgb(200, 60, 60), err);
            }

            ui.horizontal(|ui| {
                if ui
                    .add_enabled(error.is_none(), egui::Button::new("Load"))
                    .on_hover_text("Load raw raster with these parameters")
                    .clicked()
                {
                    vrt_params.array_path = PathBuf::from(&self.path_buf);
                    vrt_params.ndv = self.has_ndv.then_some(self.ndv_buf);
                    vrt_params.crs = self.has_crs.then(|| self.crs_buf.clone());
                    vrt_params.geotransform =
                        self.has_geotransform.then_some(self.geotransform_buf);
                    built = Some(std::mem::take(vrt_params));
                    close_modal = true;
                }
                if ui.button("Reset").clicked() {
                    *vrt_params = Default::default();
                    *self = Default::default();
                    self.path_buf = vrt_params.array_path.to_string_lossy().into_owned();
                }
                if ui
                    .button("Cancel")
                    .on_hover_text("Abort file loading")
                    .clicked()
                {
                    close_modal = true;
                }
            });
        });

        let res = if let Some(params) = built {
            let vrt_xml = simulate_vrt_path(&params);
            // allow GDAL to reach this path on disk from vrt, only for the duration
            if allow_raw_source(params.array_path.parent().unwrap_or(Path::new("."))).is_ok() {
                vrt_xml
                    .map_err(anyhow::Error::from)
                    // TODO HARDCODED value for cache
                    .and_then(|vrt| RasterHandler::new(vrt.as_path(), ctx.clone(), 258_000_u64))
                    .ok()
                    .map(|rh| (rh, params.array_path))
            } else {
                None
            }
        } else {
            None
        };
        if close_modal {
            self.is_open = false;
        }

        res
    }
}
