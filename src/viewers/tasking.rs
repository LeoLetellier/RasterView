use std::{ops::Deref, sync::Arc};

use crate::{
    Viewer,
    viewers::{
        ActiveViewer, ColorRanging, CpxMode, NormModePanchro,
        cmap::{ColorInterpretation, DbMode},
        tiler::TileDescriptor,
    },
};

use egui::ColorImage;
use gdal::Dataset;
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
                todo!()
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
            norm_r: (OrderedFloat(rangings.0.1), OrderedFloat(rangings.0.1)),
            norm_g: (OrderedFloat(rangings.1.1), OrderedFloat(rangings.1.1)),
            norm_b: (OrderedFloat(rangings.2.1), OrderedFloat(rangings.2.1)),
            norm_db,
        }
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
        todo!()
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
