//! The side panel answers three questions in order: what you are seeing, how to
//! change it, and where to go next. Formulas and background follow for whoever
//! wants them.

use egui::{pos2, vec2, Color32, Rect, RichText, Sense, Stroke};

use super::{Action, AtlasApp};
use crate::atlas::guides::guide;
use crate::atlas::links::{self, Neighbour};
use crate::atlas::presets;
use crate::atlas::trails::{self, TRAILS};
use crate::engine::module::InsRole;
use crate::engine::params::{PValue, Param, ParamKind};
use crate::theme::{self, DUSK, EDGE, HOVER, LIGHT, MOON, VOID};
use crate::widgets::{self, icon};

/// Display height, in points per PNG pixel, of the 600 dpi formula images.
const FORMULA_SCALE: f32 = 0.17;

const SEEING: &str = "What you're seeing";
const CHANGE: &str = "Change it";
const NEXT: &str = "Go next";
const FORMULAS: &str = "Formulas and live values";
const ABOUT: &str = "About this chamber";
const SAVED: &str = "Saved views";

impl AtlasApp {
    pub(super) fn context_panel(&mut self, ctx: &egui::Context) {
        let window = ctx.screen_rect().width();
        // A fixed width that only the handle changes: content can never push it.
        let width = self
            .session
            .prefs
            .panel_width
            .unwrap_or((window * 0.2).clamp(296.0, 400.0));
        let width = width.clamp(280.0, (window * 0.4).max(280.0));
        egui::SidePanel::right("context")
            .exact_width(width)
            .resizable(false)
            .frame(egui::Frame::new().fill(VOID).inner_margin(egui::Margin {
                left: 16,
                right: 12,
                top: 10,
                bottom: 0,
            }))
            .show(ctx, |ui| {
                self.panel_handle(ui, width);
                egui::ScrollArea::vertical()
                    .id_salt("context-scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.set_max_width(ui.available_width() - 4.0);
                        self.panel_sections(ui);
                        ui.add_space(24.0);
                    });
            });
    }

    /// Drag the panel's left edge to resize it.
    fn panel_handle(&mut self, ui: &mut egui::Ui, width: f32) {
        let panel = ui.max_rect();
        let x = panel.left() - 16.0;
        // Inside the panel, because the canvas owns every pixel to its left.
        let edge = Rect::from_min_max(pos2(x, panel.top() - 10.0), pos2(x + 7.0, panel.bottom()));
        let resp = ui
            .interact(edge, ui.id().with("panel-handle"), Sense::drag())
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
        if resp.dragged() {
            self.session.prefs.panel_width = Some(width - resp.drag_delta().x);
            self.dirty = true;
        }
        if resp.hovered() || resp.dragged() {
            ui.painter().line_segment(
                [pos2(x, edge.top()), pos2(x, edge.bottom())],
                Stroke::new(1.0, EDGE),
            );
        }
    }

    fn section(
        &mut self,
        ui: &mut egui::Ui,
        name: &str,
        add: impl FnOnce(&mut Self, &mut egui::Ui),
    ) {
        let open = !self.session.prefs.closed_sections.iter().any(|n| n == name);
        if widgets::section_header(ui, name, open).clicked() {
            self.act(Action::ToggleSection(name.to_string()));
        }
        if open {
            ui.add_space(2.0);
            add(self, ui);
        }
        ui.add_space(14.0);
    }

    fn panel_sections(&mut self, ui: &mut egui::Ui) {
        self.section(ui, SEEING, Self::seeing_section);
        self.section(ui, CHANGE, Self::change_section);
        self.section(ui, NEXT, Self::next_section);
        self.section(ui, FORMULAS, Self::formula_section);
        self.section(ui, ABOUT, Self::about_section);
        if !self.session.bookmarks.is_empty() {
            self.section(ui, SAVED, Self::saved_section);
        }
    }

    fn seeing_section(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        let accent = chamber.accent;
        let g = guide(chamber.info.id, chamber.view_id(f.mode));
        match g {
            Some(g) => {
                reading(ui, g.read, LIGHT);
                ui.add_space(6.0);
                reading(ui, &format!("Try this: {}", g.try_this), accent);
            }
            None => reading(ui, chamber.modes[f.mode].desc, LIGHT),
        }
    }

    // ---------- change it ----------

    fn change_section(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let catalog = self.catalog.clone();
        let chamber = &catalog[f.module];
        let id = chamber.info.id;
        let accent = chamber.accent;

        let view_id = chamber.view_id(f.mode);
        // Chips that change this view come first. Chips that open another view wait below the
        // parameters under their own label, so none of them looks like an edit to what is on
        // screen and then navigates away.
        let (here, elsewhere): (Vec<_>, Vec<_>) =
            presets::for_chamber(id).partition(|(_, p)| p.view.is_none_or(|v| v == view_id));
        if !here.is_empty() {
            self.preset_chips(ui, &here, f.module);
            ui.add_space(8.0);
        }

        let known = guide(id, view_id).map(|g| g.params);
        let total = self.engine.modules[f.module].params_ref().items.len();
        let shows = |key: &str| known.is_none_or(|keys| keys.contains(&key));
        let relevant: Vec<usize> = (0..total)
            .filter(|&i| shows(self.engine.modules[f.module].params_ref().items[i].key))
            .collect();
        let other: Vec<usize> = (0..total).filter(|i| !relevant.contains(i)).collect();

        let can_randomize = relevant.iter().any(|&i| {
            !matches!(
                self.engine.modules[f.module].params_ref().items[i].kind,
                ParamKind::Toggle
            )
        });
        if relevant.is_empty() {
            let line = if total == 0 {
                "This chamber has no parameters. Move through time or around the view instead."
            } else {
                "No parameter changes this view. Move through time or around it instead."
            };
            reading(ui, line, MOON);
        } else {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let randomize = can_randomize
                    && widgets::text_button(
                        ui,
                        "Randomize",
                        "New values for this view's parameters (R)",
                        false,
                    )
                    .clicked();
                if randomize {
                    self.act(Action::Randomize);
                }
                if widgets::text_button(
                    ui,
                    "Reset",
                    "Back to this chamber's starting values",
                    false,
                )
                .clicked()
                {
                    self.act(Action::ResetParams);
                }
            });
            ui.add_space(4.0);
            self.param_list(ui, &relevant, f.module, accent);
        }

        if !elsewhere.is_empty() {
            ui.add_space(8.0);
            ui.label(RichText::new("Opens another view").size(12.0).color(DUSK));
            ui.add_space(3.0);
            self.preset_chips(ui, &elsewhere, f.module);
        }

        if !other.is_empty() {
            ui.add_space(6.0);
            let open = self.show_other_params;
            let label = format!("Settings for other views ({})", other.len());
            let resp = ui
                .horizontal(|ui| {
                    let (r, resp) =
                        ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                    let p = ui.painter();
                    let mark = Rect::from_center_size(
                        pos2(r.left() + 5.0, r.center().y),
                        egui::Vec2::splat(8.0),
                    );
                    if open {
                        icon::chevron_down(p, mark, DUSK);
                    } else {
                        icon::chevron_right(p, mark, DUSK);
                    }
                    let color = if resp.hovered() { LIGHT } else { DUSK };
                    p.text(
                        pos2(r.left() + 16.0, r.center().y),
                        egui::Align2::LEFT_CENTER,
                        label,
                        theme::ui_font(12.5),
                        color,
                    );
                    resp
                })
                .inner;
            if resp.clicked() {
                self.show_other_params = !open;
            }
            if open {
                self.param_list(ui, &other, f.module, accent);
            }
        }
    }

    fn preset_chips(
        &mut self,
        ui: &mut egui::Ui,
        group: &[(usize, &presets::Preset)],
        module: usize,
    ) {
        let catalog = self.catalog.clone();
        let chamber = &catalog[module];
        let view_id = chamber.view_id(self.engine.focused().mode);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
            for &(i, p) in group {
                let here = p.view.is_none_or(|v| v == view_id);
                let active = here && p.is_active(self.engine.modules[module].params_ref());
                let opens = p
                    .view
                    .filter(|_| !here)
                    .and_then(|v| chamber.modes.iter().find(|m| m.id == v));
                let hint = match opens {
                    Some(m) => format!("{} Opens {}.", p.note, m.name),
                    None => p.note.to_string(),
                };
                if widgets::chip(ui, p.name, active, chamber.accent)
                    .on_hover_text(hint)
                    .clicked()
                {
                    self.act(Action::Preset(i));
                }
            }
        });
    }

    fn param_list(&mut self, ui: &mut egui::Ui, items: &[usize], module: usize, accent: Color32) {
        let mut last_group = "";
        for &i in items {
            let group = self.engine.modules[module].params_ref().items[i].group;
            if group != last_group {
                last_group = group;
                ui.add_space(6.0);
                ui.label(RichText::new(group).size(12.0).color(DUSK));
            }
            let key = self.engine.modules[module].params_ref().items[i].key;
            let sweeping = self.engine.is_sweeping(module, key);
            let can_sweep = presets::can_sweep(self.catalog[module].info.id, key);
            let toggle = param_row(
                ui,
                &mut self.engine.modules[module].params().items[i],
                accent,
                sweeping,
                can_sweep,
            );
            if toggle {
                self.act(Action::ToggleSweep(module, key));
            }
        }
    }

    // ---------- go next ----------

    fn next_section(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let catalog = self.catalog.clone();
        let id = catalog[f.module].info.id;

        if let Some((t, step)) = self.trail {
            let trail = &TRAILS[t];
            if let Some(next) = trail.steps.get(step + 1) {
                if let Some(v) = self.resolve(next.module, Some(next.view)) {
                    let c = &catalog[v.module];
                    let title = format!("Next on the path: {}", c.view_name(v.mode));
                    if link_row(ui, c.accent, &title, next.say) {
                        self.act(Action::TrailStep(1));
                    }
                    ui.add_space(4.0);
                }
            }
        } else if f.mode + 1 < catalog[f.module].modes.len() {
            let c = &catalog[f.module];
            let next = f.mode + 1;
            let title = format!("Next view: {}", c.view_name(next));
            if link_row(ui, c.accent, &title, c.modes[next].desc) {
                self.act(Action::NextStep);
            }
            ui.add_space(4.0);
        }

        self.neighbours(ui, "Builds on", links::builds_on(id));
        self.neighbours(ui, "Leads to", links::leads_to(id));
        self.neighbours(ui, "Related", links::related_to(id));

        let paths: Vec<usize> = trails::through(id).map(|(i, _)| i).collect();
        if !paths.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Guided paths").size(12.0).color(DUSK));
            for t in paths {
                if link_row(ui, LIGHT, TRAILS[t].name, TRAILS[t].summary) {
                    self.act(Action::StartTrail(t));
                }
            }
        }
    }

    fn neighbours(&mut self, ui: &mut egui::Ui, heading: &str, list: Vec<Neighbour>) {
        if list.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label(RichText::new(heading).size(12.0).color(DUSK));
        let catalog = self.catalog.clone();
        for n in list {
            let Some(v) = self.resolve(n.chamber, n.view) else {
                continue;
            };
            let c = &catalog[v.module];
            if link_row(ui, c.accent, c.info.name, n.why) {
                self.act(Action::Go(v));
            }
        }
    }

    // ---------- formulas ----------

    fn formula_section(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let catalog = self.catalog.clone();
        let chamber = &catalog[f.module];
        let accent = chamber.accent;
        let accent2 = chamber.info.branch.style().accent2;
        let ppp = ui.ctx().pixels_per_point();
        if (ppp - self.math_ppp).abs() > 0.01 {
            self.math_tex.clear();
            self.math_ppp = ppp;
        }
        let lines = self.engine.modules[f.module].inspector(self.engine.timeline.t);
        ui.spacing_mut().item_spacing.y = 4.0;
        for l in lines {
            match l.role {
                InsRole::Title => {
                    ui.label(RichText::new(l.text).size(13.5).color(accent));
                }
                InsRole::Formula => {
                    let mut drawn = false;
                    if let Some(key) = l.tex {
                        let ctx = ui.ctx().clone();
                        let tex = self
                            .math_tex
                            .entry(key)
                            .or_insert_with(|| load_math_texture(&ctx, key, ppp));
                        if let Some(tex) = tex {
                            let mut size = tex.size_vec2() / ppp;
                            let room = ui.available_width() - 2.0;
                            if size.x > room {
                                size *= room / size.x;
                            }
                            ui.add_space(3.0);
                            ui.add(egui::Image::new((tex.id(), size)).tint(LIGHT));
                            ui.add_space(3.0);
                            drawn = true;
                        }
                    }
                    if !drawn {
                        ui.label(
                            RichText::new(l.text)
                                .font(theme::math_font(14.5))
                                .color(LIGHT),
                        );
                    }
                }
                InsRole::Def => {
                    ui.label(
                        RichText::new(l.text)
                            .font(theme::math_font(12.5))
                            .color(MOON),
                    );
                }
                InsRole::Value => {
                    ui.label(
                        RichText::new(l.text)
                            .font(theme::math_font(12.5))
                            .color(accent2),
                    );
                }
                InsRole::Note => {
                    ui.add_space(2.0);
                    ui.label(RichText::new(l.text).size(12.0).color(DUSK));
                }
            }
        }
    }

    // ---------- about ----------

    fn about_section(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        let style = chamber.info.branch.style();
        let accent = chamber.accent;
        let (name, tagline, description, epigraph) = (
            chamber.info.name,
            chamber.info.tagline,
            chamber.info.description,
            chamber.info.epigraph,
        );
        ui.label(RichText::new(name).size(13.5).color(accent));
        ui.label(
            RichText::new(format!("{}: {}", style.name, style.motto))
                .size(12.0)
                .color(DUSK),
        );
        ui.add_space(4.0);
        reading(ui, tagline, MOON);
        ui.add_space(4.0);
        reading(ui, description, MOON);
        if let Some(line) = epigraph {
            let text = if line.verbatim {
                format!("\u{201C}{}\u{201D}", line.text)
            } else {
                line.text.to_string()
            };
            ui.add_space(8.0);
            ui.label(
                RichText::new(text)
                    .font(theme::heading_font(18.0))
                    .color(LIGHT),
            );
            ui.label(RichText::new(line.source).size(12.0).color(DUSK));
        }
    }

    // ---------- saved ----------

    fn saved_section(&mut self, ui: &mut egui::Ui) {
        let saved: Vec<(usize, String, String)> = self
            .session
            .bookmarks
            .iter()
            .enumerate()
            .map(|(i, b)| (i, b.name.clone(), b.chamber.clone()))
            .collect();
        let catalog = self.catalog.clone();
        for (i, name, chamber_id) in saved {
            let (accent, chamber_name) = self
                .index
                .get(chamber_id.as_str())
                .map_or((DUSK, chamber_id.as_str()), |&m| {
                    (catalog[m].accent, catalog[m].info.name)
                });
            ui.horizontal(|ui| {
                let gap = ui.spacing().item_spacing.x;
                let width = ui.available_width();
                ui.allocate_ui(vec2(width - 28.0 - gap, 44.0), |ui| {
                    if link_row(ui, accent, &name, chamber_name) {
                        self.act(Action::OpenBookmark(i));
                    }
                });
                if widgets::icon_button(ui, icon::close, "Remove", false, accent).clicked() {
                    self.act(Action::DeleteBookmark(i));
                }
            });
        }
    }
}

/// Reading text, in a size and leading that hold up on black.
fn reading(ui: &mut egui::Ui, text: &str, color: Color32) {
    let width = ui.available_width();
    let mut job = egui::text::LayoutJob::single_section(
        text.to_string(),
        egui::TextFormat {
            font_id: theme::ui_font(13.5),
            color,
            line_height: Some(19.0),
            ..Default::default()
        },
    );
    job.wrap.max_width = width;
    let galley = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    ui.painter().galley(rect.min, galley, color);
}

/// A destination: a lit point, a name, and why you would go there.
fn link_row(ui: &mut egui::Ui, dot: Color32, title: &str, why: &str) -> bool {
    let width = ui.available_width();
    let p = ui.painter().clone();
    let title_g = p.layout(title.to_string(), theme::ui_font(13.0), LIGHT, width - 18.0);
    let why_g = p.layout(why.to_string(), theme::ui_font(12.0), DUSK, width - 18.0);
    let height = title_g.size().y + why_g.size().y + 10.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
    if resp.hovered() {
        p.rect_filled(rect, 3, HOVER);
    }
    p.circle_filled(pos2(rect.left() + 6.0, rect.top() + 12.0), 3.0, dot);
    let title_h = title_g.size().y;
    p.galley(pos2(rect.left() + 16.0, rect.top() + 4.0), title_g, LIGHT);
    p.galley(
        pos2(rect.left() + 16.0, rect.top() + 5.0 + title_h),
        why_g,
        DUSK,
    );
    widgets::focus_ring(&p, rect, &resp);
    resp.clicked()
}

/// One parameter. Returns true when its sweep button was pressed.
fn param_row(
    ui: &mut egui::Ui,
    p: &mut Param,
    accent: Color32,
    sweeping: bool,
    can_sweep: bool,
) -> bool {
    let mut toggle_sweep = false;
    let label = p.label;
    match (&p.kind, &mut p.value) {
        (ParamKind::Slider { min, max, unit }, PValue::F(v)) => {
            let (min, max, unit) = (*min, *max, *unit);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new(label).size(13.0).color(MOON));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if can_sweep {
                        toggle_sweep = sweep_button(ui, accent, sweeping);
                    }
                    quiet_field(ui);
                    let span = max - min;
                    let decimals = if span < 2.0 {
                        3
                    } else if span < 20.0 {
                        2
                    } else if span < 200.0 {
                        1
                    } else {
                        0
                    };
                    let suffix = if unit.chars().next().is_some_and(char::is_alphabetic) {
                        format!(" {unit}")
                    } else {
                        unit.to_string()
                    };
                    ui.add(
                        egui::DragValue::new(v)
                            .range(min..=max)
                            .speed(span / 300.0)
                            .fixed_decimals(decimals)
                            .suffix(suffix),
                    );
                });
            });
            let mut f = if max > min {
                (*v - min) / (max - min)
            } else {
                0.0
            };
            if widgets::slider_track(ui, &mut f, accent) {
                *v = min + f * (max - min);
            }
            ui.add_space(4.0);
        }
        (ParamKind::IntSlider { min, max }, PValue::I(v)) => {
            let (min, max) = (*min, *max);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.label(RichText::new(label).size(13.0).color(MOON));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if can_sweep {
                        toggle_sweep = sweep_button(ui, accent, sweeping);
                    }
                    quiet_field(ui);
                    ui.add(egui::DragValue::new(v).range(min..=max).speed(0.1));
                });
            });
            let span = (max - min).max(1) as f64;
            let mut f = (*v - min) as f64 / span;
            if widgets::slider_track(ui, &mut f, accent) {
                *v = min + (f * span).round() as i64;
            }
            ui.add_space(4.0);
        }
        (ParamKind::Toggle, PValue::B(v)) => {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(13.0).color(MOON));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    widgets::switch(ui, v, accent);
                });
            });
            ui.add_space(2.0);
        }
        (ParamKind::Choice(options), PValue::C(v)) => {
            ui.label(RichText::new(label).size(13.0).color(MOON));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
                for (oi, o) in options.iter().enumerate() {
                    if widgets::chip(ui, o, *v == oi, accent).clicked() {
                        *v = oi;
                    }
                }
            });
            ui.add_space(4.0);
        }
        _ => {}
    }
    toggle_sweep
}

fn sweep_button(ui: &mut egui::Ui, accent: Color32, sweeping: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(24.0, 20.0), Sense::click());
    let p = ui.painter();
    if resp.hovered() {
        p.rect_filled(rect, 3, HOVER);
    }
    let color = if sweeping {
        accent
    } else if resp.hovered() {
        LIGHT
    } else {
        DUSK
    };
    if sweeping {
        p.rect_stroke(rect, 3, Stroke::new(1.0, accent), egui::StrokeKind::Inside);
    }
    icon::sweep(p, rect.shrink2(vec2(5.0, 5.0)), color);
    widgets::focus_ring(p, rect, &resp);
    resp.on_hover_text(if sweeping {
        "Stop sweeping; the value stays where it is"
    } else {
        "Sweep this value back and forth across its range"
    })
    .clicked()
}

fn quiet_field(ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.weak_bg_fill = VOID;
    v.widgets.inactive.bg_fill = VOID;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, LIGHT);
    v.widgets.hovered.weak_bg_fill = HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, EDGE);
}

/// Formula images are white ink, 600 dpi. Keep the alpha as a mask, sized
/// exactly for this display, so strokes stay crisp and can take any tint.
fn load_math_texture(
    ctx: &egui::Context,
    key: &'static str,
    ppp: f32,
) -> Option<egui::TextureHandle> {
    let bytes = crate::mathimg::get(key)?;
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let scale = FORMULA_SCALE * ppp;
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    let alpha = image::GrayImage::from_fn(w, h, |x, y| image::Luma([img.get_pixel(x, y)[3]]));
    let small = image::imageops::resize(&alpha, nw, nh, image::imageops::FilterType::Triangle);
    let mut rgba = Vec::with_capacity((nw * nh * 4) as usize);
    for a in small.pixels() {
        rgba.extend_from_slice(&[255, 255, 255, a[0]]);
    }
    let ci = egui::ColorImage::from_rgba_unmultiplied([nw as usize, nh as usize], &rgba);
    Some(ctx.load_texture(format!("math:{key}"), ci, egui::TextureOptions::LINEAR))
}
