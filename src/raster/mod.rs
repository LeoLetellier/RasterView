use anyhow::Result;
use egui::{ColorImage, TextureHandle};
use egui_plot::PlotPoint;
use gdal::{
    Dataset, Metadata,
    raster::{Buffer, RasterBand, ResampleAlg},
};
use rayon::prelude::*;
use std::{ops::Deref, path::Path};

use crate::viewers::{
    ViewMode,
    coords::{self, Bbox, GeoBox, GeoTransform, PixelBox},
    thread::TextureWorker,
    tiler::{TextureCache, Tile, TileDescriptor, TileWeighter},
};
use std::collections::HashSet;
use std::sync::Arc;

pub mod ui;

#[derive(Debug)]
pub struct RasterHandler {
    gdal_dataset: Dataset,
    raster_metadata: RasterMetadata,
    texture_worker: TextureWorker,
    texture_cache: TextureCache,
    pending_tiles: HashSet<TileDescriptor>,
}

#[derive(Debug)]
pub struct RasterMetadata {
    driver: String,
    description: String,
    size: (usize, usize),
    band_nb: usize,
    projection: String,
    geotransform: Option<GeoTransform>,
    bbox: Option<GeoBox>,
    bands: Vec<BandMetadata>,
}

impl RasterMetadata {
    pub fn try_from_dataset(dataset: &Dataset) -> Result<Self> {
        let size = dataset.raster_size();
        let geotransform = dataset.geo_transform().ok().map(GeoTransform::from);
        let bbox = geotransform.as_ref().and_then(|gt| gt.as_geobox(size));

        let bands = dataset
            .rasterbands()
            .enumerate()
            .map(|(i, b)| {
                let band = b?;
                Ok(BandMetadata::from_band(i, &band))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(RasterMetadata {
            driver: dataset.driver().short_name(),
            description: dataset.description()?,
            size,
            band_nb: dataset.raster_count(),
            projection: dataset.projection(),
            geotransform,
            bbox,
            bands,
        })
    }
}

#[derive(Debug)]
pub struct BandMetadata {
    band_id: usize,
    description: String,
    dtype: String,
    unit: String,
    overview_nb: usize,
    ndv: Option<f64>,
    scale: Option<f64>,
    offset: Option<f64>,
    overviews: Vec<[usize; 3]>,
    // TODO min/max
    // TODO stats
}

impl BandMetadata {
    fn from_band(band_id: usize, band: &RasterBand) -> Self {
        let overview_nb = band.overview_count().unwrap_or(0) as usize;
        let mut overviews = vec![];
        for k in 0..overview_nb {
            if let Ok(o) = band.overview(k) {
                let s = o.size();
                overviews.push([k, s.0, s.1]);
            }
        }

        BandMetadata {
            band_id,
            description: band.description().unwrap_or_default(),
            dtype: band.band_type().name(),
            unit: band.unit(),
            overview_nb,
            ndv: band.no_data_value(),
            scale: band.scale(),
            offset: band.offset(),
            overviews,
        }
    }
}

impl Deref for RasterHandler {
    type Target = Dataset;

    fn deref(&self) -> &Self::Target {
        &self.gdal_dataset
    }
}

impl RasterHandler {
    /// Fetch the raster geotransform for conversion between `PixelBox` and `GeoBox`
    pub fn get_pixel_geotransform(&self) -> Option<coords::GeoTransform> {
        self.geo_transform()
            .ok()
            .map(|gt| coords::GeoTransform::from(gt))
    }

    /// Read a portion of a specific band.
    ///
    /// Let `resampling_alg` to `None` to use Nearest Neighbor resampling (fastest)
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

    fn read_as_time_series(
        &self,
        band_range: Option<(usize, usize)>,
        x_target: usize,
        y_target: usize,
        padding_width: usize,
    ) -> Vec<gdal::raster::Buffer<f32>> {
        todo!()
    }

    pub(crate) fn new(path: impl AsRef<Path>, ctx: egui::Context) -> Result<Self> {
        let gdal_dataset = Dataset::open(&path)?;
        let dataset_for_thread = Dataset::open(&path)?;
        let raster_metadata = RasterMetadata::try_from_dataset(&gdal_dataset)?;

        let texture_worker = TextureWorker::new(ctx, dataset_for_thread);
        let texture_cache = TextureCache::with_weighter(500, 80 * 1024 * 1024, TileWeighter);

        Ok(Self {
            gdal_dataset,
            raster_metadata,
            texture_worker,
            texture_cache,
            pending_tiles: Default::default(),
        })
    }

    fn update_dataset(&mut self, path: &Path) -> Result<&Self> {
        gdal::Dataset::open(path)?;
        Ok(self)
    }

    pub fn to_colorimage_direct(&self, band: usize) -> Result<ColorImage> {
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

    pub fn to_colorimage_direct_par(&self, band: usize) -> Result<ColorImage> {
        let raster_band = self.rasterband(band)?;
        let (sizex, sizey) = self.raster_size();
        let array = raster_band.read_as::<f32>((0, 0), (sizex, sizey), (sizex, sizey), None)?;
        let data: &[f32] = &array.data();

        // Parallel min/max reduction, ignoring NaN.
        let (mut min, mut max) = data
            .par_iter()
            .filter(|v| v.is_finite())
            .fold(
                || (f32::INFINITY, f32::NEG_INFINITY),
                |(min, max), &v| (min.min(v), max.max(v)),
            )
            .reduce(
                || (f32::INFINITY, f32::NEG_INFINITY),
                |(min1, max1), (min2, max2)| (min1.min(min2), max1.max(max2)),
            );

        let range = if (max - min).abs() > f32::EPSILON {
            max - min
        } else {
            1.0
        };
        if !min.is_finite() || !max.is_finite() {
            min = 0.0;
        }

        // Parallel conversion of f32 -> RGBA8, preserving order via par_iter + flat_map_iter.
        let rgba: Vec<u8> = data
            .par_iter()
            .flat_map_iter(|&v| {
                let byte = if v.is_finite() {
                    (((v - min) / range) * 255.0).clamp(0.0, 255.0) as u8
                } else {
                    0
                };
                [byte, byte, byte, 255]
            })
            .collect();

        Ok(ColorImage::from_rgba_unmultiplied([sizex, sizey], &rgba))
    }

    pub fn read_tile(&self, tile_descriptor: &TileDescriptor) -> Result<Buffer<f32>> {
        let raster_band = self.rasterband(tile_descriptor.band)?;
        let raster_size = self.raster_size();
        let nodata = raster_band.no_data_value();

        let pixel_bbox = &tile_descriptor.pixel_bbox;

        // Full-resolution window into the raster we want to read
        // Count from ymax for Y direction offset
        let offset = (pixel_bbox.xmin(), raster_size.1 - pixel_bbox.ymax());
        let window_size = (pixel_bbox.width(), pixel_bbox.height());

        // Output buffer size after downsampling — GDAL will decimate/resample
        // while reading when this is smaller than window_size.
        let buffer_size = tile_descriptor.tile_pixel_size();

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

    pub fn tile_to_texture_direct_par(
        &self,
        tile_descriptor: TileDescriptor,
        ui: &mut egui::Ui,
    ) -> Result<Tile> {
        let raster_band = self.rasterband(tile_descriptor.band)?;
        let raster_size = self.raster_size();

        let pixel_bbox = &tile_descriptor.pixel_bbox;
        let downsampling = tile_descriptor.downsampling;

        // Full-resolution window into the raster we want to read
        // let offset = (pixel_bbox.xmin(), pixel_bbox.ymin());
        let offset = (pixel_bbox.xmin(), raster_size.1 - pixel_bbox.ymax());
        let window_size = (pixel_bbox.width(), pixel_bbox.height());

        // Output buffer size after downsampling — GDAL will decimate/resample
        // while reading when this is smaller than window_size.
        let (out_width, out_height) = pixel_bbox.size_with_downsampling(downsampling);

        // println!("raster size: {} {}", raster_size.0, raster_size.1);
        // println!("tile size: {} {}", window_size.0, window_size.1);
        // println!("offset: {} {}", offset.0, offset.1);
        // println!("downsample size: {} {}", out_width, out_height);
        // println!("proposition: {}", raster_size.1 - pixel_bbox.ymax());

        let array = raster_band.read_as::<f32>(
            (offset.0 as isize, offset.1 as isize),
            window_size,
            (out_width, out_height),
            None,
        )?;
        let data: &[f32] = &array.data();

        // Parallel min/max reduction, ignoring NaN.
        let (mut min, mut max) = data
            .par_iter()
            .filter(|v| v.is_finite())
            .fold(
                || (f32::INFINITY, f32::NEG_INFINITY),
                |(min, max), &v| (min.min(v), max.max(v)),
            )
            .reduce(
                || (f32::INFINITY, f32::NEG_INFINITY),
                |(min1, max1), (min2, max2)| (min1.min(min2), max1.max(max2)),
            );

        let range = if (max - min).abs() > f32::EPSILON {
            max - min
        } else {
            1.0
        };
        if !min.is_finite() || !max.is_finite() {
            min = 0.0;
        }

        // Parallel conversion of f32 -> RGBA8, preserving order via par_iter + flat_map_iter.
        let rgba: Vec<u8> = data
            .par_iter()
            .flat_map_iter(|&v| {
                let byte = if v.is_finite() {
                    (((v - min) / range) * 255.0).clamp(0.0, 255.0) as u8
                } else {
                    0
                };
                [byte, byte, byte, 255]
            })
            .collect();

        let image_cache = Arc::new(ColorImage::from_rgba_unmultiplied(
            [out_width, out_height],
            &rgba,
        ));

        let texure_handle = ui.load_texture(
            format!("texture_tile_{}", tile_descriptor.name()),
            image_cache,
            egui::TextureOptions::NEAREST,
        );

        Ok(Tile {
            tile_descriptor,
            texture: texure_handle,
        })
    }

    pub fn request_cache_tiles2(
        &mut self,
        tile_descriptions: &Vec<TileDescriptor>,
        view_center: PlotPoint,
        view_mode: ViewMode,
    ) -> Result<Vec<Tile>> {
        // Pull in everything the background thread finished since last frame
        for tile in self.texture_worker.poll_results() {
            self.pending_tiles.remove(&tile.tile_descriptor);
            self.texture_cache
                .insert(tile.tile_descriptor.clone(), tile.texture);
        }

        // Split requested descriptors into cache hits and misses
        let mut missing: Vec<TileDescriptor> = Vec::new();
        let mut hits: Vec<Tile> = Vec::with_capacity(tile_descriptions.len());
        for td in tile_descriptions {
            match self.texture_cache.get(td) {
                Some(texture) => hits.push(Tile {
                    tile_descriptor: td.clone(),
                    texture,
                }),
                None => missing.push(td.clone()),
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
        }

        Ok(hits)
    }

    pub fn refresh_cache(&mut self) {
        self.texture_cache = TextureCache::with_weighter(500, 80 * 1024 * 1024, TileWeighter);
    }

    pub fn request_cache_tiles(
        &mut self,
        tile_descriptions: &Vec<TileDescriptor>,
        ui: &mut egui::Ui,
    ) -> Result<Vec<Tile>> {
        // Split requested descriptors into hits and misses
        let mut missing: Vec<TileDescriptor> = Vec::new();
        let mut hits: Vec<(TileDescriptor, TextureHandle)> =
            Vec::with_capacity(tile_descriptions.len());

        for td in tile_descriptions {
            match self.texture_cache.get(td) {
                Some(handle) => hits.push((td.clone(), handle)),
                None => missing.push(td.clone()),
            }
        }

        // Load whatever wasn't in the cache
        let new_tiles: Vec<Tile> = if missing.is_empty() {
            Vec::new()
        } else {
            missing
                .into_iter()
                .map(|td| self.tile_to_texture_direct_par(td, ui))
                .collect::<Result<Vec<Tile>>>()?
        };

        // Insert newly loaded tiles into the cache
        for tile in &new_tiles {
            self.texture_cache
                .insert(tile.tile_descriptor.clone(), tile.texture.clone());
        }

        // Combine cache hits + freshly loaded tiles
        let mut result: Vec<Tile> = hits
            .into_iter()
            .map(|(tile_descriptor, texture)| Tile {
                tile_descriptor,
                texture,
            })
            .collect();
        result.extend(new_tiles);

        let cache_len = self.texture_cache.len();
        let cache_weight = self.texture_cache.weight().div_ceil(1024 * 1024);

        if cfg!(debug_assertions) {
            println!("Number of elements in cache: {} textures", cache_len);
            println!("Weight of elements in cache: {} MB", cache_weight);
        }

        Ok(result)
    }
}

impl TileDescriptor {
    pub fn read_buffer(&self, dataset: &Dataset) -> Result<Buffer<f32>> {
        let raster_band = dataset.rasterband(self.band)?;
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
