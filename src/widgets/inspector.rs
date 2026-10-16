use std::ops::RangeInclusive;
use std::time::Duration;

use egui::Response;
use egui::emath::Numeric;
use egui::{
    Align, AsId, CollapsingHeader, Color32, ComboBox, CornerRadius, CursorIcon, DragValue, Frame,
    Id, Label, Layout, Margin, RichText, Sense, Shape, Slider, Stroke, TextEdit, TextStyle, Ui,
    Vec2, pos2, text::LayoutJob, vec2,
};

/// Inspector like display of nested values
#[derive(Clone)]
pub struct Inspector {
    id: Id,
    label_fraction: f32,
    min_label_width: f32,
    row_height: f32,
    row_gap: f32,
    field_gap: f32,
    pad_x: i8,
    pad_y: i8,
    corner_radius: u8,
    striped: bool,
    default_open: bool,
    right_align_labels: bool,
    monospace_values: bool,
    copyable: bool,
    colors: InspectorColors,
    flag_style: FlagStyle,
    flag_icon_size: f32,
    flag_gap: f32,
    flag_pad: f32,
    fade_time: f32,
}

/// Color scheme to use depending on the button state, overriding the global theme
#[derive(Clone, Copy, Default)]
pub struct InspectorColors {
    pub label: Option<Color32>,
    pub value: Option<Color32>,
    pub field_name: Option<Color32>,
    pub stripe: Option<Color32>,
    pub header: Option<Color32>,
    pub flag_on: Option<Color32>,
    pub flag_off: Option<Color32>,
}

impl Inspector {
    pub fn new(id_salt: impl AsId) -> Self {
        Self {
            id: Id::new(id_salt),
            label_fraction: 0.38,
            min_label_width: 60.0,
            row_height: 22.0,
            row_gap: 0.0,
            field_gap: 6.0,
            pad_x: 6,
            pad_y: 1,
            corner_radius: 3,
            striped: true,
            default_open: true,
            right_align_labels: false,
            monospace_values: false,
            copyable: true,
            colors: InspectorColors::default(),
            flag_style: FlagStyle::Badge,
            flag_icon_size: 14.0,
            flag_gap: 5.0,
            flag_pad: 6.0,
            fade_time: 0.12,
        }
    }

    /// Fraction of the width used by the label column (0.0..0.8).
    pub fn label_width(mut self, fraction: f32) -> Self {
        self.label_fraction = fraction;
        self
    }
    pub fn min_label_width(mut self, w: f32) -> Self {
        self.min_label_width = w;
        self
    }
    pub fn row_height(mut self, h: f32) -> Self {
        self.row_height = h;
        self
    }
    pub fn row_gap(mut self, g: f32) -> Self {
        self.row_gap = g;
        self
    }
    pub fn field_gap(mut self, g: f32) -> Self {
        self.field_gap = g;
        self
    }
    pub fn padding(mut self, x: i8, y: i8) -> Self {
        self.pad_x = x;
        self.pad_y = y;
        self
    }
    pub fn corner_radius(mut self, r: u8) -> Self {
        self.corner_radius = r;
        self
    }
    pub fn striped(mut self, s: bool) -> Self {
        self.striped = s;
        self
    }
    pub fn default_open(mut self, o: bool) -> Self {
        self.default_open = o;
        self
    }
    /// Blender-style right-aligned labels.
    pub fn right_align_labels(mut self, r: bool) -> Self {
        self.right_align_labels = r;
        self
    }
    pub fn monospace_values(mut self, m: bool) -> Self {
        self.monospace_values = m;
        self
    }
    /// Click-to-copy on read-only values (default: true).
    pub fn copyable(mut self, c: bool) -> Self {
        self.copyable = c;
        self
    }
    pub fn colors(mut self, c: InspectorColors) -> Self {
        self.colors = c;
        self
    }
    pub fn flag_style(mut self, s: FlagStyle) -> Self {
        self.flag_style = s;
        self
    }
    pub fn flag_icon_size(mut self, s: f32) -> Self {
        self.flag_icon_size = s;
        self
    }
    pub fn flag_gap(mut self, g: f32) -> Self {
        self.flag_gap = g;
        self
    }
    /// Horizontal padding inside `FlagStyle::Pill`.
    pub fn flag_pad(mut self, p: f32) -> Self {
        self.flag_pad = p;
        self
    }
    pub fn fade_time(mut self, t: f32) -> Self {
        self.fade_time = t;
        self
    }
    /// Shortcut: accent colors for true / false flags.
    pub fn flag_colors(mut self, on: Color32, off: Color32) -> Self {
        self.colors.flag_on = Some(on);
        self.colors.flag_off = Some(off);
        self
    }

    /// Returns `true` if any editable field changed this frame.
    pub fn show(self, ui: &mut Ui, add: impl FnOnce(&mut InspectorUi<'_>)) -> bool {
        let mut changed = false;
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.y = self.row_gap;
            ui.push_id(self.id, |ui| {
                let mut ins = InspectorUi {
                    ui,
                    cfg: &self,
                    row_idx: 0,
                    changed: false,
                };
                add(&mut ins);
                changed = ins.changed;
            });
        });
        changed
    }
}

struct FieldOpts {
    prefix: Option<String>,
    suffix: Option<String>,
    speed: Option<f64>,
    range: Option<(f64, f64)>,
    decimals: Option<usize>,
    tooltip: Option<String>,
    color: Option<Color32>,
    monospace: Option<bool>,
    copyable: Option<bool>,
    enabled: bool,
    weight: f32,
    own_tooltip: bool,
    true_text: Option<String>,
    false_text: Option<String>,
    flag_text: Option<bool>,
    flag_style: Option<FlagStyle>,
}

impl Default for FieldOpts {
    fn default() -> Self {
        Self {
            prefix: None,
            suffix: None,
            speed: None,
            range: None,
            decimals: None,
            tooltip: None,
            color: None,
            monospace: None,
            copyable: None,
            enabled: true,
            weight: 1.0,
            own_tooltip: false,
            true_text: None,
            false_text: None,
            flag_text: None,
            flag_style: None,
        }
    }
}

type FieldFn<'a> = Box<dyn FnOnce(&mut Ui, &FieldOpts, &Inspector) -> bool + 'a>;

/// One value cell inside a row. Build with the constructors, tweak with the chain methods.
pub struct Field<'a> {
    name: Option<String>,
    opts: FieldOpts,
    kind: FieldFn<'a>,
}

impl<'a> Field<'a> {
    fn new(kind: impl FnOnce(&mut Ui, &FieldOpts, &Inspector) -> bool + 'a) -> Self {
        Self {
            name: None,
            opts: FieldOpts::default(),
            kind: Box::new(kind),
        }
    }

    /// Read-only text, copied to the clipboard on click.
    pub fn value(v: impl ToString) -> Self {
        let s = v.to_string();
        let mut f = Self::new(move |ui, o, cfg| {
            copyable_label(ui, &s, o, cfg);
            false
        });
        f.opts.own_tooltip = true;
        f
    }

    /// Small centered weak text, for column headers (see `InspectorUi::header_row`).
    pub fn header(text: impl Into<String>) -> Field<'static> {
        let t = text.into();
        Field::new(move |ui, _, cfg| {
            let c = cfg
                .colors
                .field_name
                .unwrap_or(ui.visuals().weak_text_color());
            ui.with_layout(
                Layout::centered_and_justified(egui::Direction::LeftToRight),
                |ui| ui.label(RichText::new(t).small().color(c)),
            );
            false
        })
    }

    pub fn drag<T: Numeric>(v: &'a mut T) -> Self {
        Self::new(move |ui, o, cfg| {
            let mut d = DragValue::new(v);
            if let Some(s) = o.speed {
                d = d.speed(s);
            }
            if let Some((lo, hi)) = o.range {
                d = d.range(lo..=hi);
            }
            if let Some(n) = o.decimals {
                d = d.fixed_decimals(n);
            }
            if let Some(p) = &o.prefix {
                d = d.prefix(p.clone());
            }
            if let Some(s) = &o.suffix {
                d = d.suffix(s.clone());
            }
            let w = ui.available_width();
            ui.add_sized([w, cfg.row_height - 2.0], d).changed()
        })
    }

    pub fn slider<T: Numeric>(v: &'a mut T, range: RangeInclusive<T>) -> Self {
        Self::new(move |ui, o, _| {
            ui.spacing_mut().slider_width = (ui.available_width() - 64.0).max(40.0);
            let mut s = Slider::new(v, range);
            if let Some(n) = o.decimals {
                s = s.fixed_decimals(n);
            }
            if let Some(p) = &o.prefix {
                s = s.prefix(p.clone());
            }
            if let Some(x) = &o.suffix {
                s = s.suffix(x.clone());
            }
            ui.add(s).changed()
        })
    }

    pub fn checkbox(v: &'a mut bool) -> Self {
        Self::new(move |ui, _, _| ui.checkbox(v, "").changed())
    }

    /// Color button plus a copyable hex code.
    pub fn color(v: &'a mut Color32) -> Self {
        Self::new(move |ui, o, cfg| {
            let changed = ui.color_edit_button_srgba(v).changed();
            let hex = format!("#{:02X}{:02X}{:02X}", v.r(), v.g(), v.b());
            copyable_label(ui, &hex, o, cfg);
            changed
        })
    }

    pub fn color_rgb(v: &'a mut [f32; 3]) -> Self {
        Self::new(move |ui, _, _| ui.color_edit_button_rgb(v).changed())
    }

    pub fn text_edit(v: &'a mut String) -> Self {
        Self::new(move |ui, _, cfg| {
            let w = ui.available_width();
            ui.add_sized([w, cfg.row_height - 2.0], TextEdit::singleline(v))
                .changed()
        })
    }

    /// Dropdown. `options` is a list of `(value, display name)`.
    pub fn combo<T, S>(v: &'a mut T, options: impl IntoIterator<Item = (T, S)>) -> Self
    where
        T: PartialEq + Clone + 'a,
        S: Into<String>,
    {
        let options: Vec<(T, String)> = options.into_iter().map(|(t, s)| (t, s.into())).collect();
        Self::new(move |ui, _, _| {
            let selected = options
                .iter()
                .find(|(t, _)| t == &*v)
                .map(|(_, n)| n.clone())
                .unwrap_or_default();
            let mut changed = false;
            ComboBox::from_id_salt("combo")
                .width(ui.available_width())
                .selected_text(selected)
                .show_ui(ui, |ui| {
                    for (t, n) in &options {
                        if ui.selectable_value(v, t.clone(), n.as_str()).changed() {
                            changed = true;
                        }
                    }
                });
            changed
        })
    }

    /// Radio buttons (wraps if the cell is narrow).
    pub fn radio<T, S>(v: &'a mut T, options: impl IntoIterator<Item = (T, S)>) -> Self
    where
        T: PartialEq + Clone + 'a,
        S: Into<String>,
    {
        let options: Vec<(T, String)> = options.into_iter().map(|(t, s)| (t, s.into())).collect();
        Self::new(move |ui, _, _| {
            let mut changed = false;
            ui.horizontal_wrapped(|ui| {
                for (t, n) in &options {
                    if ui.radio_value(v, t.clone(), n.as_str()).changed() {
                        changed = true;
                    }
                }
            });
            changed
        })
    }

    /// Escape hatch: draw anything, return `true` if it changed something.
    pub fn custom(f: impl FnOnce(&mut Ui) -> bool + 'a) -> Self {
        Self::new(move |ui, _, _| f(ui))
    }

    /// Small name drawn before the widget (e.g. "X", "Min").
    pub fn named(mut self, n: impl Into<String>) -> Self {
        self.name = Some(n.into());
        self
    }
    pub fn prefix(mut self, p: impl Into<String>) -> Self {
        self.opts.prefix = Some(p.into());
        self
    }
    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.opts.suffix = Some(s.into());
        self
    }
    pub fn speed(mut self, s: f64) -> Self {
        self.opts.speed = Some(s);
        self
    }
    /// Clamp range for `drag`.
    pub fn range(mut self, r: RangeInclusive<f64>) -> Self {
        self.opts.range = Some((*r.start(), *r.end()));
        self
    }
    pub fn decimals(mut self, n: usize) -> Self {
        self.opts.decimals = Some(n);
        self
    }
    pub fn tooltip(mut self, t: impl Into<String>) -> Self {
        self.opts.tooltip = Some(t.into());
        self
    }
    pub fn color_override(mut self, c: Color32) -> Self {
        self.opts.color = Some(c);
        self
    }
    pub fn monospace(mut self, m: bool) -> Self {
        self.opts.monospace = Some(m);
        self
    }
    pub fn copyable(mut self, c: bool) -> Self {
        self.opts.copyable = Some(c);
        self
    }
    pub fn disabled(mut self, d: bool) -> Self {
        self.opts.enabled = !d;
        self
    }
    /// Relative width within a multi-field row (default 1.0).
    pub fn weight(mut self, w: f32) -> Self {
        self.opts.weight = w.max(0.05);
        self
    }
    /// Read-only boolean indicator, click-to-copy.
    pub fn flag(v: bool) -> Self {
        let mut f = Self::new(move |ui, o, cfg| {
            flag_widget(ui, v, o, cfg);
            false
        });
        f.opts.own_tooltip = true;
        f
    }
    /// Texts shown next to the indicator (defaults: "Yes" / "No").
    pub fn flag_labels(mut self, on: impl Into<String>, off: impl Into<String>) -> Self {
        self.opts.true_text = Some(on.into());
        self.opts.false_text = Some(off.into());
        self
    }
    /// Show or hide the text next to the indicator (default: shown).
    pub fn flag_text(mut self, show: bool) -> Self {
        self.opts.flag_text = Some(show);
        self
    }
    pub fn flag_style(mut self, s: FlagStyle) -> Self {
        self.opts.flag_style = Some(s);
        self
    }

    fn show(self, ui: &mut Ui, cfg: &Inspector) -> bool {
        let Field { name, opts, kind } = self;
        if let Some(n) = &name {
            let c = cfg
                .colors
                .field_name
                .unwrap_or(ui.visuals().weak_text_color());
            ui.label(RichText::new(n).small().color(c));
        }
        let mut changed = false;
        let r = ui.add_enabled_ui(opts.enabled, |ui| {
            changed = kind(ui, &opts, cfg);
        });
        if !opts.own_tooltip
            && let Some(t) = &opts.tooltip {
                r.response.on_hover_text(t);
            }
        changed
    }
}

/// Draw bolean state
fn flag_widget(ui: &mut Ui, on: bool, o: &FieldOpts, cfg: &Inspector) {
    let accent = o.color.unwrap_or_else(|| {
        if on {
            cfg.colors.flag_on.unwrap_or(ui.visuals().hyperlink_color)
        } else {
            cfg.colors
                .flag_off
                .unwrap_or(ui.visuals().weak_text_color())
        }
    });
    let style = o.flag_style.unwrap_or(cfg.flag_style);
    let text: &str = if on {
        o.true_text.as_deref().unwrap_or("Yes")
    } else {
        o.false_text.as_deref().unwrap_or("No")
    };
    let show_text = o.flag_text.unwrap_or(true) && !text.is_empty();
    let text_color = cfg.colors.value.unwrap_or(ui.visuals().text_color());
    let can_copy = o.copyable.unwrap_or(cfg.copyable);

    let pad = if style == FlagStyle::Pill {
        cfg.flag_pad
    } else {
        0.0
    };
    let icon = cfg.flag_icon_size;
    let gap = cfg.flag_gap;
    let h = (cfg.row_height - 2.0).max(icon);
    let avail = ui.available_width();

    // 1. measure (text is truncated with "…" when the cell is narrow)
    let galley = show_text.then(|| {
        let style_id = if o.monospace.unwrap_or(cfg.monospace_values) {
            TextStyle::Monospace
        } else {
            TextStyle::Body
        };
        let font = style_id.resolve(ui.style());
        let mut job = LayoutJob::simple_singleline(text.to_owned(), font, text_color);
        job.wrap.max_width = (avail - 2.0 * pad - icon - gap).max(1.0);
        job.wrap.max_rows = 1;
        job.wrap.break_anywhere = true;
        job.wrap.overflow_character = Some('…');
        ui.fonts_mut(|f| f.layout_job(job))
    });
    let text_w = galley.as_ref().map_or(0.0, |g| gap + g.size().x);
    let size = vec2((2.0 * pad + icon + text_w).min(avail), h);

    // 2. allocate
    let sense = if can_copy {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, resp) = ui.allocate_exact_size(size, sense);

    // 3. paint
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool_with_time(
            resp.id.with("flag_hover"),
            resp.hovered(),
            cfg.fade_time,
        );
        let p = ui.painter();
        let r = icon * 0.5;
        let sw = (icon * 0.11).max(1.2);
        let c = pos2(rect.left() + pad + r, rect.center().y);

        if style == FlagStyle::Pill {
            let cr = CornerRadius::same((rect.height() * 0.5).round() as u8);
            p.rect_filled(rect, cr, accent.gamma_multiply(0.10 + 0.10 * t));
        }

        match style {
            FlagStyle::Badge => {
                p.circle_filled(c, r, accent.gamma_multiply(0.18 + 0.12 * t));
                let s = Stroke::new(sw, accent);
                if on {
                    p.add(Shape::line(
                        vec![
                            c + vec2(-0.45 * r, 0.0),
                            c + vec2(-0.10 * r, 0.38 * r),
                            c + vec2(0.50 * r, -0.32 * r),
                        ],
                        s,
                    ));
                } else {
                    let d = 0.35 * r;
                    p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], s);
                    p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], s);
                }
            }
            FlagStyle::Dot | FlagStyle::Pill => {
                let dr = r * (0.55 + 0.10 * t);
                if on {
                    p.circle_filled(c, dr, accent);
                } else {
                    p.circle_stroke(c, dr, Stroke::new(sw, accent));
                }
            }
        }

        if let Some(g) = &galley {
            let pos = pos2(
                rect.left() + pad + icon + gap,
                rect.center().y - g.size().y * 0.5,
            );
            p.galley(pos, g.clone(), text_color);
        }
    }

    // 4. interaction
    let raw = if show_text {
        text.to_owned()
    } else {
        on.to_string()
    };
    let tip = o.tooltip.as_deref().or((!show_text).then_some(text));
    copy_response(ui, resp, &raw, tip, can_copy);
}

fn copyable_label(ui: &mut Ui, raw: &str, o: &FieldOpts, cfg: &Inspector) {
    let shown = format!(
        "{}{}{}",
        o.prefix.as_deref().unwrap_or(""),
        raw,
        o.suffix.as_deref().unwrap_or("")
    );
    let mut rt = RichText::new(shown);
    if let Some(c) = o.color.or(cfg.colors.value) {
        rt = rt.color(c);
    }
    if o.monospace.unwrap_or(cfg.monospace_values) {
        rt = rt.monospace();
    }

    let can_copy = o.copyable.unwrap_or(cfg.copyable);
    let sense = if can_copy {
        Sense::click()
    } else {
        Sense::hover()
    };
    let resp = ui.add(Label::new(rt).truncate().selectable(false).sense(sense));
    copy_response(ui, resp, raw, o.tooltip.as_deref(), can_copy);
}

fn copy_response(ui: &mut Ui, resp: Response, raw: &str, tooltip: Option<&str>, can_copy: bool) {
    if !can_copy {
        if let Some(t) = tooltip {
            resp.on_hover_text(t);
        }
        return;
    }

    let resp = resp.on_hover_cursor(CursorIcon::Copy);
    let now = ui.input(|i| i.time);
    if resp.clicked() {
        ui.ctx().copy_text(raw.to_owned());
        ui.data_mut(|d| d.insert_temp(resp.id, now));
    }
    let recent = ui
        .data(|d| d.get_temp::<f64>(resp.id))
        .is_some_and(|t| now - t < 1.2);
    if recent {
        ui.ctx().request_repaint_after(Duration::from_millis(1300));
    }
    let tip = if recent { "Copied!" } else { "Click to copy" };
    resp.on_hover_text(match tooltip {
        Some(t) => format!("{t}\n{tip}"),
        None => tip.to_owned(),
    });
}

/// Structure fields in row
pub struct Row<'a> {
    label: String,
    hint: Option<String>,
    label_color: Option<Color32>,
    fields: Vec<Field<'a>>,
}

impl<'a> Row<'a> {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            label_color: None,
            fields: Vec::new(),
        }
    }
    pub fn hint(mut self, h: impl Into<String>) -> Self {
        self.hint = Some(h.into());
        self
    }
    pub fn label_color(mut self, c: Color32) -> Self {
        self.label_color = Some(c);
        self
    }
    pub fn field(mut self, f: Field<'a>) -> Self {
        self.fields.push(f);
        self
    }
    pub fn fields(mut self, fs: impl IntoIterator<Item = Field<'a>>) -> Self {
        self.fields.extend(fs);
        self
    }
}

/// Layout the inspector
pub struct InspectorUi<'a> {
    ui: &'a mut Ui,
    cfg: &'a Inspector,
    row_idx: usize,
    changed: bool,
}

impl<'a> InspectorUi<'a> {
    /// Raw access to the underlying `Ui` (for custom content).
    pub fn ui(&mut self) -> &mut Ui {
        self.ui
    }
    pub fn changed(&self) -> bool {
        self.changed
    }

    /// Collapsible group, nestable. Uses the inspector's `default_open`.
    pub fn section<R>(
        &mut self,
        title: impl Into<String>,
        add: impl FnOnce(&mut InspectorUi<'_>) -> R,
    ) -> Option<R> {
        let open = self.cfg.default_open;
        self.section_opts(title, open, add)
    }

    pub fn section_opts<R>(
        &mut self,
        title: impl Into<String>,
        default_open: bool,
        add: impl FnOnce(&mut InspectorUi<'_>) -> R,
    ) -> Option<R> {
        let title = title.into();
        let cfg = self.cfg;
        let mut rt = RichText::new(title.clone()).strong();
        if let Some(c) = cfg.colors.header {
            rt = rt.color(c);
        }
        let mut changed = false;
        let resp = CollapsingHeader::new(rt)
            .id_salt(&title)
            .default_open(default_open)
            .show(self.ui, |ui| {
                let mut sub = InspectorUi {
                    ui,
                    cfg,
                    row_idx: 0,
                    changed: false,
                };
                let r = add(&mut sub);
                changed = sub.changed;
                r
            });
        self.changed |= changed;
        resp.body_returned
    }

    /// Single-field row.
    pub fn prop(&mut self, label: impl Into<String>, field: Field<'_>) -> bool {
        self.row(Row::new(label).field(field))
    }

    /// Multi-field row (e.g. X / Y / Z).
    pub fn props<'f>(
        &mut self,
        label: impl Into<String>,
        fields: impl IntoIterator<Item = Field<'f>>,
    ) -> bool {
        self.row(Row::new(label).fields(fields))
    }

    /// Shortcut for a read-only, copyable value.
    pub fn text(&mut self, label: impl Into<String>, value: impl ToString) {
        self.prop(label, Field::value(value));
    }

    /// Column titles aligned with the fields of the rows below it.
    pub fn header_row(&mut self, names: &[&str]) {
        let fields: Vec<Field<'static>> = names.iter().map(|n| Field::header(*n)).collect();
        self.row(Row::new("").fields(fields));
    }

    /// Wrapped, selectable paragraph.
    pub fn description(&mut self, text: &str) {
        let cfg = self.cfg;
        Frame::new()
            .inner_margin(Margin::symmetric(cfg.pad_x, 3))
            .show(self.ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.add(Label::new(text));
            });
    }

    pub fn flag(&mut self, label: impl Into<String>, value: bool) {
        self.prop(label, Field::flag(value));
    }

    pub fn separator(&mut self) {
        self.ui.add_space(2.0);
        self.ui.separator();
    }

    pub fn row(&mut self, row: Row<'_>) -> bool {
        let cfg = self.cfg;
        let idx = self.row_idx;
        self.row_idx += 1;

        let fill = if cfg.striped && idx % 2 == 1 {
            cfg.colors
                .stripe
                .unwrap_or(self.ui.visuals().faint_bg_color)
        } else {
            Color32::TRANSPARENT
        };

        let Row {
            label,
            hint,
            label_color,
            fields,
        } = row;
        let n = fields.len();
        let total_weight: f32 = fields.iter().map(|f| f.opts.weight).sum::<f32>().max(0.05);
        let h = cfg.row_height;
        let mut changed = false;

        self.ui.push_id(idx, |ui| {
            Frame::new()
                .fill(fill)
                .inner_margin(Margin::symmetric(cfg.pad_x, cfg.pad_y))
                .corner_radius(CornerRadius::same(cfg.corner_radius))
                .show(ui, |ui| {
                    let full = ui.available_width();
                    ui.set_min_width(full);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = cfg.field_gap;

                        // label column
                        let label_w = (full * cfg.label_fraction)
                            .max(cfg.min_label_width)
                            .min(full * 0.8);
                        let layout = if cfg.right_align_labels {
                            Layout::right_to_left(Align::Center)
                        } else {
                            Layout::left_to_right(Align::Center)
                        };
                        ui.allocate_ui_with_layout(Vec2::new(label_w, h), layout, |ui| {
                            ui.set_min_size(Vec2::new(label_w, h));
                            if !label.is_empty() {
                                let c = label_color
                                    .or(cfg.colors.label)
                                    .unwrap_or(ui.visuals().weak_text_color());
                                let r =
                                    ui.add(Label::new(RichText::new(&label).color(c)).truncate());
                                if let Some(t) = &hint {
                                    r.on_hover_text(t);
                                }
                            }
                        });

                        // value column: split between fields by weight
                        if n == 0 {
                            return;
                        }
                        let avail = ui.available_width() - cfg.field_gap * (n as f32 - 1.0);
                        for (i, f) in fields.into_iter().enumerate() {
                            let fw = (avail * f.opts.weight / total_weight).max(1.0);
                            ui.push_id(i, |ui| {
                                ui.allocate_ui_with_layout(
                                    Vec2::new(fw, h),
                                    Layout::left_to_right(Align::Center),
                                    |ui| {
                                        ui.set_min_size(Vec2::new(fw, h));
                                        if f.show(ui, cfg) {
                                            changed = true;
                                        }
                                    },
                                );
                            });
                        }
                    });
                });
        });

        self.changed |= changed;
        changed
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FlagStyle {
    /// Round badge with a check mark / cross.
    Badge,
    /// Filled dot (true) or ring (false).
    Dot,
    /// Dot inside a tinted, fully rounded pill.
    Pill,
}
