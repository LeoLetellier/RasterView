use egui::ColorImage;
use gdal::raster::Buffer;
use ordered_float::OrderedFloat;
use rayon::prelude::*;
use std::{hash::Hash, sync::Arc};

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

#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub(crate) struct ColorInterpretation {
    pub(crate) ranging_values: (OrderedFloat<f32>, OrderedFloat<f32>),
    pub(crate) colormap: ColorMap,
    pub(crate) invert_cmap: bool,
    pub(crate) db_mode: DbMode,
    pub(crate) percentile_clip: OrderedFloat<f32>,
    pub(crate) cyclic_wrap: CyclicWrap,
}

impl Default for ColorInterpretation {
    fn default() -> Self {
        ColorInterpretation {
            ranging_values: (OrderedFloat(0.0), OrderedFloat(1.0)),
            colormap: ColorMap::default(),
            invert_cmap: false,
            db_mode: DbMode::default(),
            percentile_clip: OrderedFloat(2.0),
            cyclic_wrap: CyclicWrap::default(),
        }
    }
}

impl ColorInterpretation {
    pub(crate) fn new(colormap: ColorMap) -> Self {
        let mut ci = ColorInterpretation::default();
        ci.colormap = colormap;
        ci
    }

    pub(crate) fn with_ranging_values(&mut self, ranging_values: (f32, f32)) -> &Self {
        self.ranging_values = (
            OrderedFloat(ranging_values.0),
            OrderedFloat(ranging_values.1),
        );
        self
    }

    pub(crate) fn ranging_values(&self) -> (f32, f32) {
        (
            self.ranging_values.0.into_inner(),
            self.ranging_values.1.into_inner(),
        )
    }

    pub(crate) fn panchro_buffer_to_colorimage(&self, buffer: Buffer<f32>) -> Arc<ColorImage> {
        let (buffer_width, buffer_height) = buffer.shape();
        let data = buffer.data();

        // Apply colormap to data
        let color_data = self.colormap.apply(
            data,
            self.ranging_values(),
            self.db_mode,
            self.cyclic_wrap,
            self.invert_cmap,
        );

        // Convert to egui ColorImage
        Arc::new(ColorImage::from_rgba_unmultiplied(
            [buffer_width, buffer_height],
            &color_data,
        ))
    }
}

fn normalize_buffer_minmax_db(buffer: Buffer<f32>, minmax: (f32, f32), db_mode: bool) -> Vec<f32> {
    let (min, max) = if db_mode {
        (10.0 * minmax.0.log10(), 10.0 * minmax.1.log10())
    } else {
        minmax
    };
    let den = max - min;
    assert!(den != 0.0);

    buffer
        .data()
        .par_iter()
        .map(|b| {
            if db_mode {
                (10.0 * b.log10() - min) / den
            } else {
                (b - min) / den
            }
        })
        .collect()
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

    pub(crate) fn apply(
        &self,
        data: &[f32],
        vminmax: (f32, f32),
        db_mode: DbMode,
        wrap: CyclicWrap,
        invert: bool,
    ) -> Vec<u8> {
        let mut out = vec![0u8; data.len() * 4];
        let n = self.lut.len();
        let n_f = n as f32;
        let vmin = db_mode.convert(vminmax.0);
        let vmax = db_mode.convert(vminmax.1);
        debug_assert!(
            !vmin.is_nan() && !vmax.is_nan(),
            "vmin/vmax must be > 0 when using a dB mode"
        );
        let range = (vmax - vmin).max(f32::EPSILON);
        let wrap_bounds = wrap.bounds();
        let scale = if wrap_bounds.is_some() {
            n_f
        } else {
            (n - 1) as f32
        } / range;

        let resolve = move |idx: usize| -> usize { if invert { n - 1 - idx } else { idx } };

        data.par_iter()
            .zip(out.par_chunks_mut(4))
            .for_each(|(&raw, px)| {
                let mut v = db_mode.convert(raw);
                if let Some((lo, hi)) = wrap_bounds {
                    let period = hi - lo;
                    if period > 0.0 {
                        v = lo + (v - lo).rem_euclid(period);
                    }
                }
                let rgba = if v.is_nan() {
                    self.nan
                } else {
                    let t = (v - vmin) * scale;
                    if wrap_bounds.is_some() {
                        let idx = t.rem_euclid(n_f).round() as usize;
                        self.lut.get(resolve(idx.min(n - 1)))
                    } else if t < 0.0 {
                        self.below
                    } else if t > (n - 1) as f32 {
                        self.above
                    } else {
                        self.lut.get(resolve(t.round() as usize))
                    }
                };
                px.copy_from_slice(&rgba);
            });
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DbMode {
    /// Use linear values
    None,
    /// power/intensity ratio: 10 * log10(v)
    ///
    /// e.g intensity of backscatter signal
    Intensity,
    /// amplitude/voltage ratio: 20 * log10(v)
    ///
    /// e.g amplitude of backscatter signal
    Amplitude,
}

impl DbMode {
    #[inline]
    fn convert(self, v: f32) -> f32 {
        match self {
            DbMode::None => v,
            DbMode::Intensity => {
                if v > 0.0 {
                    10.0 * v.log10()
                } else {
                    f32::NAN
                }
            }
            DbMode::Amplitude => {
                if v > 0.0 {
                    20.0 * v.log10()
                } else {
                    f32::NAN
                }
            }
        }
    }
}

impl Default for DbMode {
    fn default() -> Self {
        DbMode::None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum CyclicWrap {
    None,
    Radian,
    Degree,
    /// (min, max) — not assumed symmetric, so it covers e.g. 0..360 hue/bearing
    /// as well as symmetric ranges like -x..x.
    Custom(OrderedFloat<f32>, OrderedFloat<f32>),
}

impl CyclicWrap {
    /// Natural (min, max) bounds of the cyclic domain, or `None` if not cyclic.
    fn bounds(self) -> Option<(f32, f32)> {
        match self {
            CyclicWrap::None => None,
            CyclicWrap::Radian => Some((-std::f32::consts::PI, std::f32::consts::PI)),
            CyclicWrap::Degree => Some((-360.0, 360.0)),
            CyclicWrap::Custom(lo, hi) => Some((lo.into_inner(), hi.into_inner())),
        }
    }
}

impl Default for CyclicWrap {
    fn default() -> Self {
        Self::None
    }
}
