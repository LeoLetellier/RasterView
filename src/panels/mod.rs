use crate::icon;
use egui::{Color32, Ui, widget_text::WidgetText};

use crate::RasterView;
use crate::widgets::buttons::{IconButton, Side};

pub(crate) mod bottom;
pub(crate) mod metadata;
pub(crate) mod palette;
pub(crate) mod parameters;
pub(crate) mod top;
pub(crate) mod vrt_form;

pub(super) trait Panel: PartialEq {
    fn symbol(&self) -> &'static str;
    fn symbol_highlight(&self) -> &'static str;
}

//////////////////////// LEFT PANEL /////////////////////////////

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum LeftPanel {
    Metadata,
    Palette,
}

impl Panel for LeftPanel {
    fn symbol(&self) -> &'static str {
        match &self {
            LeftPanel::Metadata => icon::regular::ARTICLE_MEDIUM,
            LeftPanel::Palette => icon::regular::PAINT_BRUSH_HOUSEHOLD,
        }
    }

    fn symbol_highlight(&self) -> &'static str {
        match &self {
            LeftPanel::Metadata => icon::fill::ARTICLE_MEDIUM,
            LeftPanel::Palette => icon::fill::PAINT_BRUSH_HOUSEHOLD,
        }
    }
}

impl RasterView {
    pub(crate) fn ui_left_panel(&mut self, ui: &mut Ui) {
        match self.left_panel {
            LeftPanel::Metadata => {
                self.ui_metadata_panel(ui);
            }
            LeftPanel::Palette => {
                self.ui_palette_panel(ui);
            }
        }
    }
}

//////////////////////// RIGHT PANEL /////////////////////////////
//
// #[derive(Debug, PartialEq, Eq, Clone, Copy)]
// pub(crate) enum RightPanel {
//     Palette,
// }
//
// impl Panel for RightPanel {
//     fn symbol(&self) -> &'static str {
//         match &self {
//             RightPanel::Palette => icon::regular::PAINT_BRUSH_HOUSEHOLD,
//         }
//     }
//
//     fn symbol_highlight(&self) -> &'static str {
//         match &self {
//             RightPanel::Palette => icon::fill::PAINT_BRUSH_HOUSEHOLD,
//         }
//     }
// }
//
// impl RasterView {
//     pub(crate) fn ui_right_panel(&mut self, ui: &mut Ui) {
//         match &self.right_panel {
//             RightPanel::Palette => self.ui_palette_panel(ui),
//         }
//     }
// }

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
    let highlight = panel_selected && *is_open;

    let response = ui
        .add(
            IconButton::new(panel.symbol(), highlight)
                .fill(panel.symbol_highlight())
                .side(Side::Bottom)
                .accent(Color32::from_rgb(30, 144, 255))
                .size(24.0),
        )
        .on_hover_text(on_hover);

    if response.clicked() {
        if panel_selected {
            *is_open = !*is_open;
        } else {
            *current_panel = panel;
        }
    }
}
