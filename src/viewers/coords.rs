// use egui::TextureHandle;
// use egui::Vec2;
// use egui_plot::PlotBounds;

// // for screen coordinates
// // egui::Pos2 / egui::Vec2 / egui::Rect
// //
// // for plot coordinates
// // egui_plot::PlotPoint / egui_plot::PlotBounds

// #[derive(Debug)]
// pub struct TexPoint {
//     pub x: usize,
//     pub y: usize,
// }

// pub struct GeoPoint {}

// /// Size relative to an egui texture
// struct TexSize {
//     x: usize,
//     y: usize,
// }

// pub struct RasterViewHandle {
//     /// Bounds of the corresponding area in plot
//     pub plot_bounds: PlotBounds,
//     pub texture_size: (usize, usize),
//     /// Downsampling factor requested
//     pub downsampling: usize,
//     /// Egui Texture
//     pub texture_handle: TextureHandle,
// }

// impl RasterViewHandle {
//     fn new(plot_bounds: PlotBounds, downsampling: usize, texture_handle: TextureHandle) -> Self {
//         Self {
//             plot_bounds,
//             downsampling,
//             texture_handle,
//         }
//     }

//     fn plot_bounds(&self) -> PlotBounds {
//         self.plot_bounds
//     }

//     /// Gives extent in pixel coordinates
//     pub fn offset_bounds(&self) -> [usize; 4] {
//         self.extent
//     }
// }
