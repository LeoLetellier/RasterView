use crate::raster::RasterHandler;
use crate::raster::tiles::TileDescriptor;
use crate::viewers::Viewer;
use crate::viewers::coords::{Bbox, PixelBox};

impl Viewer {
    /// Determine the tiles needed for a specific viewport extent
    pub(crate) fn need_tiles(
        &self,
        raster_handler: &mut RasterHandler,
    ) -> Option<Vec<TileDescriptor>> {
        let lb = self.state.last_bounds?;
        let view_style = self.task_view(raster_handler)?;

        let raster_size = raster_handler.raster_size();

        // Determine the current zoom
        let view_extent = (lb.width(), lb.height());
        let screen_size = self.state.last_screen_size?; // needs to be captured from plot_ui.response().rect

        let downsampling: usize = self.need_zoom(view_extent, screen_size, raster_size)?;

        // Check against viewport bounds to determine which tiles are needed
        let pixel_bboxes: Vec<PixelBox> =
            self.tile_in_view(raster_size, downsampling, self.parameters.viewport_padding)?;

        // Cartesian product: every needed tile, for every needed band.
        let descriptors = pixel_bboxes
            .into_iter()
            .map(|pixel_bbox| TileDescriptor::new(pixel_bbox, downsampling, view_style.clone()))
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
        tracing::trace!("View extent: {:?}", view_extent);
        tracing::trace!("Screen size: {:?}", screen_size);

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
        tracing::trace!("Tile size: {}", tile_size);
        let max_dim = raster_size.0.max(raster_size.1).max(1);
        tracing::trace!("Maximum dimension: {}", max_dim);
        let max_downsampling = (max_dim as f64 / tile_size as f64).log2().ceil().max(0.0) as usize;
        tracing::trace!(
            "Find downsampling: {} but max at {}",
            raw_downsampling,
            max_downsampling
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
