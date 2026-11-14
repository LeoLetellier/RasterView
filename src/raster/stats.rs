use std::fmt;
use std::path::Path;
use std::thread;

use anyhow::Result;
use eframe::egui::Context;
use gdal::Dataset;
use gdal::errors::GdalError;
use gdal::raster::RasterBand;
use poll_promise::Promise;

use super::RasterHandler;

const HIST_BUCKETS: usize = 200;

type StatsResult = Result<RasterBandStats, String>;

pub(crate) enum BandStatStatus {
    NotLoaded,
    /// Started. The promise holds Ok(stats) or Err(message) once finished.
    Requested(Promise<StatsResult>),
}

impl fmt::Debug for BandStatStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotLoaded => f.write_str("NotLoaded"),
            Self::Requested(p) => match p.ready() {
                None => f.write_str("Loading"),
                Some(Ok(s)) => write!(f, "Loaded(min={}, max={})", s.min, s.max),
                Some(Err(e)) => write!(f, "Failed({e:?})"),
            },
        }
    }
}

impl BandStatStatus {
    /// Stats if the job finished successfully.
    fn stats(&self) -> Option<&RasterBandStats> {
        match self {
            Self::Requested(p) => p.ready()?.as_ref().ok(),
            Self::NotLoaded => None,
        }
    }

    pub(crate) fn is_loading(&self) -> bool {
        matches!(self, Self::Requested(p) if p.ready().is_none())
    }

    pub(crate) fn error(&self) -> Option<&str> {
        match self {
            Self::Requested(p) => p.ready()?.as_ref().err().map(String::as_str),
            Self::NotLoaded => None,
        }
    }
}

impl RasterHandler {
    /// `band` is 1-based everywhere (GDAL convention). Convert once, here.
    fn band_slot(&self, band: usize) -> Option<&BandStatStatus> {
        self.bands_stats.get(band.checked_sub(1)?)
    }

    fn band_stats(&self, band: usize) -> Option<&RasterBandStats> {
        self.band_slot(band)?.stats()
    }

    // ---- plain getters: no lock, no loading, just read the current state ----

    pub(crate) fn band_minmax(&self, band: usize) -> Option<(f64, f64)> {
        self.band_stats(band).map(|s| (s.min, s.max))
    }

    pub(crate) fn band_histogram(&self, band: usize) -> Option<(f64, f64, &[u64])> {
        self.band_stats(band)
            .map(|s| (s.min, s.max, s.counts.as_slice()))
    }

    pub(crate) fn band_percentile(&self, band: usize, percentile: f64) -> Option<f64> {
        let s = self.band_stats(band)?;
        value_at_percentile(&s.counts, s.min, s.max, percentile)
    }

    // ---- "or_load" variants: kick off loading, then read whatever exists ----

    pub(crate) fn band_minmax_or_load(&mut self, ctx: &Context, band: usize) -> Option<(f64, f64)> {
        self.ensure_stats_loaded(ctx, band);
        self.band_minmax(band)
    }

    pub(crate) fn band_percentile_or_load(
        &mut self,
        ctx: &Context,
        band: usize,
        percentile: f64,
    ) -> Option<f64> {
        self.ensure_stats_loaded(ctx, band);
        self.band_percentile(band, percentile)
    }

    /// Start a background job for this band unless one was already started.
    pub(crate) fn ensure_stats_loaded(&mut self, ctx: &Context, band: usize) {
        let Some(idx) = band.checked_sub(1) else {
            return;
        };
        // Borrow ends at the end of this statement.
        if !matches!(self.bands_stats.get(idx), Some(BandStatStatus::NotLoaded)) {
            return; // out of range, loading, loaded or failed
        }

        let path = self.path.clone();
        let ctx = ctx.clone();
        let (sender, promise) = Promise::new();

        thread::Builder::new()
            .name(format!("band_stats_{band}"))
            .spawn(move || {
                let result =
                    compute_band_stats(&path, band, HIST_BUCKETS).map_err(|e| e.to_string());
                sender.send(result);
                ctx.request_repaint();
            })
            .expect("failed to spawn stats thread");

        self.bands_stats[idx] = BandStatStatus::Requested(promise);
    }

    /// Call once per frame: starts the next band only when nothing is in flight.
    pub(crate) fn preload_band_stats(&mut self, ctx: &Context) {
        let mut next = None;
        for (i, status) in self.bands_stats.iter().enumerate() {
            if status.is_loading() {
                return; // one at a time
            }
            if next.is_none() && matches!(status, BandStatStatus::NotLoaded) {
                next = Some(i + 1);
            }
        }
        if let Some(band) = next {
            tracing::info!(">>Loading stats for band {band}");
            self.ensure_stats_loaded(ctx, band);
        }
    }
}

/// Runs on the worker thread. Opens its own Dataset (GDAL datasets aren't Send).
fn compute_band_stats(
    path: impl AsRef<Path>,
    band: usize,
    buckets: usize,
) -> gdal::errors::Result<RasterBandStats> {
    let dataset = Dataset::open(path)?;
    let rb = dataset.rasterband(band)?;
    RasterBandStats::from_rasterband(&rb, buckets)
}

#[derive(Debug)]
pub(crate) struct RasterBandStats {
    min: f64,
    max: f64,
    _mean: f64,
    _std: f64,
    counts: Vec<u64>,
}

impl RasterBandStats {
    pub(crate) fn from_rasterband(
        raster_band: &RasterBand,
        buckets: usize,
    ) -> gdal::errors::Result<Self> {
        let stats = raster_band
            .get_statistics(true, true)?
            .ok_or_else(|| GdalError::BadArgument("unable to compute raster statistics".into()))?;

        let histogram = raster_band.histogram(stats.min, stats.max, buckets, false, true)?;

        Ok(Self {
            min: stats.min,
            max: stats.max,
            _mean: stats.mean,
            _std: stats.std_dev,
            counts: histogram.counts().to_vec(),
        })
    }

    pub(crate) fn _minmax(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    pub(crate) fn _percentile(&self, percentile: f64) -> Option<f64> {
        value_at_percentile(&self.counts, self.min, self.max, percentile)
    }
}

/// Compute the value at a given percentile (0.0..=100.0) from an histogram
///
/// # Option
/// return `None` if the count is empty
fn value_at_percentile(counts: &[u64], min: f64, max: f64, percentile: f64) -> Option<f64> {
    if counts.is_empty() || !(0.0..=100.0).contains(&percentile) {
        return None;
    }

    let total: u64 = counts.iter().sum();

    let bucket_width = (max - min) / counts.len() as f64;
    let target = (percentile / 100.0) * total as f64;

    let mut cumulative: u64 = 0;
    for (i, &count) in counts.iter().enumerate() {
        let next_cumulative = cumulative + count;

        if (next_cumulative as f64) >= target {
            // Interpolate within this bucket.
            let bucket_start = min + i as f64 * bucket_width;
            let frac = if count > 0 {
                (target - cumulative as f64) / count as f64
            } else {
                0.0
            };
            return Some(bucket_start + frac * bucket_width);
        }

        cumulative = next_cumulative;
    }

    // Fallback: percentile == 100, return max.
    Some(max)
}
