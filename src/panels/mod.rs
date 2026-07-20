use egui::{Color32, RichText, Ui, widget_text::WidgetText};
use egui_phosphor as icon;

use crate::RasterView;

pub(crate) mod bottom;
pub(crate) mod metadata;
pub(crate) mod palette;
pub(crate) mod parameters;
pub(crate) mod top;

pub(super) trait Panel: PartialEq {
    fn symbol(&self) -> RichText;
    fn symbol_highlight(&self) -> RichText;
}

//////////////////////// LEFT PANEL /////////////////////////////

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum LeftPanel {
    Metadata,
}

impl Panel for LeftPanel {
    fn symbol(&self) -> RichText {
        RichText::new(icon::regular::ARTICLE_MEDIUM)
    }

    fn symbol_highlight(&self) -> RichText {
        RichText::new(icon::fill::ARTICLE_MEDIUM).color(Color32::from_rgb(30, 144, 255))
    }
}

impl RasterView {
    pub(crate) fn ui_left_panel(&mut self, ui: &mut Ui) {
        match self.left_panel {
            LeftPanel::Metadata => {
                self.ui_metadata_panel(ui);
            }
        }
    }
}

//////////////////////// RIGHT PANEL /////////////////////////////

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RightPanel {
    Palette,
    Parameters,
}

impl Panel for RightPanel {
    fn symbol(&self) -> RichText {
        match &self {
            RightPanel::Palette => RichText::new(icon::regular::PAINT_BRUSH_HOUSEHOLD),
            RightPanel::Parameters => RichText::new(icon::regular::GEAR),
        }
    }

    fn symbol_highlight(&self) -> RichText {
        match &self {
            RightPanel::Palette => RichText::new(icon::fill::PAINT_BRUSH_HOUSEHOLD)
                .color(Color32::from_rgb(30, 144, 255)),
            RightPanel::Parameters => {
                RichText::new(icon::fill::GEAR).color(Color32::from_rgb(30, 144, 255))
            }
        }
    }
}

impl RasterView {
    pub(crate) fn ui_right_panel(&mut self, ui: &mut Ui) {
        match &self.right_panel {
            RightPanel::Palette => self.ui_palette_panel(ui),
            RightPanel::Parameters => self.ui_parameters_panel(ui),
        }
    }
}

//////////////////////// HELPERS /////////////////////////////

/// Create a button linked to a panel state, switching between panels or toggling the panel visibility
pub(super) fn panel_button<P: Panel>(
    is_open: &mut bool,
    current_panel: &mut P,
    panel: P,
    ui: &mut Ui,
    on_hover: impl Into<WidgetText>,
) {
    let panel_selected = *current_panel == panel;
    let highlight = panel_selected & *is_open;
    let panel_symbol = if highlight {
        panel.symbol_highlight()
    } else {
        panel.symbol()
    };

    ui.scope(|ui| {
        ui.style_mut().spacing.button_padding = egui::vec2(0.0, 0.0);

        ui.visuals_mut().widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
        ui.visuals_mut().widgets.active.bg_fill = egui::Color32::TRANSPARENT;

        let btn = ui
            .add(
                egui::Button::new(panel_symbol)
                    .frame(false)
                    .min_size(egui::Vec2::ZERO),
            )
            .on_hover_text(on_hover);
        let btn = if highlight { btn.highlight() } else { btn };
        if btn.clicked() {
            if panel_selected {
                *is_open = !*is_open;
            } else {
                *current_panel = panel;
            }
        }
    });
}
