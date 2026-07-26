use anyhow::Result;
use gdal::{Dataset, Metadata, raster::RasterBand};
use std::collections::HashSet;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::{ops::Deref, path::Path};

use crate::{
    raster::stats::BandStatStatus,
    viewers::{
        coords::{self, GeoBox, GeoTransform},
        thread::TextureWorker,
        tiler::{TextureCache, Tile, TileDescriptor, TileWeighter},
    },
};

pub(crate) mod loading;
pub(crate) mod stats;
pub(crate) mod ui;

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
    pub(crate) fn get_pixel_geotransform(&self) -> Option<coords::GeoTransform> {
        self.geo_transform()
            .ok()
            .map(|gt| coords::GeoTransform::from(gt))
    }

    pub(crate) fn refresh_cache(&mut self, cache_size: u64) {
        self.texture_cache = TextureCache::with_weighter(
            Self::CACHE_EXPECTED_MAXIMUM_ELEMENTS,
            cache_size,
            TileWeighter,
        );
        self.on_screen_texture_retainer = Default::default();
    }

    pub(crate) fn band_is_complex(&self, band: usize) -> bool {
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
                coarsest_available = coarsest_available.min(ov_size).max(0);
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
    pub(crate) fn try_from_dataset(dataset: &Dataset) -> Result<Self> {
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
pub(crate) struct BandMetadata {
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
