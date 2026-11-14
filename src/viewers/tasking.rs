use std::sync::Arc;

use crate::Viewer;
use crate::raster::RasterHandler;
use crate::raster::tiles::TileDescriptor;
use crate::viewers::{ActiveViewer, ColorRanging, CpxMode, cmap::ColorInterpretation};

use eframe::egui::ColorImage;
use gdal::{Dataset, raster::Buffer};
use ordered_float::OrderedFloat;

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) enum ViewStyle {
    Panchro(PanchroTask),
    PanchroCpx(PanchroCpxTask),
    Color(ColorTask),
}

impl TileDescriptor {
    pub(crate) fn tile_to_colorimage(&self, dataset: &Dataset) -> Option<Arc<ColorImage>> {
        match &self.view_style {
            ViewStyle::Panchro(style) => {
                let buffer = self.read_buffer(dataset, style.band).ok()?;
                Some(
                    style
                        .color_interpretation
                        .panchro_buffer_to_colorimage(buffer),
                )
            }
            ViewStyle::PanchroCpx(_style) => {
                todo!()
            }
            ViewStyle::Color(style) => {
                let buffers = self
                    .read_3buffers(dataset, (style.band_r, style.band_g, style.band_b))
                    .ok()?;
                Some(style.color_buffers_to_colorimage(buffers))
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct PanchroTask {
    pub(crate) band: usize,
    pub(crate) color_interpretation: ColorInterpretation,
}

impl PanchroTask {
    fn new(band: usize, color_interpretation: ColorInterpretation) -> Self {
        PanchroTask {
            band,
            color_interpretation,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct PanchroCpxTask {
    band: usize,
    color_interpretation: ColorInterpretation,
    cpx_mode: CpxMode,
}

impl PanchroCpxTask {
    fn new(band: usize, color_interpretation: ColorInterpretation, cpx_mode: CpxMode) -> Self {
        PanchroCpxTask {
            band,
            color_interpretation,
            cpx_mode,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct ColorTask {
    band_r: usize,
    band_g: usize,
    band_b: usize,
    norm_r: (OrderedFloat<f32>, OrderedFloat<f32>),
    norm_g: (OrderedFloat<f32>, OrderedFloat<f32>),
    norm_b: (OrderedFloat<f32>, OrderedFloat<f32>),
    norm_db: bool,
}

impl ColorTask {
    fn new(
        bands: (usize, usize, usize),
        rangings: ((f32, f32), (f32, f32), (f32, f32)),
        norm_db: bool,
    ) -> Self {
        ColorTask {
            band_r: bands.0,
            band_g: bands.1,
            band_b: bands.2,
            norm_r: (OrderedFloat(rangings.0.0), OrderedFloat(rangings.0.1)),
            norm_g: (OrderedFloat(rangings.1.0), OrderedFloat(rangings.1.1)),
            norm_b: (OrderedFloat(rangings.2.0), OrderedFloat(rangings.2.1)),
            norm_db,
        }
    }

    pub(crate) fn color_buffers_to_colorimage(
        &self,
        buffers: (Buffer<f32>, Buffer<f32>, Buffer<f32>),
    ) -> Arc<ColorImage> {
        let (buf_r, buf_g, buf_b) = buffers;

        let (width, height) = buf_r.shape();
        debug_assert_eq!(buf_g.shape(), (width, height), "green band size mismatch");
        debug_assert_eq!(buf_b.shape(), (width, height), "blue band size mismatch");

        let chan_r = Self::normalize_channel(buf_r.data(), self.norm_r);
        let chan_g = Self::normalize_channel(buf_g.data(), self.norm_g);
        let chan_b = Self::normalize_channel(buf_b.data(), self.norm_b);

        let pixel_count = width * height;
        let mut rgba = Vec::with_capacity(pixel_count * 4);
        for i in 0..pixel_count {
            rgba.push(chan_r[i]);
            rgba.push(chan_g[i]);
            rgba.push(chan_b[i]);
            rgba.push(255);
        }

        Arc::new(ColorImage::from_rgba_unmultiplied([width, height], &rgba))
    }

    /// Normalizes a single band's raw f32 data into a 0-255 u8 channel.
    /// Kept separate from `Colormap::apply` since RGB compositing needs plain
    /// normalized intensity, not a colormap lookup.
    ///
    /// NOTE: `norm_db` is stored in the task (so it invalidates the cache) but
    /// is not applied here yet.
    fn normalize_channel(
        data: &[f32],
        (min, max): (OrderedFloat<f32>, OrderedFloat<f32>),
    ) -> Vec<u8> {
        let min = min.into_inner();
        let max = max.into_inner();
        let range = (max - min).max(f32::EPSILON);

        data.iter()
            .map(|&v| {
                let t = ((v - min) / range).clamp(0.0, 1.0);
                (t * 255.0).round() as u8
            })
            .collect()
    }
}

impl Viewer {
    /// Single source of truth for "which (lo, hi) does this band render with".
    ///
    /// Pure read: returns `None` while the needed stats are not available yet
    /// (not requested, still loading, or failed).
    pub(crate) fn resolve_range(
        &self,
        rh: &RasterHandler,
        band: usize,
        mode: &ColorRanging,
    ) -> Option<(f32, f32)> {
        let vm = &self.view_mode;
        match mode {
            ColorRanging::Manual => {
                let (lo, hi) = vm.manual_range;
                Some((lo.into_inner(), hi.into_inner()))
            }
            ColorRanging::MinMax => rh.band_minmax(band).map(|(mn, mx)| (mn as f32, mx as f32)),
            ColorRanging::Percentile => {
                let p = (vm.color_interpretation.percentile_clip.into_inner() as f64)
                    .clamp(0.00001, 49.9999);
                let lo = rh.band_percentile(band, p);
                let hi = rh.band_percentile(band, 100.0 - p);
                lo.zip(hi).map(|(lo, hi)| (lo as f32, hi as f32))
            }
        }
    }

    /// Band the UI uses as "the" reference for displayed range / complex check.
    pub(crate) fn reference_band(&self) -> usize {
        match self.view_mode.active_viewer {
            ActiveViewer::Band => self.view_mode.panchro_band,
            ActiveViewer::Color => self.view_mode.rgb_bands.0,
        }
    }

    fn compute_range_panchro_cpx(&self) -> Option<(f32, f32)> {
        todo!()
    }

    fn compute_range_color(
        &self,
        rh: &RasterHandler,
    ) -> Option<((f32, f32), (f32, f32), (f32, f32))> {
        let mode = &self.view_mode.ranging_mode;
        let (r, g, b) = self.view_mode.rgb_bands;
        Some((
            self.resolve_range(rh, r, mode)?,
            self.resolve_range(rh, g, mode)?,
            self.resolve_range(rh, b, mode)?,
        ))
    }

    /// Resolves the current UI state into a render task.
    /// `None` means "not ready yet" (stats missing).
    pub(crate) fn task_view(&self, rh: &RasterHandler) -> Option<ViewStyle> {
        let vm = &self.view_mode;
        let view_task = match vm.active_viewer {
            ActiveViewer::Band if rh.band_is_complex(vm.panchro_band) => {
                let range = self.compute_range_panchro_cpx()?;
                let mut ci = vm.color_interpretation.clone();
                ci.with_ranging_values(range);
                ViewStyle::PanchroCpx(PanchroCpxTask::new(vm.panchro_band, ci, vm.cpx_mode))
            }
            ActiveViewer::Band => {
                let range = self.resolve_range(rh, vm.panchro_band, &vm.ranging_mode)?;
                let mut ci = vm.color_interpretation.clone();
                ci.with_ranging_values(range);
                ViewStyle::Panchro(PanchroTask::new(vm.panchro_band, ci))
            }
            ActiveViewer::Color => {
                let range = self.compute_range_color(rh)?;
                ViewStyle::Color(ColorTask::new(
                    vm.rgb_bands,
                    range,
                    vm.color_interpretation.db_mode,
                ))
            }
        };
        Some(view_task)
    }
}
