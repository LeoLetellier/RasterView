use crate::viewers::coords::{GeoBox, PixelBox};

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Cache the raw data from gdal
pub struct CacheTile {
    /// Tiles mapping on the raster
    metadata: Vec<RasterTile>,
    /// Data in cache
    entries: HashMap<TileId, Arc<TilePayload>>,
    /// Order of last use
    lru_order: VecDeque<TileId>,
    /// Current size in cache
    current_bytes: usize,
    /// Maximum size in cache allowed
    max_bytes: usize,
}

impl CacheTile {
    /// Get or fetch to cache if not already
    pub fn get_or_load(
        &mut self,
        id: TileId,
        loader: impl FnOnce(&RasterTile) -> TilePayload,
    ) -> Option<Arc<TilePayload>> {
        // 1. déjà en cache -> touch + retour
        if let Some(payload) = self.entries.get(&id) {
            let payload = payload.clone();
            self.touch(&id);
            return Some(payload);
        }

        // 2. retrouver les métadonnées de la tuile demandée
        let meta = self.metadata.iter().find(|t| t.id == id)?;

        // 3. charger, évincer si besoin, insérer
        let payload = loader(meta);
        Some(self.insert(id, payload))
    }

    /// actualize last use if exists
    fn touch(&mut self, id: &TileId) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == id) {
            let k = self.lru_order.remove(pos).unwrap();
            self.lru_order.push_back(k);
        }
    }

    /// create entry in cache if not exists
    fn insert(&mut self, id: TileId, payload: TilePayload) -> Arc<TilePayload> {
        let size = payload.byte_size();
        self.evict_to_fit(size);

        let arc = Arc::new(payload);
        self.entries.insert(id, arc.clone());
        self.lru_order.push_back(id);
        self.current_bytes += size;
        arc
    }

    // is not used drop to fit in memory bounds
    fn evict_to_fit(&mut self, incoming_size: usize) {
        while self.current_bytes + incoming_size > self.max_bytes {
            let evictable_pos = self.lru_order.iter().position(|id| {
                self.entries
                    .get(id)
                    .map(|v| Arc::strong_count(v) == 1)
                    .unwrap_or(false)
            });

            match evictable_pos {
                Some(pos) => {
                    let id = self.lru_order.remove(pos).unwrap();
                    if let Some(v) = self.entries.remove(&id) {
                        self.current_bytes = self.current_bytes.saturating_sub(v.byte_size());
                    }
                }
                None => break, // tout est utilisé, on ne peut rien libérer de plus
            }
        }
    }
}

/// Identify a specific tile
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileId {
    pub downscaling: usize,
    pub tile_x: usize,
    pub tile_y: usize,
}

/// Actual buffer inside the cache for each tile
pub struct TilePayload {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

impl TilePayload {
    /// Size of the cache for this tile
    pub fn byte_size(&self) -> usize {
        self.bytes.len()
    }
}

/// Representation of the cached tile in project context (position)
pub struct RasterTile {
    pub id: TileId, // <- champ manquant dans ton fichier actuel
    raster_bbox: PixelBox,
    world_bbox: GeoBox,
}

impl RasterTile {
    /// Wrapper to easily get the downscaling value
    pub fn downscaling(&self) -> usize {
        self.id.downscaling
    }
}

/// Define the parameters of the caching strategy
pub struct TilingConfig {
    pub tile_size: u32,
    pub downscalings: Vec<usize>,
    pub max_memory: usize,
}

/// Hold the cache for Textures
pub struct CacheTexture {
    /// actual cached buffers
    entries: HashMap<TileId, egui::TextureHandle>,
    /// last use order
    lru_order: VecDeque<TileId>,
    /// limit of texture nb to limit memory
    max_textures: usize,
}

impl CacheTexture {
    pub fn get_or_upload(
        &mut self,
        ctx: &egui::Context,
        id: TileId,
        payload: &TilePayload,
    ) -> egui::TextureHandle {
        // 1. déjà en cache -> touch + retour
        if let Some(tex) = self.entries.get(&id) {
            let tex = tex.clone();
            self.touch(&id);
            return tex;
        }

        // 2. pas en cache -> évincer si besoin, uploader, insérer
        self.evict_if_needed();

        let color_image = egui::ColorImage::from_rgba_unmultiplied(
            [payload.width as usize, payload.height as usize],
            &payload.bytes,
        );

        let tex = ctx.load_texture(
            format!("tile-{}-{}-{}", id.downscaling, id.tile_x, id.tile_y),
            color_image,
            egui::TextureOptions::default(),
        );

        self.entries.insert(id, tex.clone());
        self.lru_order.push_back(id);
        tex
    }

    fn touch(&mut self, id: &TileId) {
        if let Some(pos) = self.lru_order.iter().position(|k| k == id) {
            let k = self.lru_order.remove(pos).unwrap();
            self.lru_order.push_back(k);
        }
    }

    fn evict_if_needed(&mut self) {
        while self.entries.len() >= self.max_textures {
            match self.lru_order.pop_front() {
                Some(id) => {
                    self.entries.remove(&id);
                }
                None => break,
            }
        }
    }
}

pub struct TileViewer {
    raw_cache: CacheTile,
    texture_cache: CacheTexture,
    dataset: gdal::Dataset, // ou toute autre source de données
}

impl TileViewer {
    pub fn update(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        view: &GeoBox,
        target_downscaling: usize,
    ) {
        let visible_ids = self.tiles_for_view(view, target_downscaling);

        for id in visible_ids {
            // (a) RAW: récupère ou charge les données brutes
            let dataset = &self.dataset; // capturé par référence dans la closure
            let payload = self
                .raw_cache
                .get_or_load(id, |meta| load_tile_from_gdal(meta, dataset));

            let Some(payload) = payload else {
                // tuile absente des métadonnées (ou pas encore chargée en mode async)
                // -> on saute cette tuile pour cette frame, pas d'erreur fatale
                continue;
            };

            // (b) TEXTURE: récupère ou uploade la texture GPU
            let texture = self.texture_cache.get_or_upload(ctx, id, &payload);

            // (c) paint: dessine la tuile à sa position dans le viewport
            let rect = self.screen_rect_for_tile(id, view, ui);
            ui.painter().image(
                texture.id(),
                rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }
    }

    fn tiles_for_view(&self, view: &GeoBox, downscaling: usize) -> Vec<TileId> {
        self.raw_cache
            .metadata_iter() // méthode à ajouter sur CacheTile pour exposer &self.metadata
            .filter(|t| t.downscaling() == downscaling && geo_intersects(t.world_bbox(), view))
            .map(|t| t.id)
            .collect()
    }
}
