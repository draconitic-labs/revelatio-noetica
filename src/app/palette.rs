//! The command palette: one search box over every chamber, view, preset, guided
//! path, saved view, and action. It also picks what to compare against.

use egui::text::{LayoutJob, TextFormat};
use egui::{pos2, vec2, Align2, Color32, Key, Modifiers, Order, Rect, Sense, Stroke};

use super::{Action, AtlasApp};
use crate::atlas::guides::guide;
use crate::atlas::links;
use crate::atlas::presets::PRESETS;
use crate::atlas::search::{self, fold};
use crate::atlas::trails::TRAILS;
use crate::engine::module::{ConceptModule, ViewRef};
use crate::engine::params::ParamKind;
use crate::session;
use crate::theme::{self, DUSK, EDGE, HOVER, LIGHT, MOON, VOID};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PaletteMode {
    Search,
    /// Choosing a view to open beside the current one.
    Compare,
}

#[derive(Clone)]
pub struct Item {
    pub title: String,
    pub sub: String,
    pub kind: &'static str,
    pub dot: Option<Color32>,
    pub keys: &'static str,
    pub action: Action,
    pub view: Option<ViewRef>,
    title_folded: String,
    body_folded: String,
}

impl Item {
    fn new(title: String, sub: String, kind: &'static str, body: String, action: Action) -> Self {
        Self {
            title_folded: fold(&title),
            body_folded: fold(&format!("{sub} {body}")),
            title,
            sub,
            kind,
            dot: None,
            keys: "",
            action,
            view: None,
        }
    }
}

pub struct PaletteState {
    pub mode: PaletteMode,
    query: String,
    selected: usize,
    just_opened: bool,
    items: Vec<Item>,
    results: Vec<usize>,
    last_query: Option<String>,
    scroll_to_selected: bool,
}

impl Default for PaletteState {
    fn default() -> Self {
        Self {
            mode: PaletteMode::Search,
            query: String::new(),
            selected: 0,
            just_opened: false,
            items: Vec::new(),
            results: Vec::new(),
            last_query: None,
            scroll_to_selected: false,
        }
    }
}

impl PaletteState {
    pub fn open(&mut self, mode: PaletteMode) {
        self.mode = mode;
        self.query.clear();
        self.selected = 0;
        self.just_opened = true;
        self.last_query = None;
    }

    pub fn close(&mut self, _ctx: &egui::Context) {
        self.mode = PaletteMode::Search;
    }
}

fn param_words(module: &dyn ConceptModule, keys: Option<&[&str]>) -> String {
    let mut out = String::new();
    for p in &module.params_ref().items {
        if keys.is_some_and(|k| !k.contains(&p.key)) {
            continue;
        }
        out.push_str(p.label);
        out.push(' ');
        if let ParamKind::Choice(options) = &p.kind {
            for o in options {
                out.push_str(o);
                out.push(' ');
            }
        }
    }
    out
}

type Command = (&'static str, &'static str, &'static str, fn() -> Action);

const COMMANDS: &[Command] = &[
    (
        "Open the map of the atlas",
        "chart graph connections stars overview",
        "M",
        || Action::OpenMap,
    ),
    (
        "Go to the next step",
        "next concept continue forward path",
        "N",
        || Action::NextStep,
    ),
    ("Go to the previous step", "back path return", "P", || {
        Action::PrevStep
    }),
    (
        "Back",
        "history previous undo navigation",
        "Alt+Left",
        || Action::Back,
    ),
    ("Forward", "history redo navigation", "Alt+Right", || {
        Action::Forward
    }),
    (
        "Compare with another view",
        "side by side beside split two panes",
        "C",
        || Action::CompareToggle,
    ),
    (
        "Randomize this view's parameters",
        "random dice surprise explore",
        "R",
        || Action::Randomize,
    ),
    (
        "Reset this chamber's parameters",
        "defaults undo restore",
        "",
        || Action::ResetParams,
    ),
    (
        "Save this view",
        "bookmark star favourite favorite remember",
        "B",
        || Action::ToggleBookmark,
    ),
    (
        "Copy a link to this scene",
        "share url clipboard send",
        "",
        || Action::CopyLink,
    ),
    ("Save scene file", "share json export", "", || {
        Action::SaveScene
    }),
    (
        "Save image, 1920\u{00D7}1080",
        "export png still screenshot picture hd",
        "",
        || Action::ExportStill(1920, 1080),
    ),
    (
        "Save image, 3840\u{00D7}2160",
        "export png still screenshot picture 4k uhd",
        "",
        || Action::ExportStill(3840, 2160),
    ),
    (
        "Save 240 frames for video",
        "export movie animation sequence ffmpeg",
        "",
        || Action::ExportFrames,
    ),
    ("Play or pause", "time stop start", "Space", || {
        Action::TogglePlay
    }),
    (
        "Back to t = 0",
        "restart rewind time beginning",
        "Home",
        || Action::Restart,
    ),
    (
        "Show or hide the atlas",
        "navigator sidebar left panel",
        "A",
        || Action::ToggleAtlas,
    ),
    (
        "Show or hide the side panel",
        "inspector parameters right panel",
        "I",
        || Action::TogglePanel,
    ),
    (
        "Show or hide the filmstrip",
        "views thumbnails strip",
        "V",
        || Action::ToggleFilm,
    ),
    (
        "Focus on the visual",
        "fullscreen hide chrome presentation",
        "F",
        || Action::ToggleFocus,
    ),
    (
        "Reduce interface motion",
        "animation accessibility fade",
        "",
        || Action::ToggleMotion,
    ),
    (
        "Make the interface larger",
        "zoom scale bigger",
        "Ctrl +",
        || Action::Scale(1.1),
    ),
    (
        "Make the interface smaller",
        "zoom scale",
        "Ctrl \u{2212}",
        || Action::Scale(1.0 / 1.1),
    ),
    (
        "Reset the interface size",
        "zoom scale default",
        "Ctrl 0",
        || Action::Scale(0.0),
    ),
    ("Show keyboard shortcuts", "help keys", "?", || {
        Action::OpenHelp
    }),
];

impl AtlasApp {
    fn build_items(&self) -> Vec<Item> {
        let mut items = Vec::with_capacity(220);
        // Recents come first so an empty box offers a way back.
        for key in &self.session.recent {
            let Some((c, v)) = key.split_once('/') else {
                continue;
            };
            let Some(view) = self.resolve(c, Some(v)) else {
                continue;
            };
            let chamber = &self.catalog[view.module];
            let mut item = Item::new(
                chamber.view_name(view.mode).to_string(),
                chamber.info.name.to_string(),
                "Recent",
                String::new(),
                Action::Go(view),
            );
            item.dot = Some(chamber.accent);
            item.view = Some(view);
            items.push(item);
        }
        for (i, b) in self.session.bookmarks.iter().enumerate() {
            let mut item = Item::new(
                b.name.clone(),
                b.chamber.clone(),
                "Saved",
                String::new(),
                Action::OpenBookmark(i),
            );
            item.dot = self
                .index
                .get(b.chamber.as_str())
                .map(|&m| self.catalog[m].accent);
            items.push(item);
        }
        for (i, t) in TRAILS.iter().enumerate() {
            let body = t.steps.iter().map(|s| s.say).collect::<Vec<_>>().join(" ");
            items.push(Item::new(
                t.name.to_string(),
                t.summary.to_string(),
                "Guided path",
                body,
                Action::StartTrail(i),
            ));
        }
        for (m, chamber) in self.catalog.iter().enumerate() {
            let module = self.engine.modules[m].as_ref();
            let style = chamber.info.branch.style();
            let body = format!(
                "{} {} {} {} {}",
                chamber.info.tagline,
                chamber.info.description,
                style.name,
                style.motto,
                param_words(module, None)
            );
            let mut item = Item::new(
                chamber.info.name.to_string(),
                chamber.info.tagline.to_string(),
                "Chamber",
                body,
                Action::GoChamber(m),
            );
            item.dot = Some(chamber.accent);
            items.push(item);
            for (mode, r) in chamber.modes.iter().enumerate() {
                let g = guide(chamber.info.id, r.id);
                let hands_on = chamber.gesture.get(mode).copied().flatten().is_some();
                let body = format!(
                    "{} {} {} {} {} {}",
                    r.desc,
                    style.name,
                    g.map_or("", |g| g.read),
                    g.map_or("", |g| g.try_this),
                    param_words(module, g.map(|g| g.params)),
                    if hands_on {
                        "drag direct manipulation interactive hands-on"
                    } else {
                        ""
                    },
                );
                let view = ViewRef { module: m, mode };
                let mut item = Item::new(
                    r.name.to_string(),
                    format!("{}: {}", chamber.info.name, r.desc),
                    if hands_on {
                        "View you can drag"
                    } else {
                        "View"
                    },
                    body,
                    Action::Go(view),
                );
                item.dot = Some(chamber.accent);
                item.view = Some(view);
                items.push(item);
            }
        }
        for (i, p) in PRESETS.iter().enumerate() {
            let Some(&m) = self.index.get(p.module) else {
                continue;
            };
            let chamber = &self.catalog[m];
            let mut item = Item::new(
                p.name.to_string(),
                format!("{}: {}", chamber.info.name, p.note),
                "Preset",
                String::new(),
                Action::Preset(i),
            );
            item.dot = Some(chamber.accent);
            items.push(item);
        }
        for &(title, words, keys, make) in COMMANDS {
            let mut item = Item::new(
                title.to_string(),
                String::new(),
                "Action",
                words.to_string(),
                make(),
            );
            item.keys = keys;
            items.push(item);
        }
        items
    }

    /// In compare mode, an empty box suggests what this view pairs with: the
    /// chamber's own suggestion, its other views, then connected chambers.
    fn compare_suggestions(&self) -> Vec<ViewRef> {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        let mut out: Vec<ViewRef> = Vec::new();
        for mode in self.engine.modules[f.module].default_modes() {
            out.push(ViewRef {
                module: f.module,
                mode,
            });
        }
        for mode in 0..chamber.modes.len() {
            out.push(ViewRef {
                module: f.module,
                mode,
            });
        }
        let id = chamber.info.id;
        for n in links::related_to(id)
            .into_iter()
            .chain(links::leads_to(id))
            .chain(links::builds_on(id))
        {
            if let Some(v) = self.resolve(n.chamber, n.view) {
                out.push(v);
            }
        }
        let mut seen = Vec::new();
        out.retain(|v| {
            let fresh = *v != f && !seen.contains(v);
            seen.push(*v);
            fresh
        });
        out
    }

    fn refresh_results(&mut self) {
        let mode = self.palette.mode;
        let query = self.palette.query.clone();
        if self.palette.last_query.as_deref() == Some(query.as_str()) {
            return;
        }
        if self.palette.last_query.is_none() {
            self.palette.items = self.build_items();
        }
        self.palette.last_query = Some(query.clone());
        self.palette.selected = 0;
        let tokens = search::tokens(&query);
        let items = &self.palette.items;
        let eligible = |it: &Item| match mode {
            PaletteMode::Search => tokens.is_empty() || it.kind != "Recent",
            PaletteMode::Compare => it.view.is_some() && it.kind != "Recent",
        };
        let mut results: Vec<usize> = if tokens.is_empty() {
            match mode {
                PaletteMode::Search => (0..items.len()).filter(|&i| eligible(&items[i])).collect(),
                PaletteMode::Compare => self
                    .compare_suggestions()
                    .into_iter()
                    .filter_map(|v| {
                        items
                            .iter()
                            .position(|it| it.view == Some(v) && it.kind != "Recent")
                    })
                    .collect(),
            }
        } else {
            let mut scored: Vec<(i32, usize)> = items
                .iter()
                .enumerate()
                .filter(|(_, it)| eligible(it))
                .filter_map(|(i, it)| {
                    search::score(&tokens, &it.title_folded, &it.body_folded).map(|s| (s, i))
                })
                .collect();
            scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
            scored.into_iter().map(|(_, i)| i).collect()
        };
        results.truncate(80);
        self.palette.results = results;
    }

    /// A pasted link or a path opens directly instead of searching.
    fn direct_open(&self) -> Option<(String, Action)> {
        let q = self.palette.query.trim();
        if session::is_link(q) {
            return Some((
                "Open this scene link".to_string(),
                Action::OpenLink(q.to_string()),
            ));
        }
        if q.ends_with(".json") {
            let path = std::path::PathBuf::from(q);
            return Some((format!("Open scene file {q}"), Action::OpenSceneFile(path)));
        }
        None
    }

    pub(super) fn palette_layer(&mut self, ctx: &egui::Context) {
        self.refresh_results();
        let direct = self.direct_open();
        let count = self.palette.results.len() + usize::from(direct.is_some());
        let mut chosen: Option<Action> = None;

        // Keys the text box must not see.
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::NONE, Key::ArrowDown) && count > 0 {
                self.palette.selected = (self.palette.selected + 1) % count;
                self.palette.scroll_to_selected = true;
            }
            if i.consume_key(Modifiers::NONE, Key::ArrowUp) && count > 0 {
                self.palette.selected = (self.palette.selected + count - 1) % count;
                self.palette.scroll_to_selected = true;
            }
            if i.consume_key(Modifiers::NONE, Key::Enter) {
                chosen = Some(Action::CloseLayer);
                if let Some((_, a)) = &direct {
                    if self.palette.selected == 0 {
                        chosen = Some(a.clone());
                    }
                }
                let offset = usize::from(direct.is_some());
                if let Some(&r) = self
                    .palette
                    .selected
                    .checked_sub(offset)
                    .and_then(|s| self.palette.results.get(s))
                {
                    chosen = Some(self.chosen_action(r));
                }
            }
        });

        let screen = ctx.screen_rect();
        let scrim = egui::Area::new(egui::Id::new("palette-scrim"))
            .order(Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, 0, Color32::from_black_alpha(190));
                resp
            })
            .inner;
        if scrim.clicked() {
            chosen = Some(Action::CloseLayer);
        }

        let width = (screen.width() - 48.0).min(680.0);
        let max_list = (screen.height() * 0.62).max(180.0);
        let at = pos2(
            screen.center().x - width * 0.5,
            screen.top() + screen.height() * 0.09,
        );
        let compare = self.palette.mode == PaletteMode::Compare;
        egui::Area::new(egui::Id::new("palette"))
            .order(Order::Foreground)
            .fixed_pos(at)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(VOID)
                    .stroke(Stroke::new(1.0, EDGE))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::same(0))
                    .show(ui, |ui| {
                        ui.set_width(width);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        let hint = if compare {
                            "Open which view beside this one?"
                        } else {
                            "Search chambers, views, presets, paths, and actions"
                        };
                        ui.add_space(6.0);
                        ui.horizontal(|ui| {
                            ui.add_space(14.0);
                            let edit = ui.add(
                                egui::TextEdit::singleline(&mut self.palette.query)
                                    .id(egui::Id::new("palette-input"))
                                    .frame(false)
                                    .font(theme::ui_font(16.0))
                                    .text_color(LIGHT)
                                    .hint_text(egui::RichText::new(hint).font(theme::ui_font(16.0)).color(DUSK))
                                    .desired_width(width - 28.0)
                                    .margin(egui::Margin::symmetric(0, 8)),
                            );
                            if self.palette.just_opened {
                                edit.request_focus();
                                self.palette.just_opened = false;
                            } else if !edit.has_focus() {
                                edit.request_focus();
                            }
                        });
                        ui.add_space(6.0);
                        let line_y = ui.cursor().top();
                        ui.painter().line_segment(
                            [pos2(at.x, line_y), pos2(at.x + width, line_y)],
                            Stroke::new(1.0, EDGE),
                        );
                        egui::ScrollArea::vertical()
                            .id_salt("palette-list")
                            .max_height(max_list)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                if count == 0 {
                                    ui.add_space(14.0);
                                    ui.horizontal(|ui| {
                                        ui.add_space(14.0);
                                        ui.label(
                                            egui::RichText::new(
                                                "Nothing matches. Try a concept, like wave, prime, or curvature.",
                                            )
                                            .size(13.5)
                                            .color(MOON),
                                        );
                                    });
                                    ui.add_space(14.0);
                                }
                                let tokens = search::tokens(&self.palette.query);
                                let offset = usize::from(direct.is_some());
                                if let Some((title, action)) = &direct {
                                    if self.direct_row(ui, title, self.palette.selected == 0) {
                                        chosen = Some(action.clone());
                                    }
                                }
                                for row in 0..self.palette.results.len() {
                                    let r = self.palette.results[row];
                                    let selected = self.palette.selected == row + offset;
                                    let clicked = self.palette_row(ui, r, selected, &tokens);
                                    if clicked {
                                        chosen = Some(self.chosen_action(r));
                                    }
                                }
                                self.palette.scroll_to_selected = false;
                            });
                        ui.add_space(6.0);
                    });
            });

        if let Some(action) = chosen {
            self.act(action);
        }
    }

    fn chosen_action(&self, item: usize) -> Action {
        let it = &self.palette.items[item];
        match (self.palette.mode, it.view) {
            (PaletteMode::Compare, Some(view)) => Action::AddPane(view),
            _ => it.action.clone(),
        }
    }

    fn direct_row(&self, ui: &mut egui::Ui, title: &str, selected: bool) -> bool {
        let width = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(width, 34.0), Sense::click());
        let p = ui.painter();
        if selected || resp.hovered() {
            p.rect_filled(rect, 0, HOVER);
        }
        p.text(
            rect.left_center() + vec2(14.0, 0.0),
            Align2::LEFT_CENTER,
            title,
            theme::ui_font(13.5),
            LIGHT,
        );
        resp.clicked()
    }

    fn palette_row(
        &mut self,
        ui: &mut egui::Ui,
        r: usize,
        selected: bool,
        tokens: &[String],
    ) -> bool {
        let it = &self.palette.items[r];
        let width = ui.available_width();
        let two_lines = !it.sub.is_empty();
        let height = if two_lines { 46.0 } else { 32.0 };
        let (rect, resp) = ui.allocate_exact_size(vec2(width, height), Sense::click());
        if selected && self.palette.scroll_to_selected {
            ui.scroll_to_rect(rect, None);
        }
        let p = ui.painter();
        if selected || resp.hovered() {
            p.rect_filled(rect, 0, HOVER);
        }
        let accent = it.dot.unwrap_or(LIGHT);
        if selected {
            p.line_segment(
                [rect.left_top(), rect.left_bottom()],
                Stroke::new(2.0, accent),
            );
        }
        if let Some(dot) = it.dot {
            let y = if two_lines {
                rect.top() + 15.0
            } else {
                rect.center().y
            };
            p.circle_filled(pos2(rect.left() + 18.0, y), 3.2, dot);
        }

        let tag = if it.keys.is_empty() { it.kind } else { it.keys };
        let tag_g = p.layout_no_wrap(tag.to_string(), theme::ui_font(12.0), DUSK);
        let tag_w = tag_g.size().x;
        let text_x = rect.left() + 32.0;
        let room = rect.right() - 14.0 - tag_w - 14.0 - text_x;

        let marks = search::highlights(tokens, &it.title);
        let mut job = LayoutJob::default();
        for (i, ch) in it.title.chars().enumerate() {
            let color = if marks.binary_search(&i).is_ok() {
                accent
            } else {
                LIGHT
            };
            job.append(
                ch.encode_utf8(&mut [0u8; 4]),
                0.0,
                TextFormat {
                    font_id: theme::ui_font(13.5),
                    color,
                    ..Default::default()
                },
            );
        }
        job.wrap = egui::text::TextWrapping {
            max_width: room.max(40.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('\u{2026}'),
        };
        let title_g = ui.painter().layout_job(job);
        let top = if two_lines {
            rect.top() + 6.0
        } else {
            rect.center().y - title_g.size().y * 0.5
        };
        p.galley(pos2(text_x, top), title_g, LIGHT);
        p.galley(pos2(rect.right() - 14.0 - tag_w, top + 1.0), tag_g, DUSK);
        if two_lines {
            let sub = crate::widgets::elided(
                p,
                &it.sub,
                theme::ui_font(12.0),
                DUSK,
                rect.right() - 14.0 - text_x,
            );
            p.galley(pos2(text_x, rect.top() + 25.0), sub, DUSK);
        }
        resp.clicked()
    }

    // ---------- help ----------

    pub(super) fn help_layer(&mut self, ctx: &egui::Context) {
        const KEYS: &[(&str, &str)] = &[
            ("Ctrl K or /", "Search everything"),
            ("M", "Map of the atlas"),
            ("N and P", "Next and previous step"),
            ("[ and ]", "Previous and next view"),
            ("Alt+Left, Alt+Right", "Back and forward"),
            ("Space", "Play or pause"),
            ("Left, Right", "Step time by 0.5 s"),
            ("Home", "Back to t = 0"),
            ("C", "Compare with another view"),
            ("B", "Save this view"),
            ("R", "Randomize parameters"),
            ("A", "Fold or open the atlas"),
            ("I", "Show or hide the side panel"),
            ("V", "Show or hide the filmstrip"),
            ("F", "Focus on the visual"),
            ("Ctrl +, Ctrl \u{2212}, Ctrl 0", "Interface size"),
            ("Ctrl V", "Open a copied scene link"),
            ("Esc", "Close, or leave focus"),
        ];
        let screen = ctx.screen_rect();
        let scrim = egui::Area::new(egui::Id::new("help-scrim"))
            .order(Order::Middle)
            .fixed_pos(screen.min)
            .show(ctx, |ui| {
                let (rect, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter()
                    .rect_filled(rect, 0, Color32::from_black_alpha(190));
                resp
            })
            .inner;
        if scrim.clicked() {
            self.act(Action::CloseLayer);
        }
        // Sized so all of it fits in the smallest window the app allows.
        let row_h = ((screen.height() - 150.0) / KEYS.len() as f32).clamp(20.0, 25.0);
        let size = vec2(460.0, KEYS.len() as f32 * row_h + 110.0);
        let rect = Rect::from_center_size(screen.center(), size);
        egui::Area::new(egui::Id::new("help"))
            .order(Order::Foreground)
            .fixed_pos(rect.min)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(VOID)
                    .stroke(Stroke::new(1.0, EDGE))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::same(20))
                    .show(ui, |ui| {
                        ui.set_width(rect.width() - 42.0);
                        ui.label(
                            egui::RichText::new("Keyboard")
                                .font(theme::heading_font(28.0))
                                .color(LIGHT),
                        );
                        ui.add_space(6.0);
                        ui.spacing_mut().item_spacing.y = 0.0;
                        for (key, what) in KEYS {
                            let width = ui.available_width();
                            let (r, _) = ui.allocate_exact_size(vec2(width, row_h), Sense::hover());
                            let p = ui.painter();
                            p.text(
                                r.left_center(),
                                Align2::LEFT_CENTER,
                                what,
                                theme::ui_font(13.5),
                                MOON,
                            );
                            p.text(
                                r.right_center(),
                                Align2::RIGHT_CENTER,
                                key,
                                theme::ui_font(13.0),
                                LIGHT,
                            );
                        }
                    });
            });
    }

    // ---------- toast ----------

    pub(super) fn toast_layer(&mut self, ctx: &egui::Context) {
        let screen = ctx.screen_rect();
        if let Some((done, total)) = self.export.as_ref().map(|j| (j.next, j.total)) {
            egui::Area::new(egui::Id::new("export-progress"))
                .order(Order::Tooltip)
                .anchor(Align2::CENTER_TOP, vec2(0.0, 76.0))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(VOID)
                        .stroke(Stroke::new(1.0, EDGE))
                        .corner_radius(4)
                        .inner_margin(egui::Margin::symmetric(14, 6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(format!("Saving frame {done} of {total}"))
                                        .size(13.0)
                                        .color(LIGHT),
                                );
                                if crate::widgets::text_button(
                                    ui,
                                    "Cancel",
                                    "Stop the export",
                                    false,
                                )
                                .clicked()
                                {
                                    self.act(Action::CancelFrames);
                                }
                            });
                        });
                });
        }
        // Overlays have the window's attention; a toast would land on top of them.
        if self.layer != super::Layer::None {
            return;
        }
        let Some((message, when)) = &self.toast else {
            return;
        };
        let age = when.elapsed().as_secs_f32();
        if age > 6.0 {
            self.toast = None;
            return;
        }
        let alpha = ((6.0 - age) / 0.4).clamp(0.0, 1.0);
        let max_w = (screen.width() - 64.0).min(720.0);
        let message = message.clone();
        egui::Area::new(egui::Id::new("toast"))
            .order(Order::Tooltip)
            .anchor(Align2::CENTER_TOP, vec2(0.0, 76.0))
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(VOID)
                    .stroke(Stroke::new(1.0, EDGE.gamma_multiply(alpha)))
                    .corner_radius(4)
                    .inner_margin(egui::Margin::symmetric(14, 8))
                    .show(ui, |ui| {
                        ui.set_max_width(max_w);
                        ui.label(
                            egui::RichText::new(message)
                                .size(13.0)
                                .color(LIGHT.gamma_multiply(alpha)),
                        );
                    });
            });
    }
}
