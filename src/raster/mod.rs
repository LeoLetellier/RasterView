use anyhow::{Result, anyhow};
use egui::ColorImage;
use gdal::{
    Dataset, Metadata,
    raster::{RasterBand, ResampleAlg},
};
use rayon::prelude::*;
use std::{ops::Deref, path::Path};

use crate::viewers::{
    coords::{self, Bbox, GeoBox, GeoTransform, PixelBox},
    tiler::{Tile, TileDescriptor},
};
use std::sync::Arc;

pub mod ui;

#[derive(Debug)]
pub struct RasterHandler(Dataset, RasterMetadata);

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
        &self.0
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

    pub(crate) fn new(path: impl AsRef<Path>) -> Result<Self> {
        let dataset = Dataset::open(&path)?;
        let metadata = RasterMetadata::try_from_dataset(&dataset)?;

        Ok(Self(dataset, metadata))
    }

    fn update_dataset(&mut self, path: &Path) -> Result<&Self> {
        gdal::Dataset::open(path)?;
        Ok(self)
    }

    pub fn raster_size(&self) -> (usize, usize) {
        self.0.raster_size()
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

    pub fn tile_to_texture_direct_par(
        &self,
        tile_descriptor: TileDescriptor,
        ui: &mut egui::Ui,
    ) -> Result<Tile> {
        let raster_band = self.rasterband(tile_descriptor.band)?;

        let pixel_bbox = &tile_descriptor.pixel_bbox;
        let downsampling = tile_descriptor.downsampling;

        // Full-resolution window into the raster we want to read
        let offset = (pixel_bbox.xmin(), pixel_bbox.ymin());
        let window_size = (pixel_bbox.width(), pixel_bbox.height());

        // Output buffer size after downsampling — GDAL will decimate/resample
        // while reading when this is smaller than window_size.
        let (out_width, out_height) = pixel_bbox.size_with_downsampling(downsampling);

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
}
