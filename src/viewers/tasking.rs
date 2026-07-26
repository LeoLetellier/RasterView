use std::{ops::Deref, sync::Arc};

use crate::{
    Viewer,
    viewers::{
        ActiveViewer, ColorRanging, CpxMode, NormModePanchro, NormModeRGB,
        cmap::{ColorInterpretation, DbMode},
        tiler::TileDescriptor,
    },
};

use egui::ColorImage;
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
            ViewStyle::PanchroCpx(style) => {
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
struct PanchroTask {
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
struct PanchroCpxTask {
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
struct ColorTask {
    band_r: usize,
    band_g: usize,
    band_b: usize,
    norm_r: (OrderedFloat<f32>, OrderedFloat<f32>),
    norm_g: (OrderedFloat<f32>, OrderedFloat<f32>),
    norm_b: (OrderedFloat<f32>, OrderedFloat<f32>),
    norm_db: DbMode,
}

impl ColorTask {
    fn new(
        bands: (usize, usize, usize),
        rangings: ((f32, f32), (f32, f32), (f32, f32)),
        norm_db: DbMode,
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

    /// Normalizes a single band's raw f32 data into a 0-255 u8 channel,
    /// applying dB conversion first if requested. Kept separate from
    /// `Colormap::apply` since RGB compositing needs plain normalized
    /// intensity, not a colormap lookup.
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
    fn compute_range_panchro(&self) -> Option<(f32, f32)> {
        let rh = &self.raster_handler;
        let vm = &self.view_mode;
        let normal_mode = &vm.norm_mode_panchro;
        let manual_range = (
            vm.color_interpretation.ranging_values.0.into_inner(),
            vm.color_interpretation.ranging_values.1.into_inner(),
        );
        let perc = vm
            .color_interpretation
            .percentile_clip
            .deref()
            .clamp(0.00001, 49.9999) as f64;
        // Get state of the stats
        let band = self.view_mode.panchro_band;
        let band_minmax = rh.band_minmax(band).map(|mm| (mm.0 as f32, mm.1 as f32));
        let all_minmax: Option<(f32, f32)> = (1..=rh.raster_count())
            .map(|b| rh.band_minmax(b))
            .try_fold((f32::INFINITY, f32::NEG_INFINITY), |acc, mm| {
                mm.map(|(mn, mx)| (acc.0.min(mn as f32), acc.1.max(mx as f32)))
            });
        let band_percentile = rh
            .band_percentile(band, perc)
            .zip(rh.band_percentile(band, 100.0 - perc))
            .map(|(lo, hi)| (lo as f32, hi as f32));
        let all_percentile: Option<(f32, f32)> = (1..=rh.raster_count())
            .map(|b| {
                rh.band_percentile(b, perc)
                    .zip(rh.band_percentile(b, 100.0 - perc))
            })
            .try_fold((f32::INFINITY, f32::NEG_INFINITY), |acc, pp| {
                pp.map(|(lo, hi)| (acc.0.min(lo as f32), acc.1.max(hi as f32)))
            });

        // Do the branching — no more silent fallback to manual_range on missing stats
        match vm.ranging_mode {
            ColorRanging::Manual => Some(manual_range),
            ColorRanging::MinMax => match normal_mode {
                NormModePanchro::PerBand => band_minmax,
                NormModePanchro::AllBands => all_minmax,
            },
            ColorRanging::Percentile => match normal_mode {
                NormModePanchro::PerBand => band_percentile,
                NormModePanchro::AllBands => all_percentile,
            },
        }
    }

    fn compute_range_panchro_cpx(&self) -> Option<(f32, f32)> {
        todo!()
    }

    fn compute_range_color(&self) -> Option<((f32, f32), (f32, f32), (f32, f32))> {
        let rh = &self.raster_handler;
        let vm = &self.view_mode;
        let normal_mode = &vm.norm_mode_rgb;
        let manual_range = (
            vm.color_interpretation.ranging_values.0.into_inner(),
            vm.color_interpretation.ranging_values.1.into_inner(),
        );
        let perc = vm
            .color_interpretation
            .percentile_clip
            .deref()
            .clamp(0.00001, 49.9999) as f64;

        let red = vm.rgb_bands.0;
        let green = vm.rgb_bands.1;
        let blue = vm.rgb_bands.2;

        // --- per-band stats (one for each of R, G, B) ---
        let band_minmax = |band: usize| -> Option<(f32, f32)> {
            rh.band_minmax(band).map(|mm| (mm.0 as f32, mm.1 as f32))
        };
        let band_percentile = |band: usize| -> Option<(f32, f32)> {
            rh.band_percentile(band, perc)
                .zip(rh.band_percentile(band, 100.0 - perc))
                .map(|(lo, hi)| (lo as f32, hi as f32))
        };

        // --- combined stats across an arbitrary set of bands ---
        let combined_minmax = |bands: &[usize]| -> Option<(f32, f32)> {
            bands
                .iter()
                .map(|&b| rh.band_minmax(b))
                .try_fold((f32::INFINITY, f32::NEG_INFINITY), |acc, mm| {
                    mm.map(|(mn, mx)| (acc.0.min(mn as f32), acc.1.max(mx as f32)))
                })
        };
        let combined_percentile = |bands: &[usize]| -> Option<(f32, f32)> {
            bands
                .iter()
                .map(|&b| {
                    rh.band_percentile(b, perc)
                        .zip(rh.band_percentile(b, 100.0 - perc))
                })
                .try_fold((f32::INFINITY, f32::NEG_INFINITY), |acc, pp| {
                    pp.map(|(lo, hi)| (acc.0.min(lo as f32), acc.1.max(hi as f32)))
                })
        };

        let all_bands: Vec<usize> = (1..=rh.raster_count()).collect();
        let rgb_bands = [red, green, blue];

        match vm.ranging_mode {
            ColorRanging::Manual => Some((manual_range, manual_range, manual_range)),

            ColorRanging::MinMax => match normal_mode {
                NormModeRGB::PerBand => {
                    let r = band_minmax(red)?;
                    let g = band_minmax(green)?;
                    let b = band_minmax(blue)?;
                    Some((r, g, b))
                }
                NormModeRGB::RGBBands => {
                    let combined = combined_minmax(&rgb_bands)?;
                    Some((combined, combined, combined))
                }
                NormModeRGB::AllBands => {
                    let combined = combined_minmax(&all_bands)?;
                    Some((combined, combined, combined))
                }
            },

            ColorRanging::Percentile => match normal_mode {
                NormModeRGB::PerBand => {
                    let r = band_percentile(red)?;
                    let g = band_percentile(green)?;
                    let b = band_percentile(blue)?;
                    Some((r, g, b))
                }
                NormModeRGB::RGBBands => {
                    let combined = combined_percentile(&rgb_bands)?;
                    Some((combined, combined, combined))
                }
                NormModeRGB::AllBands => {
                    let combined = combined_percentile(&all_bands)?;
                    Some((combined, combined, combined))
                }
            },
        }
    }

    pub(crate) fn task_view(&self) -> Option<ViewStyle> {
        let vm = &self.view_mode;
        let rh = &self.raster_handler;
        let view_task = match vm.active_viewer {
            ActiveViewer::Panchro if rh.band_is_complex(vm.panchro_band) => {
                let range = self.compute_range_panchro_cpx()?;
                let mut task_ci = vm.color_interpretation.clone();
                task_ci.with_ranging_values(range);
                ViewStyle::PanchroCpx(PanchroCpxTask::new(vm.panchro_band, task_ci, vm.cpx_mode))
            }
            ActiveViewer::Panchro => {
                let range = self.compute_range_panchro()?;
                let mut task_ci = vm.color_interpretation.clone();
                task_ci.with_ranging_values(range);
                ViewStyle::Panchro(PanchroTask::new(vm.panchro_band, task_ci))
            }
            ActiveViewer::Color => {
                let range = self.compute_range_color()?;
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
