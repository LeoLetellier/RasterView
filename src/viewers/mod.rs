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
    pub raster_handler: Option<RasterHandler>,
    /// View handler
    pub view_mode: ViewMode,
    /// User parameters for the viewer
    pub parameters: ViewerParams,
    /// Some state parameters for the viewer
    pub state: ViewerState,
}

#[derive(Debug)]
pub struct ViewerParams {
    tile_size: usize,
    viewport_padding: f64,
    cache_size: u64,
}

impl Default for ViewerParams {
    fn default() -> Self {
        ViewerParams {
            tile_size: 256,
            viewport_padding: 0.0,
            cache_size: 256 * 1024 * 1024, // 256MB
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
        let mut viewer = Self::default();
        let raster_handler = RasterHandler::new(path, ctx, viewer.parameters.cache_size)?;
        viewer.raster_handler = Some(raster_handler);

        Ok(viewer)
    }

    pub fn refresh_cache(&mut self) {
        if let Some(rh) = &mut self.raster_handler {
            rh.refresh_cache(self.parameters.cache_size);
        }
    }
}

impl Default for Viewer {
    fn default() -> Self {
        Self {
            raster_handler: None,
            view_mode: Default::default(),
            parameters: Default::default(),
            state: Default::default(),
        }
    }
}

/// Sub-mode for complex (CPX) rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CpxView {
    /// Use amplitude only.
    AmplitudeOnly,
    /// Use wrapped phase only (your “wrapped panchro”).
    WrappedPhaseOnly,
    /// Superpose amplitude and phase into a composite visualization.
    CompositeAmpPhase,
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub struct ViewMode {
    active_viewer: ActiveViewer,
    band_1: usize,             // R - PANCHRO - CPX
    band_2: usize,             // G
    band_3: usize,             // B
    band_alpha: Option<usize>, // A
    cpx: CpxView,
    color: ColorInterpretation,
}

impl Default for ViewMode {
    fn default() -> Self {
        Self {
            active_viewer: ActiveViewer::Panchro,
            band_1: 1,
            band_2: 2,
            band_3: 3,
            band_alpha: None,
            cpx: CpxView::CompositeAmpPhase,
            color: ColorInterpretation::default(),
        }
    }
}

impl ViewMode {
    pub fn need_bands(&self) -> Vec<usize> {
        match &self.active_viewer {
            ActiveViewer::Panchro | ActiveViewer::Cpx => vec![self.band_1],
            ActiveViewer::Color if let Some(alpha) = self.band_alpha => {
                vec![self.band_1, self.band_2, self.band_3, alpha]
            }
            ActiveViewer::Color => {
                vec![self.band_1, self.band_2, self.band_3]
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum ActiveViewer {
    Panchro,
    Color,
    Cpx,
}

pub fn dummy_checkerboard(width: usize, height: usize, cell_size: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let is_light = ((x / cell_size) + (y / cell_size)) % 2 == 0;
            let (r, g, b) = if is_light {
                (220, 220, 220)
            } else {
                (40, 40, 40)
            };
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(255); // alpha
        }
    }

    pixels
}

pub fn dummy_gradient(width: usize, height: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            let v = ((x * 255) / width.max(1)) as u8;
            pixels.push(v);
            pixels.push(v);
            pixels.push(v);
            pixels.push(255);
        }
    }
    pixels
}
