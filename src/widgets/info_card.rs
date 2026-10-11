use std::sync::Arc;

use egui::{
    Color32, CornerRadius, CursorIcon, FontId, Galley, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, TextStyle, Ui, Vec2, Widget, epaint::text::TextWrapping, text::LayoutJob,
};

/// Card to display main information
pub struct InfoCard {
    title: String,
    value: String,
    unit: Option<String>,
    width: Option<f32>,
    min_width: f32,
    padding: Vec2,
    line_gap: f32,
    corner_radius: u8,
    stroke_width: f32,
    accent: AccentSide,
    accent_thickness: f32,
    align: CardAlign,
    title_size: Option<f32>,
    value_size: Option<f32>,
    monospace_value: bool,
    clickable: bool,
    fade_time: f32,
    tooltip: Option<String>,
    colors: InfoCardColors,
}

/// Show indicator for the card
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AccentSide {
    None,
    Left,
    Top,
    Bottom,
}

/// Alignment of the card
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardAlign {
    Left,
    Center,
    Right,
}

/// Color scheme to use depending on the card state, overriding the global theme
#[derive(Clone, Copy, Default)]
pub struct InfoCardColors {
    pub fill: Option<Color32>,
    pub hover_fill: Option<Color32>,
    pub stroke: Option<Color32>,
    pub title: Option<Color32>,
    pub value: Option<Color32>,
    pub unit: Option<Color32>,
    pub accent: Option<Color32>,
}

impl InfoCard {
    pub fn new(title: impl Into<String>, value: impl ToString) -> Self {
        Self {
            title: title.into(),
            value: value.to_string(),
            unit: None,
            width: None,
            min_width: 0.0,
            padding: Vec2::new(10.0, 6.0),
            line_gap: 2.0,
            corner_radius: 6,
            stroke_width: 1.0,
            accent: AccentSide::None,
            accent_thickness: 3.0,
            align: CardAlign::Center,
            title_size: None,
            value_size: None,
            monospace_value: false,
            clickable: false,
            fade_time: 0.12,
            tooltip: None,
            colors: InfoCardColors::default(),
        }
    }

    pub fn unit(mut self, unit: impl Into<String>) -> Self {
        self.unit = Some(unit.into());
        self
    }

    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
    pub fn min_width(mut self, w: f32) -> Self {
        self.min_width = w;
        self
    }
    pub fn padding(mut self, p: impl Into<Vec2>) -> Self {
        self.padding = p.into();
        self
    }
    pub fn line_gap(mut self, g: f32) -> Self {
        self.line_gap = g;
        self
    }
    pub fn corner_radius(mut self, r: u8) -> Self {
        self.corner_radius = r;
        self
    }
    pub fn stroke_width(mut self, w: f32) -> Self {
        self.stroke_width = w;
        self
    }
    pub fn accent(mut self, side: AccentSide) -> Self {
        self.accent = side;
        self
    }
    pub fn accent_thickness(mut self, t: f32) -> Self {
        self.accent_thickness = t;
        self
    }
    pub fn align(mut self, a: CardAlign) -> Self {
        self.align = a;
        self
    }
    pub fn title_size(mut self, s: f32) -> Self {
        self.title_size = Some(s);
        self
    }
    pub fn value_size(mut self, s: f32) -> Self {
        self.value_size = Some(s);
        self
    }
    pub fn monospace_value(mut self, m: bool) -> Self {
        self.monospace_value = m;
        self
    }
    pub fn clickable(mut self, c: bool) -> Self {
        self.clickable = c;
        self
    }
    pub fn fade_time(mut self, t: f32) -> Self {
        self.fade_time = t;
        self
    }
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.tooltip = Some(t.into());
        self
    }
    pub fn colors(mut self, c: InfoCardColors) -> Self {
        self.colors = c;
        self
    }

    // Shortcuts for the most common color tweaks
    pub fn value_color(mut self, c: Color32) -> Self {
        self.colors.value = Some(c);
        self
    }
    pub fn accent_color(mut self, c: Color32) -> Self {
        self.colors.accent = Some(c);
        self
    }
    /// Accent bar + value in the same color (nice for status-like values).
    pub fn tint(mut self, c: Color32) -> Self {
        self.colors.value = Some(c);
        self.colors.accent = Some(c);
        if self.accent == AccentSide::None {
            self.accent = AccentSide::Left;
        }
        self
    }

    /// Extra space taken by the accent bar (added on top of the padding).
    fn accent_extra(&self) -> Vec2 {
        match self.accent {
            AccentSide::None => Vec2::ZERO,
            AccentSide::Left => Vec2::new(self.accent_thickness, 0.0),
            AccentSide::Top | AccentSide::Bottom => Vec2::new(0.0, self.accent_thickness),
        }
    }

    fn fonts(&self, ui: &Ui) -> (FontId, FontId, FontId) {
        let style = ui.style();
        let title = {
            let mut f = TextStyle::Small.resolve(style);
            if let Some(s) = self.title_size {
                f.size = s;
            }
            f
        };
        let value = {
            let mut f = if self.monospace_value {
                TextStyle::Monospace.resolve(style)
            } else {
                TextStyle::Body.resolve(style)
            };
            f.size = self.value_size.unwrap_or(f.size * 1.15);
            f
        };
        let mut unit = title.clone();
        unit.size = (value.size * 0.7).max(title.size);
        (title, value, unit)
    }

    fn galley(ui: &Ui, text: &str, font: FontId, max_w: f32) -> Arc<Galley> {
        let mut job = LayoutJob::simple_singleline(text.to_owned(), font, Color32::PLACEHOLDER);
        job.wrap = TextWrapping {
            max_width: max_w,
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        };
        ui.fonts_mut(|f| f.layout_job(job))
    }

    fn layout(&self, ui: &Ui, max_text_w: f32) -> Laid {
        let (tf, vf, uf) = self.fonts(ui);
        let title = Self::galley(ui, &self.title, tf, max_text_w);
        let value = Self::galley(ui, &self.value, vf, max_text_w);
        let unit = self
            .unit
            .as_ref()
            .map(|u| Self::galley(ui, u, uf, max_text_w));

        let unit_gap = 3.0;
        let value_row_w = value.size().x + unit.as_ref().map_or(0.0, |u| unit_gap + u.size().x);
        let size = Vec2::new(
            title.size().x.max(value_row_w),
            title.size().y + self.line_gap + value.size().y,
        );
        Laid {
            title,
            value,
            unit,
            value_row_w,
            unit_gap,
            size,
        }
    }

    /// Width this card wants when unconstrained.
    pub fn natural_width(&self, ui: &Ui) -> f32 {
        let l = self.layout(ui, f32::INFINITY);
        (l.size.x + 2.0 * self.padding.x + self.accent_extra().x).max(self.min_width)
    }
}

struct Laid {
    title: Arc<Galley>,
    value: Arc<Galley>,
    unit: Option<Arc<Galley>>,
    value_row_w: f32,
    unit_gap: f32,
    size: Vec2,
}

impl Widget for InfoCard {
    fn ui(self, ui: &mut Ui) -> Response {
        let extra = self.accent_extra();
        let max_text_w = self
            .width
            .map(|w| (w - 2.0 * self.padding.x - extra.x).max(1.0))
            .unwrap_or(f32::INFINITY);
        let laid = self.layout(ui, max_text_w);

        let width = self
            .width
            .unwrap_or((laid.size.x + 2.0 * self.padding.x + extra.x).max(self.min_width));
        let height = laid.size.y + 2.0 * self.padding.y + extra.y;

        let sense = if self.clickable {
            Sense::click()
        } else {
            Sense::hover()
        };
        let (rect, mut resp) = ui.allocate_exact_size(Vec2::new(width, height), sense);
        if let Some(t) = &self.tooltip {
            resp = resp.on_hover_text(t);
        }
        if self.clickable {
            resp = resp.on_hover_cursor(CursorIcon::PointingHand);
        }
        if !ui.is_rect_visible(rect) {
            return resp;
        }

        // ---- resolve colors (theme fallbacks) ----
        let v = ui.visuals();
        let c = &self.colors;
        let fill = c.fill.unwrap_or(v.widgets.inactive.weak_bg_fill);
        let hover_fill = c.hover_fill.unwrap_or(v.widgets.hovered.weak_bg_fill);
        let stroke_c = c.stroke.unwrap_or(v.widgets.noninteractive.bg_stroke.color);
        let title_c = c.title.unwrap_or(v.weak_text_color());
        let value_c = c.value.unwrap_or(v.strong_text_color());
        let unit_c = c.unit.unwrap_or(title_c);
        let accent_c = c.accent.unwrap_or(v.hyperlink_color);

        let t = ui.ctx().animate_bool_with_time(
            resp.id,
            self.clickable && resp.hovered(),
            self.fade_time,
        );
        let fill = fill.lerp_to_gamma(hover_fill, t);

        // ---- paint ----
        let painter = ui.painter();
        let r = self.corner_radius;
        painter.rect_filled(rect, CornerRadius::same(r), fill);
        if self.stroke_width > 0.0 {
            painter.rect_stroke(
                rect,
                CornerRadius::same(r),
                Stroke::new(self.stroke_width, stroke_c),
                StrokeKind::Inside,
            );
        }

        let th = self.accent_thickness;
        match self.accent {
            AccentSide::None => {}
            AccentSide::Left => {
                let bar = Rect::from_min_size(rect.min, Vec2::new(th, rect.height()));
                let cr = CornerRadius {
                    nw: r,
                    sw: r,
                    ne: 0,
                    se: 0,
                };
                painter.rect_filled(bar, cr, accent_c);
            }
            AccentSide::Top => {
                let bar = Rect::from_min_size(rect.min, Vec2::new(rect.width(), th));
                let cr = CornerRadius {
                    nw: r,
                    ne: r,
                    sw: 0,
                    se: 0,
                };
                painter.rect_filled(bar, cr, accent_c);
            }
            AccentSide::Bottom => {
                let bar = Rect::from_min_max(Pos2::new(rect.min.x, rect.max.y - th), rect.max);
                let cr = CornerRadius {
                    sw: r,
                    se: r,
                    nw: 0,
                    ne: 0,
                };
                painter.rect_filled(bar, cr, accent_c);
            }
        }

        // content area = rect minus accent bar minus padding
        let mut content = rect.shrink2(self.padding);
        match self.accent {
            AccentSide::Left => content.min.x = rect.min.x + th + self.padding.x,
            AccentSide::Top => content.min.y = rect.min.y + th + self.padding.y,
            AccentSide::Bottom => content.max.y = rect.max.y - th - self.padding.y,
            AccentSide::None => {}
        }

        let x_for = |w: f32| match self.align {
            CardAlign::Left => content.min.x,
            CardAlign::Center => content.center().x - w / 2.0,
            CardAlign::Right => content.max.x - w,
        };
        let mut y = content.center().y - laid.size.y / 2.0;

        painter.galley(
            Pos2::new(x_for(laid.title.size().x), y),
            laid.title.clone(),
            title_c,
        );
        y += laid.title.size().y + self.line_gap;

        let mut x = x_for(laid.value_row_w);
        painter.galley(Pos2::new(x, y), laid.value.clone(), value_c);
        if let Some(u) = &laid.unit {
            x += laid.value.size().x + laid.unit_gap;
            let uy = y + laid.value.size().y - u.size().y; // bottom-align unit
            painter.galley(Pos2::new(x, uy), u.clone(), unit_c);
        }

        resp
    }
}

/// Lays cards out in equal-width columns that adapt to the available width.
pub struct InfoCardGrid {
    spacing: Vec2,
    max_cols: Option<usize>,
    min_card_width: f32,
    stretch: bool,
}

impl Default for InfoCardGrid {
    fn default() -> Self {
        Self {
            spacing: Vec2::splat(8.0),
            max_cols: None,
            min_card_width: 0.0,
            stretch: true,
        }
    }
}

impl InfoCardGrid {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn spacing(mut self, s: impl Into<Vec2>) -> Self {
        self.spacing = s.into();
        self
    }
    pub fn max_cols(mut self, n: usize) -> Self {
        self.max_cols = Some(n.max(1));
        self
    }
    pub fn min_card_width(mut self, w: f32) -> Self {
        self.min_card_width = w;
        self
    }
    /// Grow cards so each row fills the full width (default: true).
    pub fn stretch(mut self, s: bool) -> Self {
        self.stretch = s;
        self
    }

    pub fn show(self, ui: &mut Ui, cards: Vec<InfoCard>) -> Vec<Response> {
        let mut responses = Vec::with_capacity(cards.len());
        if cards.is_empty() {
            return responses;
        }
        let sp = self.spacing;
        let avail = ui.available_width();

        let natural = cards
            .iter()
            .map(|c| c.natural_width(ui))
            .fold(self.min_card_width, f32::max);

        let mut cols = ((avail + sp.x) / (natural + sp.x)).floor().max(1.0) as usize;
        cols = cols.min(cards.len());
        if let Some(m) = self.max_cols {
            cols = cols.min(m);
        }
        let card_w = if self.stretch {
            ((avail - sp.x * (cols as f32 - 1.0)) / cols as f32).max(natural.min(avail))
        } else {
            natural
        };

        ui.scope(|ui| {
            ui.spacing_mut().item_spacing = sp;
            let mut it = cards.into_iter().peekable();
            while it.peek().is_some() {
                ui.horizontal_top(|ui| {
                    for card in it.by_ref().take(cols) {
                        responses.push(ui.add(card.width(card_w)));
                    }
                });
            }
        });
        responses
    }
}
