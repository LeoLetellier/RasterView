use crate::RasterView;
use egui::Ui;

impl RasterView {
    pub(crate) fn ui_top_panel(&mut self, ui: &mut Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            let button_file_name = if let Some(path) = &self.raster_path {
                format!(
                    "File: {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                )
            } else {
                "File".to_string()
            };
            if ui
                .button(button_file_name)
                .on_hover_text("Select a raster file")
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new().pick_file() {
                    let _ = self.update_path(path.as_path(), ui.ctx().clone());
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                egui::widgets::global_theme_preference_switch(ui);
                if ui
                    .button("Refresh")
                    .on_hover_text("Refresh cache and metadata")
                    .clicked()
                {
                    if let Some(view) = &mut self.viewer {
                        let _ = view.refresh_cache();
                        if cfg!(debug_assertions) {
                            let minmax = view.raster_handler.band_minmax(1);
                            let actual_state = view.raster_handler.bands_stats.clone();
                            println!(
                                "\n>>>>>>    Got minmax: {:?} with status {:?}    <<<<<<\n",
                                minmax, actual_state
                            );
                        }
                    }
                }
            });
        });
    }
}
