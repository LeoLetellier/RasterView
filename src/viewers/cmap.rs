use eframe::egui::ColorImage;
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
        Self {
            colormap,
            ..Default::default()
        }
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
        Self::from_entry(&COLORMAPS[DEFAULT_COLORMAP_IDX])
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

    fn from_entry(entry: &ColormapEntry) -> Self {
        let start = entry.offset * 4;
        let end = start + entry.len * 4;
        ColorMap {
            name: entry.name.to_string(),
            lut: ColorMapLut {
                data: &COLORMAP_BLOB[start..end],
            },
            below: entry.below,
            above: entry.above,
            nan: entry.nan,
            cmap_type: entry.cmap_type.clone(),
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        COLORMAPS
            .iter()
            .find(|e| e.name == name)
            .map(Self::from_entry)
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
    pub(crate) fn preview_samples(&self, n: usize) -> Vec<eframe::egui::Color32> {
        let len = self.lut.len();
        (0..n)
            .map(|i| {
                let t = i as f32 / (n.saturating_sub(1)).max(1) as f32;
                let idx = (t * (len - 1) as f32).round() as usize;
                let [r, g, b, a] = self.lut.get(idx);
                eframe::egui::Color32::from_rgba_unmultiplied(r, g, b, a)
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

        // Cyclic: the wrap domain is the period; ranging values are ignored.
        let wrap_bounds = wrap.bounds();
        let cyclic = wrap_bounds.is_some();
        let (mut vmin, mut vmax) = match wrap_bounds {
            Some(b) => b,
            None => (db_mode.convert(vminmax.0), db_mode.convert(vminmax.1)),
        };

        // Invalid limits (NaN, or <= 0 in dB mode): render as "no data".
        if vmin.is_nan() || vmax.is_nan() {
            out.par_chunks_mut(4)
                .for_each(|px| px.copy_from_slice(&self.nan));
            return out;
        }

        // Reversed limits behave like an inverted colormap.
        let mut invert = invert;
        if vmin > vmax {
            std::mem::swap(&mut vmin, &mut vmax);
            invert = !invert;
        }

        let range = (vmax - vmin).max(f32::EPSILON);
        let cyc_scale = n_f / range;
        let top = (n - 1) as f32;

        let resolve = move |idx: usize| -> usize { if invert { n - 1 - idx } else { idx } };
        let (below, above) = if invert {
            (self.above, self.below)
        } else {
            (self.below, self.above)
        };

        data.par_iter()
            .zip(out.par_chunks_mut(4))
            .for_each(|(&raw, px)| {
                let v = db_mode.convert(raw);
                let rgba = if v.is_nan() || (cyclic && !v.is_finite()) {
                    self.nan
                } else if cyclic {
                    let t = (v - vmin) * cyc_scale;
                    // `% n` handles rem_euclid rounding up to exactly n (the seam).
                    let idx = (t.rem_euclid(n_f).round() as usize) % n;
                    self.lut.get(resolve(idx))
                } else if v < vmin {
                    below
                } else if v > vmax {
                    above
                } else {
                    let idx = (((v - vmin) / range) * top).round() as usize;
                    self.lut.get(resolve(idx.min(n - 1)))
                };
                px.copy_from_slice(&rgba);
            });
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum DbMode {
    /// Use linear values
    #[default]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub(crate) enum CyclicWrap {
    #[default]
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
            CyclicWrap::Degree => Some((-180.0, 180.0)),
            CyclicWrap::Custom(lo, hi) => Some((lo.into_inner(), hi.into_inner())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::Color32;
    use std::collections::HashSet;
    use std::f32::consts::{FRAC_PI_2, PI};

    // ---------- helpers ----------

    const LIN: DbMode = DbMode::None;
    const NOWRAP: CyclicWrap = CyclicWrap::None;

    const BELOW: [u8; 4] = [1, 1, 1, 255];
    const ABOVE: [u8; 4] = [2, 2, 2, 255];
    const NAN: [u8; 4] = [3, 3, 3, 255];

    const G0: [u8; 4] = [0, 0, 0, 255];
    const G1: [u8; 4] = [128, 128, 128, 255];
    const G2: [u8; 4] = [255, 255, 255, 255];

    // 3-entry ramp
    static LUT3: [u8; 12] = [0, 0, 0, 255, 128, 128, 128, 255, 255, 255, 255, 255];

    // 4 clearly distinct entries, for wrap tests
    const W0: [u8; 4] = [10, 0, 0, 255];
    const W1: [u8; 4] = [0, 20, 0, 255];
    const W2: [u8; 4] = [0, 0, 30, 255];
    const W3: [u8; 4] = [40, 40, 0, 255];
    static LUT4: [u8; 16] = [10, 0, 0, 255, 0, 20, 0, 255, 0, 0, 30, 255, 40, 40, 0, 255];

    fn cmap_from(lut: &'static [u8]) -> ColorMap {
        ColorMap {
            name: "t".into(),
            lut: ColorMapLut { data: lut },
            below: BELOW,
            above: ABOVE,
            nan: NAN,
            cmap_type: ColorMapType::Sequential,
        }
    }
    fn cmap() -> ColorMap {
        cmap_from(&LUT3)
    }
    fn cmap4() -> ColorMap {
        cmap_from(&LUT4)
    }

    /// 256-entry LUT where entry i = [i, 0, 0, 255].
    fn cmap256() -> ColorMap {
        let lut: Vec<u8> = (0..256).flat_map(|i| [i as u8, 0, 0, 255]).collect();
        cmap_from(Box::leak(lut.into_boxed_slice()))
    }

    fn custom(lo: f32, hi: f32) -> CyclicWrap {
        CyclicWrap::Custom(OrderedFloat(lo), OrderedFloat(hi))
    }

    fn px(out: &[u8], i: usize) -> [u8; 4] {
        out[i * 4..i * 4 + 4].try_into().unwrap()
    }
    fn pixels(out: &[u8]) -> Vec<[u8; 4]> {
        out.chunks_exact(4).map(|c| c.try_into().unwrap()).collect()
    }
    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    const DATA: [f32; 6] = [-1.0, 0.0, 0.5, 1.0, 2.0, f32::NAN];

    #[test]
    fn below_above_normal() {
        let o = cmap().apply(&DATA, (0.0, 1.0), LIN, NOWRAP, false);
        assert_eq!(px(&o, 0), BELOW);
        assert_eq!(px(&o, 1), G0);
        assert_eq!(px(&o, 2), G1);
        assert_eq!(px(&o, 3), G2);
        assert_eq!(px(&o, 4), ABOVE);
        assert_eq!(px(&o, 5), NAN);
    }

    #[test]
    fn below_above_inverted() {
        let o = cmap().apply(&DATA, (0.0, 1.0), LIN, NOWRAP, true);
        assert_eq!(px(&o, 0), ABOVE); // below vmin -> original `above`
        assert_eq!(px(&o, 1), G2); // vmin -> LUT[n-1]
        assert_eq!(px(&o, 3), G0); // vmax -> LUT[0]
        assert_eq!(px(&o, 4), BELOW); // above vmax -> original `below`
        assert_eq!(px(&o, 5), NAN);
    }

    #[test]
    fn invalid_db_limits_give_nan_color() {
        let o = cmap().apply(&[1.0], (0.0, 1.0), DbMode::Intensity, NOWRAP, false);
        assert_eq!(px(&o, 0), NAN);
    }

    // ---------- apply: linear mapping ----------

    #[test]
    fn output_length_and_empty_input() {
        assert!(cmap().apply(&[], (0.0, 1.0), LIN, NOWRAP, false).is_empty());
        assert_eq!(
            cmap()
                .apply(&[0.0; 7], (0.0, 1.0), LIN, NOWRAP, false)
                .len(),
            28
        );
    }

    #[test]
    fn rounding_to_nearest_entry() {
        let o = cmap().apply(&[0.24, 0.26, 0.74, 0.76], (0.0, 1.0), LIN, NOWRAP, false);
        assert_eq!(pixels(&o), vec![G0, G1, G1, G2]);
    }

    #[test]
    fn offset_and_negative_ranges() {
        let o = cmap().apply(
            &[-10.01, -10.0, 0.0, 10.0, 10.01],
            (-10.0, 10.0),
            LIN,
            NOWRAP,
            false,
        );
        assert_eq!(pixels(&o), vec![BELOW, G0, G1, G2, ABOVE]);
    }

    #[test]
    fn non_finite_data_non_wrap() {
        let o = cmap().apply(
            &[f32::INFINITY, f32::NEG_INFINITY, f32::NAN],
            (0.0, 1.0),
            LIN,
            NOWRAP,
            false,
        );
        assert_eq!(pixels(&o), vec![ABOVE, BELOW, NAN]);
    }

    #[test]
    fn alpha_is_copied_verbatim_from_lut() {
        static L: [u8; 8] = [10, 20, 30, 77, 40, 50, 60, 99];
        let o = cmap_from(&L).apply(&[0.0, 1.0], (0.0, 1.0), LIN, NOWRAP, false);
        assert_eq!(pixels(&o), vec![[10, 20, 30, 77], [40, 50, 60, 99]]);
    }

    #[test]
    fn endpoints_never_fall_outside_for_awkward_limits() {
        let cm = cmap256();
        for k in 1..200 {
            let vmin = -(k as f32) * 0.00731;
            let vmax = k as f32 * 0.0137;
            for inv in [false, true] {
                let o = cm.apply(&[vmin, vmax], (vmin, vmax), LIN, NOWRAP, inv);
                let (lo, hi) = if inv { (255, 0) } else { (0, 255) };
                assert_eq!(
                    px(&o, 0),
                    [lo, 0, 0, 255],
                    "vmin, limits=({vmin},{vmax}) inv={inv}"
                );
                assert_eq!(
                    px(&o, 1),
                    [hi, 0, 0, 255],
                    "vmax, limits=({vmin},{vmax}) inv={inv}"
                );
            }
        }
    }

    #[test]
    fn gray_default_is_monotonic_and_invert_reverses() {
        let cm = ColorMap::default();
        let data: Vec<f32> = (0..=1000).map(|i| i as f32 / 1000.0).collect();
        let n = cm.apply(&data, (0.0, 1.0), LIN, NOWRAP, false);
        let i = cm.apply(&data, (0.0, 1.0), LIN, NOWRAP, true);
        for k in 1..data.len() {
            assert!(
                px(&n, k)[0] >= px(&n, k - 1)[0],
                "normal not non-decreasing at {k}"
            );
            assert!(
                px(&i, k)[0] <= px(&i, k - 1)[0],
                "inverted not non-increasing at {k}"
            );
        }
        assert!(px(&n, 0)[0] < px(&n, data.len() - 1)[0]);
    }

    #[test]
    fn invert_equals_reflected_input() {
        // inverted(v) == normal(vmin + vmax - v) for in-range values
        let cm = cmap();
        let a = cm.apply(&[0.0, 0.5, 1.0], (0.0, 1.0), LIN, NOWRAP, true);
        let b = cm.apply(&[1.0, 0.5, 0.0], (0.0, 1.0), LIN, NOWRAP, false);
        assert_eq!(a, b);
    }

    // ---------- apply: reversed / degenerate limits ----------

    #[test]
    fn reversed_limits_equal_inverted_map() {
        let cm = cmap();
        for inv in [false, true] {
            let a = cm.apply(&DATA, (1.0, 0.0), LIN, NOWRAP, inv);
            let b = cm.apply(&DATA, (0.0, 1.0), LIN, NOWRAP, !inv);
            assert_eq!(a, b);
        }
    }

    #[test]
    fn equal_limits_do_not_panic_and_split_three_ways() {
        let o = cmap().apply(&[0.4999, 0.5, 0.5001], (0.5, 0.5), LIN, NOWRAP, false);
        assert_eq!(pixels(&o), vec![BELOW, G0, ABOVE]);
    }

    #[test]
    fn nan_limits_give_all_nan_even_without_db() {
        let o = cmap().apply(&[0.0, 0.5, 1.0], (f32::NAN, 1.0), LIN, NOWRAP, false);
        assert_eq!(pixels(&o), vec![NAN; 3]);
    }

    // ---------- apply: dB modes ----------

    #[test]
    fn db_intensity_mapping() {
        // limits (1, 100) -> 0 dB .. 20 dB
        let data = [0.1, 1.0, 10.0, 100.0, 1000.0, 0.0, -1.0];
        let o = cmap().apply(&data, (1.0, 100.0), DbMode::Intensity, NOWRAP, false);
        assert_eq!(pixels(&o), vec![BELOW, G0, G1, G2, ABOVE, NAN, NAN]);
    }

    #[test]
    fn db_amplitude_mapping() {
        // limits (1, 10) -> 0 dB .. 20 dB; sqrt(10) -> 10 dB (midpoint)
        let data = [0.5, 1.0, 3.162_277_7, 10.0, 20.0];
        let o = cmap().apply(&data, (1.0, 10.0), DbMode::Amplitude, NOWRAP, false);
        assert_eq!(pixels(&o), vec![BELOW, G0, G1, G2, ABOVE]);
    }

    #[test]
    fn db_inverted_swaps_below_above() {
        let o = cmap().apply(
            &[0.1, 1000.0],
            (1.0, 100.0),
            DbMode::Intensity,
            NOWRAP,
            true,
        );
        assert_eq!(pixels(&o), vec![ABOVE, BELOW]);
    }

    #[test]
    fn db_nonpositive_limits_give_all_nan() {
        for lim in [(0.0, 1.0), (-1.0, 1.0), (1.0, 0.0), (0.0, 0.0)] {
            for mode in [DbMode::Intensity, DbMode::Amplitude] {
                let o = cmap().apply(&[0.5, 1.0], lim, mode, NOWRAP, false);
                assert_eq!(pixels(&o), vec![NAN; 2], "limits {lim:?}, {mode:?}");
            }
        }
    }

    #[test]
    fn db_convert_values() {
        assert!(approx(DbMode::None.convert(3.0), 3.0));
        assert!(approx(DbMode::Intensity.convert(100.0), 20.0));
        assert!(approx(DbMode::Amplitude.convert(100.0), 40.0));
        for m in [DbMode::Intensity, DbMode::Amplitude] {
            assert!(m.convert(0.0).is_nan());
            assert!(m.convert(-1.0).is_nan());
        }
    }

    // ---------- apply: cyclic wrap ----------

    #[test]
    fn wrap_custom_period_and_seam() {
        let data = [0.0, 0.25, 0.5, 0.75, 1.0, 1.25, -0.25, -1.0];
        let o = cmap4().apply(&data, (0.0, 1.0), LIN, custom(0.0, 1.0), false);
        // 1.0 wraps to entry 0 (not clamped to the last entry)
        assert_eq!(pixels(&o), vec![W0, W1, W2, W3, W0, W1, W3, W0]);
    }

    #[test]
    fn wrap_tiny_negative_stays_on_first_entry() {
        // rem_euclid rounds up to exactly n here; must wrap to 0, not clamp to n-1
        let o = cmap4().apply(&[-1e-9], (0.0, 1.0), LIN, custom(0.0, 1.0), false);
        assert_eq!(px(&o, 0), W0);
    }

    #[test]
    fn wrap_radian() {
        let data = [-PI, -FRAC_PI_2, 0.0, FRAC_PI_2, PI];
        let o = cmap4().apply(&data, (0.0, 1.0), LIN, CyclicWrap::Radian, false);
        assert_eq!(pixels(&o), vec![W0, W1, W2, W3, W0]);
    }

    #[test]
    fn wrap_degree() {
        let data = [-180.0, -90.0, 0.0, 90.0, 180.0, 360.0];
        let o = cmap4().apply(&data, (0.0, 1.0), LIN, CyclicWrap::Degree, false);
        assert_eq!(pixels(&o), vec![W0, W1, W2, W3, W0, W2]);
    }

    #[test]
    fn wrap_radian_and_degree_agree() {
        let rad = cmap4().apply(
            &[-PI, -FRAC_PI_2, 0.0, FRAC_PI_2, 3.0 * FRAC_PI_2],
            (0.0, 1.0),
            LIN,
            CyclicWrap::Radian,
            false,
        );
        let deg = cmap4().apply(
            &[-180.0, -90.0, 0.0, 90.0, 270.0],
            (0.0, 1.0),
            LIN,
            CyclicWrap::Degree,
            false,
        );
        assert_eq!(rad, deg);
    }

    #[test]
    fn wrap_ignores_ranging_values() {
        let data = [0.0, 0.3, 0.6, 0.9, 1.7];
        let a = cmap4().apply(&data, (0.0, 1.0), LIN, custom(0.0, 1.0), false);
        let b = cmap4().apply(&data, (5.0, 9.0), LIN, custom(0.0, 1.0), false);
        assert_eq!(a, b);
    }

    #[test]
    fn wrap_never_uses_below_above() {
        let data: Vec<f32> = (-400..400).map(|i| i as f32 * 0.013).collect();
        for wrap in [CyclicWrap::Radian, CyclicWrap::Degree, custom(0.0, 1.0)] {
            for inv in [false, true] {
                let o = cmap4().apply(&data, (0.0, 1.0), LIN, wrap, inv);
                for p in pixels(&o) {
                    assert!(p != BELOW && p != ABOVE && p != NAN, "{wrap:?} inv={inv}");
                }
            }
        }
    }

    #[test]
    fn wrap_invert_reverses_entries() {
        let data = [0.0, 0.25, 0.5, 0.75];
        let o = cmap4().apply(&data, (0.0, 1.0), LIN, custom(0.0, 1.0), true);
        assert_eq!(pixels(&o), vec![W3, W2, W1, W0]);
    }

    #[test]
    fn wrap_non_finite_is_nan_color() {
        let data = [f32::NAN, f32::INFINITY, f32::NEG_INFINITY];
        let o = cmap4().apply(&data, (0.0, 1.0), LIN, custom(0.0, 1.0), false);
        assert_eq!(pixels(&o), vec![NAN; 3]);
    }

    #[test]
    fn wrap_reversed_bounds_equal_inverted() {
        let data = [0.0, 0.1, 0.25, 0.5, 0.9];
        let a = cmap4().apply(&data, (0.0, 1.0), LIN, custom(1.0, 0.0), false);
        let b = cmap4().apply(&data, (0.0, 1.0), LIN, custom(0.0, 1.0), true);
        assert_eq!(a, b);
    }

    #[test]
    fn wrap_degenerate_bounds_do_not_panic() {
        let o = cmap4().apply(&[0.0, 0.5, 1.0], (0.0, 1.0), LIN, custom(0.5, 0.5), false);
        assert_eq!(o.len(), 12);
    }

    // ---------- apply: parallel path consistency ----------

    #[test]
    fn bulk_apply_matches_per_pixel_apply() {
        let data: Vec<f32> = (0..5000)
            .map(|i| {
                if i % 97 == 0 {
                    f32::NAN
                } else {
                    (i as f32 * 0.37).sin() * 2.0
                }
            })
            .collect();
        let cm = cmap();
        for inv in [false, true] {
            for wrap in [NOWRAP, CyclicWrap::Radian, custom(-1.0, 1.0)] {
                let bulk = cm.apply(&data, (-1.0, 1.0), LIN, wrap, inv);
                for (i, &v) in data.iter().enumerate() {
                    let single = cm.apply(&[v], (-1.0, 1.0), LIN, wrap, inv);
                    assert_eq!(
                        px(&bulk, i),
                        px(&single, 0),
                        "i={i} v={v} wrap={wrap:?} inv={inv}"
                    );
                }
            }
        }
    }

    // ---------- ColorMapLut / ColorMapType ----------

    #[test]
    fn lut_len_and_get() {
        let lut = ColorMapLut { data: &LUT4 };
        assert_eq!(lut.len(), 4);
        assert_eq!(lut.get(0), W0);
        assert_eq!(lut.get(3), W3);
    }

    #[test]
    #[should_panic]
    fn lut_get_out_of_range_panics() {
        ColorMapLut { data: &LUT4 }.get(4);
    }

    #[test]
    fn type_labels() {
        assert_eq!(ColorMapType::Sequential.label(), "Sequential");
        assert_eq!(ColorMapType::Divergent.label(), "Divergent");
        assert_eq!(ColorMapType::Cyclic.label(), "Cyclic");
        assert_eq!(ColorMapType::Other.label(), "Other");
    }

    // ---------- registry (generated data) ----------

    const DEFAULT_NAME: &str = "matplotlib/gray";

    #[test]
    fn registry_is_not_empty() {
        assert!(COLORMAPS.len() > 0);
    }

    #[test]
    fn registry_offsets_are_contiguous_and_cover_blob() {
        let mut expected = 0;
        for e in COLORMAPS {
            assert!(e.len > 0, "{} has empty LUT", e.name);
            assert_eq!(e.offset, expected, "{} offset not contiguous", e.name);
            expected += e.len;
        }
        assert_eq!(expected * 4, COLORMAP_BLOB.len());
    }

    #[test]
    fn registry_names_unique_and_well_formed() {
        let mut seen = HashSet::new();
        for e in COLORMAPS {
            assert!(seen.insert(e.name), "duplicate colormap name {}", e.name);
            let parts: Vec<&str> = e.name.split('/').collect();
            assert_eq!(parts.len(), 2, "{} should be `category/name`", e.name);
            assert!(parts.iter().all(|p| !p.is_empty()), "{}", e.name);
        }
    }

    #[test]
    fn registry_luts_are_opaque() {
        for e in COLORMAPS {
            let cm = ColorMap::from_name(e.name).unwrap();
            for i in 0..cm.lut.len() {
                assert_eq!(cm.lut.get(i)[3], 255, "{} entry {i} not opaque", e.name);
            }
        }
    }

    #[test]
    fn registry_cyclic_maps_close_at_the_seam() {
        // Data-dependent: loosen TOL if a legitimate cyclic map ends slightly off.
        const TOL: i32 = 12;
        for e in COLORMAPS
            .iter()
            .filter(|e| e.cmap_type == ColorMapType::Cyclic)
        {
            let cm = ColorMap::from_name(e.name).unwrap();
            let (a, b) = (cm.lut.get(0), cm.lut.get(cm.lut.len() - 1));
            for c in 0..3 {
                assert!(
                    (a[c] as i32 - b[c] as i32).abs() <= TOL,
                    "{}: first/last differ on channel {c}: {a:?} vs {b:?}",
                    e.name
                );
            }
        }
    }

    #[test]
    fn every_colormap_applies_without_panic() {
        let data = [
            -5.0,
            -0.5,
            0.0,
            0.5,
            1.0,
            5.0,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ];
        let wraps = [
            NOWRAP,
            CyclicWrap::Radian,
            CyclicWrap::Degree,
            custom(0.0, 1.0),
        ];
        let dbs = [LIN, DbMode::Intensity, DbMode::Amplitude];
        for name in ColorMap::names() {
            let cm = ColorMap::from_name(name).unwrap();
            for wrap in wraps {
                for db in dbs {
                    for inv in [false, true] {
                        let o = cm.apply(&data, (0.1, 1.0), db, wrap, inv);
                        assert_eq!(o.len(), data.len() * 4, "{name}");
                    }
                }
            }
        }
    }

    // ---------- ColorMap: construction / lookup ----------

    #[test]
    fn default_is_registry_gray() {
        assert_eq!(COLORMAPS[DEFAULT_COLORMAP_IDX].name, DEFAULT_NAME);
        let d = ColorMap::default();
        assert_eq!(d.name(), DEFAULT_NAME);
        assert_eq!(d, ColorMap::from_name(DEFAULT_NAME).unwrap());
    }

    #[test]
    fn default_is_a_black_to_white_neutral_ramp() {
        let d = ColorMap::default();
        let n = d.lut.len();
        let mut prev = 0;
        for i in 0..n {
            let [r, g, b, _] = d.lut.get(i);
            assert!(r == g && g == b, "entry {i} not neutral");
            assert!(r >= prev, "entry {i} not monotonic");
            prev = r;
        }
        assert!(d.lut.get(0)[0] < 10);
        assert!(d.lut.get(n - 1)[0] > 245);
    }

    #[test]
    fn from_name_roundtrips_every_entry() {
        for e in COLORMAPS {
            let cm = ColorMap::from_name(e.name).unwrap();
            assert_eq!(cm.name(), e.name);
            assert_eq!(cm.lut.len(), e.len);
            assert_eq!(cm.cmap_type(), &e.cmap_type);
            assert_eq!((cm.below, cm.above, cm.nan), (e.below, e.above, e.nan));
        }
    }

    #[test]
    fn from_name_unknown_is_none() {
        assert!(ColorMap::from_name("does/not_exist").is_none());
        assert!(ColorMap::from_name("").is_none());
        assert!(ColorMap::from_name("gray").is_none()); // category prefix required
    }

    #[test]
    fn try_from_ok_and_error_message() {
        assert!(ColorMap::try_from(DEFAULT_NAME).is_ok());
        let err = ColorMap::try_from("nope").unwrap_err().to_string();
        assert!(err.contains("unknown colormap: nope"), "{err}");
    }

    #[test]
    fn names_helpers_match_registry() {
        assert_eq!(ColorMap::names().count(), COLORMAPS.len());
        assert!(ColorMap::names().any(|n| n == DEFAULT_NAME));
        let typed: Vec<_> = ColorMap::names_with_type().collect();
        assert_eq!(typed.len(), COLORMAPS.len());
        for ((n, t), e) in typed.iter().zip(COLORMAPS) {
            assert_eq!((*n, *t), (e.name, &e.cmap_type));
        }
    }

    // ---------- preview_samples ----------

    #[test]
    fn preview_samples_lengths_and_endpoints() {
        let cm = cmap4();
        assert!(cm.preview_samples(0).is_empty());
        assert_eq!(cm.preview_samples(1), vec![Color32::from_rgb(10, 0, 0)]);
        let s = cm.preview_samples(2);
        assert_eq!(
            s,
            vec![Color32::from_rgb(10, 0, 0), Color32::from_rgb(40, 40, 0)]
        );
        assert_eq!(cm.preview_samples(9).len(), 9);
    }

    // ---------- ColorInterpretation ----------

    #[test]
    fn interpretation_defaults() {
        let ci = ColorInterpretation::default();
        assert_eq!(ci.ranging_values(), (0.0, 1.0));
        assert!(!ci.invert_cmap);
        assert_eq!(ci.db_mode, DbMode::None);
        assert_eq!(ci.percentile_clip, OrderedFloat(2.0));
        assert_eq!(ci.cyclic_wrap, CyclicWrap::None);
        assert_eq!(ci.colormap, ColorMap::default());
    }

    #[test]
    fn interpretation_new_keeps_other_defaults() {
        let name = ColorMap::names().find(|n| *n != DEFAULT_NAME).unwrap();
        let ci = ColorInterpretation::new(ColorMap::from_name(name).unwrap());
        assert_eq!(ci.colormap.name(), name);
        assert_eq!(ci.ranging_values(), (0.0, 1.0));
        assert_eq!(ci.db_mode, DbMode::None);
    }

    #[test]
    fn interpretation_ranging_roundtrip() {
        let mut ci = ColorInterpretation::default();
        ci.with_ranging_values((-3.5, 12.25));
        assert_eq!(ci.ranging_values(), (-3.5, 12.25));
    }

    #[test]
    fn interpretation_eq_and_hash_track_every_field() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let h = |c: &ColorInterpretation| {
            let mut s = DefaultHasher::new();
            c.hash(&mut s);
            s.finish()
        };
        let a = ColorInterpretation::default();
        let b = ColorInterpretation::default();
        assert_eq!(a, b);
        assert_eq!(h(&a), h(&b));

        let variants: Vec<ColorInterpretation> = vec![
            ColorInterpretation {
                invert_cmap: true,
                ..Default::default()
            },
            ColorInterpretation {
                db_mode: DbMode::Intensity,
                ..Default::default()
            },
            ColorInterpretation {
                cyclic_wrap: CyclicWrap::Radian,
                ..Default::default()
            },
            ColorInterpretation {
                percentile_clip: OrderedFloat(5.0),
                ..Default::default()
            },
            ColorInterpretation {
                ranging_values: (OrderedFloat(0.0), OrderedFloat(2.0)),
                ..Default::default()
            },
        ];
        for v in &variants {
            assert_ne!(&a, v);
        }
    }

    #[test]
    fn panchro_buffer_to_colorimage_dimensions_and_pixels() {
        use gdal::raster::Buffer;
        // 3 wide x 2 high
        let buf = Buffer::new((3, 2), vec![0.0f32, 0.5, 1.0, 2.0, -1.0, f32::NAN]);
        let ci = ColorInterpretation::default();
        let img = ci.panchro_buffer_to_colorimage(buf);

        assert_eq!(img.size, [3, 2]);
        assert_eq!(img.pixels.len(), 6);

        let c = |p: [u8; 4]| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]);
        let cm = &ci.colormap;
        assert_eq!(img.pixels[0], c(cm.lut.get(0)));
        assert_eq!(img.pixels[2], c(cm.lut.get(cm.lut.len() - 1)));
        assert_eq!(img.pixels[3], c(cm.above));
        assert_eq!(img.pixels[4], c(cm.below));
        assert_eq!(img.pixels[5], c(cm.nan));
    }

    #[test]
    fn panchro_respects_invert_and_ranging() {
        use gdal::raster::Buffer;
        let mut ci = ColorInterpretation::default();
        ci.with_ranging_values((10.0, 20.0));
        ci.invert_cmap = true;
        let img = ci.panchro_buffer_to_colorimage(Buffer::new((2, 1), vec![10.0f32, 20.0]));
        let n = ci.colormap.lut.len();
        let c = |p: [u8; 4]| Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]);
        assert_eq!(img.pixels[0], c(ci.colormap.lut.get(n - 1)));
        assert_eq!(img.pixels[1], c(ci.colormap.lut.get(0)));
    }

    // ---------- normalize_buffer_minmax_db ----------

    #[test]
    fn normalize_linear() {
        use gdal::raster::Buffer;
        let out = normalize_buffer_minmax_db(
            Buffer::new((3, 1), vec![0.0f32, 5.0, 10.0]),
            (0.0, 10.0),
            false,
        );
        assert!(approx(out[0], 0.0) && approx(out[1], 0.5) && approx(out[2], 1.0));
    }

    #[test]
    fn normalize_db() {
        use gdal::raster::Buffer;
        // 10*log10: 1 -> 0 dB, 10 -> 10 dB, 100 -> 20 dB
        let out = normalize_buffer_minmax_db(
            Buffer::new((3, 1), vec![1.0f32, 10.0, 100.0]),
            (1.0, 100.0),
            true,
        );
        assert!(approx(out[0], 0.0) && approx(out[1], 0.5) && approx(out[2], 1.0));
    }

    #[test]
    #[should_panic]
    fn normalize_zero_range_panics() {
        use gdal::raster::Buffer;
        normalize_buffer_minmax_db(Buffer::new((1, 1), vec![1.0f32]), (2.0, 2.0), false);
    }

    // ---------- CyclicWrap::bounds ----------

    #[test]
    fn wrap_bounds() {
        assert_eq!(CyclicWrap::None.bounds(), None);
        assert_eq!(CyclicWrap::Radian.bounds(), Some((-PI, PI)));
        assert_eq!(CyclicWrap::Degree.bounds(), Some((-180.0, 180.0)));
        assert_eq!(custom(0.0, 360.0).bounds(), Some((0.0, 360.0)));
    }
}
