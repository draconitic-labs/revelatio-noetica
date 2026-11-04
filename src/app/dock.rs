//! Under the visual: the time line, the transport, and the filmstrip of every
//! view in this chamber, as small live renders.

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{pos2, vec2, Color32, CursorIcon, Rect, Sense, Shape, Stroke, StrokeKind};

use super::{Action, AtlasApp};
use crate::engine::module::ViewRef;
use crate::engine::timeline::LoopMode;
use crate::theme::{self, DUSK, EDGE, LIGHT, MOON, VOID};
use crate::widgets::{self, icon};

const THUMB_SIZE: (u32, u32) = (320, 180);
/// Frames between thumbnail refreshes once every view has been drawn once.
const THUMB_EVERY: u64 = 20;

impl AtlasApp {
    pub(super) fn dock(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("dock")
            .frame(egui::Frame::new().fill(VOID).inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 8,
            }))
            .resizable(false)
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                self.time_line(ui);
                self.transport(ui);
                if self.session.prefs.film_open {
                    self.filmstrip(ui);
                }
            });
    }

    /// One thin line across the whole stage, lit in the branch colour up to now.
    fn time_line(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let accent = self.catalog[f.module].accent;
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 16.0), Sense::click_and_drag());
        let span = self.engine.timeline.span;
        let rail = rect.shrink2(vec2(6.0, 0.0));
        if let Some(pos) = resp.interact_pointer_pos() {
            if resp.dragged() || resp.is_pointer_button_down_on() {
                let frac = ((pos.x - rail.left()) / rail.width().max(1.0)).clamp(0.0, 1.0);
                self.engine.timeline.seek(frac as f64 * span);
            }
        }
        let t = self.engine.timeline.t.clamp(0.0, span);
        let x = rail.left() + rail.width() * (t / span) as f32;
        let y = rect.center().y;
        let p = ui.painter();
        p.line_segment(
            [pos2(rail.left(), y), pos2(rail.right(), y)],
            Stroke::new(1.0, EDGE),
        );
        let mut mesh = Mesh::default();
        let (top, bottom) = (y - 0.9, y + 0.9);
        let faint = accent.gamma_multiply(0.15);
        for (px, py, c) in [
            (rail.left(), top, faint),
            (x, top, accent),
            (x, bottom, accent),
            (rail.left(), bottom, faint),
        ] {
            mesh.vertices.push(Vertex {
                pos: pos2(px, py),
                uv: WHITE_UV,
                color: c,
            });
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        p.add(Shape::mesh(mesh));
        let lit = resp.hovered() || resp.dragged();
        p.circle_filled(pos2(x, y), 8.0, accent.gamma_multiply(0.14));
        p.circle_filled(pos2(x, y), if lit { 5.0 } else { 4.0 }, accent);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }
        let hint =
            format!("t = {t:.2} s of {span:.0} s. Drag to scrub. Left and Right step by 0.5 s.");
        resp.on_hover_text(hint);
    }

    fn transport(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let accent = self.catalog[f.module].accent;
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            vec2(width, 32.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.add_space(8.0);
                let playing = self.engine.timeline.playing;
                let play = widgets::icon_button(
                    ui,
                    if playing { icon::pause } else { icon::play },
                    if playing {
                        "Pause (Space)"
                    } else {
                        "Play (Space)"
                    },
                    !playing,
                    accent,
                );
                if play.clicked() {
                    self.act(Action::TogglePlay);
                }
                if widgets::icon_button(ui, icon::restart, "Back to t = 0 (Home)", false, accent)
                    .clicked()
                {
                    self.act(Action::Restart);
                }
                ui.add_space(6.0);
                let t = self.engine.timeline.t;
                ui.label(
                    egui::RichText::new(format!("t = {t:6.2} s"))
                        .font(theme::math_font(13.0))
                        .color(LIGHT),
                );
                ui.add_space(14.0);

                quiet_values(ui);
                let tl = &mut self.engine.timeline;
                ui.label(egui::RichText::new("Speed").size(12.5).color(DUSK));
                ui.add(
                    egui::DragValue::new(&mut tl.speed)
                        .range(0.05..=4.0)
                        .speed(0.01)
                        .fixed_decimals(2)
                        .suffix("\u{00D7}"),
                )
                .on_hover_text("Generating time per real second. Drag or click to type.");
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Span").size(12.5).color(DUSK));
                ui.add(
                    egui::DragValue::new(&mut tl.span)
                        .range(1.0..=120.0)
                        .speed(0.25)
                        .fixed_decimals(0)
                        .suffix(" s"),
                )
                .on_hover_text("Length of the time line");
                if tl.t > tl.span {
                    tl.t = tl.span;
                }
                ui.add_space(10.0);
                for mode in LoopMode::ALL {
                    let on = self.engine.timeline.loop_mode == mode;
                    let color = if on { accent } else { DUSK };
                    let text = egui::RichText::new(mode.label()).size(12.5).color(color);
                    let resp = ui
                        .add(egui::Label::new(text).sense(Sense::click()))
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if resp.clicked() {
                        self.engine.timeline.loop_mode = mode;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    let film = self.session.prefs.film_open;
                    let label = if film { "Hide views" } else { "Show views" };
                    if widgets::text_button(ui, label, "Show or hide the filmstrip (V)", false)
                        .clicked()
                    {
                        self.act(Action::ToggleFilm);
                    }
                    ui.label(
                        egui::RichText::new(format!("{:.0} fps", self.fps))
                            .font(theme::math_font(12.0))
                            .color(DUSK),
                    );
                });
            },
        );
    }

    /// Every view of the focused chamber, always fully visible: thumbnails
    /// shrink to fit, and only scroll if the window is too narrow even then.
    fn filmstrip(&mut self, ui: &mut egui::Ui) {
        let f = self.engine.focused();
        let catalog = self.catalog.clone();
        let chamber = &catalog[f.module];
        let n = chamber.modes.len();
        let gap = 6.0;
        let pad = 10.0;
        let avail = ui.available_width() - pad * 2.0;
        let screen_h = ui.ctx().screen_rect().height();
        let tallest = (screen_h * 0.09).clamp(46.0, 104.0);
        let fit = (avail - gap * (n as f32 - 1.0)) / n as f32;
        let w = fit.clamp(82.0, tallest * 16.0 / 9.0);
        let thumb = vec2(w, w * 9.0 / 16.0);
        let row_h = thumb.y + 36.0;

        egui::ScrollArea::horizontal()
            .id_salt(("film", f.module))
            .auto_shrink([false, true])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    ui.add_space(pad);
                    for mode in 0..n {
                        let view = ViewRef {
                            module: f.module,
                            mode,
                        };
                        self.film_cell(ui, view, thumb, row_h);
                    }
                    ui.add_space(pad);
                });
            });
    }

    fn film_cell(&mut self, ui: &mut egui::Ui, view: ViewRef, thumb: egui::Vec2, row_h: f32) {
        let chamber = &self.catalog[view.module];
        let accent = chamber.accent;
        let name = chamber.view_name(view.mode);
        let desc = chamber.modes[view.mode].desc;
        let hands_on = chamber.gesture.get(view.mode).copied().flatten().is_some();
        let open = self.engine.panes.contains(&view);
        let focused = self.engine.focused() == view;
        let seen = self
            .session
            .has_seen(chamber.info.id, chamber.view_id(view.mode));

        let (cell, resp) = ui.allocate_exact_size(vec2(thumb.x, row_h), Sense::click());
        let image = Rect::from_min_size(cell.min, thumb);
        let p = ui.painter().clone();
        if let Some(slot) = self.thumbs.get(&view) {
            let tint = if focused || resp.hovered() {
                Color32::WHITE
            } else {
                Color32::from_gray(190)
            };
            p.image(
                slot.tex,
                image,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                tint,
            );
        }
        let frame = if focused {
            accent
        } else if open {
            accent.gamma_multiply(0.5)
        } else if resp.hovered() {
            DUSK
        } else {
            theme::LINE
        };
        p.rect_stroke(
            image,
            2,
            Stroke::new(if focused { 1.5 } else { 1.0 }, frame),
            StrokeKind::Inside,
        );
        if hands_on {
            let glyph = Rect::from_min_size(image.min + vec2(4.0, 4.0), egui::Vec2::splat(13.0));
            p.circle_filled(glyph.center(), 9.0, Color32::from_black_alpha(190));
            icon::hand(&p, glyph.shrink(1.0), accent);
        }
        let color = if focused {
            LIGHT
        } else if resp.hovered() || seen {
            MOON
        } else {
            DUSK
        };
        // A long single word may use the gap, rather than break mid-word.
        let text = p.layout(name.to_string(), theme::ui_font(11.5), color, thumb.x + 5.0);
        p.galley(pos2(cell.left(), image.bottom() + 4.0), text, color);

        // The "open beside" mark appears on hover, top right.
        let plus_rect = Rect::from_min_size(
            pos2(image.right() - 22.0, image.top() + 2.0),
            egui::Vec2::splat(20.0),
        );
        let plus = if resp.hovered() || ui.rect_contains_pointer(plus_rect) {
            let plus = ui.interact(
                plus_rect,
                ui.id().with(("film-plus", view.mode)),
                Sense::click(),
            );
            p.circle_filled(plus_rect.center(), 10.0, Color32::from_black_alpha(200));
            icon::plus(
                &p,
                plus_rect.shrink(5.0),
                if plus.hovered() { LIGHT } else { MOON },
            );
            Some(plus.on_hover_text("Open beside the current view"))
        } else {
            None
        };
        let plus_clicked = plus.is_some_and(|r| r.clicked());
        let ctrl = ui.input(|i| i.modifiers.command);
        if plus_clicked || (resp.clicked() && ctrl) {
            self.act(Action::AddPane(view));
        } else if resp.on_hover_text(desc).clicked() {
            self.act(Action::Go(view));
        }
    }

    /// Bring one thumbnail up to date. Fresh chambers fill quickly, then the
    /// strip keeps breathing at a slow, cheap rate.
    pub(super) fn refresh_one_thumbnail(&mut self) {
        let module = self.engine.active();
        let n = self.catalog[module].modes.len();
        let filling = self.thumbs.len() < n;
        if !filling && !self.frame_count.is_multiple_of(THUMB_EVERY) {
            return;
        }
        let view = if filling {
            (0..n)
                .map(|mode| ViewRef { module, mode })
                .find(|v| !self.thumbs.contains_key(v))
        } else {
            self.thumb_turn = (self.thumb_turn + 1) % n;
            Some(ViewRef {
                module,
                mode: self.thumb_turn,
            })
        };
        let Some(view) = view else { return };
        Self::slot_for(
            &mut self.thumbs,
            &self.render_state,
            &self.engine,
            view,
            THUMB_SIZE,
        );
        if let Some(slot) = self.thumbs.get(&view) {
            self.engine.render_view(view, &slot.target);
        }
    }
}

/// Number fields without boxes: only the text, until hovered.
fn quiet_values(ui: &mut egui::Ui) {
    let v = ui.visuals_mut();
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.inactive.weak_bg_fill = VOID;
    v.widgets.inactive.bg_fill = VOID;
    v.widgets.hovered.weak_bg_fill = theme::HOVER;
    v.widgets.hovered.bg_fill = theme::HOVER;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, EDGE);
}
