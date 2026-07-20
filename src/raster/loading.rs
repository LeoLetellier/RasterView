use anyhow::Result;
use egui::ColorImage;
use egui_plot::PlotPoint;
use gdal::Dataset;
use gdal::raster::{Buffer, ResampleAlg};
use std::collections::HashSet;

use super::RasterHandler;
use crate::viewers::ViewMode;
use crate::viewers::coords::Bbox;
use crate::viewers::tiler::{Tile, TileDescriptor};

impl RasterHandler {
    /// Read a portion of a specific band.
    ///
    /// Let `resampling_alg` to `None` to use Nearest Neighbor resampling (fastest)
    #[deprecated]
    fn read_band_at_scale(
        &self,
        band: &gdal::raster::RasterBand,
        x_off: isize,
        y_off: isize,
        native_w: usize,
        native_h: usize,
        out_w: usize,
        out_h: usize,
        target_downscaling: usize,
        resampling_alg: Option<ResampleAlg>,
    ) -> gdal::raster::Buffer<f32> {
        let overview_count = band.overview_count().unwrap_or(0);

        // Raster Size
        let (band_full_w, band_full_h) = band.size();

        if overview_count > 0 {
            let mut best: Option<(usize, gdal::raster::RasterBand)> = None;

            // Search for a suitable overview
            for idx in 0..overview_count {
                if let Ok(ov) = band.overview(idx as usize) {
                    let (ov_w, _ov_h) = ov.size();
                    let factor = (band_full_w / ov_w.max(1)).max(1);

                    if factor <= target_downscaling {
                        let is_better = best
                            .as_ref()
                            .map(|(best_factor, _)| factor > *best_factor)
                            .unwrap_or(true);
                        if is_better {
                            best = Some((factor, ov));
                        }
                    }
                }
            }

            // Then read from nearest overview
            if let Some((factor, overview_band)) = best {
                let ov_x_off = x_off / factor as isize;
                let ov_y_off = y_off / factor as isize;
                let ov_w = (native_w / factor).max(1);
                let ov_h = (native_h / factor).max(1);

                if let Ok(buffer) = overview_band.read_as::<f32>(
                    (ov_x_off, ov_y_off),
                    (ov_w, ov_h),
                    (out_w, out_h),
                    resampling_alg,
                ) {
                    return buffer;
                }
            }
        }

        // If no overview just read from whole raster
        band.read_as::<f32>(
            (x_off, y_off),
            (native_w, native_h),
            (out_w, out_h),
            resampling_alg,
        )
        .expect("failed to read raster band")
    }

    #[deprecated]
    fn read_as_time_series(
        &self,
        band_range: Option<(usize, usize)>,
        x_target: usize,
        y_target: usize,
        padding_width: usize,
    ) -> Vec<gdal::raster::Buffer<f32>> {
        todo!()
    }

    #[deprecated]
    pub(crate) fn to_colorimage_direct(&self, band: usize) -> Result<ColorImage> {
        let raster_band = self.rasterband(band)?;
        let (sizex, sizey) = self.raster_size();
        let array = raster_band.read_as::<f32>((0, 0), (sizex, sizey), (sizex, sizey), None)?;

        let data: &[f32] = &array.data();

        // Compute min/max, ignoring NaN (nodata is often read back as NaN by GDAL,
        // but check raster_band.no_data_value() too if you need exact nodata masking).
        let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
        for &v in data {
            if v.is_finite() {
                if v < min {
                    min = v;
                }
                if v > max {
                    max = v;
                }
            }
        }

        // Guard against an all-NaN band or a flat (min == max) band, both of which
        // would otherwise divide by zero / produce a blank image.
        let range = if (max - min).abs() > f32::EPSILON {
            max - min
        } else {
            1.0
        };
        if !min.is_finite() || !max.is_finite() {
            min = 0.0;
        }

        let mut rgba = Vec::with_capacity(sizex * sizey * 4);
        for &v in data {
            let byte = if v.is_finite() {
                (((v - min) / range) * 255.0).clamp(0.0, 255.0) as u8
            } else {
                0 // or some sentinel color for nodata, e.g. magenta for debugging
            };
            rgba.push(byte);
            rgba.push(byte);
            rgba.push(byte);
            rgba.push(255);
        }

        Ok(ColorImage::from_rgba_unmultiplied([sizex, sizey], &rgba))
    }

    pub(crate) fn request_cache_tiles(
        &mut self,
        tile_descriptions: &Vec<TileDescriptor>,
        view_center: PlotPoint,
        view_mode: ViewMode,
    ) -> Result<Vec<Tile>> {
        // Pull in everything the background thread finished since last frame
        for tile in self.texture_worker.poll_results() {
            self.pending_tiles.remove(&tile.tile_descriptor);
            self.texture_cache
                .insert(tile.tile_descriptor.clone(), tile);
        }

        // Split requested descriptors into cache hits and misses
        let mut missing: Vec<TileDescriptor> = Vec::new();
        let mut hits: Vec<Tile> = Vec::with_capacity(tile_descriptions.len());
        for td in tile_descriptions {
            match self.texture_cache.get(td) {
                Some(tile) => hits.push(tile),
                None => missing.push(td.clone()),
            }
        }

        // Guard against tiles not fitting in memory
        if let Some(first_hit) = hits.first() {
            let texture_size = first_hit.texture.byte_size();
            let hits_size = hits.len() * texture_size;
            let capacity = self.texture_cache.capacity() as usize;
            if hits_size < capacity {
                let nb_texture_fitting = (capacity - hits_size)
                    .div_ceil(texture_size)
                    .min(missing.len());
                missing.truncate(nb_texture_fitting);
            }
        }

        // Tell the worker what's still relevant, so it can drop stale queued jobs
        self.texture_worker.set_wanted(missing.iter().cloned());

        // Drop pending-tracking for tiles no longer requested this frame —
        // otherwise they can get stuck "pending" forever once the worker skips them
        let missing_set: HashSet<&TileDescriptor> = missing.iter().collect();
        self.pending_tiles.retain(|td| missing_set.contains(td));

        // Nearest-first, so the worker chews through what's on screen first
        missing.sort_by(|a, b| {
            a.distance_to(view_center)
                .partial_cmp(&b.distance_to(view_center))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for td in missing {
            if self.pending_tiles.insert(td.clone()) {
                self.texture_worker.request_load((td, view_mode.clone()))?;
            }
        }

        if cfg!(debug_assertions) {
            let cache_len = self.texture_cache.len();
            let cache_weight = self.texture_cache.weight().div_ceil(1024 * 1024);
            println!("Number of elements in cache: {} textures", cache_len);
            println!("Weight of elements in cache: {} MB", cache_weight);
            println!(
                "Total memory use in cache: {} MB",
                self.texture_cache
                    .memory_used()
                    .total()
                    .div_ceil(1024 * 1024)
            )
        }

        Ok(hits)
    }
}

impl TileDescriptor {
    pub(crate) fn read_buffer(&self, dataset: &Dataset, band: usize) -> Result<Buffer<f32>> {
        let raster_band = dataset.rasterband(band)?;
        let raster_size = dataset.raster_size();
        let nodata = raster_band.no_data_value();

        let pixel_bbox = &self.pixel_bbox;

        // Full-resolution window into the raster we want to read
        // Count from ymax for Y direction offset
        let offset = (pixel_bbox.xmin(), raster_size.1 - pixel_bbox.ymax());
        let window_size = (pixel_bbox.width(), pixel_bbox.height());

        // Output buffer size after downsampling — GDAL will decimate/resample
        // while reading when this is smaller than window_size.
        let buffer_size = self.tile_pixel_size();

        let mut buffer = raster_band.read_as::<f32>(
            (offset.0 as isize, offset.1 as isize),
            window_size,
            buffer_size,
            None,
        )?;

        // Treat ndv and non finite nbs as NaN
        clean_nodata_and_nonfinite(&mut buffer, nodata);

        Ok(buffer)
    }

    pub(crate) fn read_3buffers(
        &self,
        dataset: &Dataset,
        bands: (usize, usize, usize),
    ) -> Result<(Buffer<f32>, Buffer<f32>, Buffer<f32>)> {
        let (b0, b1, b2) = bands;

        let buf0 = self.read_buffer(dataset, b0)?;
        let buf1 = self.read_buffer(dataset, b1)?;
        let buf2 = self.read_buffer(dataset, b2)?;

        Ok((buf0, buf1, buf2))
    }
}

/// Replace nodata and non-finite (NaN/Inf) values in-place with NaN,
/// so downstream code has a single, consistent sentinel to check for.
fn clean_nodata_and_nonfinite(buffer: &mut Buffer<f32>, nodata: Option<f64>) {
    match nodata {
        Some(nd) => {
            let nd = nd as f32;
            let tol = f32::EPSILON.max(nd.abs() * 1e-5);
            for px in buffer.data_mut() {
                if !px.is_finite() || (*px - nd).abs() <= tol {
                    *px = f32::NAN;
                }
            }
        }
        None => {
            for px in buffer.data_mut() {
                if !px.is_finite() {
                    *px = f32::NAN;
                }
            }
        }
    }
}
