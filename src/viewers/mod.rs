use anyhow::Result;
use egui_plot::{PlotBounds, PlotPoint};
use std::path::Path;

use crate::raster::RasterHandler;
use crate::viewers::coords::GeoBox;
use crate::viewers::tiler::{TextureCache, Tile, TileWeighter};

pub mod cmap;
pub mod coords;
pub mod thread;
pub mod tiler;
pub mod ui;

use cmap::ColorInterpretation;

/// Core of the raster viewer / visualization
///
/// It holds both the cache and the user input parameters
#[derive(Debug)]
pub struct Viewer {
    /// Actual raster
    pub raster_handler: RasterHandler,
    /// View handler
    pub view_mode: ViewMode,
    /// User parameters for the viewer
    pub parameters: ViewerParams,
    /// Some state parameters for the viewer
    pub state: ViewerState,
}

#[derive(Debug)]
pub struct ViewerParams {
    /// Size in pixels of the tiles
    ///
    /// Default is 256 pixels
    tile_size: usize,
    /// Padding outside the viewport to find tiles to display at current frame
    ///
    /// A value of `1` means no padding, more than one add padding,
    /// less than one restrain the tile loading inside the viewport
    viewport_padding: f64,
    /// Maximum byte size allowed for the cache
    ///
    /// When loading a new texture in a full cache, the older used texture will be dropped
    ///
    /// Be carefull to be able to fit all tiles neeeded on screen to avoid loop where
    /// while loading needed tile, drop also needed tile
    cache_size: u64,
}

impl Default for ViewerParams {
    fn default() -> Self {
        ViewerParams {
            tile_size: 256,                // 256 pixels wide tiles
            viewport_padding: 1.1,         // 10% padding of loaded
            cache_size: 256 * 1024 * 1024, // 256MB cache size
        }
    }
}

#[derive(Debug, Default)]
pub struct ViewerState {
    pub last_cursor_pos: Option<PlotPoint>,
    pub last_bounds: Option<PlotBounds>,
    pub last_screen_size: Option<(f64, f64)>,
}

impl Viewer {
    pub fn with_raster(path: &Path, ctx: egui::Context) -> Result<Self> {
        let parameters = ViewerParams::default();
        let raster_handler = RasterHandler::new(path, ctx, parameters.cache_size)?;

        Ok(Self {
            raster_handler,
            view_mode: Default::default(),
            parameters,
            state: Default::default(),
        })
    }

    pub fn refresh_cache(&mut self) {
        self.raster_handler
            .refresh_cache(self.parameters.cache_size);
    }
}

/// Sub-mode for complex (CPX) rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PanchroCpxView {
    /// Use amplitude only.
    AmplitudeOnly,
    /// Use wrapped phase only (your “wrapped panchro”).
    WrappedPhaseOnly,
    /// Superpose amplitude and phase into a composite visualization.
    CompositeAmpPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ColorCpxView {
    Amplitude,
    WrappedPhase,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ViewMode {
    pub active_viewer: ActiveViewer,
    pub panchro_band: usize,
    pub rgb_bands: (usize, usize, usize),
    pub panchro_cpx: PanchroCpxView,
    pub color_cpx: ColorCpxView,
    pub color_interpretation: ColorInterpretation,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self {
            active_viewer: ActiveViewer::Panchro,
            panchro_band: 1,
            rgb_bands: (1, 2, 3),
            panchro_cpx: PanchroCpxView::CompositeAmpPhase,
            color_cpx: ColorCpxView::WrappedPhase,
            color_interpretation: ColorInterpretation::default(),
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum ActiveViewer {
    Panchro,
    Color,
}
