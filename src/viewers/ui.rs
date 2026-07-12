use crate::raster::RasterHandler;
use crate::viewers::{Viewer, dummy_checkerboard, dummy_gradient};
use egui::{Ui, vec2};
use egui_plot::{Plot, PlotBounds, PlotImage, PlotPoint, PlotUi};

impl Viewer {
    pub fn ui(&mut self, ui: &mut Ui) {
        // egui::Panel::right("right panel").show_inside(ui, |_ui| {});
        // let color_image = if let Some(rh) = &self.raster_handler {
        //     rh.to_colorimage_direct_par(1).ok()
        // } else {
        //     None
        // };

        // let width = 256;
        // let height = 256;
        // let raw = dummy_checkerboard(width, height, 16);
        // let raw = dummy_gradient(width, height);

        // let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &raw);

        let handle = if let Some(ci) = &self.color_image {
            Some(ui.load_texture(
                "dummy_checkerboard",
                ci.clone(),
                egui::TextureOptions::default(),
            ))
        } else {
            None
        };

        Plot::new("main_plot")
            .data_aspect(1.0)
            .pan_pointer_button(egui::PointerButton::Primary)
            .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
            .allow_scroll(false)
            .allow_zoom(true)
            .show(ui, |plot_ui| {
                if let Some(h) = handle {
                    plot_ui.image(PlotImage::new(
                        "raster",
                        h.id(),
                        PlotPoint::new(128.0, 128.0),
                        vec2(256.0, 256.0),
                    ))
                };
            });
    }

    // pub fn show(&mut self, ui: &mut Ui, ctx: &egui::Context) {
    //     let Some(raster_handler) = &self.raster_handler else {
    //         return;
    //     };
    //     let Some(data_cube) = &mut self.data_cube else {
    //         return;
    //     };

    //     let mode = self.current_vis_mode();
    //     // let visible_tiles: Vec<TileId> = data_cube
    //     //     .tiles_for_view(/* geo view courant, downscaling courant */)
    //     //     .map(|t| t.id)
    //     //     .collect();

    //     // for tile_id in visible_tiles {
    //     //     let output =
    //     //         data_cube.render(&mode, tile_id, &mut self.texture_cache, ctx, raster_handler);
    //     //     // dispatcher output vers ui.painter().image(...) ou egui_plot::Line
    //     // }
    // }
}

// use super::{ReadOptions, TextureWorker, ViewMode, ViewModeWorker};
// use egui::{Ui, vec2};
// use egui_plot::{Plot, PlotBounds, PlotImage, PlotPoint, PlotUi};

// impl ViewMode {
//     /// All central panel viewer goes in this one
//     pub fn ui(&mut self, ui: &mut Ui, texture_worker: &mut TextureWorker) {
//         // 1. Poll for new texture
//         self.try_update_texture(texture_worker);

//         // 2. Draw plot, capture bounds
//         let raster_size = self.raster_size();
//         let (rw, rh) = raster_size;
//         let available = ui.available_size();
//         let mut new_bounds: Option<PlotBounds> = None;

//         Plot::new("main_plot")
//             .data_aspect(1.0)
//             .pan_pointer_button(egui::PointerButton::Primary)
//             .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
//             .allow_scroll(false)
//             .allow_zoom(true)
//             .default_x_bounds(0.0, rw as f64)
//             .default_y_bounds(0.0, rh as f64)
//             // .x_grid_spacer(|input| {
//             //     let (min, max) = (input.bounds.0, input.bounds.1);
//             //     (min.floor() as i64..=max.ceil() as i64)
//             //         .map(|n| egui_plot::GridMark {
//             //             value: n as f64,
//             //             step_size: 1.0,
//             //         })
//             //         .collect()
//             // })
//             // .y_grid_spacer(|input| {
//             //     let (min, max) = (input.bounds.0, input.bounds.1);
//             //     (min.floor() as i64..=max.ceil() as i64)
//             //         .map(|n| egui_plot::GridMark {
//             //             value: n as f64,
//             //             step_size: 1.0,
//             //         })
//             //         .collect()
//             // })
//             .x_grid_spacer(move |input| raster_grid_spacer(input, rw as f64))
//             .y_grid_spacer(move |input| raster_grid_spacer(input, rh as f64))
//             .show(ui, |plot_ui| {
//                 // Bounds in pixel coordinates
//                 new_bounds = Some(plot_ui.plot_bounds());

//                 if let Some(handle) = self.handle() {
//                     let [xmin, xmax, ymin, ymax] = handle.offset_bounds();
//                     // Center in pixel coordinates
//                     // let cx = (xmin + xmax) as f64 / 2.0;
//                     // let cy = (ymin + ymax) as f64 / 2.0;
//                     let (_, rh) = raster_size; // already available in scope
//                     let cx = (xmin + xmax) as f64 / 2.0;
//                     let cy = rh as f64 - (ymin + ymax) as f64 / 2.0;
//                     // Width and height in pixel coordinates
//                     let w = (xmax - xmin) as f32;
//                     let h = (ymax - ymin) as f32;
//                     log::debug!(
//                         "draw: extent {:?} center ({}, {}) size ({}, {})",
//                         handle.extent,
//                         cx,
//                         cy,
//                         w,
//                         h
//                     );
//                     plot_ui.image(PlotImage::new(
//                         "raster",
//                         handle.texture_handle.id(),
//                         PlotPoint::new(cx, cy),
//                         vec2(w, h),
//                     ));
//                 }

//                 // make impossible to zoom subpixel
//                 clamp_max_zoom(plot_ui);
//                 clamp_pan(plot_ui, rw as f64, rh as f64);
//             });

//         // 3. Check if a new load is needed
//         if let Some(bounds) = new_bounds {
//             // With these bounds we should need the texture extent
//             let opts = ReadOptions::from_plot_bounds(1, bounds, available, raster_size);
//             // If not out of screen
//             if let Some(o) = opts {
//                 // And if need a new texture
//                 if self.needs_reload(&o) {
//                     // Ask the worker a new texture
//                     log::debug!("Ask worker new texture");
//                     dbg!(&o);
//                     let worker: ViewModeWorker = self.to_worker_with_opts(o);
//                     let _ = texture_worker.request_load(worker);
//                 }
//             }
//         }
//     }
// }

// /// Make imposible to zoom more than that pixel value in plot
// fn clamp_max_zoom(plot_ui: &mut PlotUi) {
//     let bounds = plot_ui.plot_bounds();

//     let range_x = bounds.max()[0] - bounds.min()[0];
//     let range_y = bounds.max()[1] - bounds.min()[1];

//     if range_x < 1.0 || range_y < 1.0 {
//         let cx = (bounds.min()[0] + bounds.max()[0]) / 2.0;
//         let cy = (bounds.min()[1] + bounds.max()[1]) / 2.0;

//         // clamp each axis independently, only fix the ones that went too far
//         let half_w = if range_x < 1.0 { 0.5 } else { range_x / 2.0 };
//         let half_h = if range_y < 1.0 { 0.5 } else { range_y / 2.0 };

//         plot_ui.set_plot_bounds(PlotBounds::from_min_max(
//             [cx - half_w, cy - half_h],
//             [cx + half_w, cy + half_h],
//         ));
//     }
// }

// fn clamp_pan(plot_ui: &mut PlotUi, image_w: f64, image_h: f64) {
//     let bounds = plot_ui.plot_bounds();

//     let range_x = bounds.max()[0] - bounds.min()[0];
//     let range_y = bounds.max()[1] - bounds.min()[1];

//     // window must always overlap the raster by at least 1 unit
//     let new_min_x = bounds.min()[0].clamp(1.0 - range_x, image_w - 1.0);
//     let new_min_y = bounds.min()[1].clamp(1.0 - range_y, image_h - 1.0);

//     if (new_min_x - bounds.min()[0]).abs() > f64::EPSILON
//         || (new_min_y - bounds.min()[1]).abs() > f64::EPSILON
//     {
//         plot_ui.set_plot_bounds(PlotBounds::from_min_max(
//             [new_min_x, new_min_y],
//             [new_min_x + range_x, new_min_y + range_y],
//         ));
//     }
// }

// fn raster_grid_spacer(input: egui_plot::GridInput, image_size: f64) -> Vec<egui_plot::GridMark> {
//     let (min, max) = (input.bounds.0, input.bounds.1);
//     let range = max - min;

//     if range > image_size * 1.5 {
//         // zoomed out past full image — just the two borders
//         return vec![
//             egui_plot::GridMark {
//                 value: 0.0,
//                 step_size: image_size,
//             },
//             egui_plot::GridMark {
//                 value: image_size,
//                 step_size: image_size,
//             },
//         ];
//     }

//     let step = if range < 20.0 {
//         1.0
//     } else if range < 100.0 {
//         10.0
//     } else if range < 500.0 {
//         50.0
//     } else if range < 1000.0 {
//         100.0
//     } else if range < 5000.0 {
//         500.0
//     } else {
//         1000.0
//     };

//     let first = (min / step).ceil() as i64;
//     let last = (max / step).floor() as i64;

//     (first..=last)
//         .map(|n| egui_plot::GridMark {
//             value: n as f64 * step,
//             step_size: step,
//         })
//         .collect()
// }
