use crate::viewers::cmap::{ColorMap, ColorMapType, DbMode};
use crate::viewers::{
    ActiveViewer, ColorRanging, CpxMode, NormModeExtent, NormModePanchro, NormModeRGB,
};
use crate::{RasterView, viewers::Viewer};
use egui::Ui;
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

impl RasterView {
    pub(crate) fn ui_palette_panel(&mut self, ui: &mut Ui) {
        let Some(view) = &mut self.viewer else {
            return;
        };
        let old_view = view.view_mode.clone();
        view.load_minmax();

        ui.add_space(6.0);
        ui.heading("View settings");
        ui.add_space(10.0);

        // VIEWMODE
        ui.columns(2, |cols| {
            let panchro_selected = view.view_mode.active_viewer == ActiveViewer::Panchro;
            let color_selected = view.view_mode.active_viewer == ActiveViewer::Color;

            if big_mode_button(&mut cols[0], "Panchromatic", panchro_selected) {
                view.view_mode.active_viewer = ActiveViewer::Panchro;
            } // drop, square
            if big_mode_button(&mut cols[1], "RGB", color_selected) {
                view.view_mode.active_viewer = ActiveViewer::Color;
            } // equalizer, stack
        });

        ui.add_space(14.0);

        // BANDS
        section_frame(ui, "Bands", |ui| {
            let band_count = view.raster_handler.raster_count();
            match view.view_mode.active_viewer {
                ActiveViewer::Panchro => {
                    ui_band_combo(ui, "Band", &mut view.view_mode.panchro_band, band_count);
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
                                &mut view.view_mode.rgb_bands.0,
                                band_count,
                            );
                            ui.end_row();

                            ui.colored_label(egui::Color32::from_rgb(90, 200, 90), "● G");
                            ui_band_combo_bare(
                                ui,
                                "g_band",
                                &mut view.view_mode.rgb_bands.1,
                                band_count,
                            );
                            ui.end_row();

                            ui.colored_label(egui::Color32::from_rgb(90, 140, 230), "● B");
                            ui_band_combo_bare(
                                ui,
                                "b_band",
                                &mut view.view_mode.rgb_bands.2,
                                band_count,
                            );
                            ui.end_row();
                        });
                }
            }

            let reference_band = match view.view_mode.active_viewer {
                ActiveViewer::Panchro => view.view_mode.panchro_band,
                ActiveViewer::Color => view.view_mode.rgb_bands.0,
            };
            if view.raster_handler.band_is_complex(reference_band) {
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
                        ui.selectable_value(
                            &mut view.view_mode.cpx_mode,
                            mode,
                            cpx_mode_label(mode),
                        );
                    }
                });
            }
        });

        ui.add_space(14.0);

        // CMAPS
        section_frame(ui, "Colormap", |ui| {
            ui_cmap_combo(ui, &mut view.view_mode.color_interpretation.colormap);
            ui.add_space(8.0);

            // Full-width preview, much bigger than the tiny row swatch
            let full_width = ui.available_width();
            let (OrderedFloat(left_bound), OrderedFloat(right_bound)) =
                view.view_mode.color_interpretation.ranging_values;
            cmap_preview_swatch(
                ui,
                &view.view_mode.color_interpretation.colormap,
                egui::vec2(full_width, 22.0),
                view.view_mode.color_interpretation.invert_cmap,
                Some((left_bound, right_bound)),
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.checkbox(
                    &mut view.view_mode.color_interpretation.invert_cmap,
                    "Invert",
                );
                ui.add_space(16.0);
                egui::ComboBox::from_label("dB scale")
                    .selected_text(match view.view_mode.color_interpretation.db_mode {
                        DbMode::None => "None",
                        DbMode::Intensity => "Intensity (10·log10)",
                        DbMode::Amplitude => "Amplitude (20·log10)",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut view.view_mode.color_interpretation.db_mode,
                            DbMode::None,
                            "None",
                        )
                        .on_hover_text("raw");
                        ui.selectable_value(
                            &mut view.view_mode.color_interpretation.db_mode,
                            DbMode::Intensity,
                            "Intensity",
                        )
                        .on_hover_text("10·log10");
                        ui.selectable_value(
                            &mut view.view_mode.color_interpretation.db_mode,
                            DbMode::Amplitude,
                            "Amplitude  ",
                        )
                        .on_hover_text("20·log10");
                    });
            });
        });

        ui.add_space(14.0);

        // NORMALIZATION
        section_frame(ui, "Value range", |ui| {
            egui::ComboBox::from_id_salt("ranging_mode")
                .width(ui.available_width())
                .selected_text(ranging_mode_label(&view.view_mode.ranging_mode))
                .show_ui(ui, |ui| {
                    for mode in [
                        ColorRanging::MinMax,
                        ColorRanging::Percentile,
                        ColorRanging::Manual,
                    ] {
                        ui.selectable_value(
                            &mut view.view_mode.ranging_mode,
                            mode.clone(),
                            ranging_mode_label(&mode),
                        );
                    }
                });

            ui.add_space(10.0);

            match &view.view_mode.ranging_mode {
                ColorRanging::MinMax => {
                    let (OrderedFloat(min), OrderedFloat(max)) =
                        view.view_mode.color_interpretation.ranging_values;
                    ui.horizontal(|ui| {
                        stat_pill(ui, "min", min);
                        stat_pill(ui, "max", max);
                    });
                }
                ColorRanging::Percentile => {
                    let mut clip = view
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
                        view.view_mode.color_interpretation.percentile_clip = OrderedFloat(clip);
                    }
                }
                ColorRanging::Manual => {
                    let (min_of, max_of) = view.view_mode.color_interpretation.ranging_values;
                    let mut min = min_of.into_inner();
                    let mut max = max_of.into_inner();

                    ui.add(egui::Slider::new(&mut min, -1000.0..=max).text("min"));
                    ui.add(egui::Slider::new(&mut max, min..=1000.0).text("max"));

                    view.view_mode.color_interpretation.ranging_values =
                        (OrderedFloat(min), OrderedFloat(max));
                }
            }

            if matches!(
                view.view_mode.active_viewer,
                ActiveViewer::Color | ActiveViewer::Panchro
            ) {
                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Normalize across").small().weak());

                match view.view_mode.active_viewer {
                    ActiveViewer::Color => {
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut view.view_mode.norm_mode_rgb,
                                NormModeRGB::PerBand,
                                "Per band",
                            );
                            ui.selectable_value(
                                &mut view.view_mode.norm_mode_rgb,
                                NormModeRGB::RGBBands,
                                "RGB bands",
                            );
                            ui.selectable_value(
                                &mut view.view_mode.norm_mode_rgb,
                                NormModeRGB::AllBands,
                                "All bands",
                            );
                        });
                    }
                    ActiveViewer::Panchro => {
                        ui.horizontal(|ui| {
                            ui.selectable_value(
                                &mut view.view_mode.norm_mode_panchro,
                                NormModePanchro::PerBand,
                                "Per band",
                            );
                            ui.selectable_value(
                                &mut view.view_mode.norm_mode_panchro,
                                NormModePanchro::AllBands,
                                "All bands",
                            );
                        });
                    }
                    _ => {}
                }
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Normalize over").small().weak());
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut view.view_mode.norm_mode_extent,
                        NormModeExtent::Raster,
                        "Full raster",
                    );
                    ui.selectable_value(
                        &mut view.view_mode.norm_mode_extent,
                        NormModeExtent::CurrentView,
                        "Current view",
                    );
                });
            }
        });

        if old_view != view.view_mode {
            view.update_view();
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
    fn update_view(&mut self) {
        self.raster_handler
            .refresh_cache(self.parameters.cache_size);
    }

    fn load_minmax(&mut self) {
        let minmax = self.raster_handler.band_minmax(self.view_mode.panchro_band);
        if let Some((min, max)) = minmax {
            let new_range = (min as f32, max as f32);
            self.view_mode
                .color_interpretation
                .with_ranging_values(new_range);
        }
    }
}
