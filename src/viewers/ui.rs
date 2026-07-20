use crate::viewers::Viewer;
use crate::viewers::coords::{Bbox, PixelBox};
use crate::viewers::tiler::TileDescriptor;
use egui::Ui;
use egui_plot::{Plot, PlotBounds, PlotPoint, PlotPoints, PlotUi, Polygon};

impl Viewer {
    pub(crate) fn ui(&mut self, ui: &mut Ui) {
        if cfg!(debug_assertions) {
            let context_count = ui.ctx().tex_manager().read().allocated().count();
            println!("Context count: {}", context_count);
        }

        let tiles_needed = self.need_tiles();

        let last_view_center = self.state.last_bounds.map(|lb| lb.center());
        let tiles = tiles_needed
            .clone()
            .zip(last_view_center)
            .map(|(tn, lvc)| {
                self.raster_handler
                    .request_cache_tiles(&tn, lvc, self.view_mode.clone())
                    .ok()
            })
            .flatten();

        let ppp = ui.ctx().pixels_per_point() as f64;
        let mut last_bounds = None;
        let mut last_screen_size = None;

        let raster_size = self.raster_handler.raster_size();
        // Avoid weird default plot position
        let plot = if raster_size.0 > raster_size.1 {
            Plot::new("main_plot")
                .default_x_bounds(-0.1 * raster_size.0 as f64, 1.1 * raster_size.0 as f64)
                .data_aspect(1.0)
                .pan_pointer_button(egui::PointerButton::Primary)
                .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
                .allow_scroll(false)
                .allow_zoom(false)
                .show_grid(false)
        } else {
            Plot::new("main_plot")
                .default_x_bounds(-0.1 * raster_size.1 as f64, 1.1 * raster_size.1 as f64)
                .data_aspect(1.0)
                .pan_pointer_button(egui::PointerButton::Primary)
                .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
                .allow_scroll(false)
                .allow_zoom(false)
                .show_grid(false)
        };

        // Grab wheel delta + modifier state BEFORE the closure borrows `ui`.
        let (wheel_delta_y, ctrl_held): (Option<f32>, bool) = ui.input(|i| {
            i.events
                .iter()
                .find_map(|e| {
                    if let egui::Event::MouseWheel {
                        delta, modifiers, ..
                    } = e
                    {
                        return Some((delta.y, modifiers.ctrl || modifiers.command));
                    }
                    None
                })
                .map_or((None, false), |(dy, ctrl)| (Some(dy), ctrl))
        });

        let plot_response = plot.show(ui, |plot_ui| {
            scroll_zoom(plot_ui, wheel_delta_y, ctrl_held, 0.2);

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

        if let Some(cpp) = cursor_plot_pos {
            let cpp = PlotPoint::new(cpp.x, self.raster_handler.raster_size().1 as f64 - cpp.y);
            self.state.last_cursor_pos = Some(cpp)
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

fn scroll_zoom(
    plot_ui: &mut PlotUi,
    wheel_delta_y: Option<f32>,
    ctrl_held: bool,
    zoom_velocity: f32,
) {
    let resp = plot_ui.response();
    if !resp.contains_pointer() {
        return;
    }
    let Some(wy) = wheel_delta_y else { return };
    if wy == 0.0 {
        return;
    }

    let effective_velocity = if ctrl_held {
        zoom_velocity / 5.0
    } else {
        zoom_velocity
    };

    let zoom_factor = (wy * effective_velocity).exp();
    let current_bounds = plot_ui.plot_bounds();
    let Some(cursor_plot) = plot_ui.pointer_coordinate() else {
        return;
    };

    let min = current_bounds.min();
    let max = current_bounds.max();
    let new_min = [
        cursor_plot.x - (cursor_plot.x - min[0]) / zoom_factor as f64,
        cursor_plot.y - (cursor_plot.y - min[1]) / zoom_factor as f64,
    ];
    let new_max = [
        cursor_plot.x + (max[0] - cursor_plot.x) / zoom_factor as f64,
        cursor_plot.y + (max[1] - cursor_plot.y) / zoom_factor as f64,
    ];

    if (new_max[0] - new_min[0]).abs() > 1e-12 && (new_max[1] - new_min[1]).abs() > 1e-12 {
        plot_ui.set_plot_bounds(PlotBounds::from_min_max(new_min, new_max));
    }
}
