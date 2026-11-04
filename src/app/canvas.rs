//! The stage: one hero view, or several side by side, each rendered by the GPU
//! and shown with its gesture hint and its honesty label directly beneath it.

use egui::{pos2, vec2, Color32, CursorIcon, Rect, Sense, Stroke, UiBuilder};

use super::{Action, AtlasApp};
use crate::engine::camera::CamKind;
use crate::engine::module::{PointerEvent as PE, ViewRef};
use crate::theme::{self, DUSK, EDGE, LIGHT, LINE, MOON, VOID};
use crate::widgets::{self, icon};

const GAP: f32 = 10.0;
const HEADER: f32 = 26.0;

impl AtlasApp {
    pub(super) fn central(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(VOID))
            .show(ctx, |ui| {
                let full = ui.max_rect();
                let mut stage = full;
                if !self.focus_mode {
                    let band =
                        Rect::from_min_size(full.min, vec2(full.width(), self.band_height()));
                    self.title_band(ui, band);
                    stage.set_top(band.bottom());
                }
                self.stage(ui, stage);
            });
    }

    fn stage(&mut self, ui: &mut egui::Ui, stage: Rect) {
        let panes = self.engine.panes.clone();
        let stage = if panes.len() > 1 {
            stage.shrink2(vec2(GAP, 0.0))
        } else {
            stage
        };
        match panes.len() {
            0 | 1 => self.pane(ui, stage, 0, panes[0], false),
            2 => {
                let split_x = stage.left() + stage.width() * self.split;
                let left = Rect::from_min_max(stage.min, pos2(split_x - GAP * 0.5, stage.bottom()));
                let right = Rect::from_min_max(pos2(split_x + GAP * 0.5, stage.top()), stage.max);
                self.pane(ui, left, 0, panes[0], true);
                self.pane(ui, right, 1, panes[1], true);
                self.splitter(ui, stage, split_x);
            }
            n => {
                // The focused view is the hero; the others wait in a column.
                let hero_x = stage.left() + stage.width() * 0.64;
                let hero = Rect::from_min_max(stage.min, pos2(hero_x - GAP * 0.5, stage.bottom()));
                self.pane(ui, hero, 0, panes[0], true);
                let others = n - 1;
                let h = (stage.height() - GAP * (others - 1) as f32) / others as f32;
                for (i, &view) in panes.iter().enumerate().skip(1) {
                    let top = stage.top() + (i - 1) as f32 * (h + GAP);
                    let r = Rect::from_min_size(
                        pos2(hero_x + GAP * 0.5, top),
                        vec2(stage.right() - hero_x - GAP * 0.5, h),
                    );
                    self.pane(ui, r, i, view, true);
                }
            }
        }
    }

    fn splitter(&mut self, ui: &mut egui::Ui, stage: Rect, x: f32) {
        let hit = Rect::from_center_size(pos2(x, stage.center().y), vec2(GAP, stage.height()));
        let resp = ui
            .interact(hit, ui.id().with("pane-splitter"), Sense::drag())
            .on_hover_cursor(CursorIcon::ResizeHorizontal);
        if let Some(pos) = resp.interact_pointer_pos() {
            if resp.dragged() {
                self.split = ((pos.x - stage.left()) / stage.width()).clamp(0.25, 0.75);
            }
        }
        if resp.hovered() || resp.dragged() {
            ui.painter().line_segment(
                [pos2(x, stage.top()), pos2(x, stage.bottom())],
                Stroke::new(1.0, EDGE),
            );
        }
    }

    /// The gesture line and the honesty label for a view, as text.
    fn caption_texts(&mut self, view: ViewRef) -> (String, bool, Option<&'static str>) {
        let chamber = &self.catalog[view.module];
        let gesture = chamber.gesture.get(view.mode).copied().flatten();
        let cam = self.engine.camera(view.module, view.mode).kind;
        let camera_hint = match cam {
            CamKind::Plane { .. } => "drag to pan, scroll to zoom",
            CamKind::Orbit { .. } => "drag to orbit, right-drag to pan, scroll to zoom",
        };
        let hint = match gesture {
            Some(g) => format!("{}. Shift-drag to pan, scroll to zoom.", capitalized(g)),
            None => format!("{}.", capitalized(camera_hint)),
        };
        let label = self.engine.modules[view.module].approx(view.mode);
        (hint, gesture.is_some(), label)
    }

    fn pane(&mut self, ui: &mut egui::Ui, rect: Rect, idx: usize, view: ViewRef, framed: bool) {
        let catalog = self.catalog.clone();
        let chamber = &catalog[view.module];
        let accent = chamber.accent;
        let focused = idx == self.engine.focus;

        let mut area = rect;
        if framed {
            let header = Rect::from_min_size(rect.min, vec2(rect.width(), HEADER));
            self.pane_header(ui, header, idx, view, focused);
            area.set_top(header.bottom() + 2.0);
        }

        // Below the image: gesture hint and honesty label, on one line if they fit.
        let (hint, hands_on, label) = self.caption_texts(view);
        let moved = self.engine.camera_moved(view.module, view.mode);
        let reset_w = if moved { 78.0 } else { 0.0 };
        let pad = if framed { 0.0 } else { 16.0 };
        let text_w = (area.width() - pad * 2.0).max(40.0);
        let p = ui.painter().clone();
        let hint_color = if hands_on { accent } else { DUSK };
        let hint_left = if hands_on { 18.0 } else { 0.0 };
        let hint_g = p.layout(
            hint,
            theme::ui_font(12.0),
            hint_color,
            text_w - hint_left - reset_w,
        );
        let label_wide = label.map(|l| p.layout_no_wrap(l.to_string(), theme::ui_font(12.0), DUSK));
        let same_line = label_wide.as_ref().is_some_and(|g| {
            hint_g.rows.len() == 1
                && hint_g.size().x + hint_left + reset_w + g.size().x + 24.0 <= text_w
        });
        let label_g = if same_line {
            label_wide
        } else {
            label.map(|l| p.layout(l.to_string(), theme::ui_font(12.0), DUSK, text_w))
        };
        let mut caption_h = hint_g.size().y + 8.0;
        if !same_line {
            if let Some(g) = &label_g {
                caption_h += g.size().y + 2.0;
            }
        }
        let image = Rect::from_min_max(area.min, pos2(area.right(), area.bottom() - caption_h));

        self.render_pane(ui, image, view, idx);

        let cap_top = image.bottom() + 4.0;
        let x0 = area.left() + pad;
        if hands_on {
            let glyph = Rect::from_min_size(pos2(x0, cap_top + 1.0), egui::Vec2::splat(14.0));
            icon::hand(&p, glyph, accent);
        }
        let hint_h = hint_g.size().y;
        p.galley(pos2(x0 + hint_left, cap_top), hint_g.clone(), hint_color);
        if moved {
            let r = Rect::from_min_size(
                pos2(x0 + hint_left + hint_g.size().x + 8.0, cap_top - 5.0),
                vec2(72.0, 26.0),
            );
            let mut child = ui.new_child(UiBuilder::new().max_rect(r));
            let reset = widgets::text_button(
                &mut child,
                "Reset view",
                "Double-click the canvas also resets it",
                false,
            );
            if reset.clicked() {
                self.act(Action::ResetCamera(view));
            }
        }
        if let Some(g) = label_g {
            let at = if same_line {
                pos2(area.right() - pad - g.size().x, cap_top)
            } else {
                pos2(x0, cap_top + hint_h + 2.0)
            };
            p.galley(at, g, DUSK);
        }
    }

    fn pane_header(
        &mut self,
        ui: &mut egui::Ui,
        header: Rect,
        idx: usize,
        view: ViewRef,
        focused: bool,
    ) {
        let chamber = &self.catalog[view.module];
        let accent = chamber.accent;
        let resp = ui.interact(header, ui.id().with(("pane-head", idx)), Sense::click());
        let p = ui.painter();
        let other_chamber = view.module != self.engine.panes[0].module
            || self.engine.panes.iter().any(|q| q.module != view.module);
        let text = if other_chamber {
            format!("{}, {}", chamber.view_name(view.mode), chamber.info.name)
        } else {
            chamber.view_name(view.mode).to_string()
        };
        let color = if focused { LIGHT } else { MOON };
        let g = widgets::elided(p, &text, theme::ui_font(13.0), color, header.width() - 36.0);
        p.galley(
            pos2(header.left() + 2.0, header.center().y - g.size().y * 0.5),
            g,
            color,
        );
        let line = if focused { accent } else { LINE };
        p.line_segment(
            [header.left_bottom(), header.right_bottom()],
            Stroke::new(if focused { 1.5 } else { 1.0 }, line),
        );
        let close_rect = Rect::from_center_size(
            pos2(header.right() - 12.0, header.center().y),
            egui::Vec2::splat(22.0),
        );
        let close = ui.interact(
            close_rect,
            ui.id().with(("pane-close", idx)),
            Sense::click(),
        );
        widgets::paint_icon_button(ui, close_rect, &close, icon::close, false, accent);
        if close.on_hover_text("Close this view").clicked() {
            self.act(Action::ClosePane(idx));
        } else if resp
            .on_hover_text("Follow this view in the side panel")
            .clicked()
        {
            self.act(Action::FocusPane(idx));
        }
    }

    fn render_pane(&mut self, ui: &mut egui::Ui, rect: Rect, view: ViewRef, idx: usize) {
        let ppp = ui.ctx().pixels_per_point();
        let w = (rect.width() * ppp).round().max(8.0) as u32;
        let h = (rect.height() * ppp).round().max(8.0) as u32;
        let tex = Self::slot_for(
            &mut self.slots,
            &self.render_state,
            &self.engine,
            view,
            (w, h),
        );

        let module = view.module;
        let mode = view.mode;
        let resp = ui.interact(
            rect,
            ui.id().with(("canvas", module, mode)),
            Sense::click_and_drag(),
        );
        let shift = ui.input(|i| i.modifiers.shift);
        let plane = matches!(self.engine.camera(module, mode).kind, CamKind::Plane { .. });
        let claims = plane && self.engine.modules[module].wants_pointer(mode);

        if (resp.clicked() || resp.drag_started()) && idx != self.engine.focus {
            self.act(Action::FocusPane(idx));
        }
        if claims && !shift {
            let mut cam = *self.engine.camera(module, mode);
            cam.aspect = rect.width() / rect.height().max(1.0);
            if let Some(pos) = resp.interact_pointer_pos() {
                let ndc = glam::vec2(
                    (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                    1.0 - (pos.y - rect.top()) / rect.height() * 2.0,
                );
                let w = cam.unproject_plane(ndc);
                if resp.drag_started_by(egui::PointerButton::Primary) || resp.clicked() {
                    self.engine.modules[module].pointer(mode, PE::Down(w));
                }
                if resp.dragged_by(egui::PointerButton::Primary) {
                    self.engine.modules[module].pointer(mode, PE::Drag(w));
                }
                if resp.drag_stopped_by(egui::PointerButton::Primary) || resp.clicked() {
                    self.engine.modules[module].pointer(mode, PE::Up);
                }
            }
        }
        {
            let cam = self.engine.camera(module, mode);
            cam.aspect = rect.width() / rect.height().max(1.0);
            let size = (rect.width(), rect.height());
            if resp.dragged_by(egui::PointerButton::Primary) && (!claims || shift) {
                let d = resp.drag_delta();
                if shift {
                    cam.pan(d.x, d.y, size);
                } else {
                    cam.drag(d.x, d.y, size);
                }
            }
            if resp.dragged_by(egui::PointerButton::Secondary) {
                let d = resp.drag_delta();
                cam.pan(d.x, d.y, size);
            }
            if resp.hovered() {
                let (scroll, pinch) = ui.input(|i| (i.raw_scroll_delta.y, i.zoom_delta()));
                if scroll.abs() > 0.0 {
                    cam.zoom(scroll);
                }
                if (pinch - 1.0).abs() > 1e-4 {
                    cam.zoom(pinch.ln() / 0.0015);
                }
                let cursor = if claims && !shift {
                    CursorIcon::Crosshair
                } else if resp.dragged() {
                    CursorIcon::Grabbing
                } else {
                    CursorIcon::Grab
                };
                ui.ctx().set_cursor_icon(cursor);
            }
        }
        if resp.double_clicked() {
            self.engine.reset_camera(module, mode);
        }

        if let Some(slot) = self.slots.get(&view) {
            self.engine.render_view(view, &slot.target);
        }

        let painter = ui.painter_at(rect);
        painter.image(
            tex,
            rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
        let cam = *self.engine.camera(module, mode);
        let t = self.engine.timeline.t;
        self.engine.modules[module].overlay(&painter, rect, &cam, mode, t);

        // Arriving at a view fades it up from black: it says what changed.
        if !self.session.prefs.reduce_motion {
            let age = self.fade_from.elapsed().as_secs_f32();
            if age < 0.22 {
                let alpha = ((1.0 - age / 0.22) * 255.0) as u8;
                painter.rect_filled(rect, 0, Color32::from_black_alpha(alpha));
                ui.ctx().request_repaint();
            }
        }
    }
}

fn capitalized(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}
