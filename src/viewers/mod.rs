use crate::raster::RasterHandler;
use crate::viewers::coords::GeoBox;
use crate::viewers::tiler::CacheHandler;

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
    /// Active View
    pub live_bbox: GeoBox,
    pub downscaling: usize,
    /// User parameters
    pub view_mode: ViewMode,
    /// Caching
    pub cache: Option<CacheHandler>,
}

impl Default for Viewer {
    fn default() -> Self {
        let live_bbox = [0.0, 1.0, 0.0, 1.0].into();
        Self {
            raster_handler: None,
            live_bbox,
            downscaling: 0,
            view_mode: Default::default(),
            cache: None,
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

/// Whether `Color` mode uses one shared normalization across all selected channels
/// or prepares mappings per-channel.
///
/// (You can use this in your worker to decide whether to compute one set of min/max
/// stats or one per band.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelNorm {
    /// One normalization/range strategy for all RGB(A) channels.
    Shared,
    /// One normalization/range strategy per channel.
    PerChannel,
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

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub enum ActiveViewer {
    Panchro,
    Color,
    Cpx,
}
