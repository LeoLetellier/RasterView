use crate::{raster::RasterHandler, texture_thread::TextureWorker};
use anyhow::Result;
use egui::{ColorImage, TextureOptions};
use egui::{TextureHandle, Ui, vec2};
use egui_plot::PlotBounds;
use egui_plot::{Plot, PlotImage, PlotPoint};
use ndarray::Array2;
use std::path::{Path, PathBuf};

/// Image viewer composition
pub enum ViewMode {
    /// Display one image
    Single(ColorInterpretation),
    /// Display one image using Walkers
    SingleWeb(ColorInterpretation),
    /// Display two images blending with an alpha value
    Alpha2(ColorInterpretation, ColorInterpretation),
    /// Display two images transitionning with a slider
    Slide2(ColorInterpretation, ColorInterpretation),
    // /// Display two images side by side
    // Synchro2(ColorInterpretation, ColorInterpretation),
}

impl ViewMode {
    /// All central panel viewer goes in this one
    pub fn ui(&mut self, ui: &mut Ui, texture_worker: &mut TextureWorker) {
        match self {
            ViewMode::Single(ci) => {
                // 1. Poll for new texture
                ci.try_update_texture(texture_worker);

                // 2. Draw plot, capture bounds
                let raster_size = ci.raster_size();
                let (rw, rh) = raster_size;
                let available = ui.available_size();
                let mut new_bounds: Option<PlotBounds> = None;

                Plot::new("main_plot")
                    .data_aspect(1.0)
                    .pan_pointer_button(egui::PointerButton::Primary)
                    .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
                    .allow_scroll(false)
                    .allow_zoom(true)
                    .default_x_bounds(0.0, rw as f64)
                    .default_y_bounds(0.0, rh as f64)
                    .show(ui, |plot_ui| {
                        new_bounds = Some(plot_ui.plot_bounds());

                        if let Some(handle) = ci.handle() {
                            let [xmin, xmax, ymin, ymax] = handle.offset_bounds();
                            let cx = (xmin + xmax) as f64 / 2.0;
                            let cy = (ymin + ymax) as f64 / 2.0;
                            let w = (xmax - xmin) as f32;
                            let h = (ymax - ymin) as f32;
                            plot_ui.image(PlotImage::new(
                                "raster",
                                handle.texture_handle.id(),
                                PlotPoint::new(cx, cy),
                                vec2(w, h),
                            ));
                        }
                    });

                // 3. Check if a new load is needed
                if let Some(bounds) = new_bounds {
                    let opts = ReadOptions::from_plot_bounds(1, bounds, available, raster_size);
                    if ci.needs_reload(&opts) {
                        let worker: ColorInterpretationWorker = ci.to_worker_with_opts(opts);
                        let _ = texture_worker.request_load(worker);
                    }
                }
            }
            _ => {
                ui.label("not done yet");
            }
        }
    }

    /// Poll for new textures and update handles
    pub fn try_update_texture(&mut self, texture_worker: &mut TextureWorker) {
        match self {
            ViewMode::Single(ci) | ViewMode::SingleWeb(ci) => {
                ci.try_update_texture(texture_worker);
            }
            ViewMode::Alpha2(ci1, ci2) | ViewMode::Slide2(ci1, ci2) => {
                ci1.try_update_texture(texture_worker);
                ci2.try_update_texture(texture_worker);
            }
        }
    }

    pub fn raster(&self) -> Option<&RasterHandler> {
        match self {
            ViewMode::Single(ci) | ViewMode::SingleWeb(ci) => ci.raster(),
            ViewMode::Alpha2(ci, _) | ViewMode::Slide2(ci, _) => ci.raster(),
        }
    }
}

/// How an image should be rendered
///
/// * Information about the current raster
/// * Parameters to create the view
/// * Current texture
pub enum ColorInterpretation {
    /// One band in greys or with palette color
    Panchromatic(RasterHandler, PanchromaticParams, Option<RasterViewHandle>),
    /// Three bands into RGB
    Color(RasterHandler, ColorParams, Option<RasterViewHandle>),
    /// Ratio A / B
    Ratio2(RasterHandler, Ratio2Params, Option<RasterViewHandle>),
    /// Ratio (A - B) / (A + B)
    Ratio4(RasterHandler, Ratio4Params, Option<RasterViewHandle>),
    /// Complex view with amp and phi values
    Cpx(RasterHandler, CpxParams, Option<RasterViewHandle>),
}

impl ColorInterpretation {
    fn handle(&self) -> &Option<RasterViewHandle> {
        match self {
            Self::Panchromatic(_, _, h)
            | Self::Color(_, _, h)
            | Self::Ratio2(_, _, h)
            | Self::Ratio4(_, _, h)
            | Self::Cpx(_, _, h) => h,
        }
    }

    fn handle_mut(&mut self) -> &mut Option<RasterViewHandle> {
        match self {
            Self::Panchromatic(_, _, h)
            | Self::Color(_, _, h)
            | Self::Ratio2(_, _, h)
            | Self::Ratio4(_, _, h)
            | Self::Cpx(_, _, h) => h,
        }
    }

    fn try_update_texture(&mut self, texture_worker: &mut TextureWorker) {
        if let Some(new_handle) = texture_worker.poll_result() {
            *self.handle_mut() = Some(new_handle);
        }
    }

    /// Raster pixel dimensions for plot default bounds
    fn raster_size(&self) -> (usize, usize) {
        match self {
            Self::Panchromatic(raster, _, _)
            | Self::Color(raster, _, _)
            | Self::Ratio2(raster, _, _)
            | Self::Ratio4(raster, _, _)
            | Self::Cpx(raster, _, _) => raster.raster_size(),
        }
    }

    /// True if the current handle doesn't cover the desired opts
    fn needs_reload(&self, opts: &ReadOptions) -> bool {
        let Some(handle) = self.handle() else {
            return true;
        };
        let [xmin, xmax, ymin, ymax] = handle.offset_bounds();
        let [dxmin, dxmax, dymin, dymax] = opts.extent;

        let dx = (xmin as isize - dxmin as isize).unsigned_abs();
        let dy = (ymin as isize - dymin as isize).unsigned_abs();
        let span_x = dxmax - dxmin;
        let span_y = dymax - dymin;

        handle.downsampling != opts.downsampling
                || dx > span_x / 5        // >20% drift
                || dy > span_y / 5
    }

    /// Clone self into a worker with updated read options
    fn to_worker_with_opts(&self, opts: ReadOptions) -> ColorInterpretationWorker {
        match self {
            Self::Panchromatic(raster, _, _) => ColorInterpretationWorker::Panchromatic(
                raster.path().to_path_buf(),
                PanchromaticParams {
                    read_opts: Some(opts),
                },
            ),
            // other variants follow the same pattern
            _ => todo!(),
        }
    }

    pub fn raster(&self) -> Option<&RasterHandler> {
        match self {
            Self::Panchromatic(r, _, _)
            | Self::Color(r, _, _)
            | Self::Ratio2(r, _, _)
            | Self::Ratio4(r, _, _)
            | Self::Cpx(r, _, _) => Some(r),
        }
    }
}

#[derive(Default)]
pub(super) struct PanchromaticParams {
    read_opts: Option<ReadOptions>,
}
pub(super) struct ColorParams {}
pub(super) struct Ratio2Params {}
pub(super) struct Ratio4Params {}
pub(super) struct CpxParams {}

/// Worker producing the textures for the mage display
pub enum ViewModeWorker {
    Single(ColorInterpretationWorker),
    /// Display one image using Walkers
    SingleWeb(ColorInterpretationWorker),
    /// Display two images blending with an alpha value
    Alpha2(ColorInterpretationWorker, ColorInterpretationWorker),
    /// Display two images transitionning with a slider
    Slide2(ColorInterpretationWorker, ColorInterpretationWorker),
    // /// Display two images side by side
    // Synchro2(ColorInterpretationWorker, ColorInterpretationWorker),
}

impl From<ViewMode> for ViewModeWorker {
    fn from(value: ViewMode) -> Self {
        match value {
            ViewMode::Single(color) => ViewModeWorker::Single(color.into()),
            ViewMode::SingleWeb(color) => ViewModeWorker::SingleWeb(color.into()),
            ViewMode::Alpha2(color, color2) => ViewModeWorker::Alpha2(color.into(), color2.into()),
            ViewMode::Slide2(color, color2) => ViewModeWorker::Slide2(color.into(), color2.into()),
            // ViewMode::Synchro2(color, color2) => {
            //     ViewModeWorker::Synchro2(color.into(), color2.into())
            // }
        }
    }
}

/// Worker handling the texture generation from file reading
pub enum ColorInterpretationWorker {
    /// One band in greys or with palette color
    Panchromatic(PathBuf, PanchromaticParams),
    /// Three bands into RGB
    Color(PathBuf, ColorParams),
    /// Ratio A / B
    Ratio2(PathBuf, Ratio2Params),
    /// Ratio (A - B) / (A + B)
    Ratio4(PathBuf, Ratio4Params),
    /// Complex view with amp and phi values
    Cpx(PathBuf, CpxParams),
}

impl From<ColorInterpretation> for ColorInterpretationWorker {
    fn from(value: ColorInterpretation) -> Self {
        match value {
            ColorInterpretation::Panchromatic(raster, color, _) => {
                ColorInterpretationWorker::Panchromatic(raster.path().to_path_buf(), color)
            }
            ColorInterpretation::Color(raster, color, _) => {
                ColorInterpretationWorker::Color(raster.path().to_path_buf(), color)
            }
            ColorInterpretation::Ratio2(raster, color, _) => {
                ColorInterpretationWorker::Ratio2(raster.path().to_path_buf(), color)
            }
            ColorInterpretation::Ratio4(raster, color, _) => {
                ColorInterpretationWorker::Ratio4(raster.path().to_path_buf(), color)
            }
            ColorInterpretation::Cpx(raster, color, _) => {
                ColorInterpretationWorker::Cpx(raster.path().to_path_buf(), color)
            }
        }
    }
}

impl ColorInterpretationWorker {
    /// Get the raster path
    pub fn path(&self) -> &Path {
        match self {
            Self::Panchromatic(p, _)
            | Self::Color(p, _)
            | Self::Ratio2(p, _)
            | Self::Ratio4(p, _)
            | Self::Cpx(p, _) => p.as_path(),
        }
    }

    /// Read the raster from file on a specific crop and downsampling
    fn read_array2(&self, opts: &ReadOptions) -> Result<Array2<f32>> {
        let ReadOptions {
            band,
            extent,
            downsampling,
        } = opts;
        let dataset = gdal::Dataset::open(&self.path())?;
        let raster_band = dataset.rasterband(*band)?;
        let window = (extent[0] as isize, extent[2] as isize);
        let window_size = (extent[1] - extent[0], extent[3] - extent[2]);
        let buffer_size = if *downsampling > 0 {
            (
                window_size.0.div_euclid(2 * downsampling),
                window_size.1.div_euclid(2 * downsampling),
            )
        } else {
            window_size.clone()
        };

        let buffer = raster_band.read_as::<f32>(
            window,
            window_size,
            buffer_size,
            Some(gdal::raster::ResampleAlg::NearestNeighbour),
        )?;
        let array2 = buffer.to_array()?;
        Ok(array2)
    }

    /// Transform the array2 into an egui-ready texture
    fn array2_to_texture(&self, arr: Array2<f32>, ctx: &egui::Context) -> Result<TextureHandle> {
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
        Ok(ctx.load_texture(
            "raster",
            ColorImage::from_rgba_unmultiplied([cols, rows], &pixels),
            TextureOptions::LINEAR,
        ))
    }

    /// Entry point called by the worker thread
    pub fn texture(&self, ctx: &egui::Context) -> Result<RasterViewHandle> {
        let opts = self.read_options()?;
        let array2 = self.read_array2(&opts)?;
        let texture = self.array2_to_texture(array2, ctx)?;
        Ok(RasterViewHandle::new(
            [opts.extent[0], opts.extent[2]],
            opts.downsampling,
            texture,
        ))
    }

    /// Derive read options from the variant's params
    fn read_options(&self) -> Result<ReadOptions> {
        match self {
            Self::Panchromatic(_, params) => {
                let opts = params
                    .read_opts
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("no read options set"))?;
                Ok(ReadOptions {
                    band: opts.band,
                    extent: opts.extent,
                    downsampling: opts.downsampling,
                })
            }
            // other variants: fill in as needed
            _ => anyhow::bail!("read_options not implemented for this variant"),
        }
    }
}

#[derive(Clone)]
struct ReadOptions {
    band: usize,
    extent: [usize; 4],
    downsampling: usize,
}

impl ReadOptions {
    fn new(band: usize, extent: [usize; 4], downsampling: usize) -> Self {
        ReadOptions {
            band,
            extent,
            downsampling,
        }
    }

    /// must be updated from canvas bounds
    fn from_plot_bounds(
        band: usize,
        bounds: PlotBounds,
        screen_size: egui::Vec2,
        raster_size: (usize, usize),
    ) -> Self {
        let (rw, rh) = raster_size;
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
        let downsampling = if px_per_screen > 1.0 {
            (px_per_screen.log2().floor() as usize).clamp(0, 6)
        } else {
            0
        };

        Self {
            band,
            extent: [xmin, xmax, ymin, ymax],
            downsampling,
        }
    }
}

pub struct RasterViewHandle {
    pub offset: [usize; 2],
    pub downsampling: usize,
    pub texture_handle: TextureHandle,
}

impl RasterViewHandle {
    fn new(offset: [usize; 2], downsampling: usize, texture_handle: TextureHandle) -> Self {
        Self {
            offset,
            downsampling,
            texture_handle,
        }
    }

    /// [xmin, xmax, ymin, ymax] derived from offset and texture size
    pub fn offset_bounds(&self) -> [usize; 4] {
        let [ox, oy] = self.offset;
        let [w, h] = self.texture_handle.size();
        [ox, ox + w, oy, oy + h]
    }
}
