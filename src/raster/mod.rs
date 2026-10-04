use anyhow::Result;
use gdal::{Dataset, Metadata, raster::RasterBand};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::{ops::Deref, path::Path};

use crate::{
    raster::stats::BandStatStatus,
    viewers::{
        coords::GeoBox,
        thread::TextureWorker,
        tiler::{TextureCache, Tile, TileDescriptor, TileWeighter},
    },
};

pub(crate) mod loading;
pub(crate) mod stats;
pub(crate) mod xml_vrt;

#[derive(Debug)]
pub(crate) struct RasterHandler {
    path: String,
    gdal_dataset: Dataset,
    raster_metadata: RasterMetadata,
    texture_worker: TextureWorker,
    texture_cache: TextureCache,
    on_screen_texture_retainer: HashSet<Tile>,
    pending_tiles: HashSet<TileDescriptor>,
    pub(crate) bands_stats: Arc<Mutex<Vec<BandStatStatus>>>,
}

impl Deref for RasterHandler {
    type Target = Dataset;

    fn deref(&self) -> &Self::Target {
        &self.gdal_dataset
    }
}

impl RasterHandler {
    const CACHE_EXPECTED_MAXIMUM_ELEMENTS: usize = 500;

    pub(crate) fn raster_path(&self) -> &String {
        &self.path
    }

    pub(crate) fn is_single_band(&self) -> bool {
        self.raster_count() == 1
    }

    pub(crate) fn metadata(&self) -> &RasterMetadata {
        &self.raster_metadata
    }

    pub(crate) fn as_owned_dataset(&self) -> Result<Dataset> {
        let dataset = gdal::Dataset::open(&self.path)?;
        Ok(dataset)
    }

    pub(crate) fn refresh_dataset_only(&mut self) -> Result<()> {
        self.gdal_dataset = self.as_owned_dataset()?;
        Ok(())
    }

    pub(crate) fn new(path: impl AsRef<Path>, ctx: egui::Context, cache_size: u64) -> Result<Self> {
        let gdal_dataset = Dataset::open(&path)?;
        let dataset_for_thread = Dataset::open(&path)?;
        let raster_metadata = RasterMetadata::try_from_dataset(&gdal_dataset)?;

        let texture_worker = TextureWorker::new(ctx, dataset_for_thread);
        let texture_cache = TextureCache::with_weighter(
            Self::CACHE_EXPECTED_MAXIMUM_ELEMENTS,
            cache_size,
            TileWeighter,
        );

        let bands_stats = Arc::new(Mutex::new(
            gdal_dataset
                .rasterbands()
                .enumerate()
                .map(|_| BandStatStatus::NotLoaded)
                .collect(),
        ));

        Ok(Self {
            path: path.as_ref().to_string_lossy().into_owned(),
            gdal_dataset,
            raster_metadata,
            texture_worker,
            texture_cache,
            on_screen_texture_retainer: Default::default(),
            pending_tiles: Default::default(),
            bands_stats,
        })
    }

    /// Fetch the raster geotransform for conversion between `PixelBox` and `GeoBox`
    pub(crate) fn get_pixel_geotransform(&self) -> Option<GeoTransform> {
        self.geo_transform().ok().map(GeoTransform::from)
    }

    pub(crate) fn refresh_cache(&mut self, cache_size: u64) {
        self.texture_cache = TextureCache::with_weighter(
            Self::CACHE_EXPECTED_MAXIMUM_ELEMENTS,
            cache_size,
            TileWeighter,
        );
        self.on_screen_texture_retainer = Default::default();
    }

    pub(crate) fn update_cache_size(&mut self, capacity: u64) {
        self.texture_cache.set_capacity(capacity);
    }

    pub(crate) fn band_is_complex(&self, _band: usize) -> bool {
        false // TODO
    }

    /// Returns true if the dataset already has an overview fine enough to
    /// serve reads at roughly `tile_size` resolution, so a new pyramid
    /// build isn't needed.
    pub(crate) fn has_all_overviews(&self, tile_size: usize) -> bool {
        let band_count = self.raster_count();
        if band_count == 0 {
            return false;
        }

        for band_idx in 1..=band_count {
            let Ok(band) = self.rasterband(band_idx) else {
                return false;
            };

            let overview_count = band.overview_count().unwrap_or(0);
            if overview_count == 0 {
                return false;
            }

            let base_size = band.size();
            // Smallest (most decimated) overview's size tells us the coarsest
            // level available; if even that is still finer than tile_size,
            // we don't have enough overview levels.
            let mut finest_available = base_size.0;
            let mut coarsest_available = base_size.0;

            for i in 0..overview_count {
                let Ok(ov) = band.overview(i as usize) else {
                    return false;
                };
                let ov_size = ov.size().0;
                finest_available = finest_available.min(ov_size);
                coarsest_available = coarsest_available.min(ov_size);
                let _ = finest_available; // silence unused if not needed elsewhere
                if ov_size < coarsest_available || i == 0 {
                    coarsest_available = ov_size;
                }
            }

            // We're satisfied only if the pyramid goes at least as coarse as
            // what tile_size needs (i.e. some overview level is <= tile_size).
            if coarsest_available > tile_size {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
pub(crate) struct RasterMetadata {
    pub(crate) driver: String,
    pub(crate) description: String,
    pub(crate) size: (usize, usize),
    pub(crate) band_nb: usize,
    pub(crate) projection: String,
    pub(crate) geotransform: GeoTransform,
    pub(crate) bbox: Option<GeoBox>,
    pub(crate) bands: Vec<BandMetadata>,
}

impl RasterMetadata {
    pub(crate) fn try_from_dataset(dataset: &Dataset) -> Result<Self> {
        let size = dataset.raster_size();
        let geotransform = dataset
            .geo_transform()
            .ok()
            .map(GeoTransform::from)
            .unwrap_or_default();
        let bbox = geotransform.as_geobox(size);

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
pub(crate) struct BandMetadata {
    pub(crate) description: String,
    pub(crate) dtype: String,
    pub(crate) unit: String,
    pub(crate) ndv: Option<f64>,
    pub(crate) scale: Option<f64>,
    pub(crate) offset: Option<f64>,
    pub(crate) overviews: Vec<[usize; 3]>,
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
        let description = band.description().unwrap_or_default();

        tracing::info!(
            "Loaded band metadata {} with description '{}'",
            band_id,
            description
        );

        BandMetadata {
            description,
            dtype: band.band_type().name(),
            unit: band.unit(),
            ndv: band.no_data_value(),
            scale: band.scale(),
            offset: band.offset(),
            overviews,
        }
    }
}

/// > A geotransform is an affine transformation from the image coordinate space (row, column), also known as (pixel, line) to the georeferenced coordinate space (projected or geographic coordinates).
///
/// [GDAL documentation](https://gdal.org/en/stable/tutorials/geotransforms_tut.html)
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct GeoTransform {
    /// x-coordinate of the upper-left corner of the upper-left pixel
    pub(crate) x_off: f64,
    /// w-e pixel resolution / pixel width
    pub(crate) x_res: f64,
    /// row rotation (typically zero)
    pub(crate) x_rot: f64,
    /// y-coordinate of the upper-left corner of the upper-left pixel
    pub(crate) y_off: f64,
    /// column rotation (typically zero)
    pub(crate) y_rot: f64,
    /// n-s pixel resolution / pixel height (negative value for a north-up image)
    pub(crate) y_res: f64,
}

impl From<[f64; 6]> for GeoTransform {
    fn from(value: [f64; 6]) -> Self {
        GeoTransform {
            x_off: value[0],
            x_res: value[1],
            x_rot: value[2],
            y_off: value[3],
            y_rot: value[4],
            y_res: value[5],
        }
    }
}

impl Default for GeoTransform {
    fn default() -> Self {
        GeoTransform {
            x_off: 0.0,
            x_res: 1.0,
            x_rot: 0.0,
            y_off: 0.0,
            y_rot: 0.0,
            y_res: 1.0,
        }
    }
}
