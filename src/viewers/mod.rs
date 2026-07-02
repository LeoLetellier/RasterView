use crate::{raster::RasterHandler, texture_thread::TextureWorker};
use anyhow::Result;
use egui::{ColorImage, TextureOptions};
use egui::{TextureHandle, Ui, vec2};
use egui_plot::PlotBounds;
use egui_plot::{Plot, PlotImage, PlotPoint};
use ndarray::Array2;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

mod coords;
mod ui;

/// Image viewer composition
/// /// How an image should be rendered
///
/// * Information about the current raster
/// * Parameters to create the view
/// * Current texture
pub enum ViewMode {
    /// One band in greys or with palette color
    Panchromatic(RasterHandler, PanchromaticParams, Option<RasterViewHandle>),
    /// Three bands into RGB + one for alpha
    RGBA(RasterHandler, ColorParams, Option<RasterViewHandle>),
    /// Ratio A / B
    Ratio2(RasterHandler, Ratio2Params, Option<RasterViewHandle>),
    /// Ratio (A - B) / (A + B)
    Ratio4(RasterHandler, Ratio4Params, Option<RasterViewHandle>),
    /// Complex view with amp and phi values
    Cpx(RasterHandler, CpxParams, Option<RasterViewHandle>),
}

impl ViewMode {
    // /// All central panel viewer goes in this one
    // pub fn ui(&mut self, ui: &mut Ui, texture_worker: &mut TextureWorker) {
    //     // 1. Poll for new texture
    //     self.try_update_texture(texture_worker);

    //     // 2. Draw plot, capture bounds
    //     let raster_size = self.raster_size();
    //     let (rw, rh) = raster_size;
    //     let available = ui.available_size();
    //     let mut new_bounds: Option<PlotBounds> = None;

    //     Plot::new("main_plot")
    //         .data_aspect(1.0)
    //         .pan_pointer_button(egui::PointerButton::Primary)
    //         .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
    //         .allow_scroll(false)
    //         .allow_zoom(true)
    //         .default_x_bounds(0.0, rw as f64)
    //         .default_y_bounds(0.0, rh as f64)
    //         .show(ui, |plot_ui| {
    //             // Bounds in pixel coordinates
    //             new_bounds = Some(plot_ui.plot_bounds());

    //             if let Some(handle) = self.handle() {
    //                 let [xmin, xmax, ymin, ymax] = handle.offset_bounds();
    //                 // Center in pixel coordinates
    //                 // let cx = (xmin + xmax) as f64 / 2.0;
    //                 // let cy = (ymin + ymax) as f64 / 2.0;
    //                 let (_, rh) = raster_size; // already available in scope
    //                 let cx = (xmin + xmax) as f64 / 2.0;
    //                 let cy = rh as f64 - (ymin + ymax) as f64 / 2.0;
    //                 // Width and height in pixel coordinates
    //                 let w = (xmax - xmin) as f32;
    //                 let h = (ymax - ymin) as f32;
    //                 log::debug!(
    //                     "draw: extent {:?} center ({}, {}) size ({}, {})",
    //                     handle.extent,
    //                     cx,
    //                     cy,
    //                     w,
    //                     h
    //                 );
    //                 plot_ui.image(PlotImage::new(
    //                     "raster",
    //                     handle.texture_handle.id(),
    //                     PlotPoint::new(cx, cy),
    //                     vec2(w, h),
    //                 ));
    //             }
    //         });

    //     // 3. Check if a new load is needed
    //     if let Some(bounds) = new_bounds {
    //         // With these bounds we should need the texture extent
    //         let opts = ReadOptions::from_plot_bounds(1, bounds, available, raster_size);
    //         // If not out of screen
    //         if let Some(o) = opts {
    //             // And if need a new texture
    //             if self.needs_reload(&o) {
    //                 // Ask the worker a new texture
    //                 log::debug!("Ask worker new texture");
    //                 dbg!(&o);
    //                 let worker: ViewModeWorker = self.to_worker_with_opts(o);
    //                 let _ = texture_worker.request_load(worker);
    //             }
    //         }
    //     }
    // }

    fn handle(&self) -> &Option<RasterViewHandle> {
        match self {
            Self::Panchromatic(_, _, h)
            | Self::RGBA(_, _, h)
            | Self::Ratio2(_, _, h)
            | Self::Ratio4(_, _, h)
            | Self::Cpx(_, _, h) => h,
        }
    }

    fn handle_mut(&mut self) -> &mut Option<RasterViewHandle> {
        match self {
            Self::Panchromatic(_, _, h)
            | Self::RGBA(_, _, h)
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
            | Self::RGBA(raster, _, _)
            | Self::Ratio2(raster, _, _)
            | Self::Ratio4(raster, _, _)
            | Self::Cpx(raster, _, _) => raster.raster_size(),
        }
    }

    /// True if the current handle doesn't cover the desired opts
    fn needs_reload(&self, opts: &ReadOptions) -> bool {
        log::debug!("check if need reload of texture");
        let Some(handle) = self.handle() else {
            log::debug!("needs_reload: no handle yet");
            return true;
        };
        log::debug!(
            "handle  extent: {:?} ds: {}",
            handle.extent,
            handle.downsampling
        );
        log::debug!(
            "desired extent: {:?} ds: {}",
            opts.extent,
            opts.downsampling
        );
        // If change of zoom needed
        if handle.downsampling != opts.downsampling {
            log::debug!(
                "needs_reload: downsampling changed {} -> {}",
                handle.downsampling,
                opts.downsampling
            );
            return true;
        }

        // Bounds of current texture
        let [xmin, xmax, ymin, ymax] = handle.offset_bounds();
        // Bounds needed by the hypothetic new texture
        let [dxmin, dxmax, dymin, dymax] = opts.extent;

        // Use desired window size as normalizer — stable across zoom levels
        let span_x = (dxmax - dxmin) as f64;
        let span_y = (dymax - dymin) as f64;

        let pan_threshold = 0.1;
        // Isize downcasting to avoid overflow
        let pan_update_needed = ((xmin as isize - dxmin as isize).abs() as f64 / span_x)
            > pan_threshold
            || ((xmax as isize - dxmax as isize).abs() as f64 / span_x) > pan_threshold
            || ((ymin as isize - dymin as isize).abs() as f64 / span_y) > pan_threshold
            || ((ymax as isize - dymax as isize).abs() as f64 / span_y) > pan_threshold;
        pan_update_needed
    }

    /// Clone self into a worker with updated read options
    fn to_worker_with_opts(&self, opts: ReadOptions) -> ViewModeWorker {
        match self {
            Self::Panchromatic(raster, _, _) => ViewModeWorker::Panchromatic(
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
            | Self::RGBA(r, _, _)
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

/// Worker handling the texture generation from file reading
pub enum ViewModeWorker {
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

impl From<ViewMode> for ViewModeWorker {
    fn from(value: ViewMode) -> Self {
        match value {
            ViewMode::Panchromatic(raster, color, _) => {
                ViewModeWorker::Panchromatic(raster.path().to_path_buf(), color)
            }
            ViewMode::RGBA(raster, color, _) => {
                ViewModeWorker::Color(raster.path().to_path_buf(), color)
            }
            ViewMode::Ratio2(raster, color, _) => {
                ViewModeWorker::Ratio2(raster.path().to_path_buf(), color)
            }
            ViewMode::Ratio4(raster, color, _) => {
                ViewModeWorker::Ratio4(raster.path().to_path_buf(), color)
            }
            ViewMode::Cpx(raster, color, _) => {
                ViewModeWorker::Cpx(raster.path().to_path_buf(), color)
            }
        }
    }
}

impl ViewModeWorker {
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
        // max at 1 to prevent zero dimension
        let buffer_size = if *downsampling > 0 {
            (
                window_size.0.div_euclid(2 * downsampling).max(1),
                window_size.1.div_euclid(2 * downsampling).max(1),
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

        // parallel min/max
        let min = arr.par_iter().cloned().reduce(|| f32::INFINITY, f32::min);
        let max = arr
            .par_iter()
            .cloned()
            .reduce(|| f32::NEG_INFINITY, f32::max);
        let range = (max - min).max(f32::EPSILON);

        // parallel pixel building — each element produces 4 bytes
        let pixels: Vec<u8> = arr
            .par_iter()
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
            opts.extent,
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

#[derive(Clone, Debug)]
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
    ) -> Option<Self> {
        let (rw, rh) = raster_size;
        // Span is plot bounds size in pixel coordinates
        let span_x = bounds.max()[0] - bounds.min()[0];
        let span_y = bounds.max()[1] - bounds.min()[1];
        // Pad it with 15% margin
        let pad_x = span_x * 0.15;
        let pad_y = span_y * 0.15;

        // Determine full intersection betwen padded bounds and raster extent in pixel coordinates
        // Dont get lower than 0 for lower bounds
        let xmin = (bounds.min()[0] - pad_x).max(0.0) as usize;
        // let ymin = (bounds.min()[1] - pad_y).max(0.0) as usize;
        // Dont get higher than raster size for higher bounds
        let xmax = ((bounds.max()[0] + pad_x) as usize).min(rw);
        // let ymax = ((bounds.max()[1] + pad_y) as usize).min(rh);

        let ymin = (rh as f64 - bounds.max()[1] - pad_y).max(0.0) as usize;
        let ymax = ((rh as f64 - bounds.min()[1] + pad_y) as usize).min(rh);

        // Guard
        if xmin >= xmax || ymin >= ymax {
            return None;
        }

        // usize casting already make them positive but anyway...
        let out_of_screen = xmin > rw || xmax < 0 || ymin > rh || ymax < 0;

        if !out_of_screen {
            // Plot coordinates actually needed
            let visible_raster_pixels = ((xmax - xmin) as f64, (ymax - ymin) as f64);
            // Screen pixel is the actual number of pixels for display in screen cxoordinates
            let screen_pixels = (screen_size.x as f64, screen_size.y as f64);
            // Ratio is
            // * > 1 if more pixel in raster to show than in screen so need downsampling
            // * < 1 if less pixel in raster than on screen so show full res
            let visible_pixel_ratio = (
                visible_raster_pixels.0 / screen_pixels.0,
                visible_raster_pixels.1 / screen_pixels.1,
            );

            // Check the worst case ratio, so the max value between x and y
            let px_per_screen = visible_pixel_ratio.0.max(visible_pixel_ratio.1);

            // Determine the downsampling to apply (px / (2 * downsampling))
            let downsampling = if px_per_screen > 1.0 {
                (px_per_screen.log2().floor() as usize).clamp(0, 6)
            } else {
                // No downsampling
                0
            };

            log::debug!(
                "Plot bounds is {} {} {} {}",
                bounds.min()[0],
                bounds.min()[1],
                bounds.max()[0],
                bounds.max()[1]
            );

            log::debug!("Ask for extent {} {} {} {}", xmin, ymin, xmax, ymax);

            Some(Self {
                band,
                extent: [xmin, xmax, ymin, ymax],
                downsampling,
            })
        } else {
            None
        }
    }
}

pub struct RasterViewHandle {
    /// Extent in pixel coordinates
    pub extent: [usize; 4],
    /// Downsampling factor requested
    pub downsampling: usize,
    /// Egui Texture
    pub texture_handle: TextureHandle,
}

impl RasterViewHandle {
    fn new(extent: [usize; 4], downsampling: usize, texture_handle: TextureHandle) -> Self {
        Self {
            extent,
            downsampling,
            texture_handle,
        }
    }

    /// Gives extent in pixel coordinates
    pub fn offset_bounds(&self) -> [usize; 4] {
        self.extent
    }
}
