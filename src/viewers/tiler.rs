use crate::raster::RasterHandler;
use crate::viewers::coords::{Bbox, GeoBox, PixelBox};
use crate::viewers::{ActiveViewer, ViewMode, Viewer};

use anyhow::{Result, anyhow};
use egui::{ColorImage, TextureHandle, vec2};
use egui_plot::{PlotImage, PlotUi};
use quick_cache::sync::Cache;
use quick_cache::{
    Weighter,
    sync::{EntryAction, EntryResult},
};

use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;
use std::sync::Arc;

impl Viewer {
    /// Determine the tiles needed for a specific viewport extent
    pub fn need_tiles(&self) -> Option<Vec<TileDescriptor>> {
        let rh = self.raster_handler.as_ref()?;
        let lb = self.state.last_bounds?;

        let (full_width, full_height) = rh.raster_size();

        // Determine the current zoom
        let view_extent = (lb.width(), lb.height());
        let screen_size = self.state.last_screen_size?; // needs to be captured from plot_ui.response().rect

        let downsampling: usize = self.need_zoom(view_extent, screen_size)?;
        // Check against the viewmode for which band to load
        let bands: Vec<usize> = self.view_mode.need_bands();

        // Check against viewport bounds to determine which tiles are needed
        let pixel_bboxes: Vec<PixelBox> = self.tile_in_view(
            (full_width, full_height),
            downsampling,
            self.parameters.viewport_padding,
        )?;

        // Cartesian product: every needed tile, for every needed band.
        let descriptors = pixel_bboxes
            .into_iter()
            .flat_map(|pixel_bbox| {
                bands.iter().map(move |&band| TileDescriptor {
                    pixel_bbox: pixel_bbox.clone(),
                    band,
                    downsampling,
                })
            })
            .collect();

        Some(descriptors)
    }

    /// Determine the level of zoom needed from a specific viewport extent
    fn need_zoom(&self, view_extent: (f64, f64), screen_size: (f64, f64)) -> Option<usize> {
        let (view_w, view_h) = view_extent; // visible raster pixels (from PlotBounds width/height)
        let (screen_w, screen_h) = screen_size; // actual widget size in screen pixels

        if screen_w == 0.0 || screen_h == 0.0 {
            return None;
        }

        // Raster pixels shown per screen pixel, per axis.
        let ratio_x = view_w / screen_w;
        let ratio_y = view_h / screen_h;

        let ratio = ratio_x.max(ratio_y).max(1.0);

        Some(ratio.log2().floor().max(0.0) as usize)
    }

    /// Tile the raster at given downsampling factor and gives all possible tile bboxes
    fn tiler(&self, raster_size: (usize, usize), downsampling: usize) -> Vec<PixelBox> {
        let tile_size = self.parameters.tile_size;
        debug_assert!(tile_size > 0, "tile_size must be nonzero, got {tile_size}");

        let factor = 1usize << downsampling;
        let raster_tile_extent = tile_size * factor;
        debug_assert!(
            raster_tile_extent > 0,
            "raster_tile_extent must be nonzero (tile_size={tile_size}, factor={factor})"
        );

        let (raster_w, raster_h) = raster_size;

        (0..raster_h)
            .step_by(raster_tile_extent)
            .flat_map(|y| {
                let ymax = (y + raster_tile_extent).min(raster_h);
                (0..raster_w).step_by(raster_tile_extent).map(move |x| {
                    let xmax = (x + raster_tile_extent).min(raster_w);
                    PixelBox::from([x, xmax, y, ymax])
                })
            })
            .collect()
    }

    /// Find all tiles for a specific downsampling factor that are in view or
    /// nearing the view by `viewport_padding`
    fn tile_in_view(
        &self,
        raster_size: (usize, usize),
        downsampling: usize,
        viewport_padding: f64,
    ) -> Option<Vec<PixelBox>> {
        let tiles_at_downsampling = self.tiler(raster_size, downsampling);
        let viewer_bounds = self.state.last_bounds?;

        let x_range = viewer_bounds.range_x();
        let y_range = viewer_bounds.range_y();
        let (view_xmin, view_xmax) = (*x_range.start(), *x_range.end());
        let (view_ymin, view_ymax) = (*y_range.start(), *y_range.end());

        // Pad outward by `viewport_padding` fraction of each axis's extent
        // (e.g. 0.1 = 10% extra on each side), so tiles just outside the
        // visible area are pre-fetched before they're needed.
        let x_extent = view_xmax - view_xmin;
        let y_extent = view_ymax - view_ymin;
        let x_pad = x_extent * viewport_padding;
        let y_pad = y_extent * viewport_padding;

        let padded_xmin = view_xmin - x_pad;
        let padded_xmax = view_xmax + x_pad;
        let padded_ymin = view_ymin - y_pad;
        let padded_ymax = view_ymax + y_pad;

        let visible = tiles_at_downsampling
            .into_iter()
            .filter(|tile| {
                let txmin = tile.xmin() as f64;
                let txmax = tile.xmax() as f64;
                let tymin = tile.ymin() as f64;
                let tymax = tile.ymax() as f64;

                // AABB overlap test against the padded bounds.
                txmin < padded_xmax
                    && txmax > padded_xmin
                    && tymin < padded_ymax
                    && tymax > padded_ymin
            })
            .collect();

        Some(visible)
    }

    pub fn refresh_tiles(
        &mut self,
        tile_descriptions: &Vec<TileDescriptor>,
        ui: &mut egui::Ui,
    ) -> Result<&Vec<Tile>> {
        let requested: HashSet<&TileDescriptor> = tile_descriptions.iter().collect();

        // Find image colors already in cache
        let cached_tiles: Vec<Tile> = self
            .color_images
            .iter()
            .filter(|t| requested.contains(&t.tile_descriptor))
            .cloned()
            .collect();
        let found: HashSet<&TileDescriptor> =
            cached_tiles.iter().map(|t| &t.tile_descriptor).collect();

        // Figure out which requested descriptors weren't in the cache
        let missing_descriptions: Vec<TileDescriptor> = tile_descriptions
            .iter()
            .filter(|desc| !found.contains(*desc))
            .cloned()
            .collect();

        // Else load them, one at a time (each call is internally parallel)
        let new_tiles: Vec<Tile> = if missing_descriptions.is_empty() {
            Vec::new()
        } else {
            let raster_handler = self
                .raster_handler
                .as_ref()
                .ok_or_else(|| anyhow!("no raster loaded"))?;

            missing_descriptions
                .into_iter()
                .map(|td| raster_handler.tile_to_texture_direct_par(td, ui))
                .collect::<Result<Vec<Tile>>>()?
        };

        // Merge newly loaded tiles into the cache
        self.color_images.extend(new_tiles.iter().cloned());

        // Combine cache hits and newly loaded tiles (order not preserved)
        let new_color_images = cached_tiles.into_iter().chain(new_tiles).collect();
        self.color_images = new_color_images;
        println!("Tiles loaded: {}", self.color_images.len());
        println!("{:?}", self.color_images);
        Ok(&self.color_images)
    }
}

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct TileDescriptor {
    pub pixel_bbox: PixelBox,
    pub band: usize,
    pub downsampling: usize,
}

impl TileDescriptor {
    pub fn pixel_box(&self) -> &PixelBox {
        &self.pixel_bbox
    }

    pub fn name(&self) -> String {
        format!(
            "b{}_s{}_x{}_xx{}_y{}_yy{}",
            self.band,
            self.downsampling,
            self.pixel_bbox.xmin(),
            self.pixel_bbox.xmax(),
            self.pixel_bbox.ymin(),
            self.pixel_bbox.ymax()
        )
    }
}

#[derive(Clone)]
pub struct Tile {
    pub tile_descriptor: TileDescriptor,
    pub texture: TextureHandle,
}

impl fmt::Debug for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tile")
            .field("tile_descriptor", &self.tile_descriptor)
            // .field("texture", &self.texture)
            .finish()
    }
}

impl Tile {
    pub fn plot_ui(&self, plot_ui: &mut PlotUi) {
        let tile_name = self.tile_descriptor.name();
        plot_ui.image(PlotImage::new(
            format!("plot_tile_{tile_name}"),
            self.texture.id(),
            self.tile_descriptor.pixel_bbox.center(),
            vec2(
                self.tile_descriptor.pixel_bbox.width() as f32,
                self.tile_descriptor.pixel_bbox.height() as f32,
            ),
        ));
    }
}

#[derive(Clone)]
struct TileWeighter;

impl Weighter<TileDescriptor, Arc<ColorImage>> for TileWeighter {
    fn weight(&self, _key: &TileDescriptor, val: &Arc<ColorImage>) -> u64 {
        (val.width() * val.height() * 4) as u64 // bytes
    }
}

/// Cache from quick cache
/// # Usage
/// ```rs
/// let cache = ImageCache::with_weighter(500, 64 * 1024 * 1024, TileWeighter); // ~64MB budget
/// ```
pub type ImageCache = Cache<TileDescriptor, Arc<ColorImage>, TileWeighter>;

// pub fn fetch_cache_tile(
//     raster_handler: &RasterHandler,
//     cache: &Cache<TileDescriptor, Arc<ColorImage>, TileWeighter>,
//     tile: TileDescriptor,
// ) -> Result<Tile> {
//     let result = cache.entry(&tile, None, |_key, val| EntryAction::Retain(val.clone()));
//     let TileDescriptor {
//         pixel_bbox,
//         band,
//         downsampling,
//     } = tile;
//     match result {
//         EntryResult::Retained(img) => Ok(Tile {
//             tile_descriptor: tile,
//             texture: img,
//         }),
//         EntryResult::Vacant(guard) => {
//             let img = Arc::new(raster_handler.to_colorimage_direct_par(band)?);
//             let _ = guard.insert(img.clone());
//             Ok(Tile {
//                 tile_descriptor: tile,
//                 texture: img,
//             })
//         }
//         _ => Err(anyhow!(
//             "tile cache unreachable: neither retained nor vacant"
//         )),
//     }
// }

/////////////////////////////////////////////

// /// Getter for the object in-memory size
// pub trait CacheSized {
//     fn cache_size(&self) -> usize;
// }

// /// Common architecture for caching data
// #[derive(Debug)]
// pub struct BoundedCache<K: Eq + Hash + Clone, V: CacheSized> {
//     entries: HashMap<K, Arc<V>>,
//     lru_order: VecDeque<K>,
//     current_cache_size: usize,
//     max_cache_size: usize,
// }

// impl<K: Eq + Hash + Clone, V: CacheSized> BoundedCache<K, V> {
//     /// Initialize the cache with max_bytes
//     pub fn new(max_cache_size: usize) -> Self {
//         Self {
//             entries: HashMap::new(),
//             lru_order: VecDeque::new(),
//             current_cache_size: 0,
//             max_cache_size,
//         }
//     }

//     /// Get the cache tile or fetch-it if not in cache
//     pub fn get_or_load(&mut self, key: K, loader: impl FnOnce() -> Result<V>) -> Result<Arc<V>> {
//         if let Some(v) = self.entries.get(&key) {
//             let v = v.clone();
//             self.touch(&key);
//             return Ok(v);
//         }
//         let value = loader()?;
//         Ok(self.insert(key, value))
//     }

//     /// Actualize the last use of in-cache tile
//     fn touch(&mut self, key: &K) {
//         if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
//             let k = self.lru_order.remove(pos).unwrap();
//             self.lru_order.push_back(k);
//         }
//     }

//     /// Get a tile to cache
//     fn insert(&mut self, key: K, value: V) -> Arc<V> {
//         let size = value.cache_size();
//         self.evict_to_fit(size);
//         let arc = Arc::new(value);
//         self.entries.insert(key.clone(), arc.clone());
//         self.lru_order.push_back(key);
//         self.current_cache_size += size;
//         arc
//     }

//     /// Check if need to release a tile due to memory bounds
//     fn evict_to_fit(&mut self, incoming: usize) {
//         while self.current_cache_size + incoming > self.max_cache_size {
//             let pos = self.lru_order.iter().position(|k| {
//                 self.entries
//                     .get(k)
//                     .map(|v| Arc::strong_count(v) == 1)
//                     .unwrap_or(false)
//             });
//             match pos {
//                 Some(i) => {
//                     let k = self.lru_order.remove(i).unwrap();
//                     if let Some(v) = self.entries.remove(&k) {
//                         self.current_cache_size =
//                             self.current_cache_size.saturating_sub(v.cache_size());
//                     }
//                 }
//                 None => break, // all in use
//             }
//         }
//     }
// }

// /// Caching raw data
// #[derive(Debug)]
// pub struct CacheTile(Buffer<f32>);

// /// Identifier for raw data cache
// #[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
// pub struct TileId {
//     pub tile_x: usize,
//     pub tile_y: usize,
//     pub tile_band: usize,
//     pub downscaling: usize,
// }

// impl CacheSized for CacheTile {
//     fn cache_size(&self) -> usize {
//         // count space in bytes from size of f32
//         self.0.len() * std::mem::size_of::<f32>()
//     }
// }

// // pub struct CacheTimeSeries {
// //     pub values: Vec<f32>,
// // }

// // pub struct TimeSeriesId;

// /// Caching for the egui-texture, easy to drop
// pub struct CacheTexture(TextureHandle);

// impl fmt::Debug for CacheTexture {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(
//             f,
//             "CacheTexture containing TextureHandle with id {:?}",
//             self.0.id()
//         )
//     }
// }

// impl CacheSized for CacheTexture {
//     fn cache_size(&self) -> usize {
//         // count space as nb of texture, so unit here
//         1
//     }
// }

// impl std::ops::Deref for CacheTexture {
//     type Target = TextureHandle;

//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl CacheTexture {
//     pub fn to_texture(&self) -> TextureHandle {
//         self.0.clone()
//     }
// }

// /// Identifier for texture cache
// #[derive(Debug, PartialEq, Eq, Hash, Clone)]
// pub struct TextureId {
//     pub tile_x: usize,
//     pub tile_y: usize,
//     pub downscaling: usize,
//     pub params: ViewMode,
// }

// impl TextureId {
//     pub fn need_tiles(&self, view_mode: &ViewMode) -> Vec<TileId> {
//         let tile_ids = if view_mode.active_viewer == ActiveViewer::Color {
//             if let Some(alpha) = view_mode.band_alpha {
//                 // RGBA
//                 vec![
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling,
//                         tile_band: view_mode.band_1, // R - band_1
//                     },
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling,
//                         tile_band: view_mode.band_2, // G - band_2
//                     },
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling,
//                         tile_band: view_mode.band_3, // B - band_3
//                     },
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling, // A - band_alpha
//                         tile_band: alpha,
//                     },
//                 ]
//             } else {
//                 // RGB
//                 vec![
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling,
//                         tile_band: view_mode.band_1, // R - band_1
//                     },
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling,
//                         tile_band: view_mode.band_2, // G - band_2
//                     },
//                     TileId {
//                         tile_x: self.tile_x,
//                         tile_y: self.tile_y,
//                         downscaling: self.downscaling,
//                         tile_band: view_mode.band_3, // B - band_3
//                     },
//                 ]
//             }
//         } else {
//             // PANCHRO
//             vec![TileId {
//                 tile_x: self.tile_x,
//                 tile_y: self.tile_y,
//                 downscaling: self.downscaling,
//                 tile_band: view_mode.band_1, // PANCHRO - band_1
//             }]
//         };

//         tile_ids
//     }
// }

// /// Handler for all caching structs
// #[derive(Debug)]
// pub struct CacheHandler {
//     tiles: BoundedCache<TileId, CacheTile>,
//     // time_series: BoundedCache<TimeSeriesId, CacheTimeSeries>,
//     textures: BoundedCache<TextureId, CacheTexture>,
// }

// impl CacheHandler {
//     fn request_texture(
//         &mut self,
//         raster_path: &String,
//         live_bbox: PixelBox,
//         downsampling: usize,
//         view_mode: ViewMode,
//     ) -> Vec<Result<(TextureId, Arc<CacheTexture>)>> {
//         let ask_raw: Vec<TileId> = self.raw_tile_needs(live_bbox, downsampling);
//         let ask_texture = self.texture_tile_needs(live_bbox, downsampling);
//         let ask_texture_tile: HashSet<TileId> = ask_texture
//             .iter()
//             .flat_map(|t| t.need_tiles(&view_mode))
//             .collect();

//         // If you want to prefetch more raw bounds than texture bounds
//         let ask_raw_supp: Vec<&TileId> = ask_raw
//             .iter()
//             .filter(|k| !ask_texture_tile.contains(k))
//             .collect();

//         // Ask for the needed texture (will fetch raw if needed)
//         let mut textures = Vec::with_capacity(ask_texture.len());
//         for texture_id in ask_texture {
//             let tile_ids = texture_id.need_tiles(&view_mode);
//             let result: Result<(TextureId, Arc<CacheTexture>)> = (|| {
//                 // Load every raw tile this texture needs (1, 3, or 4 bands worth)
//                 let raws: Vec<Arc<CacheTile>> = tile_ids
//                     .iter()
//                     .map(|&tile_id| {
//                         self.tiles
//                             .get_or_load(tile_id, move || load_raw(raster_path, tile_id))
//                     })
//                     .collect::<Result<Vec<_>>>()?;

//                 let view_mode = view_mode.clone();
//                 let tid = texture_id.clone();
//                 let texture_cache = self.textures.get_or_load(texture_id.clone(), move || {
//                     load_texture(raws, tid, view_mode)
//                 })?;
//                 Ok((texture_id, texture_cache))
//             })();
//             textures.push(result);
//         }

//         // Ask for additionnal pre-fetch raw
//         for tile_id in ask_raw_supp {
//             let tile_id = *tile_id;
//             // drop error silently
//             _ = self
//                 .tiles
//                 .get_or_load(tile_id, move || load_raw(raster_path, tile_id));
//         }

//         textures
//     }

//     /// Determine the minimum texture tiles that should be loaded
//     fn texture_tile_needs(&self, live_bbox: PixelBox, downsampling: usize) -> Vec<TextureId> {
//         todo!()
//     }

//     /// Determine the minimum raw tiles that should be loaded
//     fn raw_tile_needs(&self, live_bbox: PixelBox, downsampling: usize) -> Vec<TileId> {
//         todo!()
//     }

//     /// Check what band is needed based on the viewer parameters
//     fn band_needed(view_mode: ViewMode) -> Vec<usize> {
//         if view_mode.active_viewer == ActiveViewer::Color {
//             if let Some(a) = view_mode.band_alpha {
//                 vec![view_mode.band_1, view_mode.band_2, view_mode.band_3, a]
//             } else {
//                 vec![view_mode.band_1, view_mode.band_2, view_mode.band_3]
//             }
//         } else {
//             vec![view_mode.band_1]
//         }
//     }
// }

// // Placeholders to put in thread.rs for async loading
// //
// fn load_raw(raster_path: &String, tile_id: TileId) -> Result<CacheTile> {
//     todo!()
// }
// //
// fn load_texture(
//     cache_tile: Vec<Arc<CacheTile>>,
//     texture_id: TextureId,
//     view_mode: ViewMode,
// ) -> Result<CacheTexture> {
//     todo!()
// }
