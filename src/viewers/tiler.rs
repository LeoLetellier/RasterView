use crate::raster::RasterHandler;
use crate::viewers::coords::{Bbox, GeoBox, GeoTransform, PixelBox};
use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::sync::Arc;

/// Getter for the object in-memory size
pub trait ByteSized {
    fn byte_size(&self) -> usize;
}

/// Common architecture for caching data
pub struct BoundedCache<K: Eq + Hash + Clone, V: ByteSized> {
    entries: HashMap<K, Arc<V>>,
    lru_order: VecDeque<K>,
    current_bytes: usize,
    max_bytes: usize,
}

impl<K: Eq + Hash + Clone, V: ByteSized> BoundedCache<K, V> {
    /// Initialize the cache with max_bytes
    pub fn new(max_bytes: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            current_bytes: 0,
            max_bytes,
        }
    }

    /// Get the cache tile or fetch-it if not in cache
    pub fn get_or_load(&mut self, key: K, loader: impl FnOnce() -> V) -> Arc<V> {
        if let Some(v) = self.entries.get(&key) {
            let v = v.clone();
            self.touch(&key);
            return v;
        }
        let value = loader();
        self.insert(key, value)
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
        let size = value.byte_size();
        self.evict_to_fit(size);
        let arc = Arc::new(value);
        self.entries.insert(key.clone(), arc.clone());
        self.lru_order.push_back(key);
        self.current_bytes += size;
        arc
    }

    /// Check if need to release a tile due to memory bounds
    fn evict_to_fit(&mut self, incoming: usize) {
        while self.current_bytes + incoming > self.max_bytes {
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
                        self.current_bytes = self.current_bytes.saturating_sub(v.byte_size());
                    }
                }
                None => break, // all in use
            }
        }
    }
}

/// Caching for the egui-texture, easy to drop
pub struct CacheTexture {
    entries: HashMap<TextureKey, egui::TextureHandle>,
    lru_order: VecDeque<TextureKey>,
    max_textures: usize,
}

impl CacheTexture {
    /// Initialize the texture cache
    pub fn new(max_textures: usize) -> Self {
        Self {
            entries: HashMap::new(),
            lru_order: VecDeque::new(),
            max_textures,
        }
    }

    /// Fetch the texture tile from cache or load it
    pub fn get_or_upload(
        &mut self,
        ctx: &egui::Context,
        key: TextureKey,
        payload: &TilePayload,
    ) -> egui::TextureHandle {
        if let Some(tex) = self.entries.get(&key) {
            let tex = tex.clone();
            self.touch(&key);
            return tex;
        }

        self.evict_if_needed();

        let rgba = apply_vis_params(payload, &key.params);
        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [payload.width as usize, payload.height as usize],
            &rgba,
        );

        let tex = ctx.load_texture(
            format!("tile-{:?}", key),
            color_image,
            egui::TextureOptions::default(),
        );

        self.entries.insert(key.clone(), tex.clone());
        self.lru_order.push_back(key);
        tex
    }

    /// Actualize last use of Texture tile
    fn touch(&mut self, key: &TextureKey) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == key) {
            let k = self.lru_order.remove(pos).unwrap();
            self.lru_order.push_back(k);
        }
    }

    /// Release texture tiles due to memory bounds
    fn evict_if_needed(&mut self) {
        while self.entries.len() >= self.max_textures {
            match self.lru_order.pop_front() {
                Some(k) => {
                    self.entries.remove(&k);
                }
                None => break,
            }
        }
    }
}

/// Convert Tile to RGBA
fn apply_vis_params(payload: &TilePayload, params: &VisParams) -> Vec<u8> {
    // TODO
    todo!("appliquer colormap ou composite RGB sur payload.values selon params")
}

/// Identifier for a specific raster tile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    pub downscaling: usize,
    pub tile_x: usize,
    pub tile_y: usize,
    pub time_index: usize,
}

/// Identifier for a specific raster column
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeriesId {
    pub x: usize,
    pub y: usize,
    pub band: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColormapId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum VisParams {
    ColorMap { band: usize, colormap: ColormapId },
    RgbComposite { r: usize, g: usize, b: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureKey {
    pub tile: TileId,
    pub params: VisParams,
}

// =====================================================================
// Payloads (données RAW, non colorisées)
// =====================================================================

pub struct TilePayload {
    pub values: Vec<f32>,
    pub width: u32,
    pub height: u32,
    pub bands: u8,
}

impl ByteSized for TilePayload {
    fn byte_size(&self) -> usize {
        self.values.len() * std::mem::size_of::<f32>()
    }
}

pub struct SeriesPayload {
    pub values: Vec<f32>,
}

impl ByteSized for SeriesPayload {
    fn byte_size(&self) -> usize {
        self.values.len() * std::mem::size_of::<f32>()
    }
}

// =====================================================================
// Métadonnées de la pyramide -- toujours résidentes
// =====================================================================

pub struct RasterTile {
    pub id: TileId,
    raster_bbox: PixelBox,
    world_bbox: GeoBox,
}

impl RasterTile {
    pub fn raster_bbox(&self) -> &PixelBox {
        &self.raster_bbox
    }

    pub fn world_bbox(&self) -> &GeoBox {
        &self.world_bbox
    }

    pub fn downscaling(&self) -> usize {
        self.id.downscaling
    }
}

pub struct TilingConfig {
    pub tile_size: u32,
    pub downscalings: Vec<usize>,
    pub time_steps: usize,
}

fn build_pyramid_metadata(
    raster_px: &PixelBox,
    world_bbox: &GeoBox,
    config: &TilingConfig,
) -> Vec<RasterTile> {
    let transform = GeoTransform::new(
        world_bbox.xmin(),
        (world_bbox.xmax() - world_bbox.xmin()) / (raster_px.xmax() - raster_px.xmin()) as f64,
        world_bbox.ymax(),
        (world_bbox.ymin() - world_bbox.ymax()) / (raster_px.ymax() - raster_px.ymin()) as f64,
    );

    let full_w = raster_px.xmax() - raster_px.xmin();
    let full_h = raster_px.ymax() - raster_px.ymin();

    let mut metadata = Vec::new();

    for &downscaling in &config.downscalings {
        let tile_extent = config.tile_size as usize * downscaling;

        let mut tile_y_idx = 0;
        let mut y = raster_px.ymin();
        while y < raster_px.ymin() + full_h {
            let y_end = (y + tile_extent).min(raster_px.ymin() + full_h);

            let mut tile_x_idx = 0;
            let mut x = raster_px.xmin();
            while x < raster_px.xmin() + full_w {
                let x_end = (x + tile_extent).min(raster_px.xmin() + full_w);

                let raster_bbox = PixelBox::from([x, x_end, y, y_end]);
                let world_tile_bbox = transform.pixel_box_to_geo_box(&raster_bbox);

                for t in 0..config.time_steps.max(1) {
                    metadata.push(RasterTile {
                        id: TileId {
                            downscaling,
                            tile_x: tile_x_idx,
                            tile_y: tile_y_idx,
                            time_index: t,
                        },
                        raster_bbox: raster_bbox.clone(), // TODO: vérifier si PixRect: Clone
                        world_bbox: world_tile_bbox.clone(), // idem pour GeoRect
                    });
                }

                x += tile_extent;
                tile_x_idx += 1;
            }
            y += tile_extent;
            tile_y_idx += 1;
        }
    }

    metadata
}

// =====================================================================
// Mode de visualisation -- route vers le bon cache + la bonne fonction
// =====================================================================

pub enum VisMode {
    ColorMap { band: usize, colormap: ColormapId },
    RgbComposite { r: usize, g: usize, b: usize },
    TimeSeries { x: usize, y: usize, band: usize },
}

pub enum RenderOutput {
    Texture(egui::TextureHandle),
    Series(Vec<f32>),
}

// =====================================================================
// DataCube -- assemble les caches RAW + les métadonnées
// =====================================================================

pub struct DataCube {
    tiles: BoundedCache<TileId, TilePayload>,
    series: BoundedCache<SeriesId, SeriesPayload>,
    metadata: Vec<RasterTile>,
}

impl DataCube {
    pub fn new(
        raster_px: PixelBox,
        world_bbox: GeoBox,
        config: &TilingConfig,
        max_tile_bytes: usize,
        max_series_bytes: usize,
    ) -> Self {
        let metadata = build_pyramid_metadata(&raster_px, &world_bbox, config);
        Self {
            tiles: BoundedCache::new(max_tile_bytes),
            series: BoundedCache::new(max_series_bytes),
            metadata,
        }
    }

    pub fn metadata_iter(&self) -> impl Iterator<Item = &RasterTile> {
        self.metadata.iter()
    }

    pub fn tiles_for_view(
        &self,
        view: &GeoBox,
        downscaling: usize,
        time_index: usize,
    ) -> impl Iterator<Item = &RasterTile> {
        self.metadata.iter().filter(move |t| {
            t.downscaling() == downscaling
                && t.id.time_index == time_index
                && geo_intersects(&t.world_bbox, view)
        })
    }

    pub fn render(
        &mut self,
        mode: &VisMode,
        tile_id: TileId,
        texture_cache: &mut CacheTexture,
        ctx: &egui::Context,
        raster_handler: &RasterHandler,
    ) -> RenderOutput {
        match mode {
            VisMode::ColorMap { band, colormap } => {
                let raster_bbox = self.raster_bbox_for(tile_id);
                let payload = self.tiles.get_or_load(tile_id, || {
                    raster_handler.load_tile(&tile_id, &raster_bbox, &[*band])
                });
                let key = TextureKey {
                    tile: tile_id,
                    params: VisParams::ColorMap {
                        band: *band,
                        colormap: colormap.clone(),
                    },
                };
                let tex = texture_cache.get_or_upload(ctx, key, &payload);
                RenderOutput::Texture(tex)
            }
            VisMode::RgbComposite { r, g, b } => {
                let raster_bbox = self.raster_bbox_for(tile_id);
                let payload = self.tiles.get_or_load(tile_id, || {
                    raster_handler.load_tile(&tile_id, &raster_bbox, &[*r, *g, *b])
                });
                let key = TextureKey {
                    tile: tile_id,
                    params: VisParams::RgbComposite {
                        r: *r,
                        g: *g,
                        b: *b,
                    },
                };
                let tex = texture_cache.get_or_upload(ctx, key, &payload);
                RenderOutput::Texture(tex)
            }
            VisMode::TimeSeries { x, y, band } => {
                let series_id = SeriesId {
                    x: *x,
                    y: *y,
                    band: *band,
                };
                let payload = self
                    .series
                    .get_or_load(series_id, || raster_handler.load_series(*x, *y, *band));
                RenderOutput::Series(payload.values.clone())
            }
        }
    }

    /// Retrouve le raster_bbox (coordonnées pixel natives) d'une tuile à
    /// partir de son id, en cherchant dans les métadonnées de la pyramide.
    fn raster_bbox_for(&self, tile_id: TileId) -> PixelBox {
        self.metadata
            .iter()
            .find(|t| t.id == tile_id)
            .expect("tile_id not found in metadata")
            .raster_bbox()
            .clone()
    }
}

fn geo_intersects(a: &GeoBox, b: &GeoBox) -> bool {
    a.xmin() <= b.xmax() && a.xmax() >= b.xmin() && a.ymin() <= b.ymax() && a.ymax() >= b.ymin()
}

#[cfg(test)]
mod tiling_tests {
    use super::*;

    #[test]
    fn pyramid_generates_expected_tile_count() {
        let raster_px = PixelBox::from([0, 100, 0, 100]);
        let world_bbox = GeoBox::from([0.0, 100.0, 0.0, 100.0]);
        let config = TilingConfig {
            tile_size: 50,
            downscalings: vec![1, 2], // ou via ta future fn downscalings()
            time_steps: 1,
        };

        let metadata = build_pyramid_metadata(&raster_px, &world_bbox, &config);

        // downscaling=1: tile_extent=50 -> grille 2x2 = 4 tuiles
        // downscaling=2: tile_extent=100 -> grille 1x1 = 1 tuile
        // total attendu: 5 tuiles (x1 time_step)
        assert_eq!(metadata.len(), 5);
    }

    #[test]
    fn pyramid_tile_world_bbox_matches_geotransform() {
        let raster_px = PixelBox::from([0, 100, 0, 100]);
        let world_bbox = GeoBox::from([0.0, 100.0, 0.0, 100.0]);
        let config = TilingConfig {
            tile_size: 50,
            downscalings: vec![1],
            time_steps: 1,
        };

        let metadata = build_pyramid_metadata(&raster_px, &world_bbox, &config);

        let first_tile = metadata
            .iter()
            .find(|t| t.id.tile_x == 0 && t.id.tile_y == 0)
            .unwrap();
        assert_eq!(first_tile.world_bbox().xmin(), 0.0);
        assert_eq!(first_tile.world_bbox().xmax(), 50.0);
    }
}
