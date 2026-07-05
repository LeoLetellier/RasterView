use crate::viewers::coords::{GeoBox, PixelBox};
use crate::viewers::{ActiveViewer, ViewMode};

use anyhow::Result;
use egui::TextureHandle;
use gdal::raster::Buffer;

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

/// Getter for the object in-memory size
pub trait CacheSized {
    fn cache_size(&self) -> usize;
}

/// Common architecture for caching data
#[derive(Debug)]
pub struct BoundedCache<K: Eq + Hash + Clone, V: CacheSized> {
    entries: HashMap<K, Arc<V>>,
    lru_order: VecDeque<K>,
    current_cache_size: usize,
    max_cache_size: usize,
}

impl<K: Eq + Hash + Clone, V: CacheSized> BoundedCache<K, V> {
    /// Initialize the cache with max_bytes
    pub fn new(max_cache_size: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            current_cache_size: 0,
            max_cache_size,
        }
    }

    /// Get the cache tile or fetch-it if not in cache
    pub fn get_or_load(&mut self, key: K, loader: impl FnOnce() -> Result<V>) -> Result<Arc<V>> {
        if let Some(v) = self.entries.get(&key) {
            let v = v.clone();
            self.touch(&key);
            return Ok(v);
        }
        let value = loader()?;
        Ok(self.insert(key, value))
    }

    /// Actualize the last use of in-cache tile
    fn touch(&mut self, key: &K) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
            let k = self.lru_order.remove(pos).unwrap();
            self.lru_order.push_back(k);
        }
    }

    /// Get a tile to cache
    fn insert(&mut self, key: K, value: V) -> Arc<V> {
        let size = value.cache_size();
        self.evict_to_fit(size);
        let arc = Arc::new(value);
        self.entries.insert(key.clone(), arc.clone());
        self.lru_order.push_back(key);
        self.current_cache_size += size;
        arc
    }

    /// Check if need to release a tile due to memory bounds
    fn evict_to_fit(&mut self, incoming: usize) {
        while self.current_cache_size + incoming > self.max_cache_size {
            let pos = self.lru_order.iter().position(|k| {
                self.entries
                    .get(k)
                    .map(|v| Arc::strong_count(v) == 1)
                    .unwrap_or(false)
            });
            match pos {
                Some(i) => {
                    let k = self.lru_order.remove(i).unwrap();
                    if let Some(v) = self.entries.remove(&k) {
                        self.current_cache_size =
                            self.current_cache_size.saturating_sub(v.cache_size());
                    }
                }
                None => break, // all in use
            }
        }
    }
}

/// Caching raw data
#[derive(Debug)]
pub struct CacheTile(Buffer<f32>);

/// Identifier for raw data cache
#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub struct TileId {
    pub tile_x: usize,
    pub tile_y: usize,
    pub tile_band: usize,
    pub downscaling: usize,
}

impl CacheSized for CacheTile {
    fn cache_size(&self) -> usize {
        // count space in bytes from size of f32
        self.0.len() * std::mem::size_of::<f32>()
    }
}

// pub struct CacheTimeSeries {
//     pub values: Vec<f32>,
// }

// pub struct TimeSeriesId;

/// Caching for the egui-texture, easy to drop
pub struct CacheTexture(TextureHandle);

impl fmt::Debug for CacheTexture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "CacheTexture containing TextureHandle with id {:?}",
            self.0.id()
        )
    }
}

impl CacheSized for CacheTexture {
    fn cache_size(&self) -> usize {
        // count space as nb of texture, so unit here
        1
    }
}

impl std::ops::Deref for CacheTexture {
    type Target = TextureHandle;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl CacheTexture {
    pub fn to_texture(&self) -> TextureHandle {
        self.0.clone()
    }
}

/// Identifier for texture cache
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct TextureId {
    pub tile_x: usize,
    pub tile_y: usize,
    pub downscaling: usize,
    pub params: ViewMode,
}

impl TextureId {
    pub fn need_tiles(&self, view_mode: &ViewMode) -> Vec<TileId> {
        let tile_ids = if view_mode.active_viewer == ActiveViewer::Color {
            if let Some(alpha) = view_mode.band_alpha {
                // RGBA
                vec![
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling,
                        tile_band: view_mode.band_1, // R - band_1
                    },
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling,
                        tile_band: view_mode.band_2, // G - band_2
                    },
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling,
                        tile_band: view_mode.band_3, // B - band_3
                    },
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling, // A - band_alpha
                        tile_band: alpha,
                    },
                ]
            } else {
                // RGB
                vec![
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling,
                        tile_band: view_mode.band_1, // R - band_1
                    },
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling,
                        tile_band: view_mode.band_2, // G - band_2
                    },
                    TileId {
                        tile_x: self.tile_x,
                        tile_y: self.tile_y,
                        downscaling: self.downscaling,
                        tile_band: view_mode.band_3, // B - band_3
                    },
                ]
            }
        } else {
            // PANCHRO
            vec![TileId {
                tile_x: self.tile_x,
                tile_y: self.tile_y,
                downscaling: self.downscaling,
                tile_band: view_mode.band_1, // PANCHRO - band_1
            }]
        };

        tile_ids
    }
}

/// Handler for all caching structs
#[derive(Debug)]
pub struct CacheHandler {
    tiles: BoundedCache<TileId, CacheTile>,
    // time_series: BoundedCache<TimeSeriesId, CacheTimeSeries>,
    textures: BoundedCache<TextureId, CacheTexture>,
}

impl CacheHandler {
    fn request_texture(
        &mut self,
        raster_path: &String,
        live_bbox: PixelBox,
        downsampling: usize,
        view_mode: ViewMode,
    ) -> Vec<Result<(TextureId, Arc<CacheTexture>)>> {
        let ask_raw: Vec<TileId> = self.raw_tile_needs(live_bbox, downsampling);
        let ask_texture = self.texture_tile_needs(live_bbox, downsampling);
        let ask_texture_tile: HashSet<TileId> = ask_texture
            .iter()
            .flat_map(|t| t.need_tiles(&view_mode))
            .collect();

        // If you want to prefetch more raw bounds than texture bounds
        let ask_raw_supp: Vec<&TileId> = ask_raw
            .iter()
            .filter(|k| !ask_texture_tile.contains(k))
            .collect();

        // Ask for the needed texture (will fetch raw if needed)
        let mut textures = Vec::with_capacity(ask_texture.len());
        for texture_id in ask_texture {
            let tile_ids = texture_id.need_tiles(&view_mode);
            let result: Result<(TextureId, Arc<CacheTexture>)> = (|| {
                // Load every raw tile this texture needs (1, 3, or 4 bands worth)
                let raws: Vec<Arc<CacheTile>> = tile_ids
                    .iter()
                    .map(|&tile_id| {
                        self.tiles
                            .get_or_load(tile_id, move || load_raw(raster_path, tile_id))
                    })
                    .collect::<Result<Vec<_>>>()?;

                let view_mode = view_mode.clone();
                let tid = texture_id.clone();
                let texture_cache = self.textures.get_or_load(texture_id.clone(), move || {
                    load_texture(raws, tid, view_mode)
                })?;
                Ok((texture_id, texture_cache))
            })();
            textures.push(result);
        }

        // Ask for additionnal pre-fetch raw
        for tile_id in ask_raw_supp {
            let tile_id = *tile_id;
            // drop error silently
            _ = self
                .tiles
                .get_or_load(tile_id, move || load_raw(raster_path, tile_id));
        }

        textures
    }

    /// Determine the minimum texture tiles that should be loaded
    fn texture_tile_needs(&self, live_bbox: PixelBox, downsampling: usize) -> Vec<TextureId> {
        todo!()
    }

    /// Determine the minimum raw tiles that should be loaded
    fn raw_tile_needs(&self, live_bbox: PixelBox, downsampling: usize) -> Vec<TileId> {
        todo!()
    }

    /// Check what band is needed based on the viewer parameters
    fn band_needed(view_mode: ViewMode) -> Vec<usize> {
        if view_mode.active_viewer == ActiveViewer::Color {
            if let Some(a) = view_mode.band_alpha {
                vec![view_mode.band_1, view_mode.band_2, view_mode.band_3, a]
            } else {
                vec![view_mode.band_1, view_mode.band_2, view_mode.band_3]
            }
        } else {
            vec![view_mode.band_1]
        }
    }
}

// Placeholders to put in thread.rs for async loading
//
fn load_raw(raster_path: &String, tile_id: TileId) -> Result<CacheTile> {
    todo!()
}
//
fn load_texture(
    cache_tile: Vec<Arc<CacheTile>>,
    texture_id: TextureId,
    view_mode: ViewMode,
) -> Result<CacheTexture> {
    todo!()
}
