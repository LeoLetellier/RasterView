use std::ops::RangeInclusive;

use crate::viewers::coords::Bbox;
use crate::viewers::tiler::TileDescriptor;
use crate::{panels::parameters::Settings, viewers::Viewer};
use egui::Ui;
use egui_plot::{GridInput, GridMark, Plot, PlotBounds, PlotPoint, PlotPoints, PlotUi, Polygon};

impl Viewer {
    pub(crate) fn ui(&mut self, ui: &mut Ui, settings: &Settings) {
        if cfg!(debug_assertions) {
            let context_count = ui.ctx().tex_manager().read().allocated().count();
            tracing::trace!("Context count: {}", context_count);
        }

        if self.parameters.auto_load_band_stats {
            self.raster_handler.preload_band_stats();
        }

        let tiles_needed = self.need_tiles();

        let last_view_center = self.state.last_bounds.map(|lb| lb.center());
        let tiles = tiles_needed
            .clone()
            .zip(last_view_center)
            .and_then(|(tn, lvc)| {
                self.raster_handler
                    .request_cache_tiles(&tn, lvc, self.view_mode.clone())
                    .ok()
            });

        let ppp = ui.ctx().pixels_per_point() as f64;
        let mut last_bounds = None;
        let mut last_screen_size = None;

        let raster_size = self.raster_handler.raster_size();
        // Avoid crash viewport padding <= 0.5 (min max crossed)
        let vpad = settings.viewport_padding.max(0.6);

        let (rw, rh) = (raster_size.0 as f64, raster_size.1 as f64);
        let pad = (vpad - 1.0) * rw.max(rh);
        // padded extent: used for auto-fit/reset and as the pan/zoom limit
        let full = PlotBounds::from_min_max([-pad, -pad], [rw + pad, rh + pad]);
        // Define zoom limits
        let limits = ZoomLimits {
            max_dezoom: 4.5,
            max_zoom_in: 64.0,
            min_visible_pts: 100.0,
        };

        let plot = Plot::new("main_plot")
            .include_x(full.min()[0])
            .include_x(full.max()[0])
            .include_y(full.min()[1])
            .include_y(full.max()[1])
            .set_margin_fraction(egui::Vec2::ZERO)
            .data_aspect(1.0)
            .pan_pointer_button(egui::PointerButton::Primary)
            .boxed_zoom_pointer_button(egui::PointerButton::Secondary)
            .allow_scroll(false)
            .allow_zoom(false)
            .show_grid(false)
            // invert y axis grid to match raster pixel
            .y_axis_formatter(inverted_y_formatter(raster_size.1 as f64))
            .y_grid_spacer(inverted_y_grid_spacer(raster_size.1 as f64));

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

        //         let plot_response = plot.show(ui, |plot_ui| {
        //             scroll_zoom(plot_ui, wheel_delta_y, ctrl_held, 0.2);
        //
        //             let rect = plot_ui.response().rect;
        //             last_screen_size = Some((rect.width() as f64 * ppp, rect.height() as f64 * ppp));
        //             last_bounds = Some(plot_ui.plot_bounds());
        let plot_response = plot.show(ui, |plot_ui| {
            scroll_zoom(plot_ui, wheel_delta_y, ctrl_held, 0.2, &full, limits);
            clamp_view(plot_ui, &full, limits);

            let rect = plot_ui.response().rect;
            last_screen_size = Some((rect.width() as f64 * ppp, rect.height() as f64 * ppp));
            last_bounds = Some(plot_ui.plot_bounds()); // now the clamped bounds

            if let Some(ot) = tiles {
                ot.iter().for_each(|t| t.plot_ui(plot_ui))
            }
            if settings.show_tiles
                && let Some(tiles) = tiles_needed
            {
                tiles.iter().for_each(|t| t.ui_tile_bounds(plot_ui));
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

#[derive(Clone, Copy)]
struct ZoomLimits {
    /// Maximum dezoom. 1.0 = the padded extent just fits the panel (can't zoom out
    /// further than that); 1.5 = may zoom out to 1.5x beyond fit.
    max_dezoom: f64,
    /// Maximum zoom-in, in screen points per plot unit.
    max_zoom_in: f64,
    /// Minimum part of the extent that must stay inside the view, in screen points.
    /// 0.0 = the extent may slide until it just touches the view edge.
    min_visible_pts: f64,
}

impl ZoomLimits {
    /// Allowed (min, max) scale in screen points per plot unit for the current panel size.
    fn scale_range(&self, full: &PlotBounds, pw: f64, ph: f64) -> (f64, f64) {
        let fit_scale = (pw / full.width()).min(ph / full.height());
        let min_scale = fit_scale / self.max_dezoom;
        (min_scale, self.max_zoom_in.max(min_scale))
    }
}

fn scroll_zoom(
    plot_ui: &mut PlotUi,
    wheel_delta_y: Option<f32>,
    ctrl_held: bool,
    zoom_velocity: f32,
    full: &PlotBounds,
    limits: ZoomLimits,
) {
    let resp = plot_ui.response();
    if !resp.contains_pointer() {
        return;
    }
    let Some(wy) = wheel_delta_y else { return };
    if wy == 0.0 {
        return;
    }

    let rect = resp.rect;
    let (pw, ph) = (rect.width() as f64, rect.height() as f64);
    if pw < 1.0 || ph < 1.0 {
        return;
    }

    let effective_velocity = if ctrl_held {
        zoom_velocity / 5.0
    } else {
        zoom_velocity
    };
    let requested_factor = (wy * effective_velocity).exp() as f64;

    let current_bounds = plot_ui.plot_bounds();
    if current_bounds.width() <= 0.0 {
        return;
    }
    let Some(cursor_plot) = plot_ui.pointer_coordinate() else {
        return;
    };

    // Clamp the *factor* so the resulting scale stays within limits.
    // At the limit the factor becomes 1.0 and we return: no zoom, no shift.
    let scale = pw / current_bounds.width();
    let (min_scale, max_scale) = limits.scale_range(full, pw, ph);
    let zoom_factor = (scale * requested_factor).clamp(min_scale, max_scale) / scale;
    if (zoom_factor - 1.0).abs() < 1e-9 {
        return;
    }

    let min = current_bounds.min();
    let max = current_bounds.max();
    let new_min = [
        cursor_plot.x - (cursor_plot.x - min[0]) / zoom_factor,
        cursor_plot.y - (cursor_plot.y - min[1]) / zoom_factor,
    ];
    let new_max = [
        cursor_plot.x + (max[0] - cursor_plot.x) / zoom_factor,
        cursor_plot.y + (max[1] - cursor_plot.y) / zoom_factor,
    ];

    plot_ui.set_plot_bounds(PlotBounds::from_min_max(new_min, new_max));
}

/// Limits dezoom/zoom in pixels-per-unit and keeps the view center inside `full`.
/// `max_dezoom`: 1.0 = the whole extent just fits the panel; 1.5 = may zoom out 1.5x beyond that.
/// `max_scale`: maximum zoom-in, in screen points per plot unit.
fn clamp_view(plot_ui: &mut PlotUi, full: &PlotBounds, limits: ZoomLimits) {
    let rect = plot_ui.response().rect;
    let (pw, ph) = (rect.width() as f64, rect.height() as f64);
    if pw < 1.0 || ph < 1.0 {
        return;
    }

    let b = plot_ui.plot_bounds();
    let (w, h) = (b.width(), b.height());
    if w <= 0.0 || h <= 0.0 {
        return;
    }

    let scale = pw / w;
    let (min_scale, max_scale) = limits.scale_range(full, pw, ph);
    let target_scale = scale.clamp(min_scale, max_scale);
    let (nw, nh) = (pw / target_scale, ph / target_scale);

    // Center may travel until the extent only overlaps the view by `keep` units.
    // keep <= half view size guarantees lo <= hi (clamp would panic otherwise).
    let keep_x = (limits.min_visible_pts / target_scale).min(nw * 0.5);
    let keep_y = (limits.min_visible_pts / target_scale).min(nh * 0.5);

    let (ccx, ccy) = (
        0.5 * (b.min()[0] + b.max()[0]),
        0.5 * (b.min()[1] + b.max()[1]),
    );
    let cx = ccx.clamp(
        full.min()[0] - nw * 0.5 + keep_x,
        full.max()[0] + nw * 0.5 - keep_x,
    );
    let cy = ccy.clamp(
        full.min()[1] - nh * 0.5 + keep_y,
        full.max()[1] + nh * 0.5 - keep_y,
    );

    let changed = (nw - w).abs() > 1e-9 * w
        || (nh - h).abs() > 1e-9 * h
        || (cx - ccx).abs() > 1e-9
        || (cy - ccy).abs() > 1e-9;

    if changed {
        plot_ui.set_plot_bounds(PlotBounds::from_min_max(
            [cx - nw * 0.5, cy - nh * 0.5],
            [cx + nw * 0.5, cy + nh * 0.5],
        ));
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

/// Returns a y-axis formatter that displays `size - y` instead of `y`.
///
/// This helps to match raster reference, where origin is upper left instead of lower left.
fn inverted_y_formatter(offset: f64) -> impl Fn(GridMark, &RangeInclusive<f64>) -> String {
    move |mark: GridMark, _range: &RangeInclusive<f64>| {
        let inverted = offset - mark.value;
        format!("{:.0}", inverted)
    }
}

/// Correct the tick selection when using an offset
///
/// The tick will now be aligned to the new zero instead of the offset (old zero),
/// and all ticks stay round numbers
fn inverted_y_grid_spacer(offset: f64) -> impl Fn(GridInput) -> Vec<GridMark> {
    let inner = egui_plot::log_grid_spacer(10);
    move |input: GridInput| {
        let (orig_min, orig_max) = input.bounds;
        let inv_input = GridInput {
            bounds: (offset - orig_max, offset - orig_min),
            base_step_size: input.base_step_size,
        };
        inner(inv_input)
            .into_iter()
            .map(|mark| GridMark {
                value: offset - mark.value,
                step_size: mark.step_size,
            })
            .collect()
    }
}
