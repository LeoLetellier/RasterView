use egui::{ColorImage, TextBuffer};
use gdal::raster::Buffer;
use rayon::prelude::*;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use crate::viewers::cmap::NormMode::PerBand;

static COLORMAP_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/colormaps.bin"));

pub(crate) struct ColormapEntry {
    pub(crate) name: &'static str,
    pub(crate) cmap_type: ColorMapType,
    pub(crate) offset: usize,
    pub(crate) len: usize,
    pub(crate) below: [u8; 4],
    pub(crate) above: [u8; 4],
    pub(crate) nan: [u8; 4],
}

include!(concat!(env!("OUT_DIR"), "/colormaps_registry.rs"));

#[derive(Debug, PartialEq, Clone)]
pub(crate) struct ColorInterpretation {
    pub(crate) ranging_mode: ColorRanging,
    pub(crate) ranging_values: (f32, f32),
    pub(crate) colormap: ColorMap,
    pub(crate) norm_mode: NormMode,
}

#[derive(Debug, PartialEq, Clone)]
pub(crate) enum NormMode {
    PerBand,
    AllBands,
}

impl Default for ColorInterpretation {
    fn default() -> Self {
        ColorInterpretation {
            ranging_mode: ColorRanging::Manual,
            ranging_values: (0.0, 1.0),
            colormap: ColorMap::default(),
            norm_mode: PerBand,
        }
    }
}

// Safe: equality above is defined via bit patterns, so it's a true
// equivalence relation (reflexive even for NaN), unlike f32::eq.
impl Eq for ColorInterpretation {}

impl Hash for ColorInterpretation {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.ranging_mode.hash(state);
        self.ranging_values.0.to_bits().hash(state);
        self.ranging_values.1.to_bits().hash(state);
        self.colormap.hash(state);
    }
}

impl ColorInterpretation {
    pub(crate) fn new(colormap: ColorMap) -> Self {
        let mut ci = ColorInterpretation::default();
        ci.colormap = colormap;
        ci
    }

    pub(crate) fn with_ranging_values(&mut self, ranging_values: (f32, f32)) -> &Self {
        self.ranging_values = ranging_values;
        self
    }

    pub(crate) fn panchro_buffer_to_colorimage(&self, buffer: Buffer<f32>) -> Arc<ColorImage> {
        let (buffer_width, buffer_height) = buffer.shape();
        let data = buffer.data();

        // Apply colormap to data
        let color_data = self
            .colormap
            .apply(data, &self.ranging_mode, self.ranging_values);

        // Convert to egui ColorImage
        Arc::new(ColorImage::from_rgba_unmultiplied(
            [buffer_width, buffer_height],
            &color_data,
        ))
    }

    pub(crate) fn rgb_buffers_to_colorimage(
        &self,
        buffers: (Buffer<f32>, Buffer<f32>, Buffer<f32>),
    ) -> Arc<ColorImage> {
        todo!()
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub(crate) enum ColorRanging {
    MinMax,
    Percentile,
    Manual,
    GdalInterpretation,
}

#[derive(Debug)]
struct ColorMapScheme {
    name: String,
    below: Option<[u8; 4]>,
    above: Option<[u8; 4]>,
    nan: Option<[u8; 4]>,
    stops: Vec<(f32, [u8; 4])>,
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
struct ColorMapLut {
    data: &'static [u8],
}

impl ColorMapLut {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.data.len() / 4
    }

    #[inline]
    pub(crate) fn get(&self, idx: usize) -> [u8; 4] {
        let o = idx * 4;
        [
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ]
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub(crate) enum ColorMapType {
    Sequential,
    Divergent,
    Cyclic,
    Other,
}

impl ColorMapType {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            ColorMapType::Sequential => "Sequential",
            ColorMapType::Divergent => "Divergent",
            ColorMapType::Cyclic => "Cyclic",
            ColorMapType::Other => "Other",
        }
    }
}

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub(crate) struct ColorMap {
    name: String,
    lut: ColorMapLut,
    below: [u8; 4],
    above: [u8; 4],
    nan: [u8; 4],
    cmap_type: ColorMapType,
}

impl Default for ColorMap {
    fn default() -> Self {
        // 256-level greyscale ramp, R=G=B=i, alpha=255.
        const N: usize = 256;
        const fn build_grey_lut() -> [u8; N * 4] {
            let mut data = [0u8; N * 4];
            let mut i = 0;
            while i < N {
                data[i * 4] = i as u8;
                data[i * 4 + 1] = i as u8;
                data[i * 4 + 2] = i as u8;
                data[i * 4 + 3] = 255;
                i += 1;
            }
            data
        }

        static GREY_LUT: [u8; N * 4] = build_grey_lut();

        ColorMap {
            name: "default".to_string(),
            lut: ColorMapLut { data: &GREY_LUT },
            below: [0, 0, 0, 255],       // clamp to black
            above: [255, 255, 255, 255], // clamp to white
            nan: [0, 0, 0, 0],           // transparent
            cmap_type: ColorMapType::Sequential,
        }
    }
}

impl TryFrom<&str> for ColorMap {
    type Error = anyhow::Error;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        ColorMap::from_name(value).ok_or_else(|| anyhow::anyhow!("unknown colormap: {value}"))
    }
}

impl ColorMap {
    pub(crate) fn name(&self) -> &str {
        self.name.as_str()
    }

    pub(crate) fn cmap_type(&self) -> &ColorMapType {
        &self.cmap_type
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        let entry = COLORMAPS.iter().find(|e| e.name == name)?;
        let start = entry.offset * 4;
        let end = start + entry.len * 4;
        Some(ColorMap {
            name: name.to_string(),
            lut: ColorMapLut {
                data: &COLORMAP_BLOB[start..end],
            },
            below: entry.below,
            above: entry.above,
            nan: entry.nan,
            cmap_type: entry.cmap_type.clone(),
        })
    }

    /// Handy for populating a UI dropdown.
    pub(crate) fn names() -> impl Iterator<Item = &'static str> {
        COLORMAPS.iter().map(|e| e.name)
    }

    /// Handy for populating a grouped UI dropdown without building full ColorMaps.
    pub(crate) fn names_with_type() -> impl Iterator<Item = (&'static str, &'static ColorMapType)> {
        COLORMAPS.iter().map(|e| (e.name, &e.cmap_type))
    }

    /// Evenly-spaced RGBA samples across the LUT, for lightweight UI previews.
    pub(crate) fn preview_samples(&self, n: usize) -> Vec<egui::Color32> {
        let len = self.lut.len();
        (0..n)
            .map(|i| {
                let t = i as f32 / (n.saturating_sub(1)).max(1) as f32;
                let idx = (t * (len - 1) as f32).round() as usize;
                let [r, g, b, a] = self.lut.get(idx);
                egui::Color32::from_rgba_unmultiplied(r, g, b, a)
            })
            .collect()
    }

    pub(crate) fn apply_into(
        &self,
        data: &[f32],
        ranging_mode: &ColorRanging,
        ranging_values: (f32, f32),
        out: &mut [u8],
    ) {
        debug_assert_eq!(out.len(), data.len() * 4);
        let (lo, hi) = compute_range(data, ranging_mode, ranging_values);
        let n = self.lut.len();
        let scale = (n - 1) as f32 / (hi - lo).max(f32::EPSILON);

        data.par_iter()
            .zip(out.par_chunks_mut(4))
            .for_each(|(&v, px)| {
                let rgba = if v.is_nan() {
                    self.nan
                } else {
                    let t = (v - lo) * scale;
                    if t < 0.0 {
                        self.below
                    } else if t > (n - 1) as f32 {
                        self.above
                    } else {
                        self.lut.get(t.round() as usize)
                    }
                };
                px.copy_from_slice(&rgba);
            });
    }

    pub(crate) fn apply(
        &self,
        data: &[f32],
        ranging_mode: &ColorRanging,
        ranging_values: (f32, f32),
    ) -> Vec<u8> {
        let mut out = vec![0u8; data.len() * 4];
        self.apply_into(data, ranging_mode, ranging_values, &mut out);
        out
    }
}

fn compute_range(
    data: &[f32],
    ranging_mode: &ColorRanging,
    ranging_values: (f32, f32),
) -> (f32, f32) {
    match ranging_mode {
        ColorRanging::Manual => ranging_values,
        ColorRanging::MinMax => {
            let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
            for &v in data {
                if v.is_finite() {
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
            (lo, hi)
        }
        ColorRanging::Percentile => {
            todo!();
            // let mut finite: Vec<f32> = data.iter().copied().filter(|v| v.is_finite()).collect();
            // let n = finite.len();
            // let lo_idx = ((p_lo / 100.0) * (n - 1) as f32).round() as usize;
            // let hi_idx = ((p_hi / 100.0) * (n - 1) as f32).round() as usize;
            // finite.select_nth_unstable_by(lo_idx, |a, b| a.partial_cmp(b).unwrap());
            // let lo = finite[lo_idx];
            // finite.select_nth_unstable_by(hi_idx, |a, b| a.partial_cmp(b).unwrap());
            // let hi = finite[hi_idx];
            // (lo, hi)
        }
        ColorRanging::GdalInterpretation => todo!("mirror GDAL's default stretch heuristic"),
    }
}
