use gdal::Dataset;
use gdal::errors::{GdalError, Result};
use rayon::prelude::*;

#[derive(Debug)]
pub(crate) struct SerieReader {
    point: (usize, usize),             // (x = col, y = row)
    reference: Option<(usize, usize)>, // (x = col, y = row)
    point_half_size: usize,
    reference_half_size: usize,
}

/// Window pixels for all bands, stored band-major: [band0 pixels | band1 pixels | ...]
#[derive(Debug)]
struct Window {
    data: Vec<f32>,
    pixels_per_band: usize,
    nodata: Vec<Option<f32>>,
}

impl SerieReader {
    /// Init a simple time series around a measurement point
    pub(crate) fn new(point: (usize, usize)) -> Self {
        Self {
            point,
            reference: None,
            point_half_size: 0,
            reference_half_size: 0,
        }
    }

    /// Add a reference to the time series
    pub(crate) fn with_reference(&mut self, reference: (usize, usize)) -> &Self {
        self.reference = Some(reference);
        self
    }

    /// Take the median around the point instead of a single value
    pub(crate) fn with_window(
        &mut self,
        point_half_window: usize,
        reference_half_window: usize,
    ) -> &Self {
        self.point_half_size = point_half_window;
        self.reference_half_size = reference_half_window;
        self
    }

    pub(crate) fn read_on_dataset(&self, dataset: &Dataset) -> Result<Vec<f32>> {
        // 1. Load (sequential, GDAL)
        let point_win = load_window(dataset, self.point, self.point_half_size)?;
        let ref_win = self
            .reference
            .map(|r| load_window(dataset, r, self.reference_half_size))
            .transpose()?;

        // 2. Process (parallel, rayon)
        let mut ts = median_per_band(&point_win);
        if let Some(ref_win) = ref_win {
            let ref_ts = median_per_band(&ref_win);
            ts.par_iter_mut()
                .zip(ref_ts.par_iter())
                .for_each(|(p, r)| *p -= *r);
        }
        Ok(ts)
    }
}

fn load_window(dataset: &Dataset, center: (usize, usize), half: usize) -> Result<Window> {
    let (width, height) = dataset.raster_size();
    let n_bands = dataset.raster_count();

    // Clamp the window to the raster
    let x0 = center.0.saturating_sub(half);
    let y0 = center.1.saturating_sub(half);
    let x1 = (center.0 + half + 1).min(width);
    let y1 = (center.1 + half + 1).min(height);
    if x1 <= x0 || y1 <= y0 {
        return Err(GdalError::BadArgument(format!(
            "window around {center:?} is outside the raster ({width}x{height})"
        )));
    }
    let win_size = (x1 - x0, y1 - y0);
    let pixels_per_band = win_size.0 * win_size.1;

    let mut data = Vec::with_capacity(pixels_per_band * n_bands);
    let mut nodata = Vec::with_capacity(n_bands);

    for b in 1..=n_bands {
        let band = dataset.rasterband(b)?;
        nodata.push(band.no_data_value().map(|v| v as f32));
        let buf = band.read_as::<f32>((x0 as isize, y0 as isize), win_size, win_size, None)?;
        data.extend_from_slice(buf.data());
    }

    Ok(Window {
        data,
        pixels_per_band,
        nodata,
    })
}

fn median_per_band(win: &Window) -> Vec<f32> {
    win.data
        .par_chunks(win.pixels_per_band)
        .zip(win.nodata.par_iter())
        .map(|(chunk, nodata)| {
            let mut vals: Vec<f32> = chunk
                .iter()
                .copied()
                .filter(|v| !v.is_nan() && Some(*v) != *nodata)
                .collect();
            median(&mut vals)
        })
        .collect()
}

fn median(vals: &mut [f32]) -> f32 {
    let n = vals.len();
    if n == 0 {
        return f32::NAN;
    }
    let mid = n / 2;
    let (lower, m, _) = vals.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
    if n % 2 == 1 {
        *m
    } else {
        let lo = lower.iter().copied().max_by(|a, b| a.total_cmp(b)).unwrap();
        (lo + *m) / 2.0
    }
}
