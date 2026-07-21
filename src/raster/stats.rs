use std::sync::Arc;
use std::thread;

use anyhow::Result;
use gdal::Dataset;
use gdal::errors::GdalError;
use gdal::raster::RasterBand;

use super::RasterHandler;

impl RasterHandler {
    pub(crate) fn band_minmax(&mut self, band: usize) -> Option<(f64, f64)> {
        self.ensure_stats_loaded(band);
        let cache = self.bands_stats.lock().unwrap();
        match cache.get(band - 1) {
            Some(BandStatStatus::Loaded(stats)) => Some((stats.min, stats.max)),
            _ => None,
        }
    }

    pub(crate) fn band_histogram(&mut self, band: usize) -> Option<(f64, f64, Vec<u64>)> {
        self.ensure_stats_loaded(band);
        let cache = self.bands_stats.lock().unwrap();
        match cache.get(band) {
            Some(BandStatStatus::Loaded(stats)) => {
                Some((stats.min, stats.max, stats.counts.clone()))
            }
            _ => None,
        }
    }

    pub(crate) fn band_percentile(&mut self, band: usize, percentile: f64) -> Option<f64> {
        self.ensure_stats_loaded(band);
        let cache = self.bands_stats.lock().unwrap();
        match cache.get(band) {
            Some(BandStatStatus::Loaded(stats)) => {
                value_at_percentile(&stats.counts, stats.min, stats.max, percentile)
            }
            _ => None,
        }
    }

    /// If the band isn't loaded and isn't already loading, kick off a
    /// background thread to compute stats + histogram for it.
    pub(crate) fn ensure_stats_loaded(&self, band: usize) {
        {
            let cache = self.bands_stats.lock().unwrap();
            match cache.get(band - 1) {
                Some(BandStatStatus::NotLoaded) => {}
                _ => return, // already loading / loaded / failed / out of range
            }
        }

        {
            let mut cache = self.bands_stats.lock().unwrap();
            match cache.get_mut(band - 1) {
                Some(slot @ BandStatStatus::NotLoaded) => *slot = BandStatStatus::Loading,
                _ => return, // someone else beat us to it
            }
        }

        let path = self.path.clone();
        let buckets = 200;
        let cache_handle = Arc::clone(&self.bands_stats);

        thread::spawn(move || {
            let result = (|| -> gdal::errors::Result<RasterBandStats> {
                let dataset = Dataset::open(&path)?;
                let rb = dataset.rasterband(band)?;

                let stats = rb.get_statistics(true, true)?.ok_or_else(|| {
                    GdalError::BadArgument("unable to compute raster statistics".into())
                })?;

                let hist = rb.histogram(stats.min, stats.max, buckets as usize, false, true)?;

                Ok(RasterBandStats {
                    min: stats.min,
                    max: stats.max,
                    mean: stats.mean,
                    std: stats.std_dev,
                    counts: hist.counts().to_vec(),
                })
            })();

            let mut cache = cache_handle.lock().unwrap();
            if let Some(slot) = cache.get_mut(band - 1) {
                *slot = match result {
                    Ok(stats) => BandStatStatus::Loaded(stats),
                    Err(e) => BandStatStatus::Failed(e.to_string()),
                };
            }
        });
    }

    pub(crate) fn preload_band_stats(&mut self) {
        let len = self.bands_stats.lock().unwrap().len();

        'scan: for idx in 0..len {
            let should_start = {
                let cache = self.bands_stats.lock().unwrap();
                match cache.get(idx) {
                    Some(BandStatStatus::NotLoaded) => true,
                    Some(BandStatStatus::Loading) => break 'scan,
                    _ => false,
                }
            };
            if should_start {
                self.ensure_stats_loaded(idx + 1);
                println!("\n\n\t>>Loading stats for band {}", idx * 1);
                break 'scan;
            }
        }
    }
}

#[derive(Debug)]
pub(crate) struct RasterBandStats {
    min: f64,
    max: f64,
    mean: f64,
    std: f64,
    counts: Vec<u64>,
}

#[derive(Debug)]
pub(crate) enum BandStatStatus {
    NotLoaded,
    Loading,
    Loaded(RasterBandStats),
    Failed(String),
}

impl RasterBandStats {
    pub(crate) fn minmax(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    pub(crate) fn percentile(&self, percentile: f64) -> Option<f64> {
        value_at_percentile(&self.counts, self.min, self.max, percentile)
    }

    pub(crate) fn from_rasterband(raster_band: &RasterBand, buckets: usize) -> Result<Self> {
        let stats = raster_band.get_statistics(true, true)?;

        let stats = stats.ok_or_else(|| {
            GdalError::BadArgument("unable to compute raster statistics (min/max/mean/std)".into())
        })?;

        let min = stats.min;
        let max = stats.max;
        let mean = stats.mean;
        let std = stats.std_dev;

        let histogram = raster_band.histogram(min, max, buckets, false, true)?;
        let counts: Vec<u64> = histogram.counts().to_vec();

        Ok(Self {
            min,
            max,
            mean,
            std,
            counts,
        })
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
