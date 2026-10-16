use std::sync::Arc;

use egui::{
    Align, Color32, FontId, Galley, Response, Sense, TextFormat, TextStyle, Ui, Widget,
    text::LayoutJob, vec2,
};

/// Flow of the elements (the first element is on the named side).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    LeftToRight,
    RightToLeft,
    TopToBottom,
    BottomToTop,
}

impl Direction {
    fn horizontal(self) -> bool {
        matches!(self, Self::LeftToRight | Self::RightToLeft)
    }
    fn reversed(self) -> bool {
        matches!(self, Self::RightToLeft | Self::BottomToTop)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Lat,
    Lon,
}

/// How a value is displayed.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Repr {
    /// Number with `precision` decimals.
    Float,
    /// Rounded integer.
    Int,
    /// Plain text (numbers are printed as-is).
    Text,
    /// Decimal degrees with hemisphere letter: `48.69000°N`.
    Degrees(Axis),
    /// Degrees, minutes, seconds: `48°41'24.0"N`.
    Dms(Axis),
}

#[derive(Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Text(String),
    /// Shown as a dim placeholder of the same width.
    Missing,
}

macro_rules! value_from_num {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(v: $t) -> Self { Value::Num(v as f64) }
        }
    )*};
}
value_from_num!(f64, f32, i32, i64, u32, u64, usize);

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::Text(v.to_owned())
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Text(v)
    }
}
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(v: Option<T>) -> Self {
        v.map_or(Value::Missing, Into::into)
    }
}

/// One element of a `ValueBar`. Only the value is mandatory.
pub struct ValueItem {
    value: Value,
    label: Option<String>,
    unit: Option<String>,
    repr: Option<Repr>,
    bounds: Option<(f64, f64)>,
    precision: Option<usize>,
    chars: Option<usize>,
    color: Option<Color32>,
}

impl ValueItem {
    pub fn new(value: impl Into<Value>) -> Self {
        Self {
            value: value.into(),
            label: None,
            unit: None,
            repr: None,
            bounds: None,
            precision: None,
            chars: None,
            color: None,
        }
    }

    /// Own small name (e.g. "x", "B1").
    pub fn label(mut self, l: impl Into<String>) -> Self {
        self.label = Some(l.into());
        self
    }
    /// Small unit after the value (e.g. "m").
    pub fn unit(mut self, u: impl Into<String>) -> Self {
        self.unit = Some(u.into());
        self
    }
    pub fn repr(mut self, r: Repr) -> Self {
        self.repr = Some(r);
        self
    }
    pub fn int(self) -> Self {
        self.repr(Repr::Int)
    }
    pub fn lat(self) -> Self {
        self.repr(Repr::Degrees(Axis::Lat))
    }
    pub fn lon(self) -> Self {
        self.repr(Repr::Degrees(Axis::Lon))
    }
    /// Known (min, max) of the value: the reserved width is computed from them.
    pub fn bounds(mut self, min: f64, max: f64) -> Self {
        self.bounds = Some((min, max));
        self
    }
    /// Decimals (defaults: 3 float, 5 degrees, 1 for seconds in DMS, 0 for int).
    pub fn precision(mut self, p: usize) -> Self {
        self.precision = Some(p);
        self
    }
    /// Force the number of reserved character cells (overrides bounds).
    pub fn chars(mut self, n: usize) -> Self {
        self.chars = Some(n);
        self
    }
    /// Override the value color.
    pub fn color(mut self, c: Color32) -> Self {
        self.color = Some(c);
        self
    }

    fn repr_or_default(&self) -> Repr {
        self.repr.unwrap_or(match self.value {
            Value::Text(_) => Repr::Text,
            _ => Repr::Float,
        })
    }

    fn prec(&self) -> usize {
        match self.repr_or_default() {
            Repr::Int | Repr::Text => 0,
            Repr::Float => self.precision.unwrap_or(3),
            Repr::Degrees(_) => self.precision.unwrap_or(5),
            Repr::Dms(_) => self.precision.unwrap_or(1).min(6),
        }
    }

    /// Reserved character cells.
    fn width(&self, fallback: usize) -> usize {
        if let Some(c) = self.chars {
            return c;
        }
        let p = self.prec();
        let frac = if p > 0 { p + 1 } else { 0 };
        match self.repr_or_default() {
            Repr::Float | Repr::Int => match self.bounds {
                Some((lo, hi)) if lo.is_finite() && hi.is_finite() => {
                    usize::from(lo.min(hi) < 0.0) + int_digits(lo.abs().max(hi.abs())) + frac
                }
                _ => fallback,
            },
            Repr::Text => match &self.value {
                Value::Text(t) => t.chars().count(),
                _ => 1,
            },
            // digits + fraction + '°' + hemisphere letter
            Repr::Degrees(a) => deg_digits(a) + frac + 2,
            // dd ° mm ' ss[.f] " H
            Repr::Dms(a) => deg_digits(a) + 1 + 2 + 1 + 2 + frac + 1 + 1,
        }
    }

    /// Display text (padded to the reserved width) and whether a value is present.
    fn cell(&self, fallback: usize) -> (String, bool) {
        let (w, p) = (self.width(fallback), self.prec());
        let v = match &self.value {
            Value::Missing => return (format!("{:>w$}", "-"), false),
            Value::Text(t) => return (fit_text(t, w), true),
            Value::Num(v) => *v,
        };
        let text = match self.repr_or_default() {
            Repr::Float => fmt_num(v, w, p),
            Repr::Int => fmt_num(v.round(), w, 0),
            Repr::Text => fit_text(&v.to_string(), w),
            Repr::Degrees(a) if v.is_finite() => {
                let v = normalize(v, a);
                let nw = w.saturating_sub(2);
                format!("{:>nw$.p$}°{}", v.abs(), hemi(v, a))
            }
            Repr::Dms(a) if v.is_finite() => fmt_dms(v, a, p),
            Repr::Degrees(_) | Repr::Dms(_) => fmt_num(v, w, p),
        };
        (text, true)
    }

    /// Unrounded value as text (signed decimal degrees for lat/lon).
    fn full(&self) -> Option<String> {
        match &self.value {
            Value::Missing => None,
            Value::Text(t) => Some(t.clone()),
            Value::Num(v) => Some(match self.repr_or_default() {
                Repr::Degrees(a) | Repr::Dms(a) => normalize(*v, a).to_string(),
                _ => v.to_string(),
            }),
        }
    }
}

pub struct ValueBar {
    name: Option<String>,
    direction: Direction,
    items: Vec<ValueItem>,
    gap: Option<f32>,
    font_size: Option<f32>,
    small_scale: f32,
    fallback_chars: usize,
    value_color: Option<Color32>,
    dim_color: Option<Color32>,
    clickable: bool,
    show_tooltip: bool,
    copied_time: f64,
}

impl ValueBar {
    pub fn new() -> Self {
        Self {
            name: None,
            direction: Direction::LeftToRight,
            items: Vec::new(),
            gap: None,
            font_size: None,
            small_scale: 0.8,
            fallback_chars: 8,
            value_color: None,
            dim_color: None,
            clickable: true,
            show_tooltip: true,
            copied_time: 1.5,
        }
    }

    /// Global field name, shown on the leading side.
    pub fn name(mut self, n: impl Into<String>) -> Self {
        self.name = Some(n.into());
        self
    }
    pub fn direction(mut self, d: Direction) -> Self {
        self.direction = d;
        self
    }
    pub fn item(mut self, item: ValueItem) -> Self {
        self.items.push(item);
        self
    }
    pub fn items(mut self, items: impl IntoIterator<Item = ValueItem>) -> Self {
        self.items.extend(items);
        self
    }
    /// Shortcut for a plain element with only a value.
    pub fn value(self, v: impl Into<Value>) -> Self {
        self.item(ValueItem::new(v))
    }
    /// Space between elements in points (default: theme item spacing).
    pub fn gap(mut self, g: f32) -> Self {
        self.gap = Some(g);
        self
    }
    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = Some(s);
        self
    }
    /// Size of labels and units relative to the values.
    pub fn small_scale(mut self, s: f32) -> Self {
        self.small_scale = s;
        self
    }
    /// Character cells for floats/ints that have neither `bounds` nor `chars`.
    pub fn fallback_chars(mut self, n: usize) -> Self {
        self.fallback_chars = n;
        self
    }
    pub fn value_color(mut self, c: Color32) -> Self {
        self.value_color = Some(c);
        self
    }
    pub fn dim_color(mut self, c: Color32) -> Self {
        self.dim_color = Some(c);
        self
    }
    pub fn clickable(mut self, v: bool) -> Self {
        self.clickable = v;
        self
    }
    pub fn tooltip(mut self, v: bool) -> Self {
        self.show_tooltip = v;
        self
    }
    /// How long "Copied!" stays in the tooltip (seconds).
    pub fn copied_time(mut self, secs: f64) -> Self {
        self.copied_time = secs;
        self
    }

    /// Unrounded values, `None` if any is missing.
    fn copy_string(&self) -> Option<String> {
        let mut parts = Vec::with_capacity(self.items.len());
        for it in &self.items {
            let mut s = it.full()?;
            if let Some(u) = &it.unit {
                s.push(' ');
                s.push_str(u);
            }
            parts.push(s);
        }
        (!parts.is_empty()).then(|| parts.join(", "))
    }
}

impl Default for ValueBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ValueBar {
    fn ui(self, ui: &mut Ui) -> Response {
        // Measure
        let size = self
            .font_size
            .unwrap_or_else(|| TextStyle::Monospace.resolve(ui.style()).size);
        let big = FontId::monospace(size);
        let small = FontId::monospace(size * self.small_scale);
        let value_c = self
            .value_color
            .unwrap_or_else(|| ui.visuals().text_color());
        let dim_c = self
            .dim_color
            .unwrap_or_else(|| ui.visuals().weak_text_color());

        let fmt = |font: &FontId, color: Color32| TextFormat {
            font_id: font.clone(),
            color,
            valign: Align::BOTTOM,
            ..Default::default()
        };

        let horizontal = self.direction.horizontal();
        let gap = self.gap.unwrap_or_else(|| {
            let sp = ui.spacing().item_spacing;
            if horizontal { sp.x } else { sp.y }
        });

        let mut galleys: Vec<Arc<Galley>> = Vec::with_capacity(self.items.len() + 1);
        if let Some(n) = &self.name {
            let mut job = LayoutJob::default();
            job.append(n, 0.0, fmt(&big, dim_c));
            galleys.push(ui.painter().layout_job(job));
        }
        for it in &self.items {
            let (text, present) = it.cell(self.fallback_chars);
            let mut job = LayoutJob::default();
            if let Some(l) = &it.label {
                job.append(l, 0.0, fmt(&small, dim_c));
                job.append(" ", 0.0, fmt(&small, dim_c));
            }
            let color = if present {
                it.color.unwrap_or(value_c)
            } else {
                dim_c
            };
            job.append(&text, 0.0, fmt(&big, color));
            if let Some(u) = &it.unit {
                job.append(" ", 0.0, fmt(&small, dim_c));
                job.append(u, 0.0, fmt(&small, dim_c));
            }
            galleys.push(ui.painter().layout_job(job));
        }

        let n = galleys.len();
        let gaps = gap * n.saturating_sub(1) as f32;
        let (mut w, mut h) = (0.0_f32, 0.0_f32);
        for g in &galleys {
            let s = g.size();
            if horizontal {
                w += s.x;
                h = h.max(s.y);
            } else {
                w = w.max(s.x);
                h += s.y;
            }
        }
        let total = if horizontal {
            vec2(w + gaps, h)
        } else {
            vec2(w, h + gaps)
        };

        // Allocate
        let copy = self.copy_string();
        let sense = if self.clickable && copy.is_some() {
            Sense::click()
        } else {
            Sense::hover()
        };
        let response = ui.allocate_response(total, sense);

        // Paint
        if ui.is_rect_visible(response.rect) {
            let painter = ui.painter();
            let mut pos = response.rect.min;
            for k in 0..n {
                let g = &galleys[if self.direction.reversed() {
                    n - 1 - k
                } else {
                    k
                }];
                painter.galley(pos, g.clone(), value_c);
                if horizontal {
                    pos.x += g.size().x + gap;
                } else {
                    pos.y += g.size().y + gap;
                }
            }
        }

        // Interact
        let now = ui.input(|i| i.time);
        if response.clicked() {
            if let Some(text) = copy {
                ui.ctx().copy_text(text);
                ui.ctx().data_mut(|d| d.insert_temp(response.id, now));
            }
        }
        if !self.show_tooltip {
            return response;
        }
        let copied = ui
            .ctx()
            .data(|d| d.get_temp::<f64>(response.id))
            .is_some_and(|t| now - t < self.copied_time);

        response.on_hover_ui(|ui| {
            if copied {
                ui.label("Copied!");
                return;
            }
            if let Some(n) = &self.name {
                ui.strong(n);
            }
            for it in &self.items {
                let l = it
                    .label
                    .as_deref()
                    .map(|l| format!("{l} = "))
                    .unwrap_or_default();
                let v = it.full().unwrap_or_else(|| "-".to_owned());
                let u = it
                    .unit
                    .as_deref()
                    .map(|u| format!(" {u}"))
                    .unwrap_or_default();
                ui.monospace(format!("{l}{v}{u}"));
            }
            if self.clickable {
                ui.weak("Click to copy");
            }
        })
    }
}

fn deg_digits(a: Axis) -> usize {
    if a == Axis::Lat { 2 } else { 3 }
}

fn normalize(v: f64, a: Axis) -> f64 {
    match a {
        Axis::Lat => v.clamp(-90.0, 90.0),
        Axis::Lon => (v + 180.0).rem_euclid(360.0) - 180.0,
    }
}

fn hemi(v: f64, a: Axis) -> char {
    match (a, v >= 0.0) {
        (Axis::Lat, true) => 'N',
        (Axis::Lat, false) => 'S',
        (Axis::Lon, true) => 'E',
        (Axis::Lon, false) => 'W',
    }
}

/// `48°41'24.0"N`, carrying rounding up correctly (59.96" never prints as 60.0").
fn fmt_dms(v: f64, a: Axis, p: usize) -> String {
    let v = normalize(v, a);
    let scale = 10f64.powi(p as i32);
    let units = (v.abs() * 3600.0 * scale).round() as u64;
    let per_min = (60.0 * scale) as u64;
    let per_deg = 60 * per_min;
    let (deg, min) = (units / per_deg, (units % per_deg) / per_min);
    let sec = (units % per_min) as f64 / scale;
    let sw = if p > 0 { p + 3 } else { 2 };
    let dw = deg_digits(a);
    format!("{deg:>dw$}°{min:02}'{sec:0sw$.p$}\"{}", hemi(v, a))
}

/// Left-aligned, truncated with "…" if too long.
fn fit_text(t: &str, w: usize) -> String {
    if t.chars().count() <= w {
        return format!("{t:<w$}");
    }
    if w == 0 {
        return String::new();
    }
    let mut s: String = t.chars().take(w - 1).collect();
    s.push('…');
    s
}

fn int_digits(a: f64) -> usize {
    if a < 1.0 {
        1
    } else {
        a.log10().floor() as usize + 1
    }
}

/// Right-aligned number that always fits `width` chars (NaN/inf, huge and tiny values included).
fn fmt_num(v: f64, width: usize, max_dec: usize) -> String {
    let s = if v.is_nan() {
        "NaN".to_owned()
    } else if v.is_infinite() {
        (if v > 0.0 { "inf" } else { "-inf" }).to_owned()
    } else {
        fit_finite(v, width, max_dec)
    };
    if s.chars().count() > width {
        return format!("{:>width$}", "…");
    }
    format!("{s:>width$}")
}

fn fit_finite(v: f64, width: usize, max_dec: usize) -> String {
    let a = v.abs();
    let digits = int_digits(a);
    let sign = usize::from(v < 0.0);
    if a == 0.0 || (a >= 1e-3 && digits + sign <= width) {
        let dec = width.saturating_sub(sign + digits + 1).min(max_dec);
        let s = format!("{v:.dec$}");
        if s.chars().count() <= width {
            return s;
        }
    }
    for p in (0..=3).rev() {
        let s = format!("{v:.p$e}");
        if s.chars().count() <= width {
            return s;
        }
    }
    "…".to_owned()
}
