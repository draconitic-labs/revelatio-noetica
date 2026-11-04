//! The concept-module plugin interface and the engine that hosts it.

use std::collections::HashMap;

use super::camera::{CamKind, Camera};
use super::draw::{Renderers, Scene};
use super::gpu::{GpuContext, ViewTarget, ViewUniforms};
use super::params::Params;
use super::timeline::Timeline;
use crate::theme::Branch;

#[derive(Clone)]
pub struct ModuleInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    pub branch: Branch,
    pub description: &'static str,
    /// Optional epigraph: (quotation, attribution).
    pub epigraph: Option<(&'static str, &'static str)>,
}

#[derive(Clone)]
pub struct RenderMode {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub cam: CamKind,
    pub target: glam::Vec3,
}

impl RenderMode {
    pub fn plane(id: &'static str, name: &'static str, desc: &'static str, half_h: f32) -> Self {
        Self {
            id,
            name,
            desc,
            cam: CamKind::Plane { half_h },
            target: glam::Vec3::ZERO,
        }
    }
    pub fn orbit(
        id: &'static str,
        name: &'static str,
        desc: &'static str,
        dist: f32,
        pitch: f32,
        yaw: f32,
    ) -> Self {
        Self {
            id,
            name,
            desc,
            cam: CamKind::Orbit { dist, pitch, yaw },
            target: glam::Vec3::ZERO,
        }
    }
    pub fn look_at(mut self, x: f32, y: f32, z: f32) -> Self {
        self.target = glam::Vec3::new(x, y, z);
        self
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum InsRole {
    Title,
    Formula,
    Def,
    Value,
    Note,
}

#[derive(Clone)]
pub struct InsLine {
    pub role: InsRole,
    pub text: String,
    /// Key of a pre-rendered LaTeX image (assets/math); text is the fallback.
    pub tex: Option<&'static str>,
}

pub fn title(s: impl Into<String>) -> InsLine {
    InsLine {
        role: InsRole::Title,
        text: s.into(),
        tex: None,
    }
}
pub fn formula(s: impl Into<String>) -> InsLine {
    InsLine {
        role: InsRole::Formula,
        text: s.into(),
        tex: None,
    }
}
/// A formula typeset by LaTeX when available, with a Unicode fallback.
pub fn ftex(key: &'static str, fallback: impl Into<String>) -> InsLine {
    InsLine {
        role: InsRole::Formula,
        text: fallback.into(),
        tex: Some(key),
    }
}
pub fn def(s: impl Into<String>) -> InsLine {
    InsLine {
        role: InsRole::Def,
        text: s.into(),
        tex: None,
    }
}
pub fn value(s: impl Into<String>) -> InsLine {
    InsLine {
        role: InsRole::Value,
        text: s.into(),
        tex: None,
    }
}
pub fn note(s: impl Into<String>) -> InsLine {
    InsLine {
        role: InsRole::Note,
        text: s.into(),
        tex: None,
    }
}

/// Pointer gestures delivered to modules on plane-camera views, in world
/// coordinates on the z = 0 plane.
#[derive(Clone, Copy, Debug)]
pub enum PointerEvent {
    Down(glam::Vec2),
    Drag(glam::Vec2),
    Up,
}

/// One family of mathematical concepts: symbolic objects, solvers, and
/// synchronized visual representations.
pub trait ConceptModule {
    fn info(&self) -> ModuleInfo;
    fn modes(&self) -> Vec<RenderMode>;
    /// Which representations suit this chamber best (indices into `modes`).
    /// The first opens on entry; the next is offered first when comparing.
    fn default_modes(&self) -> Vec<usize> {
        vec![0]
    }
    fn params(&mut self) -> &mut Params;
    fn params_ref(&self) -> &Params;
    /// Create GPU resources; register field shaders. Called once, lazily.
    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers);
    /// Advance simulation / dynamic geometry to time `t`.
    fn update(&mut self, gpu: &GpuContext, t: f64, dt: f64);
    /// The viewer sent time backward or restarted it: begin simulations again.
    /// A loop wrapping on its own does not call this.
    fn rewind(&mut self) {}
    /// Describe the frame for one representation.
    fn render(&mut self, gpu: &GpuContext, mode: usize, t: f64, cam: &Camera, scene: &mut Scene);
    /// Symbolic inspector content.
    fn inspector(&self, t: f64) -> Vec<InsLine>;
    /// Honest labelling of the approximation behind a representation.
    fn approx(&self, _mode: usize) -> Option<&'static str> {
        None
    }
    /// Whether this representation consumes primary-button pointer gestures
    /// (camera then moves via right-drag / shift-drag instead).
    fn wants_pointer(&self, _mode: usize) -> bool {
        false
    }
    /// Pointer gesture on the canvas, in z=0 world coordinates.
    fn pointer(&mut self, _mode: usize, _ev: PointerEvent) {}
    /// One-line hint shown on interactive views ("drag the charges", ...).
    fn pointer_hint(&self, _mode: usize) -> Option<&'static str> {
        None
    }
    /// Screen-space engraved annotation over the canvas (interactive only).
    fn overlay(
        &self,
        _painter: &egui::Painter,
        _rect: egui::Rect,
        _cam: &Camera,
        _mode: usize,
        _t: f64,
    ) {
    }
}

/// Map world position to a screen point inside `rect` for overlays.
pub fn to_screen(cam: &Camera, rect: egui::Rect, p: glam::Vec3) -> Option<egui::Pos2> {
    let ndc = cam.project(p)?;
    if ndc.x.abs() > 1.2 || ndc.y.abs() > 1.2 {
        return None;
    }
    Some(egui::pos2(
        rect.left() + (ndc.x * 0.5 + 0.5) * rect.width(),
        rect.top() + (0.5 - ndc.y * 0.5) * rect.height(),
    ))
}

/// One open view: a representation of a chamber.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewRef {
    pub module: usize,
    pub mode: usize,
}

/// A parameter being swept back and forth across its whole range.
pub struct Sweep {
    pub module: usize,
    pub key: &'static str,
    /// Seconds for one trip from minimum to maximum and back.
    pub period: f64,
    phase: f64,
}

pub const MAX_PANES: usize = 4;

pub struct Engine {
    pub gpu: GpuContext,
    pub rend: Renderers,
    pub modules: Vec<Box<dyn ConceptModule>>,
    pub timeline: Timeline,
    /// The open views, 1..=MAX_PANES of them, possibly from different chambers.
    pub panes: Vec<ViewRef>,
    /// Index into `panes` of the view the side panel and filmstrip follow.
    pub focus: usize,
    pub sweeps: Vec<Sweep>,
    cameras: HashMap<(usize, usize), Camera>,
    initialized: Vec<bool>,
    defaults: Vec<serde_json::Value>,
}

impl Engine {
    pub fn new(gpu: GpuContext, modules: Vec<Box<dyn ConceptModule>>) -> Self {
        let rend = Renderers::new(&gpu);
        let n = modules.len();
        let defaults = modules.iter().map(|m| m.params_ref().to_json()).collect();
        let mut e = Self {
            gpu,
            rend,
            modules,
            timeline: Timeline::new(),
            panes: vec![ViewRef { module: 0, mode: 0 }],
            focus: 0,
            sweeps: Vec::new(),
            cameras: HashMap::new(),
            initialized: vec![false; n],
            defaults,
        };
        e.set_active(0);
        e
    }

    /// The chamber of the focused view.
    pub fn active(&self) -> usize {
        self.focused().module
    }

    pub fn focused(&self) -> ViewRef {
        self.panes[self.focus]
    }

    /// Create a chamber's GPU resources the first time it is needed.
    pub fn ensure_init(&mut self, idx: usize) {
        if !self.initialized[idx] {
            self.modules[idx].init(&self.gpu, &mut self.rend);
            self.initialized[idx] = true;
        }
    }

    /// Enter a chamber at its opening view, alone, with time back at zero.
    pub fn set_active(&mut self, idx: usize) {
        if idx >= self.modules.len() {
            return;
        }
        self.ensure_init(idx);
        let mode = self.modules[idx]
            .default_modes()
            .first()
            .copied()
            .unwrap_or(0);
        self.panes = vec![ViewRef { module: idx, mode }];
        self.focus = 0;
        self.timeline.reset();
    }

    /// Show `view` in the focused pane. A view already open elsewhere just
    /// takes the focus. Moving a lone pane to another chamber restarts time,
    /// as entering a chamber does; side-by-side views keep their shared clock.
    pub fn open_view(&mut self, view: ViewRef) {
        if view.module >= self.modules.len() {
            return;
        }
        self.ensure_init(view.module);
        if let Some(i) = self.panes.iter().position(|&p| p == view) {
            self.focus = i;
            return;
        }
        let changes_chamber = self.focused().module != view.module;
        self.panes[self.focus] = view;
        if changes_chamber && self.panes.len() == 1 {
            self.timeline.reset();
        }
    }

    /// Open `view` beside the others. Returns false if there is no room.
    pub fn add_pane(&mut self, view: ViewRef) -> bool {
        if view.module >= self.modules.len() {
            return false;
        }
        self.ensure_init(view.module);
        if let Some(i) = self.panes.iter().position(|&p| p == view) {
            self.focus = i;
            return true;
        }
        if self.panes.len() >= MAX_PANES {
            return false;
        }
        self.panes.push(view);
        self.focus = self.panes.len() - 1;
        true
    }

    /// Close a pane, keeping at least one open.
    pub fn close_pane(&mut self, i: usize) {
        if self.panes.len() > 1 && i < self.panes.len() {
            self.panes.remove(i);
            if self.focus >= i && self.focus > 0 {
                self.focus -= 1;
            }
        }
    }

    /// The distinct chambers on screen, each once.
    pub fn open_modules(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.panes.len());
        for p in &self.panes {
            if !out.contains(&p.module) {
                out.push(p.module);
            }
        }
        out
    }

    pub fn camera(&mut self, module: usize, mode: usize) -> &mut Camera {
        let (kind, target) = self.modules[module]
            .modes()
            .get(mode)
            .map(|m| (m.cam, m.target))
            .unwrap_or((CamKind::Plane { half_h: 2.0 }, glam::Vec3::ZERO));
        self.cameras.entry((module, mode)).or_insert_with(|| {
            let mut c = Camera::new(kind);
            c.target = target;
            c
        })
    }

    pub fn reset_camera(&mut self, module: usize, mode: usize) {
        self.cameras.remove(&(module, mode));
    }

    /// True once the viewer has moved this camera away from where it starts.
    pub fn camera_moved(&self, module: usize, mode: usize) -> bool {
        let Some(c) = self.cameras.get(&(module, mode)) else {
            return false;
        };
        let modes = self.modules[module].modes();
        let Some(m) = modes.get(mode) else {
            return false;
        };
        c.kind != m.cam || c.target != m.target
    }

    /// Put a chamber's parameters back to their declared values.
    pub fn reset_params(&mut self, module: usize) {
        let defaults = self.defaults[module].clone();
        self.modules[module].params().load_json(&defaults);
        self.sweeps.retain(|s| s.module != module);
    }

    pub fn is_sweeping(&self, module: usize, key: &str) -> bool {
        self.sweeps
            .iter()
            .any(|s| s.module == module && s.key == key)
    }

    /// Start sweeping a slider from where it is now, or stop if it is running.
    pub fn toggle_sweep(&mut self, module: usize, key: &'static str) {
        if self.is_sweeping(module, key) {
            self.sweeps
                .retain(|s| !(s.module == module && s.key == key));
            return;
        }
        if let Some(fraction) = self.modules[module].params_ref().fraction(key) {
            self.sweeps.push(Sweep {
                module,
                key,
                period: 16.0,
                phase: fraction * 0.5,
            });
        }
    }

    /// Step simulation time and let every open chamber integrate.
    pub fn tick(&mut self, real_dt: f64) {
        let on_screen = self.open_modules();
        self.sweeps.retain(|s| on_screen.contains(&s.module));
        for s in &mut self.sweeps {
            s.phase = (s.phase + real_dt / s.period).rem_euclid(1.0);
            let triangle = 1.0 - (2.0 * s.phase - 1.0).abs();
            self.modules[s.module]
                .params()
                .set_fraction(s.key, triangle);
        }
        if std::mem::take(&mut self.timeline.rewound) {
            for &m in &on_screen {
                self.modules[m].rewind();
            }
        }
        let dt = self.timeline.advance(real_dt);
        let t = self.timeline.t;
        let gpu = self.gpu.clone();
        for m in on_screen {
            self.modules[m].update(&gpu, t, dt);
        }
    }

    /// Render one view into a target.
    pub fn render_view(&mut self, view: ViewRef, target: &ViewTarget) {
        let t = self.timeline.t;
        let mut cam = *self.camera(view.module, view.mode);
        cam.aspect = target.width as f32 / target.height.max(1) as f32;

        let mut scene = Scene::new();
        let gpu = self.gpu.clone();
        self.modules[view.module].render(&gpu, view.mode, t, &cam, &mut scene);

        let vp = cam.view_proj();
        let u = ViewUniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            eye: cam.eye().extend(1.0).to_array(),
            resolution: [
                target.width as f32,
                target.height as f32,
                t as f32,
                cam.aspect,
            ],
            p: scene.params,
        };
        target.write_uniforms(&self.gpu, &u);
        self.rend.render_scene(&self.gpu, target, &scene, None);
    }
}
