//! The band above the visual: which view this is, which chamber it belongs to,
//! what it shows, and the commands that act on the whole scene.

use egui::{pos2, vec2, Align, Color32, Layout, Rect, Stroke, UiBuilder};

use super::palette::PaletteMode;
use super::{Action, AtlasApp};
use crate::atlas::trails::TRAILS;
use crate::theme::{self, DUSK, EDGE, HOVER, LIGHT, MOON};
use crate::widgets::{self, icon};

pub const BAND: f32 = 68.0;
pub const TRAIL_ROW: f32 = 28.0;

impl AtlasApp {
    pub(super) fn band_height(&self) -> f32 {
        if self.trail.is_some() {
            BAND + TRAIL_ROW
        } else {
            BAND
        }
    }

    pub(super) fn title_band(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let f = self.engine.focused();
        let catalog = self.catalog.clone();
        let chamber = &catalog[f.module];
        let accent = chamber.accent;
        let mode = chamber.modes.get(f.mode);
        let mut top = rect.top();

        if let Some((t, step)) = self.trail {
            let row = Rect::from_min_size(pos2(rect.left(), top), vec2(rect.width(), TRAIL_ROW));
            self.trail_row(ui, row, t, step, accent);
            top = row.bottom();
        }

        // First line: history, title, chamber, then the scene commands on the right.
        let line = Rect::from_min_max(
            pos2(rect.left() + 8.0, top + 4.0),
            pos2(rect.right() - 12.0, top + 42.0),
        );
        let mut cluster_left = line.right();
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(line)
                .layout(Layout::right_to_left(Align::Center)),
        );
        child.spacing_mut().item_spacing.x = 2.0;
        self.scene_commands(&mut child, accent);
        cluster_left = cluster_left.min(child.min_rect().left());

        let mut left_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(Rect::from_min_size(line.min, vec2(60.0, line.height())))
                .layout(Layout::left_to_right(Align::Center)),
        );
        left_ui.spacing_mut().item_spacing.x = 0.0;
        if !self.back.is_empty() || !self.forward.is_empty() {
            let back =
                widgets::icon_button(&mut left_ui, icon::back, "Back (Alt+Left)", false, accent);
            if back.clicked() {
                self.act(Action::Back);
            }
            let fwd = widgets::icon_button(
                &mut left_ui,
                icon::forward,
                "Forward (Alt+Right)",
                false,
                accent,
            );
            if fwd.clicked() {
                self.act(Action::Forward);
            }
        }
        let text_left = left_ui.min_rect().right().max(line.left()) + 8.0;
        let room = (cluster_left - text_left - 16.0).max(40.0);

        let p = ui.painter();
        let name = mode.map_or("", |m| m.name);
        let title = widgets::elided(p, name, theme::heading_font(31.0), LIGHT, room);
        let title_rect = Rect::from_min_size(
            pos2(text_left, line.center().y - title.size().y * 0.5),
            title.size(),
        );
        let title_w = title.size().x;
        p.galley(title_rect.min, title, LIGHT);

        let chamber_room = room - title_w - 12.0;
        if chamber_room > 60.0 {
            let label = widgets::elided(
                p,
                chamber.info.name,
                theme::ui_font(13.0),
                accent,
                chamber_room,
            );
            let at = pos2(
                title_rect.right() + 12.0,
                title_rect.bottom() - label.size().y - 7.0,
            );
            let chamber_rect = Rect::from_min_size(at, label.size());
            p.galley(at, label, accent);
            let resp = ui
                .interact(
                    chamber_rect.expand(3.0),
                    ui.id().with("chamber-link"),
                    egui::Sense::click(),
                )
                .on_hover_text("Show this chamber in the atlas (A)");
            if resp.hovered() {
                ui.painter().line_segment(
                    [
                        chamber_rect.left_bottom() + vec2(0.0, 1.0),
                        chamber_rect.right_bottom() + vec2(0.0, 1.0),
                    ],
                    Stroke::new(1.0, accent),
                );
            }
            if resp.clicked() {
                self.act(Action::ToggleAtlas);
            }
        }

        // Second line: what this view shows, or the path's narration.
        let say = match self.trail {
            Some((t, step)) => TRAILS[t].steps.get(step).map_or("", |s| s.say),
            None => mode.map_or("", |m| m.desc),
        };
        let p = ui.painter();
        let desc = widgets::elided(p, say, theme::ui_font(13.5), MOON, rect.width() - 30.0);
        p.galley(pos2(rect.left() + 16.0, top + 44.0), desc, MOON);
    }

    fn trail_row(&mut self, ui: &mut egui::Ui, row: Rect, t: usize, step: usize, accent: Color32) {
        let trail = &TRAILS[t];
        let area = row.shrink2(vec2(16.0, 0.0));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(area)
                .layout(Layout::right_to_left(Align::Center)),
        );
        child.spacing_mut().item_spacing.x = 2.0;
        if widgets::text_button(&mut child, "Leave path", "Stop following this path", false)
            .clicked()
        {
            self.act(Action::EndTrail);
        }
        let label = if step + 1 == trail.steps.len() {
            "Finish"
        } else {
            "Next"
        };
        if widgets::text_button(&mut child, label, "Next step (N)", false).clicked() {
            self.act(Action::TrailStep(1));
        }
        if step > 0
            && widgets::text_button(&mut child, "Previous", "Previous step (P)", false).clicked()
        {
            self.act(Action::TrailStep(-1));
        }
        let room = child.min_rect().left() - area.left() - 16.0;
        let p = ui.painter();
        let text = format!("{}, step {} of {}", trail.name, step + 1, trail.steps.len());
        let galley = widgets::elided(p, &text, theme::ui_font(13.0), accent, room);
        p.galley(
            pos2(area.left(), area.center().y - galley.size().y * 0.5),
            galley,
            accent,
        );
        // One tick per step: how far along the path this is.
        let y = row.bottom() - 1.0;
        let w = row.width() / trail.steps.len() as f32;
        for i in 0..trail.steps.len() {
            let x0 = row.left() + i as f32 * w + 1.0;
            let color = if i <= step { accent } else { EDGE };
            p.line_segment(
                [pos2(x0, y), pos2(x0 + w - 2.0, y)],
                Stroke::new(1.5, color),
            );
        }
    }

    fn scene_commands(&mut self, ui: &mut egui::Ui, accent: Color32) {
        let panel = widgets::icon_button(
            ui,
            icon::right_panel,
            "Show or hide the side panel (I)",
            self.panel_open,
            accent,
        );
        if panel.clicked() {
            self.act(Action::TogglePanel);
        }

        let export = widgets::text_button(ui, "Export", "Save an image, frames, or a scene", false);
        egui::Popup::menu(&export)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
            .show(|ui| {
                ui.set_min_width(210.0);
                let items: [(&str, Action); 5] = [
                    ("Copy link to this scene", Action::CopyLink),
                    ("Save scene file", Action::SaveScene),
                    (
                        "Save image, 1920\u{00D7}1080",
                        Action::ExportStill(1920, 1080),
                    ),
                    (
                        "Save image, 3840\u{00D7}2160",
                        Action::ExportStill(3840, 2160),
                    ),
                    ("Save 240 frames for video", Action::ExportFrames),
                ];
                for (label, action) in items {
                    if ui.selectable_label(false, label).clicked() {
                        self.act(action);
                    }
                }
            });

        let saved = self.current_bookmark().is_some();
        let star = widgets::icon_button(
            ui,
            if saved { icon::star_filled } else { icon::star },
            if saved {
                "Remove this saved view (B)"
            } else {
                "Save this view (B)"
            },
            saved,
            accent,
        );
        if star.clicked() {
            self.act(Action::ToggleBookmark);
        }

        let comparing = self.engine.panes.len() > 1;
        let compare_hint = if comparing {
            "Back to one view (C)"
        } else {
            "Open another view beside this one (C)"
        };
        if widgets::text_button(ui, "Compare", compare_hint, comparing).clicked() {
            self.act(Action::CompareToggle);
        }
        if widgets::text_button(ui, "Map", "Map of the atlas (M)", false).clicked() {
            self.act(Action::OpenMap);
        }
        self.search_box(ui);
    }

    /// Looks like a search field because that is what it opens.
    fn search_box(&mut self, ui: &mut egui::Ui) {
        let wide = ui.available_width() > 360.0;
        let size = vec2(if wide { 178.0 } else { 88.0 }, 26.0);
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click());
        let p = ui.painter();
        if resp.hovered() {
            p.rect_filled(rect, 3, HOVER);
        }
        p.rect_stroke(
            rect,
            3,
            Stroke::new(1.0, if resp.hovered() { DUSK } else { EDGE }),
            egui::StrokeKind::Inside,
        );
        let glass = Rect::from_center_size(
            pos2(rect.left() + 14.0, rect.center().y),
            egui::Vec2::splat(11.0),
        );
        icon::search(p, glass, MOON);
        let text = p.layout_no_wrap("Search".to_string(), theme::ui_font(13.0), MOON);
        p.galley(
            pos2(rect.left() + 26.0, rect.center().y - text.size().y * 0.5),
            text,
            MOON,
        );
        if wide {
            let keys = p.layout_no_wrap("Ctrl K".to_string(), theme::ui_font(12.0), DUSK);
            p.galley(
                pos2(
                    rect.right() - keys.size().x - 8.0,
                    rect.center().y - keys.size().y * 0.5,
                ),
                keys,
                DUSK,
            );
        }
        widgets::focus_ring(p, rect, &resp);
        if resp
            .on_hover_text("Search every chamber, view, preset, and command")
            .clicked()
        {
            self.act(Action::OpenPalette(PaletteMode::Search));
        }
    }
}
