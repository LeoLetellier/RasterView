use crate::viewers::coords::{Bbox, PixelBox};
use crate::viewers::tasking::ViewStyle;
use eframe::egui::{TextureHandle, vec2};
use egui_plot::{PlotImage, PlotPoint, PlotUi};
use quick_cache::Weighter;
use quick_cache::sync::Cache;

/// Cache from quick cache
/// # Usage
/// ```rs
/// let cache = ImageCache::with_weighter(500, 64 * 1024 * 1024, TileWeighter); // ~64MB budget
/// ```
pub(crate) type TextureCache = Cache<TileDescriptor, Tile, TileWeighter>;

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub(crate) struct TileDescriptor {
    pub(crate) pixel_bbox: PixelBox,
    pub(crate) downsampling: usize,
    pub(crate) view_style: ViewStyle,
}

impl TileDescriptor {
    pub(crate) fn new(pixel_bbox: PixelBox, downsampling: usize, view_style: ViewStyle) -> Self {
        Self {
            pixel_bbox,
            downsampling,
            view_style,
        }
    }

    pub(crate) fn tile_pixel_size(&self) -> (usize, usize) {
        self.pixel_bbox.size_with_downsampling(self.downsampling)
    }

    pub(crate) fn pixel_box(&self) -> &PixelBox {
        &self.pixel_bbox
    }

    pub(crate) fn name(&self) -> String {
        format!(
            "s{}_x{}_xx{}_y{}_yy{}",
            self.downsampling,
            self.pixel_bbox.xmin(),
            self.pixel_bbox.xmax(),
            self.pixel_bbox.ymin(),
            self.pixel_bbox.ymax()
        )
    }

    pub(crate) fn distance_to(&self, point: PlotPoint) -> f64 {
        let center_point = self.pixel_bbox.center();
        let dx = center_point.x - point.x;
        let dy = center_point.y - point.y;
        (dx * dx + dy * dy).sqrt()
    }

    pub(crate) fn _size(&self) -> [usize; 2] {
        [self.pixel_bbox.width(), self.pixel_bbox.height()]
    }
}

#[derive(Clone)]
pub(crate) struct Tile {
    pub(crate) tile_descriptor: TileDescriptor,
    pub(crate) texture: TextureHandle,
}

impl std::fmt::Debug for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tile")
            .field("tile_descriptor", &self.tile_descriptor)
            // .field("texture", &self.texture)
            .finish()
    }
}

impl PartialEq for Tile {
    fn eq(&self, other: &Self) -> bool {
        self.tile_descriptor == other.tile_descriptor
    }
}
impl Eq for Tile {}

impl std::hash::Hash for Tile {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.tile_descriptor.hash(state);
    }
}

// Lets HashSet<Tile>::get / contains work with a &TileDescriptor directly,
// no need to build a dummy Tile just to look one up.
impl std::borrow::Borrow<TileDescriptor> for Tile {
    fn borrow(&self) -> &TileDescriptor {
        &self.tile_descriptor
    }
}

impl Tile {
    pub(crate) fn new(tile_descriptor: TileDescriptor, texture_handle: TextureHandle) -> Self {
        Self {
            tile_descriptor,
            texture: texture_handle,
        }
    }
    pub(crate) fn plot_ui(&self, plot_ui: &mut PlotUi) {
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
pub(crate) struct TileWeighter;

impl Weighter<TileDescriptor, Tile> for TileWeighter {
    fn weight(&self, _key: &TileDescriptor, val: &Tile) -> u64 {
        val.texture.byte_size() as u64
    }
}
