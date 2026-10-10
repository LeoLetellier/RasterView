use std::cell::Cell;

use eframe::egui::{
    self, Align, Align2, Button, Color32, ComboBox, CornerRadius, CursorIcon, DragValue, FontId,
    Id, Layout, Modal, Rect, Response, RichText, ScrollArea, Sense, Slider, TextEdit, Ui,
    UiBuilder, Vec2, pos2, vec2,
};
use serde::{Deserialize, Serialize};

use crate::RasterView;

impl RasterView {
    pub(crate) fn ui_parameters(&mut self, ui: &mut Ui) -> bool {
        self.app_state
            .settings_panel
            .show(&ui.ctx().clone(), &mut self.app_state.settings)
    }
}

const ACCENT: Color32 = Color32::from_rgb(90, 140, 255);
const ROW_H: f32 = 52.0;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub(crate) enum Resampling {
    Nearest,
    Bilinear,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    // App
    /// Search automatically for lect.in or .src headers (NSBAS)
    pub(crate) autoresolve_real4: bool,
    /// Search automatically for AMSTer headers
    pub(crate) autoresolve_amster: bool,
    // Viewer
    /// Resampling method to display raster
    pub(crate) resampling: Resampling,
    /// Tile size on screen in pixels
    pub(crate) tile_size: usize,
    /// Ratio between the viewport size and the raster
    pub(crate) viewport_padding: f64,
    // Cache
    /// Size of the cache
    pub(crate) cache_mb: u32,
    /// Preload all band statistics
    pub(crate) preload_stats: bool,
    // Debug
    /// Show the tile boundaries
    pub(crate) show_tiles: bool,
    /// Show the theme panel
    pub(crate) show_theme_panel: bool,
    /// Show the framerate
    pub(crate) show_frame_rate: bool,
    /// Enable verbose logs
    pub(crate) verbose_logs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let is_debug = cfg!(debug_assertions);
        Self {
            // App
            autoresolve_real4: false,
            autoresolve_amster: false,
            // Viewer
            resampling: Resampling::Nearest,
            tile_size: 256,
            viewport_padding: 1.1, // 10%
            // Cache
            cache_mb: 512,
            preload_stats: true,
            // Debug
            show_tiles: is_debug,
            show_theme_panel: is_debug,
            show_frame_rate: is_debug,
            verbose_logs: is_debug,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    App,
    Viewer,
    Cache,
    Debug,
}

impl Section {
    const ALL: [Section; 4] = [
        Section::App,
        Section::Viewer,
        Section::Cache,
        Section::Debug,
    ];

    fn label(self) -> &'static str {
        match self {
            Section::App => "Application",
            Section::Viewer => "Viewer",
            Section::Cache => "Cache",
            Section::Debug => "Debug",
        }
    }
}

// ───────────────────────── Small widgets ─────────────────────────

fn toggle(ui: &mut Ui, on: &mut bool) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(36.0, 20.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let bg = Color32::from_gray(70).lerp_to_gamma(ACCENT, t);
    ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
    let x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
    ui.painter()
        .circle_filled(pos2(x, rect.center().y), 7.0, Color32::WHITE);
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

fn combo<T: PartialEq + Copy>(
    ui: &mut Ui,
    id: &str,
    value: &mut T,
    options: &[(T, &str)],
) -> Response {
    let current = options
        .iter()
        .find(|(v, _)| *v == *value)
        .map(|(_, n)| *n)
        .unwrap_or("?");
    let mut changed = false;
    let mut resp = ComboBox::from_id_salt(id)
        .selected_text(current)
        .show_ui(ui, |ui| {
            for (v, name) in options {
                if ui.selectable_value(value, *v, *name).changed() {
                    changed = true;
                }
            }
        })
        .response;
    if changed {
        resp.mark_changed();
    }
    resp
}

fn rail_item(ui: &mut Ui, label: &str, active: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 32.0), Sense::click());
    let painter = ui.painter();
    if active || resp.hovered() {
        let fill = if active {
            ACCENT.gamma_multiply(0.18)
        } else {
            ui.visuals().widgets.hovered.weak_bg_fill
        };
        painter.rect_filled(rect, CornerRadius::same(6), fill);
    }
    if active {
        let bar = Rect::from_min_size(rect.min + vec2(0.0, 6.0), vec2(3.0, rect.height() - 12.0));
        painter.rect_filled(bar, CornerRadius::same(2), ACCENT);
    }
    let color = if active {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().text_color()
    };
    painter.text(
        pos2(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        color,
    );
    resp.on_hover_cursor(CursorIcon::PointingHand)
}

// ───────────────────────── Form: rows, sections, search ─────────────────────────

struct Form {
    query: String,                  // lowercase
    scroll_to: Option<Section>,     // consumed this frame
    dry: Cell<bool>,                // dry run = only count matches
    hits: Cell<usize>,              // matches in current section
    drawn: Cell<usize>,             // sections drawn this frame
    current: Cell<Option<Section>>, // scroll-spy result
    first: Cell<Option<Section>>,
}

impl Form {
    fn matches(&self, label: &str, desc: &str) -> bool {
        self.query.is_empty()
            || label.to_lowercase().contains(&self.query)
            || desc.to_lowercase().contains(&self.query)
    }

    fn section(&self, ui: &mut Ui, section: Section, mut body: impl FnMut(&Self, &mut Ui)) {
        // Pass 1: dry run to count matching rows
        self.dry.set(true);
        self.hits.set(0);
        body(self, ui);
        self.dry.set(false);
        if self.hits.get() == 0 {
            return;
        }
        self.drawn.set(self.drawn.get() + 1);
        if self.first.get().is_none() {
            self.first.set(Some(section));
        }

        // Pass 2: header + rows
        ui.add_space(24.0);
        let head = ui.label(RichText::new(section.label()).size(20.0).strong());
        ui.add_space(6.0);
        if self.scroll_to == Some(section) {
            head.scroll_to_me(Some(Align::TOP));
        }
        if head.rect.top() <= ui.clip_rect().top() + 64.0 {
            self.current.set(Some(section)); // last one above the line wins
        }
        body(self, ui);
    }

    /// One settings row. Returns true if the value changed.
    fn row<T: PartialEq + Clone>(
        &self,
        ui: &mut Ui,
        label: &str,
        desc: &str,
        value: &mut T,
        default: &T,
        control: impl FnOnce(&mut Ui, &mut T) -> Response,
    ) -> bool {
        if !self.matches(label, desc) {
            return false;
        }
        if self.dry.get() {
            self.hits.set(self.hits.get() + 1);
            return false;
        }

        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::hover());

        // Row background + divider, painted before the content
        if ui.rect_contains_pointer(rect) {
            ui.painter()
                .rect_filled(rect, CornerRadius::same(6), ui.visuals().faint_bg_color);
        }
        ui.painter().hline(
            rect.x_range(),
            rect.bottom(),
            ui.visuals().widgets.noninteractive.bg_stroke,
        );

        // Fixed split: ~55% label / rest control
        let split = rect.left() + rect.width() * 0.55;
        let left_rect =
            Rect::from_min_max(rect.min + vec2(8.0, 0.0), pos2(split - 12.0, rect.max.y));
        let right_rect = Rect::from_min_max(pos2(split, rect.min.y), rect.max - vec2(8.0, 0.0));

        // Left: label + description
        let mut left = ui.new_child(
            UiBuilder::new()
                .max_rect(left_rect)
                .layout(Layout::top_down(Align::Min)),
        );
        let content_h = if desc.is_empty() { 18.0 } else { 38.0 };
        left.add_space((ROW_H - content_h) / 2.0);
        left.label(label);
        if !desc.is_empty() {
            left.label(RichText::new(desc).small().weak());
        }

        // Right: control first (rightmost), then the reset button to its left
        let mut right = ui.new_child(
            UiBuilder::new()
                .max_rect(right_rect)
                .layout(Layout::right_to_left(Align::Center)),
        );
        let mut changed = control(&mut right, value).changed();
        if *value != *default {
            let reset = right.add(Button::new(RichText::new("reset").small().weak()).frame(false));
            if reset.clicked() {
                *value = default.clone();
                changed = true;
            }
        }
        changed
    }
}

// ───────────────────────── Panel ─────────────────────────

pub(crate) struct SettingsPanel {
    active: Section,
    pending: Option<Section>,
    locked: Option<(Section, f64)>, // (target, unlock deadline)
    query: String,
    pub(crate) is_open: bool,
}

impl Default for SettingsPanel {
    fn default() -> Self {
        Self {
            active: Section::Viewer,
            pending: None,
            locked: None,
            query: String::new(),
            is_open: false,
        }
    }
}

impl SettingsPanel {
    fn show(&mut self, ctx: &egui::Context, s: &mut Settings) -> bool {
        let open = self.is_open;
        // Fade in/out
        let currently_open = open.to_owned();
        let t = ctx.animate_bool_with_time(Id::new("settings_fade"), currently_open, 0.12);
        if t == 0.0 && !currently_open {
            return false;
        }

        let d = Settings::default();
        let size = (ctx.content_rect().size() * 0.75).min(Vec2::new(900.0, 640.0));
        let now = ctx.input(|i| i.time);

        let form = Form {
            query: self.query.to_lowercase(),
            scroll_to: self.pending.take(),
            dry: Cell::new(false),
            hits: Cell::new(0),
            drawn: Cell::new(0),
            current: Cell::new(None),
            first: Cell::new(None),
        };
        if let Some(target) = form.scroll_to {
            self.locked = Some((target, now + 1.0));
            self.active = target;
        }

        let mut clicked: Option<Section> = None;

        let modal = Modal::new(Id::new("settings_modal")).show(ctx, |ui| {
            ui.set_opacity(t);
            ui.set_max_size(size);
            ui.horizontal_top(|ui| {
                // ── Left rail ──
                ui.allocate_ui(vec2(160.0, size.y), |ui| {
                    ui.set_width(160.0);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        for sec in Section::ALL {
                            if rail_item(ui, sec.label(), self.active == sec).clicked() {
                                clicked = Some(sec);
                            }
                        }
                    });
                });
                ui.separator();

                // ── Right: search + scrolling page ──
                ui.vertical(|ui| {
                    ui.add(
                        TextEdit::singleline(&mut self.query)
                            .hint_text("Search settings…")
                            .desired_width(f32::INFINITY),
                    );
                    ui.add_space(4.0);

                    ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            form.section(ui, Section::App, |f, ui| {
                                f.row(
                                    ui,
                                    "Auto-resolve REAL4",
                                    "Resolve REAL4 files automatically (lect.in and .rsc headers).",
                                    &mut s.autoresolve_real4,
                                    &d.autoresolve_real4,
                                    |ui, v| toggle(ui, v),
                                );
                                f.row(
                                    ui,
                                    "Auto-resolve AMSTER",
                                    "Resolve AMSTER files automatically (AMSTer Engine TextFile headers).",
                                    &mut s.autoresolve_amster,
                                    &d.autoresolve_amster,
                                    |ui, v| toggle(ui, v),
                                );
                            });

                            form.section(ui, Section::Viewer, |f, ui| {
                                f.row(
                                    ui,
                                    "Resampling",
                                    "Interpolation used when zooming.",
                                    &mut s.resampling,
                                    &d.resampling,
                                    |ui, v| {
                                        combo(
                                            ui,
                                            "resampling",
                                            v,
                                            &[
                                                (Resampling::Nearest, "Nearest"),
                                                (Resampling::Bilinear, "Bilinear"),
                                            ],
                                        )
                                    },
                                );
                                f.row(ui, "Tile size", "Tile size on screen.", &mut s.tile_size, &d.tile_size, |ui, v| {
                                    ui.add(DragValue::new(v).range(64..=2048).suffix(" pixels"))
                                });
                                f.row(ui, "Viewport padding", "Ratio between the viewport size and the raster at default zoom level.", &mut s.viewport_padding, &d.viewport_padding, |ui, v| {
                                    ui.add(DragValue::new(v).range(1.0..=3.0).speed(0.1))
                                });
                            });

                            form.section(ui, Section::Cache, |f, ui| {
                                f.row(
                                    ui,
                                    "Cache size",
                                    "Upper memory bound.",
                                    &mut s.cache_mb,
                                    &d.cache_mb,
                                    |ui, v| {
                                        ui.add(DragValue::new(v).range(64..=8192).suffix(" MB"))
                                    },
                                );
                                f.row(
                                    ui,
                                    "Preload stats",
                                    "Compute statistics ahead of time.",
                                    &mut s.preload_stats,
                                    &d.preload_stats,
                                    |ui, v| toggle(ui, v),
                                );
                            });

                            form.section(ui, Section::Debug, |f, ui| {
                                f.row(
                                    ui,
                                    "Show tiles",
                                    "Draw tile boundaries.",
                                    &mut s.show_tiles,
                                    &d.show_tiles,
                                    |ui, v| toggle(ui, v),
                                );
                                if cfg!(debug_assertions) {
                                    f.row(
                                        ui,
                                        "Theme panel",
                                        "Show the theme editor panel.",
                                        &mut s.show_theme_panel,
                                        &d.show_theme_panel,
                                        |ui, v| toggle(ui, v),
                                    );
                                }
                                f.row(
                                    ui,
                                    "Frame rate",
                                    "Show frame timings on screen.",
                                    &mut s.show_frame_rate,
                                    &d.show_frame_rate,
                                    |ui, v| toggle(ui, v),
                                );
                                // `row` returns true when the value changed (including via "reset"),
                                // which is where you'd hook `set_verbose` later.
                                let _verbose_changed = f.row(
                                    ui,
                                    "Verbose logs",
                                    "",
                                    &mut s.verbose_logs,
                                    &d.verbose_logs,
                                    |ui, v| toggle(ui, v),
                                );
                            });

                            if form.drawn.get() == 0 {
                                ui.add_space(24.0);
                                ui.label(RichText::new("No matching settings").weak());
                            }
                            ui.add_space(size.y * 0.6); // lets the last section reach the top
                        });
                });
            });
        });

        // Scroll-spy with lock after rail clicks
        if let Some((target, deadline)) = self.locked {
            if form.current.get() == Some(target) || now > deadline {
                self.locked = None;
            } else {
                self.active = target;
            }
        }
        if self.locked.is_none() {
            if let Some(cur) = form.current.get().or(form.first.get()) {
                self.active = cur;
            }
        }

        if clicked.is_some() {
            self.pending = clicked;
            ctx.request_repaint();
        }
        if modal.should_close() {
            self.is_open = false;
        }
        self.is_open
    }
}
