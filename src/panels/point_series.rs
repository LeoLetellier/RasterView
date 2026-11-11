use std::sync::{Arc, Mutex};

use eframe::egui::{
    Color32, Context, CursorIcon, DragValue, Id, Key, Painter, Pos2, RichText, Shape, Spinner,
    Stroke, Ui, vec2,
};
use egui_plot::{Line, Plot, PlotPoints, VLine};
use gdal::Dataset;

use crate::{
    Viewer,
    raster::{RasterHandler, series::SerieReader},
    viewers::ActiveViewer,
    widgets::phosphor::Icon,
};

/// Band indices are 1-based (GDAL convention): valid range is FIRST..=last.
const FIRST: usize = 1;

// Same assumption as before: glyph strings come from egui_phosphor.
const CROSSHAIR: Icon =
    Icon::new(egui_phosphor::regular::CROSSHAIR).with_fill(egui_phosphor::fill::CROSSHAIR);

const POINT_COLOR: Color32 = Color32::from_rgb(255, 210, 60);
const REF_COLOR: Color32 = Color32::from_rgb(80, 220, 230);

// ---------- Shared selection (written by the panel AND by the image viewer) ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum PickTarget {
    #[default]
    None,
    Point,
    Reference,
}

#[derive(Clone, Copy)]
struct SeriesSelection {
    point: (usize, usize), // (x = col, y = row)
    use_reference: bool,
    reference: (usize, usize),
    point_half: usize,
    ref_half: usize,
    pick: PickTarget,
    live: bool,
}

fn selection_id() -> Id {
    Id::unique("point_series_selection")
}

/// True while the panel wants the viewer's attention (waiting for a click, or live mode).
pub(crate) fn series_is_picking(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<SeriesSelection>(selection_id()))
        .is_some_and(|s| s.pick != PickTarget::None || s.live)
}

/// True while the point follows the mouse.
pub(crate) fn series_is_live(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<SeriesSelection>(selection_id()))
        .is_some_and(|s| s.live)
}

/// Call on a click with the pixel under the cursor (col, row).
/// Returns true if the click was consumed (pick, or freezing live mode).
pub(crate) fn series_handle_click(ctx: &Context, pixel: (usize, usize)) -> bool {
    let id = selection_id();
    let Some(mut sel) = ctx.data(|d| d.get_temp::<SeriesSelection>(id)) else {
        return false;
    };
    if sel.pick != PickTarget::None {
        match sel.pick {
            PickTarget::Point => sel.point = pixel,
            PickTarget::Reference => sel.reference = pixel,
            PickTarget::None => {}
        }
        sel.pick = PickTarget::None; // one-shot
    } else if sel.live {
        sel.point = pixel;
        sel.live = false; // click freezes the point
    } else {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(id, sel));
    ctx.request_repaint();
    true
}

/// Call every frame with the pixel under the cursor while it hovers the image.
/// Updates the point only when live mode is on.
pub(crate) fn series_handle_hover(ctx: &Context, pixel: (usize, usize)) {
    let id = selection_id();
    let Some(mut sel) = ctx.data(|d| d.get_temp::<SeriesSelection>(id)) else {
        return;
    };
    if sel.live && sel.pick == PickTarget::None && sel.point != pixel {
        sel.point = pixel;
        ctx.data_mut(|d| d.insert_temp(id, sel));
    }
}

/// For the image viewer: draws the point / reference windows on top of the image.
/// `to_screen` maps image pixel coordinates (col, row as f32) to screen positions.
pub(crate) fn draw_series_markers(
    ctx: &Context,
    painter: &Painter,
    to_screen: impl Fn(Pos2) -> Pos2,
) {
    let Some(sel) = ctx.data(|d| d.get_temp::<SeriesSelection>(selection_id())) else {
        return;
    };
    let draw = |center: (usize, usize), half: usize, color: Color32| {
        let (cx, cy) = (center.0 as f32, center.1 as f32);
        let h = half as f32;
        let stroke = Stroke::new(1.5, color);

        // Window outline (covers the whole pixel range [c-half, c+half]).
        let corners = [
            to_screen(Pos2::new(cx - h, cy - h)),
            to_screen(Pos2::new(cx + h + 1.0, cy - h)),
            to_screen(Pos2::new(cx + h + 1.0, cy + h + 1.0)),
            to_screen(Pos2::new(cx - h, cy + h + 1.0)),
        ];
        painter.add(Shape::closed_line(corners.to_vec(), stroke));

        // Small cross at the pixel center, so single-pixel selections stay visible.
        let c = to_screen(Pos2::new(cx + 0.5, cy + 0.5));
        painter.line_segment([c - egui_vec(6.0, 0.0), c + egui_vec(6.0, 0.0)], stroke);
        painter.line_segment([c - egui_vec(0.0, 6.0), c + egui_vec(0.0, 6.0)], stroke);
    };
    draw(sel.point, sel.point_half, POINT_COLOR);
    if sel.use_reference {
        draw(sel.reference, sel.ref_half, REF_COLOR);
    }
}

fn egui_vec(x: f32, y: f32) -> eframe::egui::Vec2 {
    eframe::egui::vec2(x, y)
}

// ---------- Job (unchanged) ----------

#[derive(Clone)]
enum JobState {
    Running,
    Done(Arc<Vec<f32>>),
    Failed(String),
}

#[derive(Clone, PartialEq)]
struct SeriesKey {
    path: String,
    point: (usize, usize),
    reference: Option<(usize, usize)>,
    point_half: usize,
    ref_half: usize,
}

#[derive(Clone)]
struct SeriesJob {
    key: SeriesKey,
    state: Arc<Mutex<JobState>>,
}

// spawn_series_job(...) stays exactly as before.

// ---------- Panel ----------

fn pick_button(ui: &mut Ui, pick: &mut PickTarget, me: PickTarget, tip: &str) {
    let active = *pick == me;
    if ui
        .selectable_label(active, CROSSHAIR.rich(active))
        .on_hover_text(tip)
        .clicked()
    {
        *pick = if active { PickTarget::None } else { me };
    }
}

pub(crate) fn ui_point_series(ui: &mut Ui, viewer: &mut Viewer, rh: &mut RasterHandler) {
    let band_count = rh.raster_count();
    if band_count == 0 {
        ui.weak("No bands");
        return;
    }
    let last = band_count;
    let (width, height) = rh.raster_metadata.size;
    let max_x = width.saturating_sub(1);
    let max_y = height.saturating_sub(1);

    let sel_id = selection_id();
    let job_id = ui.scope_id().with("point_series_job");
    let shown_id = job_id.with("shown");

    let mut sel: SeriesSelection = ui.data_mut(|d| d.get_temp(sel_id)).unwrap_or_else(|| {
        let center = (width / 2, height / 2);
        SeriesSelection {
            point: center,
            use_reference: false,
            reference: center,
            point_half: 0,
            ref_half: 0,
            pick: PickTarget::None,
            live: false,
        }
    });
    let mut job: Option<SeriesJob> = ui.data_mut(|d| d.get_temp(job_id));
    // Last finished series, kept on screen while a newer read is running.
    let mut shown: Option<Arc<Vec<f32>>> = ui.data_mut(|d| d.get_temp(shown_id));
    let running = job
        .as_ref()
        .is_some_and(|j| matches!(*j.state.lock().unwrap(), JobState::Running));

    sel.point = (sel.point.0.min(max_x), sel.point.1.min(max_y));
    sel.reference = (sel.reference.0.min(max_x), sel.reference.1.min(max_y));

    if sel.pick != PickTarget::None || sel.live {
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            sel.pick = PickTarget::None;
            sel.live = false;
        } else if sel.pick != PickTarget::None {
            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
        }
    }

    // ---- Controls ----
    ui.horizontal(|ui| {
        pick_button(
            ui,
            &mut sel.pick,
            PickTarget::Point,
            "Pick the point on the image",
        );
        ui.colored_label(POINT_COLOR, "Point");
        ui.add(
            DragValue::new(&mut sel.point.0)
                .range(0..=max_x)
                .prefix("x "),
        );
        ui.add(
            DragValue::new(&mut sel.point.1)
                .range(0..=max_y)
                .prefix("y "),
        );
        ui.add(
            DragValue::new(&mut sel.point_half)
                .range(0..=50)
                .prefix("±"),
        )
        .on_hover_text("Half window size in pixels (median over the window)");
        ui.toggle_value(&mut sel.live, "Live")
            .on_hover_text("Point follows the mouse over the image. Click to freeze, Esc to stop.");
        status_spinner(ui, running);
    });

    ui.horizontal(|ui| {
        ui.add_enabled_ui(sel.use_reference, |ui| {
            pick_button(
                ui,
                &mut sel.pick,
                PickTarget::Reference,
                "Pick the reference on the image",
            );
        });
        ui.checkbox(&mut sel.use_reference, "Reference");
        ui.add_enabled_ui(sel.use_reference, |ui| {
            ui.add(
                DragValue::new(&mut sel.reference.0)
                    .range(0..=max_x)
                    .prefix("x "),
            );
            ui.add(
                DragValue::new(&mut sel.reference.1)
                    .range(0..=max_y)
                    .prefix("y "),
            );
            ui.add(DragValue::new(&mut sel.ref_half).range(0..=50).prefix("±"))
                .on_hover_text("Reference half window size in pixels");
        });
    });
    if !sel.use_reference && sel.pick == PickTarget::Reference {
        sel.pick = PickTarget::None;
    }
    match sel.pick {
        PickTarget::Point => {
            ui.weak("Click on the image to set the point (Esc to cancel)");
        }
        PickTarget::Reference => {
            ui.weak("Click on the image to set the reference (Esc to cancel)");
        }
        PickTarget::None => {}
    }

    // ---- (Re)start the job if parameters changed ----
    let key = SeriesKey {
        path: rh.raster_metadata.description.clone(),
        point: sel.point,
        reference: sel.use_reference.then_some(sel.reference),
        point_half: sel.point_half,
        ref_half: sel.ref_half,
    };
    let key_changed = job.as_ref().map(|j| &j.key) != Some(&key);
    if key_changed && !(sel.live && running) {
        job = Some(spawn_series_job(ui.ctx().clone(), key));
        ui.ctx().request_repaint(); // show the spinner on the next frame
    }

    // ---- Show state / plot ----
    let job_state = job.as_ref().map(|j| j.state.lock().unwrap().clone());
    match &job_state {
        Some(JobState::Done(s)) => shown = Some(Arc::clone(s)),
        Some(JobState::Failed(msg)) => {
            ui.colored_label(Color32::from_rgb(230, 90, 90), msg);
        }
        _ => {}
    }

    if let Some(series) = &shown {
        let points: PlotPoints = series
            .iter()
            .enumerate()
            .filter(|(_, v)| v.is_finite())
            .map(|(i, v)| [(i + 1) as f64, *v as f64])
            .collect();

        let cursors: Vec<(f64, Color32)> = match viewer.view_mode.active_viewer {
            ActiveViewer::Band => {
                vec![(viewer.view_mode.panchro_band as f64, Color32::LIGHT_GRAY)]
            }
            ActiveViewer::Color => {
                let (r, g, b) = viewer.view_mode.rgb_bands;
                vec![
                    (r as f64, Color32::from_rgb(230, 90, 90)),
                    (g as f64, Color32::from_rgb(90, 200, 90)),
                    (b as f64, Color32::from_rgb(90, 140, 230)),
                ]
            }
            ActiveViewer::Index => vec![],
        };

        let resp = Plot::new(job_id.with("plot"))
            .height(180.0)
            .x_axis_label("band")
            .allow_scroll(false)
            .show(ui, |plot_ui| {
                plot_ui.line(Line::new("series", points));
                for (x, color) in cursors {
                    plot_ui.vline(VLine::new("", x).color(color));
                }
                plot_ui.pointer_coordinate()
            });

        if resp.response.clicked() {
            if let Some(p) = resp.inner {
                let target = (p.x.round().max(FIRST as f64) as usize).min(last);
                let old_view = viewer.view_mode.clone();
                match viewer.view_mode.active_viewer {
                    ActiveViewer::Band => viewer.view_mode.panchro_band = target,
                    ActiveViewer::Color => {
                        let (r, g, b) = viewer.view_mode.rgb_bands;
                        let lo = r.min(g).min(b);
                        let span = r.max(g).max(b) - lo;
                        let target = target.min(last.saturating_sub(span)).max(FIRST);
                        shift_rgb(viewer, target as isize - lo as isize, last);
                    }
                    ActiveViewer::Index => {}
                }
                if old_view != viewer.view_mode {
                    viewer.update_view(rh);
                }
            }
        }

        let finite: Vec<f32> = series.iter().copied().filter(|v| v.is_finite()).collect();
        if let (Some(min), Some(max)) = (
            finite.iter().copied().reduce(f32::min),
            finite.iter().copied().reduce(f32::max),
        ) {
            ui.label(
                RichText::new(format!(
                    "{} bands · min {min:.4} · max {max:.4}",
                    series.len()
                ))
                .small()
                .weak(),
            );
        }
    }

    ui.data_mut(|d| {
        d.insert_temp(sel_id, sel);
        if let Some(job) = job {
            d.insert_temp(job_id, job);
        }
        if let Some(shown) = shown {
            d.insert_temp(shown_id, shown);
        }
    });
}

// shift_rgb stays as it is.
fn shift_rgb(viewer: &mut Viewer, delta: isize, last: usize) {
    let (r, g, b) = viewer.view_mode.rgb_bands;
    let shift = |v: usize| (v as isize + delta).clamp(FIRST as isize, last as isize) as usize;
    viewer.view_mode.rgb_bands = (shift(r), shift(g), shift(b));
}

/// Spawns the read on a worker thread. The dataset is reopened there because a
/// GDAL `Dataset` must not be shared with the UI thread.
fn spawn_series_job(ctx: Context, key: SeriesKey) -> SeriesJob {
    let state = Arc::new(Mutex::new(JobState::Running));
    let (thread_state, thread_key) = (Arc::clone(&state), key.clone());

    std::thread::spawn(move || {
        let result = (|| {
            let dataset = Dataset::open(&thread_key.path)?;
            let mut reader = SerieReader::new(thread_key.point);
            if let Some(r) = thread_key.reference {
                reader.with_reference(r);
            }
            reader.with_window(thread_key.point_half, thread_key.ref_half);
            reader.read_on_dataset(&dataset)
        })();

        *thread_state.lock().unwrap() = match result {
            Ok(v) => JobState::Done(Arc::new(v)),
            Err(e) => JobState::Failed(e.to_string()),
        };
        ctx.request_repaint(); // wake the UI when the result is ready
    });

    SeriesJob { key, state }
}

/// Spinner when `busy`, otherwise an empty slot of the same size,
/// so the surrounding layout never shifts.
fn status_spinner(ui: &mut Ui, busy: bool) {
    let size = 14.0;
    if busy {
        ui.add(Spinner::new().size(size));
    } else {
        ui.allocate_space(vec2(size, size));
    }
}
