use crate::RasterView;
use egui::{Label, Layout, Ui};

impl RasterView {
    pub(crate) fn ui_parameters_panel(&mut self, ui: &mut Ui) {
        ui.add_space(6.0);
        ui.heading("Parameters");
        ui.separator();
        ui.add_space(10.0);

        if let Some(view) = &mut self.viewer {
            ui.vertical(|ui| {
                ui.heading("Viewer");
                ui.separator();

                ui.checkbox(&mut view.parameters.show_tile_bounds, "Show tile");
            });
        }
    }
}
