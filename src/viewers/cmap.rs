use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

// static COLORMAP_BLOB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/colormaps.bin"));
// include!(concat!(env!("OUT_DIR"), "/colormaps_registry.rs"));

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ColorInterpretation {
    ranging: ColorRanging,
    // colormap: ColorMap,
}

impl Default for ColorInterpretation {
    fn default() -> Self {
        ColorInterpretation {
            ranging: ColorRanging::MinMax,
            // colormap: ColorMap,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ColorRanging {
    MinMax,
    Percentile(f32, f32),
    Manual(f32, f32),
    GdalInterpretation,
}

impl Eq for ColorRanging {}

impl Hash for ColorRanging {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ColorRanging::MinMax => {}
            ColorRanging::Percentile(a, b) => {
                a.to_bits().hash(state);
                b.to_bits().hash(state);
            }
            ColorRanging::Manual(a, b) => {
                a.to_bits().hash(state);
                b.to_bits().hash(state);
            }
            ColorRanging::GdalInterpretation => {}
        }
    }
}

#[derive(Debug, Deserialize)]
struct ColorMapScheme {
    name: String,
    below: Option<[u8; 4]>,
    above: Option<[u8; 4]>,
    nan: Option<[u8; 4]>,
    stops: Vec<(f32, [u8; 4])>,
}

struct ColorMapLut {
    data: &'static [u8],
}

impl ColorMapLut {
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len() / 4
    }

    #[inline]
    pub fn get(&self, idx: usize) -> [u8; 4] {
        let o = idx * 4;
        [
            self.data[o],
            self.data[o + 1],
            self.data[o + 2],
            self.data[o + 3],
        ]
    }
}

enum ColorMapType {
    Sequencial,
    Divergent,
    Cyclic,
    Other,
}

pub struct ColorMap {
    lut: ColorMapLut,
    below: [u8; 4],
    above: [u8; 4],
    nan: [u8; 4],
    cmap_type: ColorMapType,
}

impl ColorMap {
    pub fn from_name(name: &str) -> Option<Self> {
        todo!()
        // let (_, offset, len) = COLORMAPS.iter().find(|(n, _, _)| *n == name)?;
        // let data = &COLORMAP_BLOB[*offset..*offset + len * 4];
        // Some(ColorMap {
        //     lut: ColorMapLut { data },
        //     below: [0, 0, 0, 255],
        //     above: [255, 255, 255, 255],
        //     nan: [0, 0, 0, 0],
        //     cmap_type: ColorMapType::Sequencial,
        // })
    }

    pub fn apply_into(&self, data: &[f32], ranging: &ColorRanging, out: &mut [u8]) {
        debug_assert_eq!(out.len(), data.len() * 4);
        let (lo, hi) = compute_range(data, ranging);
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

    pub fn apply(&self, data: &[f32], ranging: &ColorRanging) -> Vec<u8> {
        let mut out = vec![0u8; data.len() * 4];
        self.apply_into(data, ranging, &mut out);
        out
    }
}

fn compute_range(data: &[f32], ranging: &ColorRanging) -> (f32, f32) {
    match ranging {
        ColorRanging::Manual(lo, hi) => (*lo, *hi),
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
        ColorRanging::Percentile(p_lo, p_hi) => {
            let mut finite: Vec<f32> = data.iter().copied().filter(|v| v.is_finite()).collect();
            let n = finite.len();
            let lo_idx = ((p_lo / 100.0) * (n - 1) as f32).round() as usize;
            let hi_idx = ((p_hi / 100.0) * (n - 1) as f32).round() as usize;
            finite.select_nth_unstable_by(lo_idx, |a, b| a.partial_cmp(b).unwrap());
            let lo = finite[lo_idx];
            finite.select_nth_unstable_by(hi_idx, |a, b| a.partial_cmp(b).unwrap());
            let hi = finite[hi_idx];
            (lo, hi)
        }
        ColorRanging::GdalInterpretation => todo!("mirror GDAL's default stretch heuristic"),
    }
}
