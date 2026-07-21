use crate::viewers::ViewMode;
use crate::{
    RasterView,
    viewers::{Viewer, ViewerParams},
};
use egui::{Label, Layout, Ui};

impl RasterView {
    pub(crate) fn ui_palette_panel(&mut self, ui: &mut Ui) {
        // ui.with_layout(
        //     Layout::centered_and_justified(egui::Direction::LeftToRight),
        //     |ui| ui.add(Label::new("This is empty! For now...").wrap()),
        // );
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
        ui.label(format!("panchro band: {}", view.view_mode.panchro_band));

        if old_view != view.view_mode {
            view.update_view();
        }
    }
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
                println!("\t\tREFRESHHHHHHH");
                self.view_mode
                    .color_interpretation
                    .with_ranging_values(new_range);
                self.update_view();
            }
        }
    }
}
