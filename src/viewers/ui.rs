use crate::viewers::coords::{Bbox, PixelBox};
use crate::viewers::tiler::TileDescriptor;
use crate::viewers::{Viewer, dummy_checkerboard, dummy_gradient};
use egui::Ui;
use egui_plot::{Plot, PlotPoint, PlotPoints, PlotUi, Polygon};

impl Viewer {
    pub fn ui(&mut self, ui: &mut Ui) {
        let tiles_needed = self.need_tiles();
        let Some(rh) = &mut self.raster_handler else {
            return;
        };
        // let tiles = tiles_needed
        //     .clone()
        //     .map(|tn| rh.request_cache_tiles(&tn, ui).ok())
        //     .flatten();
        let last_view_center = self.state.last_bounds.map(|lb| lb.center());
        let tiles = tiles_needed
            .clone()
            .zip(last_view_center)
            .map(|(tn, lvc)| {
                rh.request_cache_tiles2(&tn, lvc, self.view_mode.clone())
                    .ok()
            })
            .flatten();

        let ppp = ui.ctx().pixels_per_point() as f64;
        let mut last_bounds = None;
        let mut last_screen_size = None;

        let plot_response = Plot::new("main_plot")
            .data_aspect(1.0)
            .pan_pointer_button(egui::PointerButton::Primary)
            .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
            .allow_scroll(false)
            .allow_zoom(true)
            .show_grid(false)
            .show(ui, |plot_ui| {
                let rect = plot_ui.response().rect;
                last_screen_size = Some((rect.width() as f64 * ppp, rect.height() as f64 * ppp));
                last_bounds = Some(plot_ui.plot_bounds());

                tiles.map(|ot| ot.iter().for_each(|t| t.plot_ui(plot_ui)));
                if cfg!(debug_assertions) {
                    if let Some(tiles) = tiles_needed {
                        tiles.iter().for_each(|t| t.ui_tile_bounds(plot_ui));
                    }
                }
            });

        self.state.last_screen_size = last_screen_size;
        self.state.last_bounds = last_bounds;

        // With this it is None when cursor is outside the plot
        //
        // otherwise could have used cursor_plot_pos = plot_ui.pointer_coordinate(); in the
        // plot code instead
        let cursor_plot_pos = if let Some(screen_pos) = plot_response.response.hover_pos() {
            let res_pos = plot_response.transform.value_from_position(screen_pos);
            Some(PlotPoint {
                x: res_pos.x,
                y: res_pos.y,
            })
        } else {
            None
        };

        if let Some(rh) = &self.raster_handler {
            if let Some(cpp) = cursor_plot_pos {
                let cpp = PlotPoint::new(cpp.x, rh.raster_size().1 as f64 - cpp.y);
                self.state.last_cursor_pos = Some(cpp)
            }
        }
    }
}

impl TileDescriptor {
    fn ui_tile_bounds(&self, plot_ui: &mut PlotUi) {
        let bbox = self.pixel_box();

        let xmin = bbox.xmin() as f64;
        let xmax = bbox.xmax() as f64;
        let ymin = bbox.ymin() as f64;
        let ymax = bbox.ymax() as f64;

        let points: PlotPoints =
            vec![[xmin, ymin], [xmax, ymin], [xmax, ymax], [xmin, ymax]].into();

        let polygon = Polygon::new("tile_bounds", points)
            .stroke(egui::Stroke::new(1.0, egui::Color32::RED))
            .fill_color(egui::Color32::TRANSPARENT);

        plot_ui.polygon(polygon);
    }
}
