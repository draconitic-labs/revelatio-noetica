//! The interface's small parts: quiet text buttons, chips, a parameter slider,
//! a time line, and icons drawn as vectors so they stay crisp at any scale.

use std::sync::Arc;

use egui::text::{LayoutJob, TextWrapping};
use egui::{
    pos2, vec2, Color32, CornerRadius, CursorIcon, Galley, Painter, Pos2, Rect, Response, Sense,
    Shape, Stroke, StrokeKind, Ui, Vec2,
};

use crate::theme::{self, DUSK, EDGE, HOVER, LIGHT, MOON, PRESS};

/// One line of text, cut with an ellipsis if it would pass `max_width`.
pub fn elided(
    painter: &Painter,
    text: &str,
    font: egui::FontId,
    color: Color32,
    max_width: f32,
) -> Arc<Galley> {
    let mut job = LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = TextWrapping {
        max_width: max_width.max(1.0),
        max_rows: 1,
        break_anywhere: true,
        overflow_character: Some('\u{2026}'),
    };
    painter.layout_job(job)
}

pub fn focus_ring(painter: &Painter, rect: Rect, resp: &Response) {
    if resp.has_focus() {
        painter.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(1.5, LIGHT),
            StrokeKind::Outside,
        );
    }
}

/// A text-only button: the label is the affordance, the fill appears on hover.
pub fn text_button(ui: &mut Ui, label: &str, hint: &str, on: bool) -> Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_string(),
        theme::ui_font(13.0),
        if on { LIGHT } else { MOON },
    );
    let size = vec2(galley.size().x + 16.0, 26.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, CornerRadius::same(3), PRESS);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), HOVER);
    } else if on {
        p.rect_stroke(
            rect,
            CornerRadius::same(3),
            Stroke::new(1.0, EDGE),
            StrokeKind::Inside,
        );
    }
    let color = if resp.hovered() || on { LIGHT } else { MOON };
    p.galley_with_override_text_color(
        pos2(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    focus_ring(p, rect, &resp);
    resp.on_hover_text(hint)
}

/// A small outlined choice. Selected chips glow in the branch light.
pub fn chip(ui: &mut Ui, label: &str, selected: bool, accent: Color32) -> Response {
    let color = if selected { accent } else { MOON };
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), theme::ui_font(12.5), color);
    let size = vec2(galley.size().x + 16.0, 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, CornerRadius::same(3), PRESS);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), HOVER);
    }
    let stroke = if selected {
        accent
    } else if resp.hovered() {
        DUSK
    } else {
        EDGE
    };
    p.rect_stroke(
        rect,
        CornerRadius::same(3),
        Stroke::new(1.0, stroke),
        StrokeKind::Inside,
    );
    let text = if resp.hovered() && !selected {
        LIGHT
    } else {
        color
    };
    p.galley_with_override_text_color(
        pos2(rect.left() + 8.0, rect.center().y - galley.size().y * 0.5),
        galley,
        text,
    );
    focus_ring(p, rect, &resp);
    resp
}

/// An icon-only button. Always give it a tooltip.
pub fn icon_button(
    ui: &mut Ui,
    icon: fn(&Painter, Rect, Color32),
    hint: &str,
    on: bool,
    accent: Color32,
) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::click());
    paint_icon_button(ui, rect, &resp, icon, on, accent);
    resp.on_hover_text(hint)
}

pub fn paint_icon_button(
    ui: &Ui,
    rect: Rect,
    resp: &Response,
    icon: fn(&Painter, Rect, Color32),
    on: bool,
    accent: Color32,
) {
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, CornerRadius::same(3), PRESS);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), HOVER);
    }
    let color = if on {
        accent
    } else if resp.hovered() {
        LIGHT
    } else {
        MOON
    };
    icon(p, rect.shrink(6.0), color);
    focus_ring(p, rect, resp);
}

/// Thin horizontal slider. Returns true if the value changed.
pub fn slider_track(ui: &mut Ui, fraction: &mut f64, accent: Color32) -> bool {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 16.0), Sense::click_and_drag());
    let mut changed = false;
    let rail = rect.shrink2(vec2(5.0, 0.0));
    if resp.dragged() || resp.is_pointer_button_down_on() {
        if let Some(pos) = resp.interact_pointer_pos() {
            let f = ((pos.x - rail.left()) / rail.width().max(1.0)).clamp(0.0, 1.0) as f64;
            if (f - *fraction).abs() > f64::EPSILON {
                *fraction = f;
                changed = true;
            }
        }
    }
    if resp.has_focus() {
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(
                resp.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    ..Default::default()
                },
            );
        });
        let step = ui.input(|i| {
            let big = i.modifiers.shift;
            let unit = if big { 0.1 } else { 0.01 };
            let mut s = 0.0;
            if i.key_pressed(egui::Key::ArrowRight) {
                s += unit;
            }
            if i.key_pressed(egui::Key::ArrowLeft) {
                s -= unit;
            }
            s
        });
        if step != 0.0 {
            *fraction = (*fraction + step).clamp(0.0, 1.0);
            changed = true;
        }
    }
    let p = ui.painter();
    let y = rect.center().y;
    let x = rail.left() + rail.width() * *fraction as f32;
    p.line_segment(
        [pos2(rail.left(), y), pos2(rail.right(), y)],
        Stroke::new(2.0, EDGE),
    );
    p.line_segment([pos2(rail.left(), y), pos2(x, y)], Stroke::new(2.0, accent));
    let active = resp.hovered() || resp.dragged();
    p.circle_filled(pos2(x, y), if active { 5.5 } else { 4.5 }, accent);
    if resp.has_focus() {
        p.circle_stroke(pos2(x, y), 7.5, Stroke::new(1.5, LIGHT));
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
    }
    changed
}

/// A two-state switch.
pub fn switch(ui: &mut Ui, on: &mut bool, accent: Color32) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(30.0, 16.0), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_with_time(resp.id, *on, 0.1);
    let p = ui.painter();
    let radius = rect.height() * 0.5;
    if *on {
        p.rect_filled(rect, CornerRadius::same(8), accent.gamma_multiply(0.35));
    }
    p.rect_stroke(
        rect,
        CornerRadius::same(8),
        Stroke::new(1.0, if *on { accent } else { EDGE }),
        StrokeKind::Inside,
    );
    let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
    p.circle_filled(
        pos2(x, rect.center().y),
        radius - 3.5,
        if *on { accent } else { MOON },
    );
    focus_ring(p, rect, &resp);
    resp
}

/// Column-free section heading in the display face, with a disclosure mark.
pub fn section_header(ui: &mut Ui, title: &str, open: bool) -> Response {
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 30.0), Sense::click());
    let p = ui.painter();
    let color = if resp.hovered() { LIGHT } else { theme::LIGHT };
    let galley = p.layout_no_wrap(title.to_string(), theme::heading_font(21.0), color);
    p.galley(
        pos2(rect.left(), rect.center().y - galley.size().y * 0.5),
        galley,
        color,
    );
    let mark = Rect::from_center_size(pos2(rect.right() - 7.0, rect.center().y), Vec2::splat(9.0));
    if open {
        icon::chevron_down(p, mark, if resp.hovered() { LIGHT } else { DUSK });
    } else {
        icon::chevron_right(p, mark, if resp.hovered() { LIGHT } else { DUSK });
    }
    focus_ring(p, rect, &resp);
    resp
}

/// Vector icons. Each draws inside `r`, using `c` for its one colour.
pub mod icon {
    use super::*;

    fn s(c: Color32) -> Stroke {
        Stroke::new(1.4, c)
    }

    fn line(p: &Painter, c: Color32, pts: &[Pos2]) {
        p.add(Shape::line(pts.to_vec(), s(c)));
    }

    fn at(r: Rect, x: f32, y: f32) -> Pos2 {
        pos2(r.left() + x * r.width(), r.top() + y * r.height())
    }

    pub fn play(p: &Painter, r: Rect, c: Color32) {
        p.add(Shape::convex_polygon(
            vec![at(r, 0.18, 0.05), at(r, 0.92, 0.5), at(r, 0.18, 0.95)],
            c,
            Stroke::NONE,
        ));
    }

    pub fn pause(p: &Painter, r: Rect, c: Color32) {
        for (a, b) in [(0.18, 0.40), (0.60, 0.82)] {
            p.rect_filled(
                Rect::from_min_max(at(r, a, 0.06), at(r, b, 0.94)),
                CornerRadius::ZERO,
                c,
            );
        }
    }

    pub fn restart(p: &Painter, r: Rect, c: Color32) {
        let center = r.center();
        let radius = r.width() * 0.46;
        let arc: Vec<Pos2> = (0..=28)
            .map(|i| {
                let a = -0.55 + (i as f32 / 28.0) * 5.3;
                center + radius * vec2(a.cos(), a.sin())
            })
            .collect();
        let tip = arc[0];
        p.add(Shape::line(arc, s(c)));
        line(p, c, &[tip + vec2(-4.0, -3.5), tip, tip + vec2(3.8, -3.0)]);
    }

    pub fn back(p: &Painter, r: Rect, c: Color32) {
        line(
            p,
            c,
            &[at(r, 0.68, 0.08), at(r, 0.26, 0.5), at(r, 0.68, 0.92)],
        );
    }

    pub fn forward(p: &Painter, r: Rect, c: Color32) {
        line(
            p,
            c,
            &[at(r, 0.32, 0.08), at(r, 0.74, 0.5), at(r, 0.32, 0.92)],
        );
    }

    pub fn chevron_right(p: &Painter, r: Rect, c: Color32) {
        forward(p, r, c);
    }

    pub fn chevron_down(p: &Painter, r: Rect, c: Color32) {
        line(
            p,
            c,
            &[at(r, 0.08, 0.32), at(r, 0.5, 0.74), at(r, 0.92, 0.32)],
        );
    }

    pub fn close(p: &Painter, r: Rect, c: Color32) {
        line(p, c, &[at(r, 0.12, 0.12), at(r, 0.88, 0.88)]);
        line(p, c, &[at(r, 0.88, 0.12), at(r, 0.12, 0.88)]);
    }

    pub fn left_panel(p: &Painter, r: Rect, c: Color32) {
        p.rect_stroke(r, CornerRadius::same(2), s(c), StrokeKind::Inside);
        line(p, c, &[at(r, 0.36, 0.0), at(r, 0.36, 1.0)]);
    }

    pub fn right_panel(p: &Painter, r: Rect, c: Color32) {
        p.rect_stroke(r, CornerRadius::same(2), s(c), StrokeKind::Inside);
        line(p, c, &[at(r, 0.64, 0.0), at(r, 0.64, 1.0)]);
    }

    pub fn search(p: &Painter, r: Rect, c: Color32) {
        let lens = at(r, 0.42, 0.42);
        p.circle_stroke(lens, r.width() * 0.34, s(c));
        line(p, c, &[at(r, 0.68, 0.68), at(r, 0.98, 0.98)]);
    }

    /// Three stars and the lines between them.
    pub fn chart(p: &Painter, r: Rect, c: Color32) {
        let (a, b, d) = (at(r, 0.1, 0.72), at(r, 0.48, 0.18), at(r, 0.92, 0.66));
        line(p, c.gamma_multiply(0.7), &[a, b, d]);
        for q in [a, b, d] {
            p.circle_filled(q, 2.2, c);
        }
    }

    fn star_points(r: Rect) -> Vec<Pos2> {
        let center = r.center();
        (0..10)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                let radius = r.width() * if i % 2 == 0 { 0.52 } else { 0.22 };
                center + radius * vec2(a.cos(), a.sin())
            })
            .collect()
    }

    pub fn star(p: &Painter, r: Rect, c: Color32) {
        let mut pts = star_points(r);
        pts.push(pts[0]);
        p.add(Shape::line(pts, s(c)));
    }

    pub fn star_filled(p: &Painter, r: Rect, c: Color32) {
        let pts = star_points(r);
        let center = r.center();
        // Concave outline, so fill it as a fan of triangles.
        for i in 0..pts.len() {
            let tri = vec![center, pts[i], pts[(i + 1) % pts.len()]];
            p.add(Shape::convex_polygon(tri, c, Stroke::NONE));
        }
    }

    /// An open hand: this view responds to the pointer directly.
    pub fn hand(p: &Painter, r: Rect, c: Color32) {
        let st = Stroke::new(1.3, c);
        for (x, top) in [(0.24, 0.32), (0.42, 0.12), (0.60, 0.12), (0.78, 0.30)] {
            p.line_segment([at(r, x, top), at(r, x, 0.62)], st);
        }
        p.line_segment([at(r, 0.24, 0.62), at(r, 0.10, 0.46)], st);
        let palm = vec![
            at(r, 0.24, 0.62),
            at(r, 0.32, 0.96),
            at(r, 0.70, 0.96),
            at(r, 0.78, 0.62),
        ];
        p.add(Shape::line(palm, st));
    }

    /// A sine wave: sweep this parameter back and forth.
    pub fn sweep(p: &Painter, r: Rect, c: Color32) {
        let pts: Vec<Pos2> = (0..=24)
            .map(|i| {
                let f = i as f32 / 24.0;
                at(r, f, 0.5 - 0.42 * (f * std::f32::consts::TAU).sin())
            })
            .collect();
        p.add(Shape::line(pts, s(c)));
    }

    pub fn plus(p: &Painter, r: Rect, c: Color32) {
        line(p, c, &[at(r, 0.5, 0.08), at(r, 0.5, 0.92)]);
        line(p, c, &[at(r, 0.08, 0.5), at(r, 0.92, 0.5)]);
    }
}
