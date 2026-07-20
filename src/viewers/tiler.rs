use crate::viewers::coords::{Bbox, GeoBox, PixelBox};
use crate::viewers::{ActiveViewer, Viewer};

use anyhow::{Result, anyhow};
use egui::{TextureHandle, vec2};
use egui_plot::{PlotImage, PlotUi};
use quick_cache::sync::Cache;
use quick_cache::{
    Weighter,
    sync::{EntryAction, EntryResult},
};

use egui_plot::PlotPoint;
use std::collections::HashSet;
use std::fmt;
use std::hash::Hash;

impl Viewer {
    /// Determine the tiles needed for a specific viewport extent
    pub(crate) fn need_tiles(&self) -> Option<Vec<TileDescriptor>> {
        let raster_size = self.raster_handler.raster_size();
        let lb = self.state.last_bounds?;

        let (full_width, full_height) = raster_size;

        // Determine the current zoom
        let view_extent = (lb.width(), lb.height());
        let screen_size = self.state.last_screen_size?; // needs to be captured from plot_ui.response().rect

        let downsampling: usize = self.need_zoom(view_extent, screen_size, raster_size)?;
        // Check against the viewmode for which band to load
        let bands: Vec<usize> = match self.view_mode.active_viewer {
            ActiveViewer::Panchro => vec![self.view_mode.panchro_band],
            ActiveViewer::Color => {
                let bands = self.view_mode.rgb_bands;
                vec![bands.0, bands.1, bands.2]
            }
        };

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
                    downsampling,
                })
            })
            .collect();

        Some(descriptors)
    }

    /// Determine the level of zoom needed from a specific viewport extent
    fn need_zoom(
        &self,
        view_extent: (f64, f64),
        screen_size: (f64, f64),
        raster_size: (usize, usize),
    ) -> Option<usize> {
        let (view_w, view_h) = view_extent; // visible raster pixels (from PlotBounds width/height)
        let (screen_w, screen_h) = screen_size; // actual widget size in screen pixels
        println!("View extent: {:?}", view_extent);
        println!("Screen size: {:?}", screen_size);

        if screen_w == 0.0 || screen_h == 0.0 {
            return None;
        }

        // Raster pixels shown per screen pixel, per axis.
        let ratio_x = view_w / screen_w;
        let ratio_y = view_h / screen_h;

        let ratio = ratio_x.max(ratio_y).max(1.0);
        // target downsampling from ratio alone
        let raw_downsampling = ratio.log2().floor().max(0.0) as usize;

        // Get the maximum downsampling allowed by the raster
        //
        // Otherwise will continue to degrade the resolution up to an empty raster
        let tile_size = self.parameters.tile_size.max(1);
        println!("Tile size: {}", tile_size);
        let max_dim = raster_size.0.max(raster_size.1).max(1);
        println!("Maximum dimension: {}", max_dim);
        let max_downsampling = (max_dim as f64 / tile_size as f64).log2().ceil().max(0.0) as usize;
        println!(
            "Find downsampling: {} but max at {}",
            raw_downsampling, max_downsampling
        );

        Some(raw_downsampling.min(max_downsampling))
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
}

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub(crate) struct TileDescriptor {
    pub(crate) pixel_bbox: PixelBox,
    pub(crate) downsampling: usize,
}

impl TileDescriptor {
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
}

#[derive(Clone)]
pub(crate) struct Tile {
    pub(crate) tile_descriptor: TileDescriptor,
    pub(crate) texture: TextureHandle,
}

impl fmt::Debug for Tile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tile")
            .field("tile_descriptor", &self.tile_descriptor)
            // .field("texture", &self.texture)
            .finish()
    }
}

// impl Drop for Tile {
//     fn drop(&mut self) {
//         println!(
//             "Drop Tile name={} texture_id={:?}",
//             self.tile_descriptor.name(),
//             self.texture.id(),
//         );
//     }
// }

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

/// Cache from quick cache
/// # Usage
/// ```rs
/// let cache = ImageCache::with_weighter(500, 64 * 1024 * 1024, TileWeighter); // ~64MB budget
/// ```
pub(crate) type TextureCache = Cache<TileDescriptor, Tile, TileWeighter>;
