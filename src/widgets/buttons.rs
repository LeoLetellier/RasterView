use egui::{
    Align2, Color32, FontDefinitions, FontFamily, FontId, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Ui, Widget, pos2, vec2,
};

// ---------------------------------------------------------------------------
// Phosphor font handling (private to this module)
// ---------------------------------------------------------------------------

const REGULAR_FONT: &str = "phosphor-regular-font";
const FILL_FONT: &str = "phosphor-fill-font";
const REGULAR_FAMILY: &str = "phosphor-regular";
const FILL_FAMILY: &str = "phosphor-fill";

fn regular_family() -> FontFamily {
    FontFamily::Name(REGULAR_FAMILY.into())
}

fn fill_family() -> FontFamily {
    FontFamily::Name(FILL_FAMILY.into())
}

/// Call once in `setup_custom_fonts`, AFTER your text fonts are in
/// `FontDefinitions`. Replaces both `icon::add_to_fonts(...)` calls.
pub fn install_phosphor(fonts: &mut FontDefinitions) {
    fonts.font_data.insert(
        REGULAR_FONT.to_owned(),
        egui_phosphor::Variant::Regular.font_data().into(),
    );
    fonts.font_data.insert(
        FILL_FONT.to_owned(),
        egui_phosphor::Variant::Fill.font_data().into(),
    );

    // Text fonts as fallback (also silences the '◻' / '?' warning)
    let fallback = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let named = |first: &str| {
        let mut v = vec![first.to_owned()];
        v.extend(fallback.iter().cloned());
        v
    };
    fonts.families.insert(regular_family(), named(REGULAR_FONT));
    fonts.families.insert(fill_family(), named(FILL_FONT));

    // Keeps ui.button(icon::regular::X) working elsewhere.
    // Delete once everything uses IconButton.
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .push(REGULAR_FONT.to_owned());
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Side {
    Top,
    Bottom,
    #[default]
    Left,
    Right,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Indicator {
    /// Line along one edge (VS Code style)
    #[default]
    Bar,
    /// Small dot near one edge
    Dot,
    /// Outline around the whole button
    Border,
    /// Only the icon color / glyph changes
    None,
}

/// Any color left as `None` is derived from the current theme.
#[derive(Clone, Copy, Default)]
pub struct IconButtonColors {
    pub idle: Option<Color32>,
    pub hover: Option<Color32>,
    pub selected: Option<Color32>,
    pub hover_bg: Option<Color32>,
    pub indicator: Option<Color32>,
}

// ---------------------------------------------------------------------------
// Widget
// ---------------------------------------------------------------------------

pub struct IconButton {
    icon: &'static str,
    fill_icon: Option<&'static str>,
    selected: bool,
    size: Option<f32>,
    icon_size: Option<f32>,
    rounding: f32,
    indicator: Indicator,
    side: Side,
    thickness: f32,
    fade_time: f32,
    colors: IconButtonColors,
}

impl IconButton {
    /// `icon` is a Phosphor *regular* glyph, e.g. `icon::regular::GEAR`.
    pub fn new(icon: &'static str, selected: bool) -> Self {
        Self {
            icon,
            fill_icon: None,
            selected,
            size: None,
            icon_size: None,
            rounding: 6.0,
            indicator: Indicator::Bar,
            side: Side::Left,
            thickness: 2.0,
            fade_time: 0.15,
            colors: IconButtonColors::default(),
        }
    }

    /// Phosphor *fill* glyph shown while selected, e.g. `icon::fill::GEAR`.
    pub fn fill(mut self, icon: &'static str) -> Self {
        self.fill_icon = Some(icon);
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = Some(size);
        self
    }

    pub fn icon_size(mut self, size: f32) -> Self {
        self.icon_size = Some(size);
        self
    }

    pub fn rounding(mut self, r: f32) -> Self {
        self.rounding = r;
        self
    }

    pub fn indicator(mut self, i: Indicator) -> Self {
        self.indicator = i;
        self
    }

    pub fn side(mut self, s: Side) -> Self {
        self.side = s;
        self
    }

    pub fn thickness(mut self, t: f32) -> Self {
        self.thickness = t;
        self
    }

    pub fn fade_time(mut self, secs: f32) -> Self {
        self.fade_time = secs;
        self
    }

    pub fn colors(mut self, c: IconButtonColors) -> Self {
        self.colors = c;
        self
    }

    pub fn accent(mut self, c: Color32) -> Self {
        self.colors.selected = Some(c);
        self.colors.indicator = Some(c);
        self
    }
}

impl Widget for IconButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self.size.unwrap_or_else(|| ui.spacing().interact_size.y);
        let icon_size = self.icon_size.unwrap_or(size * 0.65);

        let (rect, response) = ui.allocate_exact_size(vec2(size, size), Sense::click());
        if !ui.is_rect_visible(rect) {
            return response;
        }

        // ---- animation state (all 0..1) ----
        let ctx = ui.ctx().clone();
        let id = response.id;
        let hover_t =
            ctx.animate_bool_with_time(id.with("hover"), response.hovered(), self.fade_time);
        let sel_t = ctx.animate_bool_with_time(id.with("sel"), self.selected, self.fade_time);
        let press_t = ctx.animate_bool_with_time(
            id.with("press"),
            response.is_pointer_button_down_on(),
            0.08,
        );

        // ---- colors ----
        let v = ui.visuals();
        let enabled = ui.is_enabled();
        let accent = v.hyperlink_color;

        let idle = self.colors.idle.unwrap_or_else(|| v.weak_text_color());
        let hover = self.colors.hover.unwrap_or_else(|| v.strong_text_color());
        let selected = self.colors.selected.unwrap_or(accent);
        let indicator = self.colors.indicator.unwrap_or(selected);
        let hover_bg = self
            .colors
            .hover_bg
            .unwrap_or_else(|| v.text_color().gamma_multiply(0.10));

        let base = idle.lerp_to_gamma(hover, hover_t);
        let selected_hovered = selected.lerp_to_gamma(hover, 0.3 * hover_t);
        let mut icon_color = base.lerp_to_gamma(selected_hovered, sel_t);
        if !enabled {
            icon_color = icon_color.gamma_multiply(0.4);
        }

        let painter = ui.painter();

        // ---- hover background ----
        let bg_t = hover_t * (1.0 - 0.4 * sel_t);
        if bg_t > 0.0 {
            painter.rect_filled(rect, self.rounding, hover_bg.gamma_multiply(bg_t));
        }

        // ---- selected indicator ----
        if sel_t > 0.0 {
            paint_indicator(
                painter,
                rect,
                self.indicator,
                self.side,
                indicator.gamma_multiply(sel_t),
                sel_t,
                self.thickness,
                self.rounding,
            );
        }

        // ---- icon (glyph and family chosen together) ----
        let (glyph, family) = match (self.selected, self.fill_icon) {
            (true, Some(fill)) => (fill, fill_family()),
            _ => (self.icon, regular_family()),
        };
        let icon_size = icon_size * (1.0 - 0.08 * press_t);

        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            glyph,
            FontId::new(icon_size, family),
            icon_color,
        );

        response
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_indicator(
    painter: &egui::Painter,
    rect: Rect,
    kind: Indicator,
    side: Side,
    color: Color32,
    t: f32,
    thickness: f32,
    rounding: f32,
) {
    let c = rect.center();
    match kind {
        Indicator::None => {}

        Indicator::Border => {
            painter.rect_stroke(
                rect.shrink(thickness * 0.5),
                rounding,
                Stroke::new(thickness.min(1.5), color),
                StrokeKind::Inside,
            );
        }

        Indicator::Bar => {
            let inset = 5.0;
            let k = 0.6 + 0.4 * t;
            let half_h = (rect.height() * 0.5 - inset) * k;
            let half_w = (rect.width() * 0.5 - inset) * k;
            let off = thickness * 0.5;
            let (a, b): (Pos2, Pos2) = match side {
                Side::Left => (
                    pos2(rect.left() + off, c.y - half_h),
                    pos2(rect.left() + off, c.y + half_h),
                ),
                Side::Right => (
                    pos2(rect.right() - off, c.y - half_h),
                    pos2(rect.right() - off, c.y + half_h),
                ),
                Side::Top => (
                    pos2(c.x - half_w, rect.top() + off),
                    pos2(c.x + half_w, rect.top() + off),
                ),
                Side::Bottom => (
                    pos2(c.x - half_w, rect.bottom() - off),
                    pos2(c.x + half_w, rect.bottom() - off),
                ),
            };
            painter.line_segment([a, b], Stroke::new(thickness, color));
        }

        Indicator::Dot => {
            let radius = (thickness * 1.25).max(1.5) * (0.5 + 0.5 * t);
            let margin = 4.0;
            let p = match side {
                Side::Left => pos2(rect.left() + margin, c.y),
                Side::Right => pos2(rect.right() - margin, c.y),
                Side::Top => pos2(c.x, rect.top() + margin),
                Side::Bottom => pos2(c.x, rect.bottom() - margin),
            };
            painter.circle_filled(p, radius, color);
        }
    }
}
