#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Result, bail};
use egui::{ColorImage, PointerButton, TextureHandle, TextureOptions, Ui, vec2};
use egui_plot::{Plot, PlotBounds, PlotImage, PlotPoint};
use ndarray::Array2;
use std::path::{Path, PathBuf};

fn main() -> eframe::Result {
    env_logger::init();

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };
    eframe::run_native(
        "RasterView",
        native_options,
        Box::new(|cc| Ok(Box::<RasterView>::default())),
    )
}

pub struct RasterView {
    raster: RasterHandler,
    view_mode: ViewMode,
}

impl Default for RasterView {
    fn default() -> Self {
        Self {
            raster: RasterHandler::default(),
            view_mode: ViewMode::default(),
        }
    }
}

struct RasterHandler {
    path: Option<PathBuf>,
    dataset: Option<gdal::Dataset>,
    dataset_properties: Option<DatasetProperties>,
    band_properties: Vec<BandProperties>,
    image_texture: Option<TextureHandle>,
    loaded_view: Option<LoadedView>,
    view_dirty: bool,
}

impl Default for RasterHandler {
    fn default() -> Self {
        Self {
            path: None,
            dataset: None,
            dataset_properties: None,
            band_properties: Vec::default(),
            image_texture: None,
            loaded_view: None,
            view_dirty: false,
        }
    }
}

/// Tracks what was last loaded so we can detect when a reload is needed.
#[derive(Debug, Clone, PartialEq)]
struct LoadedView {
    bounds: [f64; 4], // xmin, xmax, ymin, ymax in plot/raster coords
    downsample: usize,
}

/// What window + downsample to request for the current view.
fn desired_load_params(
    bounds: PlotBounds,
    screen_size: egui::Vec2,
    raster_size: [usize; 2],
) -> LoadedView {
    let [rw, rh] = raster_size;

    // Add 10% overdraw on each side so panning doesn't immediately show blank
    let span_x = bounds.max()[0] - bounds.min()[0];
    let span_y = bounds.max()[1] - bounds.min()[1];
    let pad_x = span_x * 0.1;
    let pad_y = span_y * 0.1;

    let xmin = (bounds.min()[0] - pad_x).max(0.0) as usize;
    let xmax = ((bounds.max()[0] + pad_x) as usize).min(rw);
    let ymin = (bounds.min()[1] - pad_y).max(0.0) as usize;
    let ymax = ((bounds.max()[1] + pad_y) as usize).min(rh);

    // How many raster pixels per screen pixel?
    let px_per_screen_x = (xmax - xmin) as f64 / screen_size.x as f64;
    let px_per_screen_y = (ymax - ymin) as f64 / screen_size.y as f64;
    let px_per_screen = px_per_screen_x.max(px_per_screen_y);

    // Downsample = floor(log2(px_per_screen)), clamped to [0, 6]
    let downsample = (px_per_screen.log2().floor() as usize).clamp(0, 6);

    LoadedView {
        bounds: [xmin as f64, xmax as f64, ymin as f64, ymax as f64],
        downsample,
    }
}

impl RasterHandler {
    fn with_path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.path = Some(path.as_ref().to_path_buf());
        self
    }

    fn has_path(&self) -> bool {
        self.path.is_some()
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn update_dataset(&mut self) -> bool {
        if let Some(path) = self.path() {
            if let Ok(ds) = gdal::Dataset::open(path) {
                self.dataset_properties = Some(DatasetProperties::from_dataset(&ds));
                for b in ds.rasterbands() {
                    if let Ok(b) = b {
                        self.band_properties
                            .push(BandProperties::from_rasterband(&b));
                    }
                }
                self.dataset = Some(ds);
                self.view_dirty = true;
                return true;
            }
        }
        false
    }

    fn fetch_texture(&mut self, ui: &mut Ui) {
        if let Some(ds) = &self.dataset {
            let extent = [0, ds.raster_size().0, 0, ds.raster_size().1];
            let array = self.read_f32(1, extent, 0);
            if let Ok(arr) = array {
                let texture = array2_to_texture(&arr, "raster texture", ui);
                self.image_texture = Some(texture);
            }
        }
    }

    fn load_view(&mut self, view: LoadedView, ui: &mut Ui, raster_size: [usize; 2]) -> Result<()> {
        if let Some(ds) = &self.dataset {
            let (rw, rh) = ds.raster_size();
            let xmin = (view.bounds[0] as usize).min(rw);
            let xmax = (view.bounds[1] as usize).min(rw);
            let ymin = (view.bounds[2] as usize).min(rh);
            let ymax = (view.bounds[3] as usize).min(rh);

            // extent = [xmin, xmax, ymin, ymax]
            let array = self.read_f32(1, [xmin, xmax, ymin, ymax], view.downsample);
            if let Ok(arr) = array {
                let texture = array2_to_texture(&arr, "raster texture", ui);
                self.image_texture = Some(texture);
                return Ok(());
            }
        }
        bail!("failed to load into view")
    }

    /// Read the data into a f32 array
    ///
    /// * band is the band to read
    /// * extent define in pixel the portion to read
    /// * downsample is a power of two that decides how to downsample the data at reading using Nearest Neighbors.
    /// set to 0 for full res
    fn read_f32(
        &self,
        band: usize,
        extent: [usize; 4],
        downsample: usize,
    ) -> Result<ndarray::Array2<f32>> {
        if let Some(ds) = &self.dataset {
            let bd = ds.rasterband(band)?;
            let window = (extent[0] as isize, extent[2] as isize);
            let window_size = (extent[1] - extent[0], extent[3] - extent[2]);
            let shape = if downsample > 0 {
                (
                    window_size.0.div_euclid(2 * downsample),
                    window_size.1.div_euclid(2 * downsample),
                )
            } else {
                window_size
            };
            let buffer = bd.read_as::<f32>(
                window,
                window_size,
                shape,
                Some(gdal::raster::ResampleAlg::NearestNeighbour),
            )?;
            let array = buffer.to_array()?;
            Ok(array)
        } else {
            bail!("No valid dataset available")
        }
    }
}

fn array2_to_texture(arr: &Array2<f32>, name: &str, ui: &mut Ui) -> TextureHandle {
    let (rows, cols) = arr.dim();

    let min = arr.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = arr.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min).max(f32::EPSILON);

    // Build RGBA pixels (egui expects [r,g,b,a, r,g,b,a, ...])
    let pixels: Vec<u8> = arr
        .iter()
        .flat_map(|&v| {
            let grey = ((v - min) / range * 255.0) as u8;
            [grey, grey, grey, 255]
        })
        .collect();

    let image = ColorImage::from_rgba_unmultiplied([cols, rows], &pixels);
    ui.ctx().load_texture(name, image, TextureOptions::LINEAR)
}

enum ViewMode {
    Panchromatic,
    Color,
    Ratio,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self::Panchromatic
    }
}

impl eframe::App for RasterView {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Top panel for Menus
        egui::Panel::top("top panel").show_inside(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open..").clicked() {
                        self.raster.path = rfd::FileDialog::new().pick_file();
                        self.raster.update_dataset();
                    }
                });
                ui.menu_button("Views", |ui| {});

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Refresh").clicked() {
                        self.raster.update_dataset();
                        self.raster.fetch_texture(ui);
                    }
                });
            });
        });

        // Bottom panels
        egui::Panel::bottom("bottom panel").show_inside(ui, |ui| {});

        // Left panel for info on raster
        egui::Panel::left("left panel")
            .size_range(100.0..=ui.ctx().content_rect().width() * 0.33)
            .show_inside(ui, |ui| {
                ui.heading("Raster Information");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Path:");
                    if let Some(path) = self.raster.path() {
                        egui::ScrollArea::horizontal()
                            .id_salt("path scroll")
                            .show(ui, |ui| {
                                if ui.monospace(path.display().to_string()).clicked() {
                                    ui.ctx().copy_text(path.display().to_string());
                                }
                            });
                    } else {
                        ui.monospace("None");
                    };
                });

                // Show info about project
                ui.horizontal(|ui| {
                    ui.label("File:");
                    if let Some(path) = self.raster.path() {
                        egui::ScrollArea::horizontal()
                            .id_salt("file scroll")
                            .max_width(ui.available_width() - 50.0)
                            .show(ui, |ui| {
                                if ui
                                    .monospace(
                                        path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Unknown"),
                                    )
                                    .clicked()
                                {
                                    ui.ctx().copy_text(
                                        path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Unknown")
                                            .to_string(),
                                    );
                                }
                            });
                    } else {
                        ui.monospace("None");
                    };
                });

                // Show info global to GDAL dataset
                if let Some(ds) = &self.raster.dataset {
                    ui.heading("Dataset");
                    ui.separator();

                    if let Some(dp) = &self.raster.dataset_properties {
                        egui::ScrollArea::horizontal()
                            .id_salt("dataset scroll")
                            .show(ui, |ui| {
                                dp.ui(ui);
                            });
                    }

                    // Show info relative to each bands
                    for i in 0..ds.raster_count() {
                        ui.collapsing(format!("Band {}", i + 1), |ui| {
                            self.raster.band_properties[i].ui(ui);
                        });
                    }
                }
            });

        // Right panel for viewer parameters
        egui::Panel::right("right panel").show_inside(ui, |ui| {});

        // Viewer
        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(ds) = &self.raster.dataset {
                let (rw, rh) = ds.raster_size();
                let available = ui.available_size();
                let mut new_bounds: Option<PlotBounds> = None; // mut

                Plot::new("Main plot")
                    .data_aspect(1.0)
                    .pan_pointer_button(PointerButton::Primary)
                    .boxed_zoom_pointer_button(PointerButton::Secondary)
                    .allow_scroll(false)
                    .allow_zoom(true)
                    .allow_axis_zoom_drag(true)
                    .default_x_bounds(0.0, rw as f64) // init bounds on first frame
                    .default_y_bounds(0.0, rh as f64)
                    .show(ui, |plot_ui| {
                        new_bounds = Some(plot_ui.plot_bounds()); // assign here

                        if let (Some(view), Some(tex)) =
                            (&self.raster.loaded_view, &self.raster.image_texture)
                        {
                            let cx = (view.bounds[0] + view.bounds[1]) / 2.0;
                            let cy = (view.bounds[2] + view.bounds[3]) / 2.0;
                            let w = view.bounds[1] - view.bounds[0];
                            let h = view.bounds[3] - view.bounds[2];
                            plot_ui.image(
                                PlotImage::new(
                                    "raster",
                                    tex.id(),
                                    PlotPoint::new(cx, cy),
                                    vec2(w as f32, h as f32),
                                )
                                .uv(egui::Rect::from_min_max(
                                    egui::pos2(0.0, 1.0),
                                    egui::pos2(1.0, 0.0),
                                )),
                            );
                        }
                    });

                if let Some(bounds) = new_bounds {
                    let desired = desired_load_params(bounds, available, [rw, rh]);
                    let needs_reload = self.raster.view_dirty
                        || self.raster.loaded_view.as_ref().map_or(true, |v| {
                            let dx = (v.bounds[0] - desired.bounds[0]).abs();
                            let dy = (v.bounds[2] - desired.bounds[2]).abs();
                            let span_x = desired.bounds[1] - desired.bounds[0];
                            let span_y = desired.bounds[3] - desired.bounds[2];
                            v.downsample != desired.downsample
                                || dx > span_x * 0.2
                                || dy > span_y * 0.2
                        });

                    if needs_reload {
                        if let Ok(()) = self.raster.load_view(desired.clone(), ui, [rw, rh]) {
                            self.raster.loaded_view = Some(desired);
                            self.raster.view_dirty = false;
                        }
                    }
                }
            }
        });
    }
}

struct DatasetProperties {
    driver: String,
    size: (usize, usize),
    band_nb: usize,
    projection: String,
    geotransform: Option<[f64; 6]>,
    bbox: Option<[f64; 4]>,
}

impl DatasetProperties {
    fn from_dataset(dataset: &gdal::Dataset) -> Self {
        let driver = dataset.driver().short_name();
        let size = dataset.raster_size();
        let band_nb = dataset.raster_count();
        let projection = dataset.projection();
        let geotransform = dataset.geo_transform().map_or(None, |gt| Some(gt));
        let bbox = if let Some(gt) = geotransform {
            Some([
                gt[0],
                gt[0] + (size.0 as f64) * gt[1],
                gt[3] + (size.1 as f64) * gt[5],
                gt[3],
            ])
        } else {
            None
        };

        Self {
            driver,
            size,
            band_nb,
            projection,
            geotransform,
            bbox,
        }
    }

    fn ui(&self, ui: &mut egui::Ui) {
        prop_section(ui, None, &[["Driver".to_string(), self.driver.to_string()]]);
        prop_section(
            ui,
            Some("Size"),
            &[
                ["x".to_string(), self.size.0.to_string()],
                ["y".to_string(), self.size.1.to_string()],
            ],
        );
        prop_section(
            ui,
            None,
            &[["Band nb".to_string(), self.band_nb.to_string()]],
        );
        prop_section(
            ui,
            None,
            &[["Projection".to_string(), self.projection.to_string()]],
        );
        if let Some(gt) = self.geotransform {
            prop_section(
                ui,
                None,
                &[[
                    "Geotransform".to_string(),
                    format!(
                        "x_ul: {} ; x_res: {} ; x_rot: {} ; y_ul: {} ; y_rot: {} ; y_res: {}",
                        gt[0], gt[1], gt[2], gt[3], gt[4], gt[5]
                    ),
                ]],
            );
        }

        if let Some(bb) = self.bbox {
            prop_section(
                ui,
                None,
                &[[
                    "Bbox".to_string(),
                    format!("x ({} : {}) ; y ({} : {})", bb[0], bb[1], bb[2], bb[3]),
                ]],
            );
        }
    }
}

struct BandProperties {
    dtype: String,
    unit: Option<String>,
    ndv: Option<f64>,
    scale: Option<f64>,
    offset: Option<f64>,
    /// ovr index, size x, size y
    overviews: Vec<[usize; 3]>,
}

impl BandProperties {
    fn from_rasterband(band: &gdal::raster::RasterBand) -> Self {
        let dtype = band.band_type().name();
        let unit = band.unit();
        // Dont care to get it if empty
        let unit = if unit.is_empty() { None } else { Some(unit) };
        let ndv = band.no_data_value();
        let scale = band.scale();
        let offset = band.offset();
        let overviews_nb = band.overview_count().unwrap_or(0) as usize;
        let mut overviews = vec![];
        for k in 0..overviews_nb {
            let ovr = band.overview(k);
            if let Ok(o) = ovr {
                let size = o.size();
                overviews.push([k, size.0, size.1]);
            }
        }

        Self {
            dtype,
            unit,
            ndv,
            scale,
            offset,
            overviews,
        }
    }

    fn ui(&self, ui: &mut egui::Ui) {
        let mut props = vec![["dtype".to_string(), self.dtype.to_string()]];
        if let Some(unit) = &self.unit {
            props.push(["unit".to_string(), unit.to_string()]);
        }
        if let Some(ndv) = &self.ndv {
            props.push(["unit".to_string(), ndv.to_string()]);
        }
        if let Some(scale) = &self.scale {
            props.push(["unit".to_string(), scale.to_string()]);
        }
        if let Some(offset) = &self.offset {
            props.push(["unit".to_string(), offset.to_string()]);
        }
        prop_section(ui, Some("Data"), &props);

        for ovr in &self.overviews {
            let mut props = vec![];
            props.push(["x_size".to_string(), ovr[1].to_string()]);
            props.push(["y_size".to_string(), ovr[2].to_string()]);
            prop_section(
                ui,
                Some(&format!("overview {}", ovr[0].to_string())),
                &props,
            );
        }
    }
}

fn prop_ui(ui: &mut egui::Ui, value: &String) {
    if ui
        .button(egui::RichText::new(value.to_string()).monospace())
        .clicked()
    {
        ui.ctx().copy_text(value.to_string());
    }
}

fn prop_section(ui: &mut egui::Ui, section_name: Option<&str>, props: &[[String; 2]]) {
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
