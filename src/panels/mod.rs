use crate::{
    Viewer, icon,
    panels::{
        metadata::ui_metadata_panel,
        palette::ui_palette_panel,
        slider::{ui_band_slider, ui_point_series},
    },
    raster::RasterHandler,
};
use eframe::egui::{Color32, Ui, widget_text::WidgetText};

use crate::widgets::icon_button::{IconButton, Indicator, Side};

pub(crate) mod bottom;
pub(crate) mod metadata;
pub(crate) mod palette;
pub(crate) mod parameters;
pub(crate) mod slider;
pub(crate) mod top;
pub(crate) mod vrt_form;

#[derive(Debug, Default)]
pub(crate) struct CommandPanel {
    focus: CommandPanelType,
    pub(crate) is_open: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum CommandPanelType {
    Metadata,
    Palette,
}

impl Default for CommandPanelType {
    fn default() -> Self {
        CommandPanelType::Palette
    }
}

impl CommandPanelType {
    fn symbol(&self) -> &'static str {
        match &self {
            CommandPanelType::Metadata => icon::regular::ARTICLE_MEDIUM,
            CommandPanelType::Palette => icon::regular::PAINT_BRUSH_HOUSEHOLD,
        }
    }

    fn symbol_highlight(&self) -> &'static str {
        match &self {
            CommandPanelType::Metadata => icon::fill::ARTICLE_MEDIUM,
            CommandPanelType::Palette => icon::fill::PAINT_BRUSH_HOUSEHOLD,
        }
    }
}

impl CommandPanel {
    pub(crate) fn ui(
        &self,
        ui: &mut Ui,
        viewer: &mut Viewer,
        raster_handler: &mut Option<RasterHandler>,
    ) {
        if let Some(rh) = raster_handler {
            match self.focus {
                CommandPanelType::Palette => ui_palette_panel(ui, viewer, rh),
                CommandPanelType::Metadata => ui_metadata_panel(ui, rh),
            }
        }
    }

    pub(crate) fn panel_button(
        &mut self,
        ui: &mut Ui,
        panel: CommandPanelType,
        on_hover: impl Into<WidgetText>,
    ) {
        let panel_selected = self.focus == panel;
        let highlight = panel_selected && self.is_open;

        let response = ui
            .add(
                IconButton::new(panel.symbol(), highlight)
                    .fill(panel.symbol_highlight())
                    .side(Side::Bottom)
                    .indicator(Indicator::Border)
                    .accent(Color32::from_rgb(30, 144, 255))
                    .size(24.0),
            )
            .on_hover_text(on_hover);

        if response.clicked() {
            if panel_selected {
                self.is_open = !self.is_open;
            } else {
                self.focus = panel;
            }
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct SliderPanel {
    focus: SliderPanelType,
    pub(crate) is_open: bool,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum SliderPanelType {
    BandSlider,
    PointSeries,
}

impl Default for SliderPanelType {
    fn default() -> Self {
        SliderPanelType::BandSlider
    }
}

impl SliderPanelType {
    fn symbol(&self) -> &'static str {
        match &self {
            SliderPanelType::BandSlider => icon::regular::BARCODE,
            SliderPanelType::PointSeries => icon::regular::CHART_SCATTER,
        }
    }

    fn symbol_highlight(&self) -> &'static str {
        match &self {
            SliderPanelType::BandSlider => icon::fill::BARCODE,
            SliderPanelType::PointSeries => icon::fill::CHART_SCATTER,
        }
    }
}

impl SliderPanel {
    pub(crate) fn ui(
        &self,
        ui: &mut Ui,
        viewer: &mut Viewer,
        raster_handler: &mut Option<RasterHandler>,
    ) {
        if let Some(rh) = raster_handler {
            match self.focus {
                SliderPanelType::PointSeries => ui_point_series(ui, viewer, rh),
                SliderPanelType::BandSlider => ui_band_slider(ui, viewer, rh),
            }
        }
    }

    pub(crate) fn panel_button(
        &mut self,
        ui: &mut Ui,
        panel: SliderPanelType,
        on_hover: impl Into<WidgetText>,
    ) {
        let panel_selected = self.focus == panel;
        let highlight = panel_selected && self.is_open;

        let response = ui
            .add(
                IconButton::new(panel.symbol(), highlight)
                    .fill(panel.symbol_highlight())
                    .side(Side::Bottom)
                    .indicator(Indicator::Border)
                    .accent(Color32::from_rgb(30, 144, 255))
                    .size(24.0),
            )
            .on_hover_text(on_hover);

        if response.clicked() {
            if panel_selected {
                self.is_open = !self.is_open;
            } else {
                self.focus = panel;
            }
        }
    }
}
