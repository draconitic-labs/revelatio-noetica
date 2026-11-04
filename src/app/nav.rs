//! The navigator: chambers grouped by shelf, with the current chamber opened to
//! its views. Folded, it becomes a rail of one lit point per chamber.

use egui::{pos2, vec2, Align, Color32, Layout, Rect, Sense, Stroke, UiBuilder};

use super::palette::PaletteMode;
use super::{Action, AtlasApp};
use crate::atlas::DOMAINS;
use crate::engine::module::ViewRef;
use crate::theme::{self, DUSK, HOVER, LIGHT, MOON, VOID};
use crate::widgets::{self, icon};

const RAIL: f32 = 44.0;
const OPEN: f32 = 260.0;

impl AtlasApp {
    pub(super) fn navigator(&mut self, ctx: &egui::Context) {
        let width = if self.atlas_open { OPEN } else { RAIL };
        egui::SidePanel::left("navigator")
            .exact_width(width)
            .resizable(false)
            .frame(egui::Frame::new().fill(VOID))
            .show_separator_line(true)
            .show(ctx, |ui| {
                if self.atlas_open {
                    self.open_navigator(ui);
                } else {
                    self.rail(ui);
                }
            });
    }

    fn rail(&mut self, ui: &mut egui::Ui) {
        let accent = self.accent;
        let full = ui.max_rect();
        let mut y = full.top() + 8.0;
        let cx = full.center().x;
        type Glyph = fn(&egui::Painter, Rect, Color32);
        let buttons: [(Glyph, &str, Action); 3] = [
            (icon::left_panel, "Open the atlas (A)", Action::ToggleAtlas),
            (
                icon::search,
                "Search everything (Ctrl K)",
                Action::OpenPalette(PaletteMode::Search),
            ),
            (icon::chart, "Map of the atlas (M)", Action::OpenMap),
        ];
        for (i, (glyph, hint, action)) in buttons.into_iter().enumerate() {
            let r = Rect::from_center_size(pos2(cx, y + 14.0), egui::Vec2::splat(28.0));
            let resp = ui.interact(r, ui.id().with(("rail-btn", i)), Sense::click());
            widgets::paint_icon_button(ui, r, &resp, glyph, false, accent);
            if resp.on_hover_text(hint).clicked() {
                self.act(action);
            }
            y += 30.0;
        }
        y += 8.0;

        let area = Rect::from_min_max(pos2(full.left(), y), full.max);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::top_down(Align::Center)),
        );
        egui::ScrollArea::vertical()
            .id_salt("rail-scroll")
            .auto_shrink([false, false])
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for domain in DOMAINS {
                    for id in domain.chambers {
                        self.rail_point(ui, id);
                    }
                    ui.add_space(9.0);
                }
            });
    }

    fn rail_point(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(&module) = self.index.get(id) else {
            return;
        };
        let catalog = self.catalog.clone();
        let chamber = &catalog[module];
        let current = self.engine.active() == module;
        let visited = self.session.has_visited(id);
        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 21.0), Sense::click());
        let c = rect.center();
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect.shrink2(vec2(6.0, 0.0)), 3, HOVER);
        }
        star(p, c, chamber.accent, visited, current);
        widgets::focus_ring(p, rect.shrink2(vec2(6.0, 0.0)), &resp);
        let hint = format!("{}\n{}", chamber.info.name, chamber.info.tagline);
        if resp.on_hover_text(hint).clicked() {
            self.act(Action::GoChamber(module));
        }
    }

    fn open_navigator(&mut self, ui: &mut egui::Ui) {
        let accent = self.accent;
        let full = ui.max_rect();
        let header = Rect::from_min_size(full.min, vec2(full.width(), 44.0));
        let p = ui.painter();
        let brand = p.layout_no_wrap(
            "Revelatio Noetica".to_string(),
            theme::heading_font(23.0),
            LIGHT,
        );
        p.galley(
            pos2(
                header.left() + 14.0,
                header.center().y - brand.size().y * 0.5,
            ),
            brand,
            LIGHT,
        );
        let fold = Rect::from_center_size(
            pos2(header.right() - 22.0, header.center().y),
            egui::Vec2::splat(28.0),
        );
        let resp = ui.interact(fold, ui.id().with("nav-fold"), Sense::click());
        widgets::paint_icon_button(ui, fold, &resp, icon::left_panel, true, accent);
        if resp.on_hover_text("Fold the atlas (A)").clicked() {
            self.act(Action::ToggleAtlas);
        }

        let actions = Rect::from_min_size(
            pos2(full.left() + 8.0, header.bottom()),
            vec2(full.width() - 16.0, 30.0),
        );
        let mut row = ui.new_child(
            UiBuilder::new()
                .max_rect(actions)
                .layout(Layout::left_to_right(Align::Center)),
        );
        row.spacing_mut().item_spacing.x = 2.0;
        if widgets::text_button(&mut row, "Search", "Search everything (Ctrl K)", false).clicked() {
            self.act(Action::OpenPalette(PaletteMode::Search));
        }
        if widgets::text_button(&mut row, "Map", "Map of the atlas (M)", false).clicked() {
            self.act(Action::OpenMap);
        }

        let list = Rect::from_min_max(pos2(full.left(), actions.bottom() + 4.0), full.max);
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(list)
                .layout(Layout::top_down(Align::Min)),
        );
        egui::ScrollArea::vertical()
            .id_salt("nav-scroll")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for domain in DOMAINS {
                    ui.add_space(8.0);
                    let width = ui.available_width();
                    let (head, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
                    let p = ui.painter();
                    let g = p.layout_no_wrap(domain.name.to_string(), theme::ui_font(12.0), DUSK);
                    p.galley(
                        pos2(head.left() + 14.0, head.center().y - g.size().y * 0.5),
                        g,
                        DUSK,
                    );
                    for id in domain.chambers {
                        self.chamber_row(ui, id);
                    }
                }
                let visited = self
                    .catalog
                    .iter()
                    .filter(|c| self.session.has_visited(c.info.id))
                    .count();
                ui.add_space(14.0);
                let text = format!("{visited} of {} chambers visited", self.catalog.len());
                let width = ui.available_width();
                let (r, _) = ui.allocate_exact_size(vec2(width, 20.0), Sense::hover());
                ui.painter().text(
                    r.left_center() + vec2(14.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    text,
                    theme::ui_font(12.0),
                    DUSK,
                );
                ui.add_space(10.0);
            });
    }

    fn chamber_row(&mut self, ui: &mut egui::Ui, id: &str) {
        let Some(&module) = self.index.get(id) else {
            return;
        };
        let catalog = self.catalog.clone();
        let chamber = &catalog[module];
        let current = self.engine.active() == module;
        let visited = self.session.has_visited(id);
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 27.0), Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect.shrink2(vec2(6.0, 0.0)), 3, HOVER);
        }
        star(
            p,
            pos2(rect.left() + 20.0, rect.center().y),
            chamber.accent,
            visited,
            current,
        );
        let color = if current || resp.hovered() {
            LIGHT
        } else {
            MOON
        };
        let name = widgets::elided(
            p,
            chamber.info.name,
            theme::ui_font(13.0),
            color,
            width - 44.0,
        );
        p.galley(
            pos2(rect.left() + 34.0, rect.center().y - name.size().y * 0.5),
            name,
            color,
        );
        widgets::focus_ring(p, rect.shrink2(vec2(6.0, 0.0)), &resp);
        if resp.on_hover_text(chamber.info.tagline).clicked() {
            self.act(Action::GoChamber(module));
        }
        if !current {
            return;
        }
        // The current chamber opens to its views.
        for mode in 0..chamber.modes.len() {
            let view = ViewRef { module, mode };
            let on = self.engine.focused() == view;
            let open = self.engine.panes.contains(&view);
            let (r, resp) = ui.allocate_exact_size(vec2(width, 23.0), Sense::click());
            let p = ui.painter();
            if resp.hovered() {
                p.rect_filled(r.shrink2(vec2(6.0, 0.0)), 3, HOVER);
            }
            if on || open {
                let x = r.left() + 20.0;
                let col = if on {
                    chamber.accent
                } else {
                    chamber.accent.gamma_multiply(0.5)
                };
                p.line_segment(
                    [pos2(x, r.top() + 3.0), pos2(x, r.bottom() - 3.0)],
                    Stroke::new(1.5, col),
                );
            }
            let c = if on {
                chamber.accent
            } else if resp.hovered() {
                LIGHT
            } else {
                DUSK
            };
            let text = widgets::elided(
                p,
                chamber.view_name(mode),
                theme::ui_font(12.5),
                c,
                width - 50.0,
            );
            p.galley(
                pos2(r.left() + 34.0, r.center().y - text.size().y * 0.5),
                text,
                c,
            );
            widgets::focus_ring(p, r.shrink2(vec2(6.0, 0.0)), &resp);
            let desc = chamber.modes[mode].desc;
            if resp.on_hover_text(desc).clicked() {
                self.act(Action::Go(view));
            }
        }
    }
}

/// A chamber's mark, the same here and on the map: filled once visited, ringed
/// until then, and with a halo while it is the one on screen.
pub(super) fn star(
    p: &egui::Painter,
    c: egui::Pos2,
    accent: Color32,
    visited: bool,
    current: bool,
) {
    if current {
        p.circle_filled(c, 8.5, accent.gamma_multiply(0.24));
    }
    if visited || current {
        p.circle_filled(c, 3.4, accent);
    } else {
        p.circle_stroke(c, 3.0, Stroke::new(1.2, accent.gamma_multiply(0.85)));
    }
}
