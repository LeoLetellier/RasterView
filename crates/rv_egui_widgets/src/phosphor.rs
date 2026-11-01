//! Phosphor icon helpers: font families, glyph selection, and composing icons
//! with text (RichText / LayoutJob).
//!
//! Usage:
//! ```ignore
//! ui.label(phosphor::text(icon::GEAR).size(18.0));
//! ui.label(IconText::new().icon(icon::GEAR).gap(6.0).text("Settings").build(ui));
//! ui.button(IconText::new().icon(icon::PLUS).text("Add").build(ui));
//! ```

use egui::{
    Align, Color32, FontFamily, FontId, Response, RichText, TextFormat, TextStyle, Ui, Widget,
    text::LayoutJob,
};

/// Font family names under which the Phosphor fonts are registered.
pub const REGULAR_FAMILY: &str = "phosphor";
pub const FILL_FAMILY: &str = "phosphor-fill";

pub fn regular_family() -> FontFamily {
    FontFamily::Name(REGULAR_FAMILY.into())
}

pub fn fill_family() -> FontFamily {
    FontFamily::Name(FILL_FAMILY.into())
}

/// Glyph weight
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Weight {
    #[default]
    Regular,
    Fill,
}

impl Weight {
    pub fn family(self) -> FontFamily {
        match self {
            Weight::Regular => regular_family(),
            Weight::Fill => fill_family(),
        }
    }
}

/// A Phosphor icon: a regular glyph with an optional fill glyph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Icon {
    pub regular: &'static str,
    pub fill: Option<&'static str>,
}

impl From<&'static str> for Icon {
    fn from(regular: &'static str) -> Self {
        Self::new(regular)
    }
}

impl Icon {
    pub const fn new(regular: &'static str) -> Self {
        Self {
            regular,
            fill: None,
        }
    }

    /// Glyph (e.g. `icon::fill::GEAR`) used when `selected` / filled.
    pub const fn with_fill(mut self, fill: &'static str) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Glyph and family to render. Falls back to the regular glyph when no fill glyph is set.
    pub fn resolve(&self, filled: bool) -> (&'static str, FontFamily) {
        match (filled, self.fill) {
            (true, Some(fill)) => (fill, fill_family()),
            _ => (self.regular, regular_family()),
        }
    }

    pub fn font_id(&self, size: f32, filled: bool) -> FontId {
        FontId::new(size, self.resolve(filled).1)
    }

    /// Icon as `RichText`, for labels, buttons, menus, etc.
    pub fn rich(&self, filled: bool) -> RichText {
        let (glyph, family) = self.resolve(filled);
        RichText::new(glyph).family(family)
    }
}

/// Regular glyph as `RichText`: `ui.label(phosphor::text(icon::GEAR).size(18.0))`
pub fn text(glyph: &'static str) -> RichText {
    RichText::new(glyph).family(regular_family())
}

/// Fill glyph as `RichText`
pub fn fill_text(glyph: &'static str) -> RichText {
    RichText::new(glyph).family(fill_family())
}

enum Part {
    Glyph(&'static str, FontFamily),
    Text(String),
    Gap(f32),
}

/// Compose icons and plain text into a single `LayoutJob`
/// (usable in `Label`, `Button`, `CollapsingHeader`, ... via `Into<WidgetText>`).
#[derive(Default)]
pub struct IconText {
    parts: Vec<Part>,
    color: Option<Color32>,
    size: Option<f32>,
}

impl IconText {
    pub fn new() -> Self {
        Self::default()
    }

    /// Regular icon
    pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
        let (g, f) = icon.into().resolve(false);
        self.parts.push(Part::Glyph(g, f));
        self
    }

    /// Filled icon (falls back to regular if the icon has no fill glyph)
    pub fn icon_filled(mut self, icon: impl Into<Icon>) -> Self {
        let (g, f) = icon.into().resolve(true);
        self.parts.push(Part::Glyph(g, f));
        self
    }

    pub fn text(mut self, s: impl Into<String>) -> Self {
        self.parts.push(Part::Text(s.into()));
        self
    }

    /// Horizontal space in points between parts
    pub fn gap(mut self, points: f32) -> Self {
        self.parts.push(Part::Gap(points));
        self
    }

    /// Override color; default follows the widget's text color
    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }

    /// Override size; default is the body text style size
    pub fn size(mut self, s: f32) -> Self {
        self.size = Some(s);
        self
    }

    pub fn build(self, ui: &Ui) -> LayoutJob {
        let body = TextStyle::Body.resolve(ui.style());
        let size = self.size.unwrap_or(body.size);
        let color = self.color.unwrap_or(Color32::PLACEHOLDER);

        let mut job = LayoutJob::default();
        let mut leading = 0.0;
        for part in self.parts {
            match part {
                Part::Gap(w) => leading += w,
                Part::Glyph(glyph, family) => {
                    job.append(
                        glyph,
                        leading,
                        TextFormat {
                            font_id: FontId::new(size, family),
                            color,
                            valign: Align::Center,
                            ..Default::default()
                        },
                    );
                    leading = 0.0;
                }
                Part::Text(s) => {
                    job.append(
                        &s,
                        leading,
                        TextFormat {
                            font_id: FontId::new(size, body.family.clone()),
                            color,
                            valign: Align::Center,
                            ..Default::default()
                        },
                    );
                    leading = 0.0;
                }
            }
        }
        job
    }
}

/// Lets you write `ui.add(IconText::new()...)` directly (non-selectable label).
pub struct IconLabel(pub IconText);

impl Widget for IconLabel {
    fn ui(self, ui: &mut Ui) -> Response {
        let job = self.0.build(ui);
        ui.label(job)
    }
}
