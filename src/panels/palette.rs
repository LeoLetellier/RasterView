use crate::RasterView;
use egui::{Label, Layout, Ui};

impl RasterView {
    pub(crate) fn ui_palette_panel(&mut self, ui: &mut Ui) {
        ui.with_layout(
            Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| ui.add(Label::new("This is empty! For now...").wrap()),
        );
    }
}
