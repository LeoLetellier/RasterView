use crate::raster::source_metadata::SourceMetadata;
use crate::widgets::info_card::{InfoCard, InfoCardGrid};
use crate::widgets::inspector::{Field, Inspector};
use crate::{RasterView, raster::BandMetadata, viewers::coords::Bbox};
use egui::{Color32, RichText, Ui};
use std::time::{SystemTime, UNIX_EPOCH};

impl RasterView {
    pub(crate) fn ui_metadata_panel(&mut self, ui: &mut Ui) {
        ui.heading("Raster Information");
        ui.separator();
        self.ui_source(ui);
        self.ui_dataset(ui);
        self.ui_bands(ui);
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

            InfoCardGrid::new().max_cols(4).show(
                ui,
                vec![
                    InfoCard::new("Driver", driver).tint(Color32::LIGHT_BLUE),
                    InfoCard::new("Bands", band_nb),
                    InfoCard::new("Width", size.0).unit("px"),
                    InfoCard::new("Height", size.1).unit("px"),
                ],
            );

            ui.separator();

            let gt = geotransform;
            let offsets = gt.offsets();
            let resolutions = gt.resolutions();
            let rotations = gt.rotations();

            Inspector::new("raster_inspector")
                .label_width(0.35)
                .show(ui, |ins| {
                    ins.section("Description", |ins| ins.description(&metadata.description));

                    ins.section("Dimensions", |ins| {
                        ins.text("Width (X)", size.0);
                        ins.text("Height (Y)", size.1);
                    });

                    ins.section("Projection", |ins| {
                        ins.text("CRS", projection.to_string());
                    });

                    ins.section("Geotransform", |ins| {
                        ins.header_row(&["X", "Y"]);
                        ins.props("Offset", [Field::value(offsets.x), Field::value(offsets.y)]);
                        ins.props(
                            "Resolution",
                            [Field::value(resolutions.x), Field::value(resolutions.y)],
                        );
                        ins.props(
                            "Rotation",
                            [Field::value(rotations.x), Field::value(rotations.y)],
                        );
                    });

                    if let Some(bb) = bbox {
                        ins.section("Bounding Box", |ins| {
                            ins.props(
                                "X",
                                [
                                    Field::value(format_decimal(bb.xmin())).named("Min"),
                                    Field::value(format_decimal(bb.xmax())).named("Max"),
                                ],
                            );
                            ins.props(
                                "Y",
                                [
                                    Field::value(format_decimal(bb.ymin())).named("Min"),
                                    Field::value(format_decimal(bb.ymax())).named("Max"),
                                ],
                            );
                        });
                    }
                });
        });
    }

    fn ui_source(&self, ui: &mut Ui) {
        let Some(view) = &self.viewer else {
            return;
        };
        let rh = &view.raster_handler;
        let src = &rh.src_metadata;

        Inspector::new("raster_info")
            .label_width(0.22)
            .min_label_width(70.0)
            .monospace_values(true)
            .show(ui, |ins| match &src {
                SourceMetadata::File(f) => {
                    ins.prop("File", Field::value(&f.name).tooltip(f.name.clone()));
                    ins.text("Extension", f.extension.as_deref().unwrap_or("-"));
                    ins.prop(
                        "Size",
                        Field::value(human_size(f.size)).tooltip(format!("{} bytes", f.size)),
                    );

                    ins.section("Attributes", |ins| {
                        ins.flag("Symlink", f.is_symlink);
                        ins.flag("Read-only", f.readonly);
                        ins.flag("Hidden", f.hidden);
                        if let Some(m) = f.mode {
                            ins.text("Mode", format!("{m:03o}"));
                        }
                    });

                    ins.section("Timestamps", |ins| {
                        ins.text("Modified", fmt_time(f.modified));
                        ins.text("Created", fmt_time(f.created));
                        ins.text("Accessed", fmt_time(f.accessed));
                    });
                }
                SourceMetadata::Vrt(v) => {
                    ins.text("Source", "In-memory VRT");
                    ins.text("VSI path", &v.vsi_path);
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
        Inspector::new("band_info")
            .label_width(0.35)
            .min_label_width(80.0)
            .monospace_values(true)
            .show(ui, |ins| {
                if !band.description.is_empty() {
                    ins.text("Description", &band.description);
                }

                ins.text("Data Type", &band.dtype);

                if !band.unit.is_empty() {
                    ins.text("Unit", &band.unit);
                }
                if let Some(v) = band.ndv {
                    ins.text("No Data Value", v);
                }
                if let Some(v) = band.scale {
                    ins.text("Scale", v);
                }
                if let Some(v) = band.offset {
                    ins.text("Offset", v);
                }

                if !band.overviews.is_empty() {
                    ins.section("Overviews", |ins| {
                        ins.header_row(&["Width", "Height"]);
                        for ovr in &band.overviews {
                            ins.props(
                                format!("Level {}", ovr[0]),
                                [
                                    Field::value(ovr[1]).suffix(" px"),
                                    Field::value(ovr[2]).suffix(" px"),
                                ],
                            );
                        }
                    });
                }
            });
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", UNITS[i])
    }
}

/// UTC timestamp as `YYYY-MM-DD HH:MM:SS UTC`, or "n/a" when the platform has none.
fn fmt_time(t: Option<SystemTime>) -> String {
    let Some(t) = t else { return "n/a".into() };
    let secs = match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    };
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);

    // Days since epoch -> civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);

    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
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
