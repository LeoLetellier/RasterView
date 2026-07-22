use crate::viewers::ViewMode;
use crate::viewers::cmap::{ColorMap, ColorMapType};
use crate::{
    RasterView,
    viewers::{Viewer, ViewerParams},
};
use egui::{Label, Layout, Ui};
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

        egui::ComboBox::from_label("Band:")
            .selected_text(format!("{}", view.view_mode.panchro_band))
            .show_ui(ui, |ui| {
                for b in 1..=view.raster_handler.raster_count() {
                    ui.selectable_value(&mut view.view_mode.panchro_band, b, format!("{}", b));
                }
            });

        ui_cmap_combo(ui, &mut view.view_mode.color_interpretation.colormap);

        if old_view != view.view_mode {
            view.update_view();
        }
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
        cmap_preview_swatch(ui, &entry_cmap, egui::vec2(40.0, 14.0));
        if ui.selectable_label(is_selected, display_name).clicked() {
            if !is_selected {
                *colormap = entry_cmap;
            }
            ui.close();
        }
    });
}

/// Display the cmap gradient in a small rectangle for preview
fn cmap_preview_swatch(ui: &mut egui::Ui, cmap: &ColorMap, size: egui::Vec2) {
    let (rect, _response) = ui.allocate_exact_size(size, egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let samples = cmap.preview_samples(16);
    let painter = ui.painter();
    let seg_w = rect.width() / samples.len() as f32;
    for (i, color) in samples.iter().enumerate() {
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
}

impl Viewer {
    fn update_view(&mut self) {
        self.raster_handler
            .refresh_cache(self.parameters.cache_size);
    }

    fn load_minmax(&mut self) {
        let minmax = self.raster_handler.band_minmax(self.view_mode.panchro_band);
        let prev_range = self.view_mode.color_interpretation.ranging_values;
        if let Some((min, max)) = minmax {
            let new_range = (min as f32, max as f32);
            if new_range != prev_range {
                self.view_mode
                    .color_interpretation
                    .with_ranging_values(new_range);
                self.update_view();
            }
        }
    }
}
