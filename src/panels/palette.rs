use crate::raster::RasterHandler;
use crate::viewers::Viewer;
use crate::viewers::cmap::{ColorMap, ColorMapType};
use crate::viewers::{ActiveViewer, ColorRanging, CpxMode};
use eframe::egui::{self, Ui};
use ordered_float::OrderedFloat;
use std::collections::HashMap;

const COMMON_CMAPS: &[&str] = &[
    "matplotlib/gray",
    "matplotlib/viridis",
    "cmocean/thermal",
    "scm/batlow",
    "scm/vik",
    "scm/romaO",
];

pub(crate) fn ui_palette_panel(
    ui: &mut Ui,
    viewer: &mut Viewer,
    raster_handler: &mut RasterHandler,
) {
    let old_ranging_mode = viewer.view_mode.ranging_mode.clone();

    ui.add_space(6.0);
    ui.heading("View settings");
    ui.add_space(10.0);

    // VIEWMODE
    ui.columns(2, |cols| {
        let panchro_selected = viewer.view_mode.active_viewer == ActiveViewer::Band;
        let color_selected = viewer.view_mode.active_viewer == ActiveViewer::Color;

        if big_mode_button(&mut cols[0], "Band", panchro_selected) {
            viewer.view_mode.active_viewer = ActiveViewer::Band;
        }
        if big_mode_button(&mut cols[1], "RGB", color_selected) {
            viewer.view_mode.active_viewer = ActiveViewer::Color;
        }
    });

    ui.add_space(14.0);

    // BANDS
    section_frame(ui, "Bands", |ui| {
        let band_count = raster_handler.raster_count();
        match viewer.view_mode.active_viewer {
            ActiveViewer::Band => {
                ui_band_combo(ui, "Band", &mut viewer.view_mode.panchro_band, band_count);
            }
            ActiveViewer::Color => {
                egui::Grid::new("rgb_band_grid")
                    .num_columns(2)
                    .spacing([12.0, 8.0])
                    .show(ui, |ui| {
                        ui.colored_label(egui::Color32::from_rgb(230, 90, 90), "● R");
                        ui_band_combo_bare(
                            ui,
                            "r_band",
                            &mut viewer.view_mode.rgb_bands.0,
                            band_count,
                        );
                        ui.end_row();

                        ui.colored_label(egui::Color32::from_rgb(90, 200, 90), "● G");
                        ui_band_combo_bare(
                            ui,
                            "g_band",
                            &mut viewer.view_mode.rgb_bands.1,
                            band_count,
                        );
                        ui.end_row();

                        ui.colored_label(egui::Color32::from_rgb(90, 140, 230), "● B");
                        ui_band_combo_bare(
                            ui,
                            "b_band",
                            &mut viewer.view_mode.rgb_bands.2,
                            band_count,
                        );
                        ui.end_row();
                    });
            }
        }

        if raster_handler.band_is_complex(viewer.reference_band()) {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Complex component").small().weak());
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                for mode in [
                    CpxMode::Combined,
                    CpxMode::Amplitude,
                    CpxMode::WrappedPhase,
                    CpxMode::Real,
                    CpxMode::Imaginary,
                ] {
                    ui.selectable_value(&mut viewer.view_mode.cpx_mode, mode, cpx_mode_label(mode));
                }
            });
        }
    });

    ui.add_space(14.0);

    // Make sure the stats needed by the current view are requested now,
    // instead of waiting for the sequential background preload to reach them.
    // `ensure_stats_loaded` is a no-op unless the band is `NotLoaded`.
    if viewer.view_mode.ranging_mode != ColorRanging::Manual {
        let ctx = ui.ctx().clone();
        match viewer.view_mode.active_viewer {
            ActiveViewer::Band => {
                raster_handler.ensure_stats_loaded(&ctx, viewer.view_mode.panchro_band);
            }
            ActiveViewer::Color => {
                let (r, g, b) = viewer.view_mode.rgb_bands;
                for band in [r, g, b] {
                    raster_handler.ensure_stats_loaded(&ctx, band);
                }
            }
        }
    }

    // Range currently on screen for the reference band (None while stats load).
    // Computed with the mode as it was at the start of the frame.
    let ref_band = viewer.reference_band();
    let mut shown_range = viewer.resolve_range(raster_handler, ref_band, &old_ranging_mode);

    // CMAPS
    if viewer.view_mode.active_viewer != ActiveViewer::Color {
        section_frame(ui, "Colormap", |ui| {
            ui_cmap_combo(ui, &mut viewer.view_mode.color_interpretation.colormap);
            ui.add_space(8.0);

            // Full-width preview, much bigger than the tiny row swatch
            let full_width = ui.available_width();
            cmap_preview_swatch(
                ui,
                &viewer.view_mode.color_interpretation.colormap,
                egui::vec2(full_width, 22.0),
                viewer.view_mode.color_interpretation.invert_cmap,
                shown_range,
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut viewer.view_mode.color_interpretation.invert_cmap,
                    "Invert",
                );
                ui.add_space(16.0);
                ui.checkbox(
                    &mut viewer.view_mode.color_interpretation.db_mode,
                    "dB scaling",
                );
            });
        });
    }

    ui.add_space(14.0);

    // VALUE RANGE
    section_frame(ui, "Value range", |ui| {
        egui::ComboBox::from_id_salt("ranging_mode")
            .width(ui.available_width())
            .selected_text(ranging_mode_label(&viewer.view_mode.ranging_mode))
            .show_ui(ui, |ui| {
                for mode in [
                    ColorRanging::MinMax,
                    ColorRanging::Percentile,
                    ColorRanging::Manual,
                ] {
                    ui.selectable_value(
                        &mut viewer.view_mode.ranging_mode,
                        mode.clone(),
                        ranging_mode_label(&mode),
                    );
                }
            });

        if viewer.view_mode.ranging_mode != old_ranging_mode {
            // Entering Manual: seed it with what was on screen, not a stale value.
            if viewer.view_mode.ranging_mode == ColorRanging::Manual {
                if let Some((lo, hi)) = shown_range {
                    viewer.view_mode.manual_range = (OrderedFloat(lo), OrderedFloat(hi));
                }
            }
            // Re-resolve for the new mode so the widgets below don't lag a frame.
            shown_range =
                viewer.resolve_range(raster_handler, ref_band, &viewer.view_mode.ranging_mode);
        }

        ui.add_space(10.0);

        match viewer.view_mode.ranging_mode.clone() {
            ColorRanging::MinMax => {
                ui.horizontal(|ui| match shown_range {
                    Some((min, max)) => {
                        stat_pill(ui, "min", min);
                        stat_pill(ui, "max", max);
                    }
                    None => stats_pending_ui(ui, raster_handler, ref_band),
                });
            }
            ColorRanging::Percentile => {
                let mut clip = viewer
                    .view_mode
                    .color_interpretation
                    .percentile_clip
                    .into_inner();
                if ui
                    .add(
                        egui::Slider::new(&mut clip, 0.0..=20.0)
                            .text("Clip")
                            .suffix("%"),
                    )
                    .changed()
                {
                    viewer.view_mode.color_interpretation.percentile_clip = OrderedFloat(clip);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| match shown_range {
                    Some((lo, hi)) => {
                        stat_pill(ui, "low", lo);
                        stat_pill(ui, "high", hi);
                    }
                    None => stats_pending_ui(ui, raster_handler, ref_band),
                });
            }
            ColorRanging::Manual => {
                let (min_of, max_of) = viewer.view_mode.manual_range;
                let mut min = min_of.into_inner();
                let mut max = max_of.into_inner();

                ui.horizontal(|ui| {
                    ui.label("min");
                    let r_min = ui.add(egui::DragValue::new(&mut min).speed(1.0));

                    ui.label("max");
                    let r_max = ui.add(egui::DragValue::new(&mut max).speed(1.0));

                    // Only ordering is enforced — no numeric bounds at all.
                    if r_min.changed() && min > max {
                        max = min;
                    }
                    if r_max.changed() && max < min {
                        min = max;
                    }
                });

                viewer.view_mode.manual_range = (OrderedFloat(min), OrderedFloat(max));
            }
        }
    });

    // Refresh the tile cache whenever the *resolved* task changes: a UI change,
    // a manual drag, or a stats job finishing (None -> Some).
    let task = viewer.task_view(raster_handler);
    if task != viewer.state.last_view_task {
        viewer.state.last_view_task = task;
        viewer.update_view(raster_handler);
    }
}

/// Spinner while stats load, or the error if the stats job failed.
fn stats_pending_ui(ui: &mut Ui, raster_handler: &RasterHandler, band: usize) {
    let error = band
        .checked_sub(1)
        .and_then(|idx| raster_handler.bands_stats.get(idx))
        .and_then(|status| status.error());
    match error {
        Some(err) => {
            ui.colored_label(egui::Color32::LIGHT_RED, format!("stats failed: {err}"));
        }
        None => {
            ui.spinner();
            ui.weak("computing stats…");
        }
    }
}

/// A section wrapped in a titled, padded frame — gives each group of
/// controls visual weight and separates it from its neighbors.
fn section_frame(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    ui.label(egui::RichText::new(title).strong().size(13.0));
    ui.add_space(6.0);
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::same(12))
        .corner_radius(6.0)
        .fill(ui.visuals().faint_bg_color)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add_contents(ui);
        });
}

/// Large selectable tile used for the Panchro/RGB top-level switch.
fn big_mode_button(ui: &mut Ui, text: &str, selected: bool) -> bool {
    ui.add_space(4.0);
    let button = egui::Button::new(egui::RichText::new(text).size(15.0))
        .min_size(egui::vec2(ui.available_width() - 6.0, 40.0))
        .fill(if selected {
            ui.visuals().selection.bg_fill
        } else {
            ui.visuals().widgets.inactive.bg_fill
        })
        .corner_radius(8.0);
    ui.add(button).clicked()
}

/// Small labeled stat display, e.g. "min  -12.400"
fn stat_pill(ui: &mut Ui, label: &str, value: f32) {
    egui::Frame::group(ui.style())
        .inner_margin(egui::Margin::symmetric(10, 4))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.label(format!("{label}  {value:.3}"));
        });
}

fn ui_band_combo(ui: &mut Ui, label: &str, band: &mut usize, count: usize) {
    egui::ComboBox::from_label(label)
        .width(ui.available_width() - 60.0)
        .selected_text(format!("{band}"))
        .show_ui(ui, |ui| {
            for b in 1..=count {
                ui.selectable_value(band, b, format!("{b}"));
            }
        });
}

/// Same as `ui_band_combo` but without an attached egui::Label (used inside
/// a Grid where the label is drawn separately as a colored dot).
fn ui_band_combo_bare(ui: &mut Ui, id: &str, band: &mut usize, count: usize) {
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .selected_text(format!("{band}"))
        .show_ui(ui, |ui| {
            for b in 1..=count {
                ui.selectable_value(band, b, format!("{b}"));
            }
        });
}

fn cpx_mode_label(mode: CpxMode) -> &'static str {
    match mode {
        CpxMode::Combined => "Combined",
        CpxMode::Amplitude => "Amplitude",
        CpxMode::WrappedPhase => "Wrapped phase",
        CpxMode::Real => "Real",
        CpxMode::Imaginary => "Imaginary",
    }
}

fn ranging_mode_label(mode: &ColorRanging) -> &'static str {
    match mode {
        ColorRanging::MinMax => "Min / Max",
        ColorRanging::Percentile => "Percentile clip",
        ColorRanging::Manual => "Manual",
    }
}

/// Renders the colormap selector combo box. Mutates `colormap` in place on selection.
pub(crate) fn ui_cmap_combo(ui: &mut Ui, colormap: &mut ColorMap) {
    egui::ComboBox::from_label("Cmap:")
        .selected_text(colormap.name())
        .height(250.0)
        .show_ui(ui, |ui| {
            for &name in COMMON_CMAPS {
                select_cmap_entry(ui, colormap, name, name);
            }

            ui.separator();

            // provider -> type -> [names]
            let mut by_provider: std::collections::BTreeMap<
                &str,
                HashMap<&'static ColorMapType, Vec<&str>>,
            > = Default::default();

            for (full_name, cmap_type) in ColorMap::names_with_type() {
                let provider = full_name.split('/').next().unwrap_or(full_name);
                by_provider
                    .entry(provider)
                    .or_default()
                    .entry(cmap_type)
                    .or_default()
                    .push(full_name);
            }

            for (provider, by_type) in by_provider {
                ui.menu_button(provider, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(300.0)
                        .show(ui, |ui| {
                            for cmap_type in [
                                ColorMapType::Sequential,
                                ColorMapType::Divergent,
                                ColorMapType::Cyclic,
                                ColorMapType::Other,
                            ] {
                                let Some(mut cmaps) = by_type.get(&cmap_type).cloned() else {
                                    continue;
                                };
                                cmaps.sort_unstable();

                                ui.label(egui::RichText::new(cmap_type.label()).small().weak());
                                for full_name in cmaps {
                                    let short_name =
                                        full_name.split_once('/').map_or(full_name, |(_, s)| s);
                                    select_cmap_entry(ui, colormap, full_name, short_name);
                                }
                                ui.separator();
                            }
                        });
                });
            }
        });
}

/// Draws one selectable row, applying the colormap change and closing the popup on click.
fn select_cmap_entry(ui: &mut Ui, colormap: &mut ColorMap, full_name: &str, display_name: &str) {
    let Some(entry_cmap) = ColorMap::from_name(full_name) else {
        return; // shouldn't happen if full_name came from ColorMap::names()
    };
    let is_selected = colormap.name() == full_name;

    ui.horizontal(|ui| {
        cmap_preview_swatch(ui, &entry_cmap, egui::vec2(40.0, 14.0), false, None);
        if ui.selectable_label(is_selected, display_name).clicked() {
            if !is_selected {
                *colormap = entry_cmap;
            }
            ui.close();
        }
    });
}

/// Display the cmap gradient in a small rectangle for preview
fn cmap_preview_swatch(
    ui: &mut egui::Ui,
    cmap: &ColorMap,
    size: egui::Vec2,
    invert: bool,
    bounds: Option<(f32, f32)>,
) {
    let font_id = egui::TextStyle::Small.resolve(ui.style());
    let text_height = if bounds.is_some() {
        ui.text_style_height(&egui::TextStyle::Small)
    } else {
        0.0
    };

    // Reserve the swatch plus an optional strip below it for bound labels.
    let total_size = egui::vec2(size.x, size.y + text_height);
    let (outer_rect, _response) = ui.allocate_exact_size(total_size, egui::Sense::hover());
    if !ui.is_rect_visible(outer_rect) {
        return;
    }

    let rect = egui::Rect::from_min_size(outer_rect.min, size);
    let samples = cmap.preview_samples(16);
    let painter = ui.painter();
    let seg_w = rect.width() / samples.len() as f32;

    // Collect first so both branches share one concrete type.
    let ordered: Vec<&egui::Color32> = if invert {
        samples.iter().rev().collect()
    } else {
        samples.iter().collect()
    };

    for (i, color) in ordered.into_iter().enumerate() {
        let x0 = rect.left() + seg_w * i as f32;
        let seg_rect = egui::Rect::from_min_size(
            egui::pos2(x0, rect.top()),
            egui::vec2(seg_w + 0.5, rect.height()),
        );
        painter.rect_filled(seg_rect, 0.0, *color);
    }

    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.0, ui.visuals().weak_text_color()),
        egui::StrokeKind::Inside,
    );

    if let Some((lo, hi)) = bounds {
        let text_color = ui.visuals().weak_text_color();
        painter.text(
            egui::pos2(rect.left(), rect.bottom()),
            egui::Align2::LEFT_TOP,
            format!("{lo:.2}"),
            font_id.clone(),
            text_color,
        );
        painter.text(
            egui::pos2(rect.right(), rect.bottom()),
            egui::Align2::RIGHT_TOP,
            format!("{hi:.2}"),
            font_id,
            text_color,
        );
    }
}

impl Viewer {
    pub(crate) fn update_view(&mut self, raster_handler: &mut RasterHandler) {
        raster_handler.refresh_cache(self.parameters.cache_size);
    }
}
