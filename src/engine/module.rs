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
        Self { id, name, desc, cam: CamKind::Plane { half_h }, target: glam::Vec3::ZERO }
    }
    pub fn orbit(
        id: &'static str,
        name: &'static str,
        desc: &'static str,
        dist: f32,
        pitch: f32,
        yaw: f32,
    ) -> Self {
        Self { id, name, desc, cam: CamKind::Orbit { dist, pitch, yaw }, target: glam::Vec3::ZERO }
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
    InsLine { role: InsRole::Title, text: s.into(), tex: None }
}
pub fn formula(s: impl Into<String>) -> InsLine {
    InsLine { role: InsRole::Formula, text: s.into(), tex: None }
}
/// A formula typeset by LaTeX when available, with a Unicode fallback.
pub fn ftex(key: &'static str, fallback: impl Into<String>) -> InsLine {
    InsLine { role: InsRole::Formula, text: fallback.into(), tex: Some(key) }
}
pub fn def(s: impl Into<String>) -> InsLine {
    InsLine { role: InsRole::Def, text: s.into(), tex: None }
}
pub fn value(s: impl Into<String>) -> InsLine {
    InsLine { role: InsRole::Value, text: s.into(), tex: None }
}
pub fn note(s: impl Into<String>) -> InsLine {
    InsLine { role: InsRole::Note, text: s.into(), tex: None }
}

/// Pointer gestures delivered to modules on plane-camera views, in world
/// coordinates on the z = 0 plane.
#[derive(Clone, Copy, Debug)]
pub enum PointerEvent {
    Down(glam::Vec2),
    Drag(glam::Vec2),
    Up(glam::Vec2),
}

/// One family of mathematical concepts: symbolic objects, solvers, and
/// synchronized visual representations.
pub trait ConceptModule {
    fn info(&self) -> ModuleInfo;
    fn modes(&self) -> Vec<RenderMode>;
    /// Which representations open by default (indices into `modes`).
    fn default_modes(&self) -> Vec<usize> {
        vec![0]
    }
    fn params(&mut self) -> &mut Params;
    fn params_ref(&self) -> &Params;
    /// Create GPU resources; register field shaders. Called once, lazily.
    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers);
    /// Advance simulation / dynamic geometry to time `t`.
    fn update(&mut self, gpu: &GpuContext, t: f64, dt: f64);
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

pub struct Engine {
    pub gpu: GpuContext,
    pub rend: Renderers,
    pub modules: Vec<Box<dyn ConceptModule>>,
    pub timeline: Timeline,
    pub active: usize,
    /// Active representation indices for the active module (1..=4 views).
    pub active_modes: Vec<usize>,
    cameras: HashMap<(usize, usize), Camera>,
    initialized: Vec<bool>,
}

impl Engine {
    pub fn new(gpu: GpuContext, modules: Vec<Box<dyn ConceptModule>>) -> Self {
        let rend = Renderers::new(&gpu);
        let n = modules.len();
        let mut e = Self {
            gpu,
            rend,
            modules,
            timeline: Timeline::new(),
            active: 0,
            active_modes: vec![0],
            cameras: HashMap::new(),
            initialized: vec![false; n],
        };
        e.set_active(0);
        e
    }

    pub fn set_active(&mut self, idx: usize) {
        if idx >= self.modules.len() {
            return;
        }
        self.active = idx;
        if !self.initialized[idx] {
            self.modules[idx].init(&self.gpu, &mut self.rend);
            self.initialized[idx] = true;
        }
        self.active_modes = self.modules[idx].default_modes();
        self.timeline.reset();
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

    /// Step simulation time and let the active module integrate.
    pub fn tick(&mut self, real_dt: f64) {
        let dt = self.timeline.advance(real_dt);
        let t = self.timeline.t;
        let gpu = self.gpu.clone();
        self.modules[self.active].update(&gpu, t, dt);
    }

    /// Render one representation of the active module into a target.
    pub fn render_view(&mut self, mode: usize, target: &ViewTarget) {
        let t = self.timeline.t;
        let active = self.active;
        let mut cam = *self.camera(active, mode);
        cam.aspect = target.width as f32 / target.height.max(1) as f32;

        let mut scene = Scene::new();
        let gpu = self.gpu.clone();
        self.modules[active].render(&gpu, mode, t, &cam, &mut scene);

        let vp = cam.view_proj();
        let u = ViewUniforms {
            view_proj: vp.to_cols_array_2d(),
            inv_view_proj: vp.inverse().to_cols_array_2d(),
            eye: cam.eye().extend(1.0).to_array(),
            resolution: [target.width as f32, target.height as f32, t as f32, cam.aspect],
            p: scene.params,
        };
        target.write_uniforms(&self.gpu, &u);
        self.rend.render_scene(&self.gpu, target, &scene, None);
    }
}
