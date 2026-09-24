use crate::{RasterView, viewers::ViewerResampling};
use egui::Ui;

impl RasterView {
    pub(crate) fn ui_parameters_panel(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.heading("Parameters");
        ui.separator();
        ui.add_space(10.0);

        if let Some(view) = &mut self.viewer {
            let _old_params = view.parameters.clone();

            ui.vertical(|ui| {
                ui.heading("Viewer");
                ui.separator();

                ui.checkbox(&mut view.parameters.show_tile_bounds, "Show tile");
                ui.horizontal(|ui| {
                    ui.label("Cache size");
                    ui.add(
                        egui::DragValue::new(&mut view.parameters.cache_size)
                            .range(64..=2096)
                            .speed(64),
                    );
                });
                egui::ComboBox::from_label("Resampling")
                    .selected_text(match view.parameters.resampling {
                        ViewerResampling::Nearest => "Nearest",
                        ViewerResampling::Bilinear => "Bilinear",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut view.parameters.resampling,
                            ViewerResampling::Nearest,
                            "Nearest",
                        );
                        ui.selectable_value(
                            &mut view.parameters.resampling,
                            ViewerResampling::Bilinear,
                            "Bilinear",
                        );
                    });
            });
        }
    }
}
