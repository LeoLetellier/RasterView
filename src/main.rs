#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Result, bail};
use egui::{ColorImage, PointerButton, TextureHandle, TextureOptions, vec2};
use egui_plot::{Plot, PlotBounds, PlotImage, PlotPoint};
use ndarray::Array2;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> eframe::Result {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default(),
        ..Default::default()
    };
    eframe::run_native(
        "RasterView",
        native_options,
        Box::new(|cc| Ok(Box::new(RasterView::new(cc.egui_ctx.clone())))),
    )
}

// ── Messages between UI thread and worker ─────────────────────────────────────

/// UI → Worker: load this view (replaces any pending job)
#[derive(Clone)]
struct LoadJob {
    path: PathBuf,
    extent: [usize; 4],
    downsample: usize,
    view: LoadedView,
}

/// Worker → UI: here is the result
struct LoadResult {
    array: Array2<f32>,
    view: LoadedView,
}

// ── LoadedView ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
struct LoadedView {
    bounds: [f64; 4], // xmin, xmax, ymin, ymax in raster pixel coords
    downsample: usize,
}

fn desired_load_params(
    bounds: PlotBounds,
    screen_size: egui::Vec2,
    raster_size: [usize; 2],
) -> LoadedView {
    let [rw, rh] = raster_size;
    let span_x = bounds.max()[0] - bounds.min()[0];
    let span_y = bounds.max()[1] - bounds.min()[1];
    let pad_x = span_x * 0.1;
    let pad_y = span_y * 0.1;
    let xmin = (bounds.min()[0] - pad_x).max(0.0) as usize;
    let xmax = ((bounds.max()[0] + pad_x) as usize).min(rw);
    let ymin = (bounds.min()[1] - pad_y).max(0.0) as usize;
    let ymax = ((bounds.max()[1] + pad_y) as usize).min(rh);
    let px_per_screen = ((xmax - xmin) as f64 / screen_size.x as f64)
        .max((ymax - ymin) as f64 / screen_size.y as f64);
    let downsample = if px_per_screen > 1.0 {
        (px_per_screen.log2().floor() as usize).clamp(0, 6)
    } else {
        0
    };
    LoadedView {
        bounds: [xmin as f64, xmax as f64, ymin as f64, ymax as f64],
        downsample,
    }
}

// ── Worker thread ─────────────────────────────────────────────────────────────
//
// Stays alive for the whole app lifetime.
// Uses a SyncSender with capacity 0 (rendezvous) so the UI thread never blocks:
// it just replaces whatever the worker hasn't started yet.
// We use a shared `Arc<Mutex<Option<LoadJob>>>` as a "latest job" slot instead
// of a channel, so superseded jobs are simply overwritten.

fn spawn_worker(result_tx: Sender<LoadResult>, ctx: egui::Context) -> Arc<Mutex<Option<LoadJob>>> {
    let job_slot: Arc<Mutex<Option<LoadJob>>> = Arc::new(Mutex::new(None));
    let job_slot_worker = Arc::clone(&job_slot);

    thread::spawn(move || {
        loop {
            // Take the latest job, if any
            let job = job_slot_worker.lock().unwrap().take();

            match job {
                None => {
                    // Nothing to do — sleep briefly to avoid busy-spin
                    thread::sleep(std::time::Duration::from_millis(10));
                }
                Some(job) => {
                    let result = read_f32_from_path(&job.path, 1, job.extent, job.downsample);

                    // After the read, check if a newer job has already arrived.
                    // If so, discard this result — it's stale.
                    let superseded = job_slot_worker
                        .lock()
                        .unwrap()
                        .as_ref()
                        .map(|j| j.view != job.view)
                        .unwrap_or(false);

                    if superseded {
                        continue;
                    }

                    if let Ok(array) = result {
                        // result_tx.send won't block because the channel is unbounded
                        // (we use std::sync::mpsc::channel, not sync_channel)
                        let _ = result_tx.send(LoadResult {
                            array,
                            view: job.view,
                        });
                        ctx.request_repaint();
                    }
                }
            }
        }
    });

    job_slot
}

fn read_f32_from_path(
    path: &Path,
    band: usize,
    extent: [usize; 4],
    downsample: usize,
) -> Result<Array2<f32>> {
    let ds = gdal::Dataset::open(path)?;
    let bd = ds.rasterband(band)?;
    let window = (extent[0] as isize, extent[2] as isize);
    let window_size = (extent[1] - extent[0], extent[3] - extent[2]);
    let shape = if downsample > 0 {
        (
            window_size.0.div_euclid(2 * downsample).max(1),
            window_size.1.div_euclid(2 * downsample).max(1),
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
    Ok(buffer.to_array()?)
}

// ── RasterHandler ─────────────────────────────────────────────────────────────

struct RasterHandler {
    path: Option<PathBuf>,
    dataset: Option<gdal::Dataset>,
    dataset_properties: Option<DatasetProperties>,
    band_properties: Vec<BandProperties>,
    image_texture: Option<TextureHandle>,
    loaded_view: Option<LoadedView>,
    view_dirty: bool,
    /// Slot shared with the worker — write a job here to request a load
    job_slot: Arc<Mutex<Option<LoadJob>>>,
    /// Results coming back from the worker
    result_rx: Receiver<LoadResult>,
    /// Track what we last requested to avoid redundant writes
    last_requested: Option<LoadedView>,
}

impl RasterHandler {
    fn new(ctx: egui::Context) -> Self {
        let (result_tx, result_rx) = mpsc::channel::<LoadResult>();
        let job_slot = spawn_worker(result_tx, ctx);
        Self {
            path: None,
            dataset: None,
            dataset_properties: None,
            band_properties: Vec::new(),
            image_texture: None,
            loaded_view: None,
            view_dirty: false,
            job_slot,
            result_rx,
            last_requested: None,
        }
    }

    fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    fn update_dataset(&mut self) -> bool {
        if let Some(path) = self.path() {
            if let Ok(ds) = gdal::Dataset::open(path) {
                self.dataset_properties = Some(DatasetProperties::from_dataset(&ds));
                self.band_properties.clear();
                for b in ds.rasterbands() {
                    if let Ok(b) = b {
                        self.band_properties
                            .push(BandProperties::from_rasterband(&b));
                    }
                }
                self.dataset = Some(ds);
                self.view_dirty = true;
                self.loaded_view = None;
                self.image_texture = None;
                self.last_requested = None;
                // Flush any pending job for the old file
                if let Ok(mut g) = self.job_slot.lock() {
                    *g = None;
                }
                return true;
            }
        }
        false
    }

    /// Post a new job to the worker. No-op if the desired view hasn't changed.
    fn request_load(&mut self, desired: LoadedView) {
        if self.last_requested.as_ref() == Some(&desired) {
            return;
        }
        let Some(path) = self.path.clone() else {
            return;
        };
        let Some(ds) = &self.dataset else { return };
        let (rw, rh) = ds.raster_size();

        let xmin = (desired.bounds[0] as usize).min(rw);
        let xmax = (desired.bounds[1] as usize).min(rw);
        let ymin = (desired.bounds[2] as usize).min(rh);
        let ymax = (desired.bounds[3] as usize).min(rh);

        let job = LoadJob {
            path,
            extent: [xmin, xmax, ymin, ymax],
            downsample: desired.downsample,
            view: desired.clone(),
        };

        // Overwrite — worker will pick up the latest job next iteration
        if let Ok(mut g) = self.job_slot.lock() {
            *g = Some(job);
        }
        self.last_requested = Some(desired);
    }

    /// Upload texture if the worker finished. Call once per frame on UI thread.
    fn poll_result(&mut self, ctx: &egui::Context) {
        // Drain all pending results, keep only the last (most recent)
        let mut latest: Option<LoadResult> = None;
        while let Ok(res) = self.result_rx.try_recv() {
            latest = Some(res);
        }
        if let Some(res) = latest {
            self.image_texture = Some(array2_to_texture(&res.array, "raster texture", ctx));
            self.loaded_view = Some(res.view);
            self.view_dirty = false;
        }
    }
}

fn array2_to_texture(arr: &Array2<f32>, name: &str, ctx: &egui::Context) -> TextureHandle {
    let (rows, cols) = arr.dim();
    let min = arr.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = arr.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = (max - min).max(f32::EPSILON);
    let pixels: Vec<u8> = arr
        .iter()
        .flat_map(|&v| {
            let g = ((v - min) / range * 255.0) as u8;
            [g, g, g, 255]
        })
        .collect();
    ctx.load_texture(
        name,
        ColorImage::from_rgba_unmultiplied([cols, rows], &pixels),
        TextureOptions::LINEAR,
    )
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct RasterView {
    raster: RasterHandler,
    view_mode: ViewMode,
}

impl RasterView {
    fn new(ctx: egui::Context) -> Self {
        Self {
            raster: RasterHandler::new(ctx),
            view_mode: ViewMode::default(),
        }
    }
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
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
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
                    }
                });
            });
        });

        egui::Panel::bottom("bottom panel").show_inside(ui, |_ui| {});

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
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("File:");
                    if let Some(path) = self.raster.path() {
                        egui::ScrollArea::horizontal()
                            .id_salt("file scroll")
                            .max_width(ui.available_width() - 50.0)
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
                });

                if let Some(ds) = &self.raster.dataset {
                    ui.heading("Dataset");
                    ui.separator();
                    if let Some(dp) = &self.raster.dataset_properties {
                        egui::ScrollArea::horizontal()
                            .id_salt("dataset scroll")
                            .show(ui, |ui| dp.ui(ui));
                    }
                    for i in 0..ds.raster_count() {
                        ui.collapsing(format!("Band {}", i + 1), |ui| {
                            self.raster.band_properties[i].ui(ui);
                        });
                    }
                }
            });

        egui::Panel::right("right panel").show_inside(ui, |_ui| {});

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if let Some(ds) = &self.raster.dataset {
                let (rw, rh) = ds.raster_size();
                let available = ui.available_size();

                // 1. Upload texture if worker finished
                self.raster.poll_result(ui.ctx());

                // 2. Draw plot
                let mut new_bounds: Option<PlotBounds> = None;

                Plot::new("Main plot")
                    .data_aspect(1.0)
                    .pan_pointer_button(PointerButton::Primary)
                    .boxed_zoom_pointer_button(PointerButton::Secondary)
                    .allow_scroll(false)
                    .allow_zoom(true)
                    .allow_axis_zoom_drag(true)
                    .default_x_bounds(0.0, rw as f64)
                    .default_y_bounds(0.0, rh as f64)
                    .grid_spacing(egui::Rangef::new(150.0, f32::INFINITY))
                    .show(ui, |plot_ui| {
                        new_bounds = Some(plot_ui.plot_bounds());
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

                // 3. Request new load if view changed enough
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
                        self.raster.request_load(desired);
                    }
                }
            }
        });
    }
}

// ── DatasetProperties ─────────────────────────────────────────────────────────

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
        let geotransform = dataset.geo_transform().ok();
        let bbox = geotransform.map(|gt| {
            [
                gt[0],
                gt[0] + (size.0 as f64) * gt[1],
                gt[3] + (size.1 as f64) * gt[5],
                gt[3],
            ]
        });
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
        prop_section(ui, None, &[["Driver".to_string(), self.driver.clone()]]);
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
            &[["Projection".to_string(), self.projection.clone()]],
        );
        if let Some(gt) = self.geotransform {
            prop_section(
                ui,
                None,
                &[[
                    "Geotransform".to_string(),
                    format!(
                        "x_ul:{} x_res:{} x_rot:{} y_ul:{} y_rot:{} y_res:{}",
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
                    format!("x({:.1}:{:.1}) y({:.1}:{:.1})", bb[0], bb[1], bb[2], bb[3]),
                ]],
            );
        }
    }
}

// ── BandProperties ────────────────────────────────────────────────────────────

struct BandProperties {
    dtype: String,
    unit: Option<String>,
    ndv: Option<f64>,
    scale: Option<f64>,
    offset: Option<f64>,
    overviews: Vec<[usize; 3]>,
}

impl BandProperties {
    fn from_rasterband(band: &gdal::raster::RasterBand) -> Self {
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
        Self {
            dtype,
            unit,
            ndv: band.no_data_value(),
            scale: band.scale(),
            offset: band.offset(),
            overviews,
        }
    }

    fn ui(&self, ui: &mut egui::Ui) {
        let mut props = vec![["dtype".to_string(), self.dtype.clone()]];
        if let Some(v) = &self.unit {
            props.push(["unit".to_string(), v.clone()]);
        }
        if let Some(v) = &self.ndv {
            props.push(["ndv".to_string(), v.to_string()]);
        }
        if let Some(v) = &self.scale {
            props.push(["scale".to_string(), v.to_string()]);
        }
        if let Some(v) = &self.offset {
            props.push(["offset".to_string(), v.to_string()]);
        }
        prop_section(ui, Some("Data"), &props);
        for ovr in &self.overviews {
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

// ── UI helpers ────────────────────────────────────────────────────────────────

fn prop_ui(ui: &mut egui::Ui, value: &str) {
    if ui.button(egui::RichText::new(value).monospace()).clicked() {
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
