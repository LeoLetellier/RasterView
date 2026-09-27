use egui::{Context, Id};
use std::path::{Path, PathBuf};

use crate::{
    Viewer,
    app::RasterView,
    raster::{GeoTransform, xml_vrt::*},
};

#[derive(Default)]
pub struct VrtFormBuffer {
    pub path_buf: String,
    pub has_ndv: bool,
    pub ndv_buf: f64,
    pub has_crs: bool,
    pub crs_buf: String,
    pub has_geotransform: bool,
    pub geotransform_buf: GeoTransform,
}

impl RasterView {
    pub(crate) fn ui_vrt_form(&mut self, ctx: &Context) {
        if !self.app_state.show_vrt_form {
            return;
        }

        let form_param = &mut self.app_state.vrt_params;
        let buf = &mut self.app_state.vrt_form;
        let mut close_modal = false;
        let mut built: Option<VrtParameters> = None;

        form_param.array_path = PathBuf::from(buf.path_buf.clone());

        egui::Modal::new(Id::new("vrt_form")).show(ctx, |ui| {
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
                            ui.label(form_param.array_path.display().to_string());
                        });
                    ui.end_row();

                    // size
                    ui.label("Size (w × h)");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut form_param.size.0).range(0..=usize::MAX));
                        ui.label("×");
                        ui.add(egui::DragValue::new(&mut form_param.size.1).range(0..=usize::MAX));
                    });
                    ui.end_row();

                    // band_nb
                    ui.label("Band count");
                    ui.add(egui::DragValue::new(&mut form_param.band_nb).range(1..=usize::MAX));
                    ui.end_row();

                    // image_offset
                    ui.label("Image offset (bytes)");
                    ui.add(egui::DragValue::new(&mut form_param.image_offset));
                    ui.end_row();

                    // pixel_offset
                    ui.label("Pixel offset (bytes)");
                    ui.add(egui::DragValue::new(&mut form_param.pixel_offset));
                    ui.end_row();

                    // interleave
                    ui.label("Interleave");
                    egui::ComboBox::from_id_salt("interleave")
                        .selected_text(format!("{:?}", form_param.interleave))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut form_param.interleave,
                                VrtInterleave::Bsq,
                                "BSQ",
                            );
                            ui.selectable_value(
                                &mut form_param.interleave,
                                VrtInterleave::Bip,
                                "BIP",
                            );
                            ui.selectable_value(
                                &mut form_param.interleave,
                                VrtInterleave::Bil,
                                "BIL",
                            );
                        });
                    ui.end_row();

                    // byte_order
                    ui.label("Byte order");
                    egui::ComboBox::from_id_salt("byte_order")
                        .selected_text(form_param.byte_order.as_gdal_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut form_param.byte_order,
                                VrtByteOrder::Lsb,
                                "LSB",
                            );
                            ui.selectable_value(
                                &mut form_param.byte_order,
                                VrtByteOrder::Msb,
                                "MSB",
                            );
                        });
                    ui.end_row();

                    // data_type
                    ui.label("Data type");
                    egui::ComboBox::from_id_salt("data_type")
                        .selected_text(form_param.data_type.as_gdal_str())
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
                                ui.selectable_value(&mut form_param.data_type, dt, label);
                            }
                        });
                    ui.end_row();

                    // ndv — optional
                    ui.label("No-data value");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut buf.has_ndv, "");
                        ui.add_enabled(
                            buf.has_ndv,
                            egui::DragValue::new(&mut buf.ndv_buf).speed(0.1),
                        );
                    });
                    ui.end_row();

                    // crs — optional
                    ui.label("CRS");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut buf.has_crs, "");
                        ui.add_enabled(buf.has_crs, egui::TextEdit::singleline(&mut buf.crs_buf));
                    });
                    ui.end_row();

                    // geotransform — optional
                    ui.label("Geotransform");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut buf.has_geotransform, "");
                    });
                    ui.end_row();

                    ui.add_enabled_ui(buf.has_geotransform, |ui| {
                        egui::Grid::new("geotransform_grid")
                            .num_columns(2)
                            .spacing([6.0, 4.0])
                            .show(ui, |ui| {
                                let fields = [
                                    (
                                        "Origin X",
                                        &mut buf.geotransform_buf.x_off,
                                        "Origin Y",
                                        &mut buf.geotransform_buf.y_off,
                                    ),
                                    (
                                        "Res X",
                                        &mut buf.geotransform_buf.x_res,
                                        "Res Y",
                                        &mut buf.geotransform_buf.y_res,
                                    ),
                                    (
                                        "Rot X",
                                        &mut buf.geotransform_buf.x_rot,
                                        "Rot Y",
                                        &mut buf.geotransform_buf.y_rot,
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
            if form_param.size.0 == 0 || form_param.size.1 == 0 {
                error = Some("Width and height must be non-zero.".into());
            } else if form_param.band_nb == 0 {
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
                    form_param.array_path = PathBuf::from(&buf.path_buf);
                    form_param.ndv = buf.has_ndv.then_some(buf.ndv_buf);
                    form_param.crs = buf.has_crs.then(|| buf.crs_buf.clone());
                    form_param.geotransform = buf.has_geotransform.then_some(buf.geotransform_buf);
                    built = Some(std::mem::take(form_param));
                    close_modal = true;
                }
                if ui.button("Reset").clicked() {
                    *form_param = Default::default();
                    *buf = Default::default();
                    buf.path_buf = form_param.array_path.to_string_lossy().into_owned();
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

        if let Some(params) = built {
            let vrt_xml = simulate_vrt_path(&params);
            // allow GDAL to reach this path on disk from vrt, only for the duration
            if let Ok(_) = allow_raw_source(params.array_path.parent().unwrap_or(Path::new("."))) {
                let viewer = vrt_xml
                    .map_err(anyhow::Error::from)
                    .and_then(|vrt| Viewer::with_raster(vrt.as_path(), ctx.clone()));
                if let Ok(view) = viewer {
                    self.viewer = Some(view);
                    self.raster_path = Some(params.array_path);
                }
            }
        }
        if close_modal {
            self.app_state.show_vrt_form = false;
        }
    }
}
