//! The atlas shell. The window is one black field: a navigator on the left, the
//! visual in the middle with its time line and filmstrip beneath it, and the
//! context panel on the right that says what you are seeing, how to change it,
//! and where to go next. Every button, key, and palette entry produces the same
//! `Action`, handled in one place at the start of the next frame.

mod canvas;
mod context;
mod dock;
mod map;
mod nav;
mod palette;
mod title;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, Key, Modifiers};
use serde_json::Value;

use crate::atlas::links;
use crate::atlas::presets::PRESETS;
use crate::atlas::trails::TRAILS;
use crate::engine::gpu::{GpuContext, ViewTarget};
use crate::engine::module::{Engine, ModuleInfo, RenderMode, ViewRef};
use crate::modules;
use crate::session::{self, Bookmark, Session};
use crate::theme;
use palette::{PaletteMode, PaletteState};

pub fn run(initial_scene: Option<Value>) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1600.0, 900.0])
            .with_min_inner_size([1024.0, 640.0])
            .with_app_id("revelatio-noetica")
            .with_icon(window_icon())
            .with_title("Revelatio Noetica"),
        ..Default::default()
    };
    eframe::run_native(
        "Revelatio Noetica",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            theme::install_style(&cc.egui_ctx);
            cc.egui_ctx.options_mut(|o| o.zoom_with_keyboard = false);
            let rs = cc
                .wgpu_render_state
                .clone()
                .ok_or_else(|| anyhow::anyhow!("the wgpu renderer did not start"))?;
            let gpu = GpuContext::from_eframe(&rs);
            let engine = Engine::new(gpu, modules::all());
            Ok(Box::new(AtlasApp::new(engine, rs, initial_scene)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(())
}

fn window_icon() -> egui::IconData {
    let decoded = image::load_from_memory(include_bytes!("../../assets/icon-256.png"))
        .map(|i| i.to_rgba8())
        .ok();
    match decoded {
        Some(img) => {
            let (width, height) = img.dimensions();
            egui::IconData {
                rgba: img.into_raw(),
                width,
                height,
            }
        }
        None => egui::IconData::default(),
    }
}

/// Everything about a chamber that never changes, gathered once.
pub struct Chamber {
    pub info: ModuleInfo,
    pub modes: Vec<RenderMode>,
    /// For each view, the pointer gesture it accepts, if any.
    pub gesture: Vec<Option<&'static str>>,
    pub accent: Color32,
}

impl Chamber {
    pub fn view_id(&self, mode: usize) -> &'static str {
        self.modes.get(mode).map_or("", |m| m.id)
    }
    pub fn view_name(&self, mode: usize) -> &'static str {
        self.modes.get(mode).map_or("", |m| m.name)
    }
}

pub struct ViewSlot {
    pub target: ViewTarget,
    pub tex: egui::TextureId,
}

/// The full-window layer on top of the interface, if any.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Layer {
    None,
    Palette,
    Map,
    Help,
}

#[derive(Clone, Debug)]
pub enum Action {
    Go(ViewRef),
    GoChamber(usize),
    AddPane(ViewRef),
    ClosePane(usize),
    FocusPane(usize),
    SinglePane,
    CompareToggle,
    Back,
    Forward,
    NextStep,
    PrevStep,
    NextView,
    PrevView,
    Preset(usize),
    Randomize,
    ResetParams,
    ResetCamera(ViewRef),
    ToggleSweep(usize, &'static str),
    StartTrail(usize),
    TrailStep(isize),
    EndTrail,
    ToggleBookmark,
    OpenBookmark(usize),
    DeleteBookmark(usize),
    CopyLink,
    OpenLink(String),
    OpenSceneFile(PathBuf),
    SaveScene,
    ExportStill(u32, u32),
    ExportFrames,
    CancelFrames,
    OpenPalette(PaletteMode),
    OpenMap,
    OpenHelp,
    CloseLayer,
    ToggleAtlas,
    TogglePanel,
    ToggleFilm,
    ToggleFocus,
    TogglePlay,
    Restart,
    ToggleMotion,
    Scale(f32),
    ToggleSection(String),
}

/// The frame sequence being written, one frame per interface frame so the
/// window stays alive and the export can be cancelled.
struct FrameExport {
    view: ViewRef,
    next: usize,
    total: usize,
    dir: PathBuf,
    prev_t: f64,
    resume_t: f64,
    resume_playing: bool,
}

pub struct AtlasApp {
    engine: Engine,
    render_state: eframe::egui_wgpu::RenderState,
    catalog: Arc<Vec<Chamber>>,
    index: HashMap<&'static str, usize>,
    slots: HashMap<ViewRef, ViewSlot>,
    thumbs: HashMap<ViewRef, ViewSlot>,
    thumb_turn: usize,
    frame_count: u64,
    last_frame: Instant,
    fps: f32,
    session: Session,
    dirty: bool,
    last_save: Instant,
    toast: Option<(String, Instant)>,
    layer: Layer,
    palette: PaletteState,
    map_hover_trail: Option<usize>,
    atlas_open: bool,
    panel_open: bool,
    focus_mode: bool,
    sizes_decided: bool,
    show_other_params: bool,
    back: Vec<(Vec<ViewRef>, usize)>,
    forward: Vec<(Vec<ViewRef>, usize)>,
    trail: Option<(usize, usize)>,
    actions: Vec<Action>,
    rng: u64,
    export: Option<FrameExport>,
    math_tex: HashMap<&'static str, Option<egui::TextureHandle>>,
    math_ppp: f32,
    split: f32,
    fade_from: Instant,
    accent: Color32,
    title: String,
}

impl AtlasApp {
    fn new(
        engine: Engine,
        render_state: eframe::egui_wgpu::RenderState,
        initial_scene: Option<Value>,
    ) -> Self {
        let catalog: Vec<Chamber> = engine
            .modules
            .iter()
            .map(|m| {
                let modes = m.modes();
                let gesture = (0..modes.len())
                    .map(|i| m.wants_pointer(i).then(|| m.pointer_hint(i)).flatten())
                    .collect();
                let info = m.info();
                let accent = info.branch.style().accent;
                Chamber {
                    info,
                    modes,
                    gesture,
                    accent,
                }
            })
            .collect();
        let index = catalog
            .iter()
            .enumerate()
            .map(|(i, c)| (c.info.id, i))
            .collect();
        let (session, load_problem) = Session::load();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9, |d| d.as_nanos() as u64)
            | 1;
        let mut app = Self {
            engine,
            render_state,
            catalog: Arc::new(catalog),
            index,
            slots: HashMap::new(),
            thumbs: HashMap::new(),
            thumb_turn: 0,
            frame_count: 0,
            last_frame: Instant::now(),
            fps: 0.0,
            session,
            dirty: false,
            last_save: Instant::now(),
            toast: None,
            layer: Layer::None,
            palette: PaletteState::default(),
            map_hover_trail: None,
            atlas_open: false,
            panel_open: true,
            focus_mode: false,
            sizes_decided: false,
            show_other_params: false,
            back: Vec::new(),
            forward: Vec::new(),
            trail: None,
            actions: Vec::new(),
            rng: seed,
            export: None,
            math_tex: HashMap::new(),
            math_ppp: 0.0,
            split: 0.5,
            fade_from: Instant::now(),
            accent: Color32::WHITE,
            title: String::new(),
        };
        if let Some(problem) = load_problem {
            app.set_toast(problem);
        }
        let start = initial_scene.or_else(|| app.session.last_scene.clone());
        if let Some(scene) = start {
            if let Err(e) = app.engine.apply_scene(&scene) {
                app.set_toast(format!("Could not open that scene: {e:#}"));
                app.engine.set_active(0);
            }
        }
        app.after_navigation();
        app
    }

    fn act(&mut self, action: Action) {
        self.actions.push(action);
    }

    fn set_toast(&mut self, s: impl Into<String>) {
        self.toast = Some((s.into(), Instant::now()));
    }

    /// A pseudo-random number in 0..1 (xorshift64*; quality is ample here).
    fn uniform(&mut self) -> f64 {
        self.rng ^= self.rng >> 12;
        self.rng ^= self.rng << 25;
        self.rng ^= self.rng >> 27;
        let x = self.rng.wrapping_mul(0x2545_F491_4F6C_DD1D);
        (x >> 11) as f64 / (1u64 << 53) as f64
    }

    pub fn resolve(&self, chamber: &str, view: Option<&str>) -> Option<ViewRef> {
        let &module = self.index.get(chamber)?;
        let mode = match view {
            Some(v) => self.catalog[module].modes.iter().position(|m| m.id == v)?,
            None => self.opening_mode(module),
        };
        Some(ViewRef { module, mode })
    }

    fn opening_mode(&self, module: usize) -> usize {
        self.engine.modules[module]
            .default_modes()
            .first()
            .copied()
            .unwrap_or(0)
    }

    // ---------- navigation ----------

    fn remember_position(&mut self) {
        self.back
            .push((self.engine.panes.clone(), self.engine.focus));
        if self.back.len() > 200 {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    fn go(&mut self, view: ViewRef) {
        if self.engine.focused() == view {
            return;
        }
        self.remember_position();
        self.engine.open_view(view);
        self.after_navigation();
    }

    fn restore_position(&mut self, (panes, focus): (Vec<ViewRef>, usize)) {
        let same_chamber = panes.get(focus).map(|p| p.module) == Some(self.engine.active());
        for p in &panes {
            self.engine.ensure_init(p.module);
        }
        self.engine.focus = focus.min(panes.len().saturating_sub(1));
        self.engine.panes = panes;
        if !same_chamber {
            self.engine.timeline.reset();
        }
        self.after_navigation();
    }

    /// Everything that follows from the focused view having changed.
    fn after_navigation(&mut self) {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        self.session
            .mark_seen(chamber.info.id, chamber.view_id(f.mode));
        self.session.last_scene = Some(self.engine.scene_json());
        self.dirty = true;
        self.fade_from = Instant::now();
        self.thumb_turn = 0;
        self.show_other_params = false;
    }

    /// The next thing to see: the path's next step if on one, then the
    /// chamber's next view, then the first chamber this one leads to.
    fn next_step(&mut self) {
        if self.trail.is_some() {
            // Away from the path, Next first returns to the step that was left.
            self.trail_step(if self.on_trail_step() { 1 } else { 0 });
            return;
        }
        let f = self.engine.focused();
        let chamber = self.catalog[f.module].info.id;
        let n = self.catalog[f.module].modes.len();
        if f.mode + 1 < n {
            self.go(ViewRef {
                module: f.module,
                mode: f.mode + 1,
            });
            return;
        }
        let neighbours = links::leads_to(chamber);
        let target = neighbours
            .first()
            .or(links::related_to(chamber).first())
            .and_then(|l| self.resolve(l.chamber, l.view));
        if let Some(v) = target {
            self.go(v);
        }
    }

    fn prev_step(&mut self) {
        if self.trail.is_some() {
            self.trail_step(-1);
        } else if let Some(pos) = self.back.pop() {
            self.forward
                .push((self.engine.panes.clone(), self.engine.focus));
            self.restore_position(pos);
        }
    }

    fn trail_step(&mut self, delta: isize) {
        let Some((t, step)) = self.trail else { return };
        let steps = TRAILS[t].steps;
        let at = step as isize + delta;
        if at < 0 {
            return;
        }
        let Some(s) = steps.get(at as usize) else {
            self.trail = None;
            self.set_toast(format!("End of the path: {}", TRAILS[t].name));
            return;
        };
        self.trail = Some((t, at as usize));
        if let Some(v) = self.resolve(s.module, Some(s.view)) {
            self.remember_position();
            self.engine.panes = vec![self.engine.focused()];
            self.engine.focus = 0;
            self.engine.open_view(v);
            self.after_navigation();
        }
    }

    // ---------- keyboard ----------

    fn keyboard(&mut self, ctx: &egui::Context) {
        let typing = ctx.wants_keyboard_input();
        let widget_focused = ctx.memory(|m| m.focused().is_some());
        let layer = self.layer;
        let mut out: Vec<Action> = Vec::new();
        ctx.input_mut(|i| {
            if i.consume_key(Modifiers::COMMAND, Key::K) {
                out.push(Action::OpenPalette(PaletteMode::Search));
            }
            if i.consume_key(Modifiers::COMMAND, Key::Plus)
                || i.consume_key(Modifiers::COMMAND, Key::Equals)
            {
                out.push(Action::Scale(1.1));
            }
            if i.consume_key(Modifiers::COMMAND, Key::Minus) {
                out.push(Action::Scale(1.0 / 1.1));
            }
            if i.consume_key(Modifiers::COMMAND, Key::Num0) {
                out.push(Action::Scale(0.0));
            }
            if i.consume_key(Modifiers::ALT, Key::ArrowLeft) {
                out.push(Action::Back);
            }
            if i.consume_key(Modifiers::ALT, Key::ArrowRight) {
                out.push(Action::Forward);
            }
            if i.consume_key(Modifiers::NONE, Key::Escape) {
                out.push(Action::CloseLayer);
            }
            if typing {
                return;
            }
            let pasted = i.events.iter().find_map(|e| match e {
                egui::Event::Paste(text) if session::is_link(text) => Some(text.clone()),
                _ => None,
            });
            if let Some(link) = pasted {
                out.push(Action::OpenLink(link));
            }
            let help_key = i
                .events
                .iter()
                .any(|e| matches!(e, egui::Event::Text(t) if t == "?"));
            if help_key {
                out.push(Action::OpenHelp);
            }
            if i.consume_key(Modifiers::NONE, Key::M) {
                out.push(if layer == Layer::Map {
                    Action::CloseLayer
                } else {
                    Action::OpenMap
                });
            }
            if layer != Layer::None {
                return;
            }
            let keys = [
                (Key::Slash, Action::OpenPalette(PaletteMode::Search)),
                (Key::A, Action::ToggleAtlas),
                (Key::I, Action::TogglePanel),
                (Key::V, Action::ToggleFilm),
                (Key::F, Action::ToggleFocus),
                (Key::C, Action::CompareToggle),
                (Key::B, Action::ToggleBookmark),
                (Key::R, Action::Randomize),
                (Key::N, Action::NextStep),
                (Key::P, Action::PrevStep),
                (Key::OpenBracket, Action::PrevView),
                (Key::CloseBracket, Action::NextView),
                (Key::Home, Action::Restart),
            ];
            for (key, action) in keys {
                if i.consume_key(Modifiers::NONE, key) {
                    out.push(action);
                }
            }
            if !widget_focused && i.consume_key(Modifiers::NONE, Key::Space) {
                out.push(Action::TogglePlay);
            }
        });
        self.actions.extend(out);

        if !typing && !widget_focused && self.layer == Layer::None {
            let scrub = ctx.input(|i| {
                let mut d = 0.0;
                if i.key_pressed(Key::ArrowRight) {
                    d += 0.5;
                }
                if i.key_pressed(Key::ArrowLeft) {
                    d -= 0.5;
                }
                d
            });
            if scrub != 0.0 {
                let tl = &mut self.engine.timeline;
                tl.seek(tl.t + scrub);
            }
        }
    }

    // ---------- actions ----------

    fn apply_actions(&mut self, ctx: &egui::Context) {
        for action in std::mem::take(&mut self.actions) {
            self.apply(ctx, action);
        }
    }

    fn apply(&mut self, ctx: &egui::Context, action: Action) {
        match action {
            Action::Go(v) => {
                self.close_layer();
                self.go(v);
            }
            Action::GoChamber(m) => {
                self.close_layer();
                let v = ViewRef {
                    module: m,
                    mode: self.opening_mode(m),
                };
                self.go(v);
            }
            Action::AddPane(v) => {
                self.close_layer();
                self.remember_position();
                if self.engine.add_pane(v) {
                    self.after_navigation();
                } else {
                    self.set_toast("Four views are open. Close one to add another.");
                }
            }
            Action::ClosePane(i) => {
                self.remember_position();
                self.engine.close_pane(i);
                self.after_navigation();
            }
            Action::FocusPane(i) => {
                if i < self.engine.panes.len() && i != self.engine.focus {
                    // With three or more views the hero is the focused one.
                    if self.engine.panes.len() > 2 {
                        self.engine.panes.swap(0, i);
                        self.engine.focus = 0;
                    } else {
                        self.engine.focus = i;
                    }
                    self.after_navigation();
                }
            }
            Action::SinglePane => {
                self.remember_position();
                self.engine.panes = vec![self.engine.focused()];
                self.engine.focus = 0;
                self.after_navigation();
            }
            Action::CompareToggle => {
                if self.engine.panes.len() > 1 {
                    self.apply(ctx, Action::SinglePane);
                } else {
                    self.apply(ctx, Action::OpenPalette(PaletteMode::Compare));
                }
            }
            Action::Back => {
                if let Some(pos) = self.back.pop() {
                    self.forward
                        .push((self.engine.panes.clone(), self.engine.focus));
                    self.restore_position(pos);
                }
            }
            Action::Forward => {
                if let Some(pos) = self.forward.pop() {
                    self.back
                        .push((self.engine.panes.clone(), self.engine.focus));
                    self.restore_position(pos);
                }
            }
            Action::NextStep => self.next_step(),
            Action::PrevStep => self.prev_step(),
            Action::NextView | Action::PrevView => {
                let f = self.engine.focused();
                let n = self.catalog[f.module].modes.len();
                let step = if matches!(action, Action::NextView) {
                    1
                } else {
                    n - 1
                };
                self.go(ViewRef {
                    module: f.module,
                    mode: (f.mode + step) % n,
                });
            }
            Action::Preset(i) => self.apply_preset(i),
            Action::Randomize => self.randomize(),
            Action::ResetParams => {
                self.engine.reset_params(self.engine.active());
                self.set_toast("Parameters reset");
            }
            Action::ResetCamera(v) => self.engine.reset_camera(v.module, v.mode),
            Action::ToggleSweep(module, key) => self.engine.toggle_sweep(module, key),
            Action::StartTrail(t) => {
                self.close_layer();
                let first = &TRAILS[t].steps[0];
                if let Some(v) = self.resolve(first.module, Some(first.view)) {
                    self.trail = Some((t, 0));
                    self.remember_position();
                    self.engine.panes = vec![self.engine.focused()];
                    self.engine.focus = 0;
                    self.engine.open_view(v);
                    self.after_navigation();
                }
            }
            Action::TrailStep(d) => self.trail_step(d),
            Action::EndTrail => self.trail = None,
            Action::ToggleBookmark => self.toggle_bookmark(),
            Action::OpenBookmark(i) => {
                self.close_layer();
                if let Some(b) = self.session.bookmarks.get(i).cloned() {
                    self.open_scene(&b.scene, Some(&b.name));
                }
            }
            Action::DeleteBookmark(i) => {
                if i < self.session.bookmarks.len() {
                    let b = self.session.bookmarks.remove(i);
                    self.dirty = true;
                    self.set_toast(format!("Removed {}", b.name));
                }
            }
            Action::CopyLink => {
                let scene = self.engine.scene_json();
                ctx.copy_text(session::encode_link(&scene));
                self.set_toast("Copied a link to this scene. Paste it into any Revelatio window.");
            }
            Action::OpenLink(link) => {
                self.close_layer();
                match session::decode_link(&link) {
                    Ok(scene) => self.open_scene(&scene, Some("the linked scene")),
                    Err(e) => self.set_toast(format!("Could not open that link: {e:#}")),
                }
            }
            Action::OpenSceneFile(path) => {
                self.close_layer();
                self.remember_position();
                match self.engine.load_scene(&path) {
                    Ok(()) => {
                        self.after_navigation();
                        self.set_toast(format!("Opened {}", path.display()));
                    }
                    Err(e) => self.set_toast(format!("Could not open that file: {e:#}")),
                }
            }
            Action::SaveScene => {
                let path = self.export_path("scene.json");
                match self.engine.save_scene(&path) {
                    Ok(()) => self.set_toast(format!("Saved {}", path.display())),
                    Err(e) => self.set_toast(format!("Could not save the scene: {e:#}")),
                }
            }
            Action::ExportStill(w, h) => {
                let path = self.export_path(&format!("{h}p.png"));
                match self.engine.export_png(self.engine.focused(), w, h, &path) {
                    Ok(()) => self.set_toast(format!("Saved {}", path.display())),
                    Err(e) => self.set_toast(format!("Could not save the image: {e:#}")),
                }
            }
            Action::ExportFrames => self.start_frame_export(),
            Action::CancelFrames => self.finish_frame_export(Some("Frame export cancelled")),
            Action::OpenPalette(mode) => {
                self.palette.open(mode);
                self.layer = Layer::Palette;
            }
            Action::OpenMap => {
                self.palette.close(ctx);
                self.layer = Layer::Map;
            }
            Action::OpenHelp => {
                self.palette.close(ctx);
                self.layer = Layer::Help;
            }
            Action::CloseLayer => {
                if self.layer != Layer::None {
                    self.palette.close(ctx);
                    self.layer = Layer::None;
                } else if self.focus_mode {
                    self.focus_mode = false;
                } else {
                    ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                }
            }
            Action::ToggleAtlas => {
                self.atlas_open = !self.atlas_open;
                self.session.prefs.atlas_open = Some(self.atlas_open);
                self.dirty = true;
            }
            Action::TogglePanel => {
                self.panel_open = !self.panel_open;
                self.session.prefs.panel_open = Some(self.panel_open);
                self.dirty = true;
            }
            Action::ToggleFilm => {
                self.session.prefs.film_open = !self.session.prefs.film_open;
                self.dirty = true;
            }
            Action::ToggleFocus => self.focus_mode = !self.focus_mode,
            Action::TogglePlay => {
                let tl = &mut self.engine.timeline;
                tl.playing = !tl.playing;
                if tl.playing && tl.t >= tl.span {
                    tl.restart();
                }
            }
            Action::Restart => self.engine.timeline.restart(),
            Action::ToggleMotion => {
                let on = !self.session.prefs.reduce_motion;
                self.session.prefs.reduce_motion = on;
                self.dirty = true;
                self.set_toast(if on {
                    "Interface motion reduced"
                } else {
                    "Interface motion restored"
                });
            }
            Action::Scale(f) => {
                let s = &mut self.session.prefs.ui_scale;
                *s = if f == 0.0 {
                    1.0
                } else {
                    (*s * f).clamp(0.6, 2.5)
                };
                let shown = (*s * 100.0).round();
                self.dirty = true;
                self.set_toast(format!("Interface scale {shown}%"));
            }
            Action::ToggleSection(name) => {
                let closed = &mut self.session.prefs.closed_sections;
                if let Some(i) = closed.iter().position(|n| n == &name) {
                    closed.remove(i);
                } else {
                    closed.push(name);
                }
                self.dirty = true;
            }
        }
    }

    fn close_layer(&mut self) {
        self.layer = Layer::None;
        self.palette.mode = PaletteMode::Search;
    }

    fn open_scene(&mut self, scene: &Value, label: Option<&str>) {
        self.remember_position();
        match self.engine.apply_scene(scene) {
            Ok(()) => {
                self.after_navigation();
                if let Some(label) = label {
                    self.set_toast(format!("Opened {label}"));
                }
            }
            Err(e) => self.set_toast(format!("Could not open that scene: {e:#}")),
        }
    }

    fn apply_preset(&mut self, i: usize) {
        let Some(preset) = PRESETS.get(i) else { return };
        let Some(&module) = self.index.get(preset.module) else {
            return;
        };
        self.engine.sweeps.retain(|s| s.module != module);
        self.engine.ensure_init(module);
        preset.apply(self.engine.modules[module].params());
        let target = match preset.view {
            Some(view) => self.resolve(preset.module, Some(view)),
            None if self.engine.active() != module => self.resolve(preset.module, None),
            None => None,
        };
        self.close_layer();
        if let Some(v) = target {
            self.go(v);
        }
        self.set_toast(format!("{}: {}", preset.name, preset.note));
    }

    fn randomize(&mut self) {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        let params = self.engine.modules[f.module].params_ref();
        let guide = crate::atlas::guides::guide(chamber.info.id, chamber.view_id(f.mode));
        let keys: Vec<&'static str> = params
            .items
            .iter()
            .filter(|p| !matches!(p.kind, crate::engine::params::ParamKind::Toggle))
            .filter(|p| guide.is_none_or(|g| g.params.contains(&p.key)))
            .map(|p| p.key)
            .collect();
        if keys.is_empty() {
            self.set_toast(
                "No slider or choice changes this view, so there is nothing to randomize.",
            );
            return;
        }
        self.engine.sweeps.retain(|s| s.module != f.module);
        let mut draws: Vec<f64> = (0..keys.len()).map(|_| self.uniform()).collect();
        self.engine.modules[f.module]
            .params()
            .randomize(&keys, || draws.pop().unwrap_or(0.5));
        self.set_toast("Randomized this view's parameters. Reset puts them back.");
    }

    fn scene_fingerprint(scene: &Value) -> Value {
        let views: Vec<Value> = scene["panes"]
            .as_array()
            .map(|panes| {
                panes
                    .iter()
                    .map(|p| serde_json::json!([p["module"], p["view"]]))
                    .collect()
            })
            .unwrap_or_default();
        serde_json::json!({ "views": views, "params": scene["all_params"] })
    }

    /// Which saved view matches the screen now, ignoring camera and time.
    fn current_bookmark(&mut self) -> Option<usize> {
        let now = Self::scene_fingerprint(&self.engine.scene_json());
        self.session
            .bookmarks
            .iter()
            .position(|b| Self::scene_fingerprint(&b.scene) == now)
    }

    fn toggle_bookmark(&mut self) {
        if let Some(i) = self.current_bookmark() {
            let b = self.session.bookmarks.remove(i);
            self.set_toast(format!("Removed {}", b.name));
        } else {
            let f = self.engine.focused();
            let chamber = &self.catalog[f.module];
            let id = chamber.info.id;
            let view_name = chamber.view_name(f.mode);
            let params = self.engine.modules[f.module].params_ref();
            let preset = crate::atlas::presets::for_chamber(id)
                .find(|(_, p)| p.is_active(params))
                .map(|(_, p)| p.name);
            let name = match preset {
                Some(p) => format!("{view_name}: {p}"),
                None => format!("{view_name}, t = {:.1} s", self.engine.timeline.t),
            };
            let scene = self.engine.scene_json();
            self.set_toast(format!("Saved {name}"));
            self.session.bookmarks.push(Bookmark {
                name,
                chamber: id.to_string(),
                scene,
            });
        }
        self.dirty = true;
    }

    // ---------- export ----------

    fn export_path(&self, ending: &str) -> PathBuf {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        session::export_dir().join(format!(
            "{}_{}_{stamp}_{ending}",
            chamber.info.id.replace('.', "_"),
            chamber.view_id(f.mode)
        ))
    }

    fn start_frame_export(&mut self) {
        if self.export.is_some() {
            return;
        }
        let dir = self.export_path("frames");
        if let Err(e) = Engine::write_video_note(&dir, 30) {
            self.set_toast(format!("Could not start the export: {e:#}"));
            return;
        }
        let tl = &mut self.engine.timeline;
        self.export = Some(FrameExport {
            view: self.engine.panes[self.engine.focus],
            next: 0,
            total: 240,
            dir,
            prev_t: 0.0,
            resume_t: tl.t,
            resume_playing: tl.playing,
        });
        tl.playing = false;
    }

    fn step_frame_export(&mut self) {
        let Some(job) = &mut self.export else { return };
        let (view, i, total, prev, dir) =
            (job.view, job.next, job.total, job.prev_t, job.dir.clone());
        match self
            .engine
            .export_frame(view, (1920, 1080), (i, total), prev, &dir)
        {
            Ok(t) => {
                if let Some(job) = &mut self.export {
                    job.prev_t = t;
                    job.next += 1;
                }
                if i + 1 >= total {
                    let done = format!(
                        "Saved {total} frames and an ffmpeg command in {}",
                        dir.display()
                    );
                    self.finish_frame_export(Some(&done));
                }
            }
            Err(e) => {
                let msg = format!("Frame export stopped: {e:#}");
                self.finish_frame_export(Some(&msg));
            }
        }
    }

    fn finish_frame_export(&mut self, message: Option<&str>) {
        if let Some(job) = self.export.take() {
            self.engine.timeline.t = job.resume_t;
            self.engine.timeline.playing = job.resume_playing;
        }
        if let Some(m) = message {
            self.set_toast(m);
        }
    }

    // ---------- GPU slots ----------

    fn slot_for(
        map: &mut HashMap<ViewRef, ViewSlot>,
        render_state: &eframe::egui_wgpu::RenderState,
        engine: &Engine,
        view: ViewRef,
        (w, h): (u32, u32),
    ) -> egui::TextureId {
        let current = map
            .get(&view)
            .filter(|s| s.target.width == w && s.target.height == h)
            .map(|s| s.tex);
        if let Some(tex) = current {
            return tex;
        }
        let target = ViewTarget::new(&engine.gpu, &engine.rend.view_bgl, w, h);
        let mut renderer = render_state.renderer.write();
        if let Some(old) = map.remove(&view) {
            renderer.free_texture(&old.tex);
        }
        let tex = renderer.register_native_texture(
            &engine.gpu.device,
            &target.display_view,
            wgpu::FilterMode::Linear,
        );
        map.insert(view, ViewSlot { target, tex });
        tex
    }

    /// Free GPU targets for views no longer on screen. Done before any painting
    /// so nothing queued this frame can reference a freed texture.
    fn retire_slots(&mut self) {
        let panes = self.engine.panes.clone();
        let active = self.engine.active();
        let mut renderer = self.render_state.renderer.write();
        self.slots.retain(|view, slot| {
            let keep = panes.contains(view);
            if !keep {
                renderer.free_texture(&slot.tex);
            }
            keep
        });
        self.thumbs.retain(|view, slot| {
            let keep = view.module == active;
            if !keep {
                renderer.free_texture(&slot.tex);
            }
            keep
        });
    }

    // ---------- per-frame bookkeeping ----------

    fn decide_sizes(&mut self, ctx: &egui::Context) {
        if self.sizes_decided {
            return;
        }
        self.sizes_decided = true;
        let width = ctx.screen_rect().width();
        let prefs = &self.session.prefs;
        self.atlas_open = prefs.atlas_open.unwrap_or(width >= 1560.0);
        self.panel_open = prefs.panel_open.unwrap_or(width >= 1100.0);
    }

    /// Scale the interface with the window so 4K is as legible as 1080p. A
    /// system scale factor counts: only extra room beyond it scales further.
    fn apply_scale(&self, ctx: &egui::Context) {
        let native = ctx.native_pixels_per_point().unwrap_or(1.0);
        let physical = ctx.screen_rect().size() * ctx.pixels_per_point();
        let auto = (physical.x / native / 1920.0)
            .min(physical.y / native / 1080.0)
            .clamp(1.0, 2.5);
        let want = auto * self.session.prefs.ui_scale;
        if (ctx.zoom_factor() - want).abs() > 0.005 {
            ctx.set_zoom_factor(want);
        }
    }

    fn sync_window_title(&mut self, ctx: &egui::Context) {
        let f = self.engine.focused();
        let chamber = &self.catalog[f.module];
        let title = format!(
            "{}, {} | Revelatio Noetica",
            chamber.view_name(f.mode),
            chamber.info.name
        );
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    fn save_if_needed(&mut self) {
        if self.dirty && self.last_save.elapsed() > Duration::from_secs(2) {
            self.save_now();
        }
    }

    fn save_now(&mut self) {
        self.session.last_scene = Some(self.engine.scene_json());
        self.dirty = false;
        self.last_save = Instant::now();
        if let Err(e) = self.session.save() {
            self.set_toast(format!("Could not save your session: {e:#}"));
        }
    }
}

impl eframe::App for AtlasApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.25);
        self.last_frame = now;
        self.fps = 0.9 * self.fps + 0.1 * (1.0 / dt.max(1e-4)) as f32;
        self.frame_count += 1;

        self.decide_sizes(ctx);
        self.apply_scale(ctx);
        self.apply_actions(ctx);
        self.retire_slots();

        let f = self.engine.focused();
        let accent = self.catalog[f.module].accent;
        if accent != self.accent {
            self.accent = accent;
            theme::apply_accent(ctx, accent);
        }
        ctx.style_mut(|s| {
            s.animation_time = if self.session.prefs.reduce_motion {
                0.0
            } else {
                0.083
            };
        });
        self.sync_window_title(ctx);

        if self.export.is_some() {
            self.step_frame_export();
        } else {
            self.engine.tick(dt);
        }
        self.keyboard(ctx);

        if !self.focus_mode {
            self.navigator(ctx);
            if self.panel_open {
                self.context_panel(ctx);
            }
            self.dock(ctx);
        }
        self.central(ctx);

        match self.layer {
            Layer::Palette => self.palette_layer(ctx),
            Layer::Map => self.map_layer(ctx),
            Layer::Help => self.help_layer(ctx),
            Layer::None => {}
        }
        self.toast_layer(ctx);
        if !self.focus_mode && self.session.prefs.film_open {
            self.refresh_one_thumbnail();
        }
        self.save_if_needed();

        // A paused, unswept scene is a still image: repaint only on input,
        // with a slow tick so live thumbnails and readouts stay honest.
        let animating = self.engine.timeline.playing
            || !self.engine.sweeps.is_empty()
            || self.export.is_some()
            || self.toast.is_some();
        if animating {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn on_exit(&mut self) {
        self.save_now();
    }
}
