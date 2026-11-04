//! Special relativity: the electrodynamics of moving bodies.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const CONE_GOLD: [f32; 4] = [0.95, 0.75, 0.30, 1.0];
const FRAME_BLUE: [f32; 4] = [0.42, 0.65, 0.95, 1.0];
const REST_GREY: [f32; 4] = [0.60, 0.65, 0.75, 1.0];
const SPACELIKE: [f32; 4] = [0.40, 0.50, 0.95, 1.0];

const PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.010, 0.011, 0.016);
    color += vec3<f32>(0.20, 0.16, 0.08) * grid_glow(w, 0.7) * 0.10;
    // The light cone glow: |x| = |ct| diagonals.
    let d1 = abs(w.x - w.y);
    let d2 = abs(w.x + w.y);
    color += vec3<f32>(0.55, 0.42, 0.12) * (exp(-d1 * d1 * 40.0) + exp(-d2 * d2 * 40.0)) * 0.10;
    return vec4<f32>(color * (1.0 - 0.28 * dot(ndc, ndc)), 1.0);
}
"#;

/// Cone-free paper for the scenes that are not spacetime diagrams.
const PLAIN_PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.010, 0.011, 0.016);
    color += vec3<f32>(0.20, 0.16, 0.08) * grid_glow(w, 0.7) * 0.10;
    return vec4<f32>(color * (1.0 - 0.28 * dot(ndc, ndc)), 1.0);
}
"#;

/// Exact boosted-Coulomb field magnitude heat.
const CHARGE_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let beta = U.p[0].x;
    let r2 = max(dot(w, w), 0.002);
    let sin2 = clamp(w.y * w.y / r2, 0.0, 1.0);
    // |E| = q(1-b^2) / (r^2 (1 - b^2 sin^2 psi)^{3/2}), field from present position.
    let e_mag = (1.0 - beta * beta) / (r2 * pow(1.0 - beta * beta * sin2, 1.5));
    var color = vec3<f32>(0.008, 0.009, 0.014);
    color += aurum(clamp(log(1.0 + e_mag * 0.8) * 0.26, 0.0, 1.0)) * 0.72;
    // B = beta x E out of/into the page: tint magenta/cyan by sign of B_z ~ beta * E_y-ish.
    let bz = beta * w.y / (r2 * pow(1.0 - beta * beta * sin2, 1.5));
    color += vec3<f32>(0.35, 0.05, 0.30) * clamp(bz * 0.5, 0.0, 1.0) * 0.5;
    color += vec3<f32>(0.05, 0.30, 0.35) * clamp(-bz * 0.5, 0.0, 1.0) * 0.5;
    return vec4<f32>(color * (1.0 - 0.25 * dot(ndc, ndc)), 1.0);
}
"#;

pub struct RelativityTheatre {
    params: Params,
    beta_now: f64,
    ev_a: glam::Vec2,
    ev_b: glam::Vec2,
}

impl RelativityTheatre {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider("beta", "Boost velocity \u{03B2}", "Observer", -0.9, 0.9, 0.6, "c");
        p.toggle("animate", "Sweep \u{03B2} with time", "Observer", true);
        p.toggle("grid", "Boosted coordinate grid", "Display", true);
        Self {
            params: p,
            beta_now: 0.0,
            ev_a: glam::vec2(-1.6, 0.9),
            ev_b: glam::vec2(1.8, 1.25),
        }
    }

    fn beta(&self) -> f64 {
        self.beta_now
    }
}

impl ConceptModule for RelativityTheatre {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "relativity.special",
            name: "The Simultaneity Theatre",
            tagline: "On the electrodynamics of moving bodies",
            branch: Branch::Relativity,
            description: "Einstein's 1905 insight was not about clocks failing but about geometry: space and time mix under boosts the way x and y mix under rotations \u{2014} except the invariant is c\u{00B2}t\u{00B2}\u{2212}x\u{00B2}, and the diagonals, the light cone, never move. Simultaneity tilts; lengths contract; the Coulomb field of a running charge flattens into a pancake.",
            epigraph: Some((
                "Henceforth space by itself, and time by itself, are doomed to fade away into mere shadows.",
                "Hermann Minkowski, 1908",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane("minkowski", "Minkowski Diagram", "Boosted axes tilt toward the cone; the cone stands still", 3.4),
            RenderMode::plane("clock", "Light Clock & Rod", "One photon, two frames: time dilation and length contraction", 3.0)
                .look_at(0.0, 1.4, 0.0),
            RenderMode::plane("charge", "Boosted Coulomb Field", "The field of a moving charge flattens by 1/(1\u{2212}\u{03B2}\u{00B2}sin\u{00B2}\u{03C8})^{3/2}", 3.2)
                .look_at(0.0, 0.0, 0.0),
            RenderMode::plane("twins", "The Twin Wager", "Same reunion, different proper times: the traveler's clock ticks less", 3.4)
                .look_at(0.0, 2.6, 0.0),
        ]
    }

    fn default_modes(&self) -> Vec<usize> {
        vec![0]
    }
    fn params(&mut self) -> &mut Params {
        &mut self.params
    }
    fn params_ref(&self) -> &Params {
        &self.params
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "sr.paper", PAPER, false);
        rend.register_field(gpu, "sr.plain", PLAIN_PAPER, false);
        rend.register_field(gpu, "sr.charge", CHARGE_SHADE, false);
    }

    fn update(&mut self, _gpu: &GpuContext, t: f64, _dt: f64) {
        self.beta_now = if self.params.b("animate") {
            0.85 * (t * std::f64::consts::TAU / 12.0).sin()
        } else {
            self.params.f("beta")
        };
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        let beta = self.beta();
        let gamma = 1.0 / (1.0 - beta * beta).sqrt();

        match mode {
            0 => {
                scene.field = Some("sr.paper");
                // Rest axes.
                scene.line(LineLayer::World, Vec3::new(-3.6, 0.0, 0.0), Vec3::new(3.6, 0.0, 0.0), 1.4, with_alpha(REST_GREY, 0.7));
                scene.line(LineLayer::World, Vec3::new(0.0, -3.6, 0.0), Vec3::new(0.0, 3.6, 0.0), 1.4, with_alpha(REST_GREY, 0.7));
                // The cone.
                for s in [-1.0f32, 1.0] {
                    scene.line_soft(LineLayer::WorldAdd, Vec3::new(-3.8, -3.8 * s, 0.0), Vec3::new(3.8, 3.8 * s, 0.0), 2.6, with_alpha(CONE_GOLD, 0.5));
                }
                // Boosted frame: x' axis slope beta, t' axis slope 1/beta.
                let b = beta as f32;
                if self.params.b("grid") {
                    // Lines of constant t' and x' (hyperbolic-calibrated lattice).
                    for k in -4..=4 {
                        let kf = k as f32;
                        // t' = k: x = gamma*(x' + b ct'), ct = gamma*(ct' + b x')
                        let g = gamma as f32;
                        let p0 = Vec3::new(g * (-4.0 + b * kf), g * (kf + b * -4.0), 0.0);
                        let p1 = Vec3::new(g * (4.0 + b * kf), g * (kf + b * 4.0), 0.0);
                        scene.line(LineLayer::World, p0, p1, 1.0, with_alpha(FRAME_BLUE, if k == 0 { 0.0 } else { 0.20 }));
                        let q0 = Vec3::new(g * (kf + b * -4.0), g * (-4.0 + b * kf), 0.0);
                        let q1 = Vec3::new(g * (kf + b * 4.0), g * (4.0 + b * kf), 0.0);
                        scene.line(LineLayer::World, q0, q1, 1.0, with_alpha(FRAME_BLUE, if k == 0 { 0.0 } else { 0.20 }));
                    }
                }
                // Primed axes bright.
                scene.line(LineLayer::WorldAdd, Vec3::new(-3.6, -3.6 * b, 0.0), Vec3::new(3.6, 3.6 * b, 0.0), 2.0, with_alpha(FRAME_BLUE, 0.85));
                scene.line(LineLayer::WorldAdd, Vec3::new(-3.6 * b, -3.6, 0.0), Vec3::new(3.6 * b, 3.6, 0.0), 2.0, with_alpha(FRAME_BLUE, 0.85));
                // Invariant hyperbolae s^2 = +-1: calibration curves.
                for sign in [1.0f32, -1.0] {
                    let hyp: Vec<Vec3> = (-60..=60)
                        .map(|i| {
                            let a = i as f32 / 20.0;
                            if sign > 0.0 {
                                Vec3::new(a.sinh(), a.cosh() * 1.0, 0.0)
                            } else {
                                Vec3::new(a.cosh(), a.sinh(), 0.0)
                            }
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &hyp, 1.0, with_alpha(CONE_GOLD, 0.22));
                    let neg: Vec<Vec3> = hyp.iter().map(|p| Vec3::new(-p.x, -p.y, 0.0)).collect();
                    scene.polyline(LineLayer::World, &neg, 1.0, with_alpha(CONE_GOLD, 0.22));
                }
                // Two draggable events; their time-order depends on the frame
                // whenever their separation is spacelike.
                let ea = Vec3::new(self.ev_a.x, self.ev_a.y, 0.0);
                let eb = Vec3::new(self.ev_b.x, self.ev_b.y, 0.0);
                scene.point(ea, 4.5, CONE_GOLD, 2.0);
                scene.point(eb, 4.5, SPACELIKE, 2.0);
                // The chord between them, tinted by its causal character.
                let d = self.ev_b - self.ev_a;
                let spacelike = d.x.abs() > d.y.abs();
                let chord_c = if spacelike { with_alpha(SPACELIKE, 0.35) } else { with_alpha(CONE_GOLD, 0.35) };
                scene.line(LineLayer::World, ea, eb, 1.2, chord_c);
                // Simultaneity slice through A in the moving frame.
                let dir = Vec3::new(1.0, b, 0.0).normalize();
                scene.line_soft(LineLayer::WorldAdd, ea - dir * 6.0, ea + dir * 6.0, 2.0, with_alpha(FRAME_BLUE, 0.30));
            }
            1 => {
                scene.field = Some("sr.plain");
                let g = gamma as f32;
                let b = beta.abs() as f32;
                let h = 1.8f32;
                // Rest clock at left: photon bounces vertically.
                let phase = ((t * 1.4) % 2.0) as f32;
                let yb = if phase < 1.0 { phase * h } else { (2.0 - phase) * h };
                let x_rest = -2.6f32;
                scene.line(LineLayer::World, Vec3::new(x_rest - 0.5, 0.0, 0.0), Vec3::new(x_rest + 0.5, 0.0, 0.0), 2.4, with_alpha(REST_GREY, 0.9));
                scene.line(LineLayer::World, Vec3::new(x_rest - 0.5, h, 0.0), Vec3::new(x_rest + 0.5, h, 0.0), 2.4, with_alpha(REST_GREY, 0.9));
                scene.point(Vec3::new(x_rest, yb, 0.0), 4.0, CONE_GOLD, 2.2);
                scene.line_soft(LineLayer::WorldAdd, Vec3::new(x_rest, 0.0, 0.0), Vec3::new(x_rest, yb, 0.0), 1.6, with_alpha(CONE_GOLD, 0.25));
                // Moving clock: mirrors glide right; photon zigzags, its period dilated by gamma.
                let x0 = -0.8f32;
                let speed = 1.1f32 * b;
                let tt = (t as f32 * 1.4 / g) % 2.0; // proper ticks slowed by gamma
                let frac = if tt < 1.0 { tt } else { 2.0 - tt };
                let mx = x0 + speed * (t as f32 * 1.4);
                let mx_wrapped = x0 + ((mx - x0) % 4.2 + 4.2) % 4.2;
                scene.line(LineLayer::World, Vec3::new(mx_wrapped - 0.5, 0.0, 0.0), Vec3::new(mx_wrapped + 0.5, 0.0, 0.0), 2.4, with_alpha(FRAME_BLUE, 0.9));
                scene.line(LineLayer::World, Vec3::new(mx_wrapped - 0.5, h, 0.0), Vec3::new(mx_wrapped + 0.5, h, 0.0), 2.4, with_alpha(FRAME_BLUE, 0.9));
                let py = frac * h;
                scene.point(Vec3::new(mx_wrapped, py, 0.0), 4.0, CONE_GOLD, 2.2);
                // The zigzag path ghost (one period).
                let zig: Vec<Vec3> = (0..=32)
                    .map(|i| {
                        let f = i as f32 / 32.0 * 2.0;
                        let yy = if f < 1.0 { f * h } else { (2.0 - f) * h };
                        Vec3::new(mx_wrapped - speed * g * (2.0 - f) * h / 1.1, yy, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &zig, 1.0, with_alpha(CONE_GOLD, 0.28));
                // Contracted rod beneath.
                let rod_l = 1.6f32 / g;
                scene.line(LineLayer::WorldAdd, Vec3::new(mx_wrapped - rod_l / 2.0, -0.8, 0.0), Vec3::new(mx_wrapped + rod_l / 2.0, -0.8, 0.0), 4.0, FRAME_BLUE);
                scene.line(LineLayer::World, Vec3::new(x_rest - 0.8, -0.8, 0.0), Vec3::new(x_rest + 0.8, -0.8, 0.0), 4.0, with_alpha(REST_GREY, 0.9));
            }
            3 => {
                scene.field = Some("sr.paper");
                let b = beta.abs().max(0.25) as f32;
                let g = 1.0 / (1.0 - b * b).sqrt();
                let turn = 2.6f32; // ct at turnaround
                // Stay-home twin: straight up. Traveler: out and back.
                scene.line(LineLayer::WorldAdd, Vec3::ZERO, Vec3::new(0.0, 2.0 * turn, 0.0), 2.4, with_alpha(REST_GREY, 0.9));
                scene.line(LineLayer::WorldAdd, Vec3::ZERO, Vec3::new(b * turn, turn, 0.0), 2.4, with_alpha(FRAME_BLUE, 0.95));
                scene.line(LineLayer::WorldAdd, Vec3::new(b * turn, turn, 0.0), Vec3::new(0.0, 2.0 * turn, 0.0), 2.4, with_alpha(FRAME_BLUE, 0.95));
                // Light cone from the start, for scale.
                for s in [-1.0f32, 1.0] {
                    scene.line_soft(LineLayer::World, Vec3::ZERO, Vec3::new(3.4 * s, 3.4, 0.0), 2.0, with_alpha(CONE_GOLD, 0.25));
                }
                // Proper-time ticks: every Delta-tau = 0.4 along each worldline.
                let dtau = 0.4f32;
                let mut tau = 0.0;
                while tau * dtau < 2.0 * turn {
                    let y = tau * dtau;
                    scene.point(Vec3::new(0.0, y, 0.02), 2.6, with_alpha(REST_GREY, 0.95), 1.2);
                    tau += 1.0;
                }
                // Traveler ticks are dilated: coordinate spacing g*dtau.
                let mut k = 0;
                loop {
                    let y = k as f32 * g * dtau;
                    if y > 2.0 * turn {
                        break;
                    }
                    let x = if y <= turn { b * y } else { b * (2.0 * turn - y) };
                    scene.point(Vec3::new(x, y, 0.03), 2.6, with_alpha(CONE_GOLD, 0.95), 1.4);
                    k += 1;
                }
                // The reunion.
                scene.point(Vec3::new(0.0, 2.0 * turn, 0.03), 5.0, [0.95, 0.95, 1.0, 1.0], 2.2);
            }
            _ => {
                scene.field = Some("sr.charge");
                scene.p(0, beta as f32);
                // Exact field lines: tan(psi_lab) = gamma tan(psi_rest).
                let g = gamma;
                for k in 0..24 {
                    // Uniform flux in the rest frame: cos(psi) uniform.
                    let cpsi = -1.0 + 2.0 * (k as f64 + 0.5) / 24.0;
                    let psi_rest = cpsi.acos();
                    let psi_lab = (g * psi_rest.tan()).atan();
                    let psi_lab = if psi_rest > std::f64::consts::FRAC_PI_2 {
                        psi_lab + std::f64::consts::PI
                    } else {
                        psi_lab
                    };
                    for mirror in [1.0f64, -1.0] {
                        let dir = Vec3::new(psi_lab.cos() as f32, (psi_lab.sin() * mirror) as f32, 0.0);
                        scene.line_soft(
                            LineLayer::WorldAdd,
                            Vec3::ZERO + dir * 0.14,
                            dir * 5.4,
                            2.0,
                            with_alpha(CONE_GOLD, 0.30),
                        );
                    }
                }
                // The charge, running.
                scene.point(Vec3::ZERO, 4.2, [1.0, 0.92, 0.7, 1.0], 1.8);
                scene.vector(Vec3::ZERO, Vec3::new(beta as f32 * 2.2, 0.0, 0.0), 4.0, FRAME_BLUE);
            }
        }
    }

    fn wants_pointer(&self, mode: usize) -> bool {
        mode == 0
    }

    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode == 0).then_some("drag the two events; watch their order swap")
    }

    fn pointer(&mut self, _mode: usize, ev: crate::engine::module::PointerEvent) {
        use crate::engine::module::PointerEvent as PE;
        if let PE::Down(w) | PE::Drag(w) = ev {
            let da = (w - self.ev_a).length_squared();
            let db = (w - self.ev_b).length_squared();
            let target = if da <= db { &mut self.ev_a } else { &mut self.ev_b };
            *target = w.clamp(glam::vec2(-3.4, -3.4), glam::vec2(3.4, 3.4));
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let beta = self.beta();
        let gamma = 1.0 / (1.0 - beta * beta).sqrt();
        // Time order of the two events in the moving frame: t' = g(t - b x).
        let ta = gamma * (self.ev_a.y as f64 - beta * self.ev_a.x as f64);
        let tb = gamma * (self.ev_b.y as f64 - beta * self.ev_b.x as f64);
        vec![
            title("Lorentz boost along x"),
            ftex("sr.lorentz", "ct' = \u{03B3}(ct \u{2212} \u{03B2}x)      x' = \u{03B3}(x \u{2212} \u{03B2}ct)"),
            ftex("sr.interval", "s\u{00B2} = c\u{00B2}t\u{00B2} \u{2212} x\u{00B2}   invariant"),
            ftex("sr.fields", "E\u{2225}' = E\u{2225}      E\u{22A5}' = \u{03B3}E\u{22A5}      B' = \u{2212}\u{03B3}(\u{03B2}/c)\u{00D7}E"),
            value(format!("\u{03B2} = {beta:+.3}    \u{03B3} = {gamma:.3}")),
            value(format!(
                "t'(gold) = {ta:+.2}   t'(blue) = {tb:+.2}   {}",
                if ta > tb { "gold is LATER in this frame" } else { "blue is later in this frame" }
            )),
            value(format!("time dilation \u{0394}t = \u{03B3}\u{0394}\u{03C4} = {gamma:.3}\u{0394}\u{03C4}    contraction L = L\u{2080}/\u{03B3} = {:.3}L\u{2080}", 1.0 / gamma)),
            note("The tilted blue slice is 'now' for the moving observer: the gold and blue events change order as \u{03B2} sweeps \u{2014} allowed, because they are spacelike separated."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            2 => Some("Exact uniform-velocity field (no radiation term); line directions via tan\u{03C8} = \u{03B3}tan\u{03C8}\u{2080}."),
            _ => Some("Geometrically exact; hyperbolae are the unit-interval calibration curves."),
        }
    }

    fn overlay(&self, painter: &egui::Painter, rect: egui::Rect, cam: &Camera, mode: usize, _t: f64) {
        if mode == 1 {
            let gamma = 1.0 / (1.0 - self.beta() * self.beta()).sqrt();
            let gold = egui::Color32::from_rgb(235, 195, 110);
            if let Some(p) = to_screen(cam, rect, Vec3::new(-2.6, 2.5, 0.0)) {
                painter.text(p, egui::Align2::CENTER_BOTTOM, "at rest", crate::theme::math_font(12.0), crate::theme::OVERLAY_DIM);
                painter.text(p + egui::vec2(0.0, -16.0), egui::Align2::CENTER_BOTTOM, format!("ticks \u{00D7} 1.000"), crate::theme::math_font(11.0), gold);
            }
            if let Some(p) = to_screen(cam, rect, Vec3::new(1.6, 2.5, 0.0)) {
                painter.text(p, egui::Align2::CENTER_BOTTOM, "moving", crate::theme::math_font(12.0), crate::theme::OVERLAY_DIM);
                painter.text(p + egui::vec2(0.0, -16.0), egui::Align2::CENTER_BOTTOM, format!("ticks \u{00D7} 1/\u{03B3} = {:.3}", 1.0 / gamma), crate::theme::math_font(11.0), gold);
            }
        }
        if mode == 0 {
            let gold = egui::Color32::from_rgb(235, 195, 110);
            let blue = egui::Color32::from_rgb(130, 175, 240);
            if let Some(p) = to_screen(cam, rect, Vec3::new(3.3, 0.15, 0.0)) {
                painter.text(p, egui::Align2::LEFT_BOTTOM, "x", crate::theme::math_font(14.0), egui::Color32::from_rgb(160, 168, 185));
            }
            if let Some(p) = to_screen(cam, rect, Vec3::new(0.12, 3.3, 0.0)) {
                painter.text(p, egui::Align2::LEFT_BOTTOM, "ct", crate::theme::math_font(14.0), egui::Color32::from_rgb(160, 168, 185));
            }
            let b = self.beta() as f32;
            if let Some(p) = to_screen(cam, rect, Vec3::new(3.2, 3.2 * b + 0.15, 0.0)) {
                painter.text(p, egui::Align2::LEFT_BOTTOM, "x\u{2032}", crate::theme::math_font(14.0), blue);
            }
            if let Some(p) = to_screen(cam, rect, Vec3::new(3.2 * b + 0.12, 3.2, 0.0)) {
                painter.text(p, egui::Align2::LEFT_BOTTOM, "ct\u{2032}", crate::theme::math_font(14.0), blue);
            }
            if let Some(p) = to_screen(cam, rect, Vec3::new(2.9, 2.9, 0.0)) {
                painter.text(p, egui::Align2::RIGHT_BOTTOM, "light", crate::theme::math_font(11.0), gold.gamma_multiply(0.7));
            }
        }
    }
}
