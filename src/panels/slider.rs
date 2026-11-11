use eframe::egui::{Button, Color32, DragValue, RichText, Slider, Ui};

use crate::widgets::phosphor::Icon;
use crate::{Viewer, raster::RasterHandler, viewers::ActiveViewer};

// Adjust the source of the glyph strings if they don't come from egui_phosphor.
pub const SKIP_BACK: Icon =
    Icon::new(egui_phosphor::regular::SKIP_BACK).with_fill(egui_phosphor::fill::SKIP_BACK);
pub const SKIP_FORWARD: Icon =
    Icon::new(egui_phosphor::regular::SKIP_FORWARD).with_fill(egui_phosphor::fill::SKIP_FORWARD);
pub const PLAY: Icon = Icon::new(egui_phosphor::regular::PLAY).with_fill(egui_phosphor::fill::PLAY);
pub const PAUSE: Icon =
    Icon::new(egui_phosphor::regular::PAUSE).with_fill(egui_phosphor::fill::PAUSE);

// Point series

pub(crate) fn ui_point_series(ui: &mut Ui, viewer: &mut Viewer, rh: &mut RasterHandler) {
    ui.label("point series");
}

// Band slider

#[derive(Clone, Copy)]
struct PlayState {
    playing: bool,
    rate: f32,  // bands per second
    accum: f32, // fractional steps carried between frames
}

impl Default for PlayState {
    fn default() -> Self {
        Self {
            playing: false,
            rate: 1.0,
            accum: 0.0,
        }
    }
}

/// Band indices are 1-based (GDAL convention): valid range is FIRST..=last.
const FIRST: usize = 1;

/// Horizontal space reserved for ◀ ▶ ⏯, the slider's value box and the rate field.
const ROW_RESERVED_WIDTH: f32 = 210.0;

pub(crate) fn ui_band_slider(ui: &mut Ui, viewer: &mut Viewer, rh: &mut RasterHandler) {
    let band_count = rh.raster_count();
    if band_count == 0 {
        ui.weak("No bands");
        return;
    }
    let last = band_count; // 1-based: last valid band == count
    let old_view = viewer.view_mode.clone();

    let play_id = ui.scope_id().with("band_player");
    let mut play: PlayState = ui.data_mut(|d| d.get_temp(play_id)).unwrap_or_default();

    match viewer.view_mode.active_viewer {
        ActiveViewer::Band => {
            let mut band = viewer.view_mode.panchro_band.clamp(FIRST, last);
            band_slider_row(ui, &mut band, FIRST, last, &mut play);
            ui.label(RichText::new(band_description(rh, band)).small().weak());
            viewer.view_mode.panchro_band = band;
        }
        ActiveViewer::Color => {
            let (r, g, b) = viewer.view_mode.rgb_bands;
            let lo = r.min(g).min(b).max(FIRST);
            let span = r.max(g).max(b).saturating_sub(lo);

            // Slider ranges over valid positions of the lowest band of the triplet.
            let max_start = last.saturating_sub(span).max(FIRST);
            let mut start = lo.clamp(FIRST, max_start);
            band_slider_row(ui, &mut start, FIRST, max_start, &mut play);

            if start != lo {
                shift_rgb(viewer, start as isize - lo as isize, last);
            }

            let (r, g, b) = viewer.view_mode.rgb_bands;
            for (label, color, idx) in [
                ("R", Color32::from_rgb(230, 90, 90), r),
                ("G", Color32::from_rgb(90, 200, 90), g),
                ("B", Color32::from_rgb(90, 140, 230), b),
            ] {
                ui.horizontal(|ui| {
                    ui.colored_label(color, format!("● {label}"));
                    ui.label(
                        RichText::new(format!("{} · {}", idx, band_description(rh, idx)))
                            .small()
                            .weak(),
                    );
                });
            }
        }
        ActiveViewer::Index => {
            play.playing = false;
            ui.weak("Band slider is not available in Index mode");
        }
    }

    // ---- Playback tick ----
    if play.playing && viewer.view_mode.active_viewer != ActiveViewer::Index {
        let dt = ui.ctx().input(|i| i.stable_dt);
        play.accum += dt * play.rate;
        let steps = play.accum.floor() as usize;
        if steps > 0 {
            play.accum -= steps as f32;
            advance(viewer, steps, last);
        }
        ui.ctx().request_repaint();
    }

    ui.data_mut(|d| d.insert_temp(play_id, play));

    if old_view != viewer.view_mode {
        viewer.update_view(rh);
    }
}

/// Moves forward by `steps`, wrapping from `last` back to FIRST.
fn advance(viewer: &mut Viewer, steps: usize, last: usize) {
    match viewer.view_mode.active_viewer {
        ActiveViewer::Band => {
            let b = viewer.view_mode.panchro_band.clamp(FIRST, last);
            let n = last - FIRST + 1; // number of bands
            viewer.view_mode.panchro_band = FIRST + (b - FIRST + steps) % n;
        }
        ActiveViewer::Color => {
            let (r, g, b) = viewer.view_mode.rgb_bands;
            let lo = r.min(g).min(b).max(FIRST);
            let span = r.max(g).max(b).saturating_sub(lo);
            let positions = (last.saturating_sub(span)).max(FIRST) - FIRST + 1; // valid starts
            let new_start = FIRST + (lo - FIRST + steps) % positions;
            shift_rgb(viewer, new_start as isize - lo as isize, last);
        }
        ActiveViewer::Index => {}
    }
}

fn shift_rgb(viewer: &mut Viewer, delta: isize, last: usize) {
    let (r, g, b) = viewer.view_mode.rgb_bands;
    let shift = |v: usize| (v as isize + delta).clamp(FIRST as isize, last as isize) as usize;
    viewer.view_mode.rgb_bands = (shift(r), shift(g), shift(b));
}

/// Single line: [⏮] [-------●-------] [⏭] [⏵/⏸] [rate /s]
/// Touching the slider or the skip buttons pauses playback.
fn band_slider_row(ui: &mut Ui, value: &mut usize, min: usize, max: usize, play: &mut PlayState) {
    ui.horizontal(|ui| {
        let mut interacted = false;

        if ui
            .add_enabled(*value > min, Button::new(SKIP_BACK.rich(false)))
            .on_hover_text("Previous band")
            .clicked()
        {
            *value -= 1;
            interacted = true;
        }

        ui.spacing_mut().slider_width = (ui.available_width() - ROW_RESERVED_WIDTH).max(60.0);
        if ui.add(Slider::new(value, min..=max)).changed() {
            interacted = true;
        }

        if ui
            .add_enabled(*value < max, Button::new(SKIP_FORWARD.rich(false)))
            .on_hover_text("Next band")
            .clicked()
        {
            *value += 1;
            interacted = true;
        }

        if interacted {
            play.playing = false;
        }

        let (icon, tip) = if play.playing {
            (PAUSE, "Pause")
        } else {
            (PLAY, "Play")
        };
        if ui
            .add(Button::new(icon.rich(play.playing)))
            .on_hover_text(tip)
            .clicked()
        {
            play.playing = !play.playing;
            play.accum = 0.0;
        }

        ui.add(
            DragValue::new(&mut play.rate)
                .speed(0.05)
                .range(0.1..=30.0)
                .suffix(" /s"),
        )
        .on_hover_text("Bands per second");
    });
}

/// `band` is 1-based; `raster_metadata.bands` is a 0-based Vec.
fn band_description(rh: &RasterHandler, band: usize) -> String {
    band.checked_sub(1)
        .and_then(|i| rh.raster_metadata.bands.get(i))
        .map(|b| b.description.trim())
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| format!("Band {band}"))
}
