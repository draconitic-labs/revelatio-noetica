//! The map of the atlas: every chamber as a star, its views as points around it,
//! prerequisites running left to right, parallels as dashed lines. Stars fill in
//! as they are visited. It is also where guided paths can be seen whole.

use std::collections::HashMap;

use egui::epaint::QuadraticBezierShape;
use egui::{pos2, vec2, Align2, Color32, Order, Pos2, Rect, Sense, Shape, Stroke};

use super::{Action, AtlasApp};
use crate::atlas::links::{self, LinkKind, CHART, LINKS};
use crate::atlas::trails::TRAILS;
use crate::theme::{self, DUSK, HOVER, LIGHT, LINE, MOON, VOID};
use crate::widgets::{self, icon};

/// Where a chamber's name goes relative to its star, in the chart's own terms.
fn label_rect(center: Pos2, size: egui::Vec2, chart: Rect) -> Rect {
    let mut r = Rect::from_min_size(pos2(center.x - size.x * 0.5, center.y + 15.0), size);
    if r.left() < chart.left() {
        r = r.translate(vec2(chart.left() - r.left(), 0.0));
    }
    if r.right() > chart.right() {
        r = r.translate(vec2(chart.right() - r.right(), 0.0));
    }
    r
}

/// Bend each line gently so lines between neighbours in a column don't overlap.
fn control_point(a: Pos2, b: Pos2) -> Pos2 {
    let mid = a + (b - a) * 0.5;
    let d = b - a;
    let normal = vec2(-d.y, d.x) * 0.09;
    mid + normal
}

impl AtlasApp {
    pub(super) fn map_layer(&mut self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        egui::Area::new(egui::Id::new("map"))
            .order(Order::Foreground)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (full, _) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(full, 0, VOID);
                self.map_contents(ui, full);
            });
    }

    fn map_contents(&mut self, ui: &mut egui::Ui, full: Rect) {
        let catalog = self.catalog.clone();
        let p = ui.painter().clone();

        let visited = catalog
            .iter()
            .filter(|c| self.session.has_visited(c.info.id))
            .count();
        let total_views: usize = catalog.iter().map(|c| c.modes.len()).sum();
        let seen_views = self.session.seen.len().min(total_views);

        p.text(
            full.left_top() + vec2(24.0, 14.0),
            Align2::LEFT_TOP,
            "Map of the atlas",
            theme::heading_font(31.0),
            LIGHT,
        );
        p.text(
            full.left_top() + vec2(24.0, 52.0),
            Align2::LEFT_TOP,
            format!(
                "{visited} of {} chambers visited, {seen_views} of {total_views} views seen. Foundations on the left, what they lead to on the right.",
                catalog.len()
            ),
            theme::ui_font(13.0),
            MOON,
        );
        let close_rect = Rect::from_center_size(
            full.right_top() + vec2(-28.0, 28.0),
            egui::Vec2::splat(28.0),
        );
        let close = ui.interact(close_rect, ui.id().with("map-close"), Sense::click());
        widgets::paint_icon_button(ui, close_rect, &close, icon::close, false, LIGHT);
        if close.on_hover_text("Close the map (M or Esc)").clicked() {
            self.act(Action::CloseLayer);
        }

        // The paths column on the right, or below when the window is short of room.
        let paths_w = 236.0;
        let foot = 66.0;
        let chart = Rect::from_min_max(
            pos2(full.left() + 44.0, full.top() + 104.0),
            pos2(full.right() - paths_w - 72.0, full.bottom() - foot - 44.0),
        );
        let paths = Rect::from_min_max(
            pos2(full.right() - paths_w - 20.0, full.top() + 96.0),
            pos2(full.right() - 20.0, full.bottom() - foot),
        );

        let at: HashMap<&str, Pos2> = CHART
            .iter()
            .map(|&(id, x, y)| {
                (
                    id,
                    pos2(
                        chart.left() + x * chart.width(),
                        chart.top() + y * chart.height(),
                    ),
                )
            })
            .collect();

        let mut hovered: Option<&str> = None;
        for &(id, ..) in CHART {
            let r = Rect::from_center_size(at[id], vec2(84.0, 44.0)).translate(vec2(0.0, 10.0));
            if ui.rect_contains_pointer(r) {
                hovered = Some(id);
            }
        }
        let current = catalog[self.engine.active()].info.id;
        let focus = hovered.unwrap_or(current);

        // Lines first, so stars sit on top of them.
        for l in LINKS {
            let (a, b) = (at[l.from], at[l.to]);
            let touches = l.from == focus || l.to == focus;
            let c = if touches {
                self.index.get(focus).map_or(MOON, |&m| catalog[m].accent)
            } else if l.kind == LinkKind::BuildsOn {
                Color32::from_gray(66)
            } else {
                LINE
            };
            let width = if touches { 1.5 } else { 1.0 };
            if l.kind == LinkKind::BuildsOn {
                p.add(QuadraticBezierShape::from_points_stroke(
                    [a, control_point(a, b), b],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(width, c),
                ));
            } else {
                let c = if touches { c } else { Color32::from_gray(58) };
                p.extend(Shape::dashed_line(&[a, b], Stroke::new(width, c), 4.0, 5.0));
            }
        }

        // A guided path, when hovered, drawn as one bright line in step order.
        if let Some(t) = self.map_hover_trail {
            let pts: Vec<Pos2> = TRAILS[t]
                .steps
                .iter()
                .filter_map(|s| at.get(s.module).copied())
                .collect();
            p.add(Shape::line(pts, Stroke::new(2.0, LIGHT)));
        }

        // Stars, each ringed by one point per view.
        let mut go_to: Option<usize> = None;
        for &(id, ..) in CHART {
            let Some(&m) = self.index.get(id) else {
                continue;
            };
            let chamber = &catalog[m];
            let c = at[id];
            let is_current = id == current;
            let is_hovered = hovered == Some(id);
            let has_visited = self.session.has_visited(id);
            let on_path = self
                .map_hover_trail
                .is_some_and(|t| TRAILS[t].steps.iter().any(|s| s.module == id));
            if is_hovered || on_path {
                p.circle_filled(c, 15.0, chamber.accent.gamma_multiply(0.18));
            }
            let n = chamber.modes.len();
            for (i, mode) in chamber.modes.iter().enumerate() {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / n as f32;
                let q = c + 11.0 * vec2(a.cos(), a.sin());
                if self.session.has_seen(id, mode.id) {
                    p.circle_filled(q, 1.5, chamber.accent);
                } else {
                    p.circle_filled(q, 1.2, chamber.accent.gamma_multiply(0.42));
                }
            }
            super::nav::star(&p, c, chamber.accent, has_visited, is_current);

            let color = if is_current || is_hovered {
                LIGHT
            } else {
                MOON
            };
            let g = p.layout_no_wrap(chamber.info.name.to_string(), theme::ui_font(12.5), color);
            let r = label_rect(c, g.size(), chart.expand(40.0));
            p.galley(r.min, g, color);
            if is_hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                if ui.input(|i| i.pointer.primary_clicked()) {
                    go_to = Some(m);
                }
            }
        }
        if let Some(m) = go_to {
            self.act(Action::GoChamber(m));
        }

        // The foot: what the star under the pointer is, and why it connects.
        let foot_rect = Rect::from_min_max(
            pos2(full.left() + 24.0, full.bottom() - foot - 6.0),
            pos2(paths.left() - 24.0, full.bottom() - 8.0),
        );
        if let Some(&m) = self.index.get(focus) {
            let chamber = &catalog[m];
            let lead = format!("{}. {}", chamber.info.name, chamber.info.tagline);
            let lead_g = widgets::elided(&p, &lead, theme::ui_font(13.5), LIGHT, foot_rect.width());
            let y = foot_rect.top();
            p.galley(foot_rect.left_top(), lead_g, LIGHT);
            let before: Vec<&str> = links::builds_on(focus)
                .iter()
                .filter_map(|n| self.index.get(n.chamber).map(|&i| catalog[i].info.name))
                .collect();
            let after: Vec<&str> = links::leads_to(focus)
                .iter()
                .filter_map(|n| self.index.get(n.chamber).map(|&i| catalog[i].info.name))
                .collect();
            let mut parts = Vec::new();
            if !before.is_empty() {
                parts.push(format!("Builds on {}.", before.join(", ")));
            }
            if !after.is_empty() {
                parts.push(format!("Leads to {}.", after.join(", ")));
            }
            if parts.is_empty() {
                parts.push("Connected by parallels, shown dashed.".to_string());
            }
            let rest = p.layout(
                parts.join(" "),
                theme::ui_font(12.5),
                MOON,
                foot_rect.width(),
            );
            p.galley(pos2(foot_rect.left(), y + 20.0), rest, MOON);
        }

        self.map_paths(ui, paths);
    }

    fn map_paths(&mut self, ui: &mut egui::Ui, area: Rect) {
        let p = ui.painter().clone();
        p.text(
            area.left_top(),
            Align2::LEFT_TOP,
            "Guided paths",
            theme::heading_font(21.0),
            LIGHT,
        );
        let mut y = area.top() + 32.0;
        let mut hover_trail = None;
        for (i, t) in TRAILS.iter().enumerate() {
            let title = p.layout(
                t.name.to_string(),
                theme::ui_font(13.0),
                LIGHT,
                area.width() - 8.0,
            );
            let sub = p.layout(
                t.summary.to_string(),
                theme::ui_font(12.0),
                DUSK,
                area.width() - 8.0,
            );
            let h = title.size().y + sub.size().y + 12.0;
            if y + h > area.bottom() {
                break;
            }
            let r = Rect::from_min_size(pos2(area.left() - 6.0, y), vec2(area.width() + 6.0, h));
            let resp = ui.interact(r, ui.id().with(("map-trail", i)), Sense::click());
            if resp.hovered() {
                p.rect_filled(r, 3, HOVER);
                hover_trail = Some(i);
            }
            let title_h = title.size().y;
            p.galley(pos2(area.left(), y + 4.0), title, LIGHT);
            p.galley(pos2(area.left(), y + 5.0 + title_h), sub, DUSK);
            if resp.clicked() {
                self.act(Action::StartTrail(i));
            }
            y += h + 4.0;
        }
        self.map_hover_trail = hover_trail;
    }
}
