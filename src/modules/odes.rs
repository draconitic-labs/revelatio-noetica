//! Ordinary differential equations: slope fields, the honesty of integrators,
//! limit cycles, predator and prey, and resonance.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, rk4, with_alpha};
use crate::theme::Branch;

const TEAL: [f32; 4] = [0.35, 0.85, 0.75, 1.0];
const APRICOT: [f32; 4] = [1.0, 0.66, 0.38, 1.0];
const BONE_O: [f32; 4] = [0.92, 0.94, 0.98, 1.0];
const RED_E: [f32; 4] = [0.95, 0.40, 0.35, 1.0];

const PAPER_O: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.0);
    color += vec3<f32>(0.10, 0.24, 0.20) * grid_glow(w, 0.7) * 0.12;
    return vec4<f32>(color * (1.0 - 0.26 * dot(ndc, ndc)), 1.0);
}
"#;

const F_NAMES: [&str; 4] = [
    "logistic  y' = y(1\u{2212}y/2)",
    "y' = x \u{2212} y",
    "y' = sin(x\u{00B7}y)",
    "y' = x\u{00B2} \u{2212} y\u{00B2}",
];

fn slope(k: usize, x: f64, y: f64) -> f64 {
    match k {
        0 => y * (1.0 - y / 2.0),
        1 => x - y,
        2 => (x * y).sin(),
        _ => x * x - y * y,
    }
}

pub struct OdeWorkshop {
    params: Params,
    /// User-seeded solution curves through the slope field.
    curves: Vec<Vec<Vec2>>,
    vdp: Vec<(Vec2, Vec<Vec2>)>,
    lv: Vec<(Vec2, Vec<Vec2>)>,
    last_f: usize,
}

impl OdeWorkshop {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("f", "Right-hand side", "Symbolic", F_NAMES.to_vec(), 1);
        p.slider("h", "Integrator step h", "Physics", 0.05, 0.6, 0.35, "");
        p.slider("mu", "Van der Pol \u{03BC}", "Physics", 0.2, 4.0, 1.6, "");
        p.slider("zeta", "Damping \u{03B6}", "Physics", 0.02, 0.9, 0.12, "");
        p.slider(
            "omega",
            "Drive frequency \u{03C9}",
            "Physics",
            0.2,
            2.4,
            0.85,
            "",
        );
        p.slider("r0", "Epidemic R\u{2080}", "Physics", 0.8, 5.0, 2.6, "");
        Self {
            params: p,
            curves: Vec::new(),
            vdp: Vec::new(),
            lv: Vec::new(),
            last_f: usize::MAX,
        }
    }

    fn solve_through(&self, k: usize, p0: Vec2) -> Vec<Vec2> {
        let f = move |x: f64, y: &[f64; 1]| [slope(k, x, y[0])];
        let mut out = Vec::new();
        for dir in [1.0f64, -1.0] {
            let mut x = p0.x as f64;
            let mut y = [p0.y as f64];
            let mut leg = vec![Vec2::new(x as f32, y[0] as f32)];
            for _ in 0..260 {
                let h = 0.03 * dir;
                y = rk4(&f, x, &y, h);
                x += h;
                if !y[0].is_finite() || y[0].abs() > 6.0 || x.abs() > 5.0 {
                    break;
                }
                leg.push(Vec2::new(x as f32, y[0] as f32));
            }
            if dir < 0.0 {
                leg.reverse();
                leg.extend(out);
                out = leg;
            } else {
                out.extend(leg);
            }
        }
        out
    }

    fn seed_defaults(&mut self) {
        let k = self.params.c("f");
        self.curves = (-3..=3)
            .map(|i| self.solve_through(k, Vec2::new(-3.6, i as f32 * 0.9)))
            .collect();
        self.vdp = (0..7)
            .map(|i| {
                let a = i as f32 / 7.0 * std::f32::consts::TAU;
                (
                    Vec2::new(
                        a.cos() * (0.3 + 0.5 * i as f32),
                        a.sin() * (0.3 + 0.4 * i as f32),
                    ),
                    Vec::new(),
                )
            })
            .collect();
        self.lv = [(2.2f32, 1.1f32), (1.6, 0.8), (3.0, 1.5), (1.2, 0.5)]
            .iter()
            .map(|&(x, y)| (Vec2::new(x, y), Vec::new()))
            .collect();
    }
}

impl ConceptModule for OdeWorkshop {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "diffeq.odes",
            name: "The Integrator's Workshop",
            tagline: "One equation, every future",
            branch: Branch::DiffEq,
            description: "A differential equation does not give you the answer; it gives you the rule the answer must obey at every instant. The slope field shows all futures at once, existence and uniqueness threading exactly one curve through each point. And when we integrate numerically, honesty demands we watch the error: Euler's polygon drifts where Runge\u{2013}Kutta holds.",
            epigraph: Some((
                "It is useful to solve differential equations.",
                "Isaac Newton (as Arnold liked to translate the second anagram)",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "slope",
                "Slope Field",
                "The rule everywhere; click to thread a solution through",
                3.4,
            ),
            RenderMode::plane(
                "euler",
                "Euler vs Runge\u{2013}Kutta",
                "Same equation, same step: one polygon lies, one holds",
                2.4,
            ),
            RenderMode::plane(
                "vdp",
                "Van der Pol Cycle",
                "Every start forgets itself; the limit cycle remains",
                3.6,
            ),
            RenderMode::plane(
                "lv",
                "Predator & Prey",
                "Lotka\u{2013}Volterra: closed orbits around an uneasy truce",
                2.6,
            )
            .look_at(2.2, 1.6, 0.0),
            RenderMode::plane(
                "resonance",
                "Resonance",
                "Drive a damped oscillator; the response curve knows where it hurts",
                1.9,
            )
            .look_at(1.4, 0.9, 0.0),
            RenderMode::plane(
                "sir",
                "The SIR Epidemic",
                "Susceptible, infected, recovered: one nonlinearity writes every plague",
                0.72,
            )
            .look_at(2.0, 0.55, 0.0),
        ]
    }

    fn default_modes(&self) -> Vec<usize> {
        vec![0, 1]
    }
    fn params(&mut self) -> &mut Params {
        &mut self.params
    }
    fn params_ref(&self) -> &Params {
        &self.params
    }

    fn wants_pointer(&self, mode: usize) -> bool {
        mode == 0
    }
    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode == 0).then_some("click to thread a solution")
    }
    fn pointer(&mut self, _mode: usize, ev: PointerEvent) {
        if let PointerEvent::Down(w) = ev {
            let k = self.params.c("f");
            if self.curves.len() > 22 {
                self.curves.remove(0);
            }
            let c = self.solve_through(k, w);
            self.curves.push(c);
        }
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "ode.paper", PAPER_O, false);
        self.seed_defaults();
    }

    fn rewind(&mut self) {
        self.seed_defaults();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, dt: f64) {
        // A new right-hand side needs new solutions. A new mu does not: the same
        // orbits carry on under it, and clicked curves and trails are kept.
        let f = self.params.c("f");
        if f != self.last_f {
            self.last_f = f;
            self.seed_defaults();
        }
        let h = dt.abs().min(0.033);
        if h <= 0.0 {
            return;
        }
        // Van der Pol flows.
        let mu = self.params.f("mu");
        let fv = move |_t: f64, y: &[f64; 2]| [y[1], mu * (1.0 - y[0] * y[0]) * y[1] - y[0]];
        for (p, trail) in &mut self.vdp {
            let mut y = [p.x as f64, p.y as f64];
            for _ in 0..2 {
                y = rk4(&fv, 0.0, &y, h);
            }
            *p = Vec2::new(y[0] as f32, y[1] as f32);
            trail.push(*p);
            if trail.len() > 1400 {
                trail.remove(0);
            }
        }
        // Lotka-Volterra (a=1.1, b=0.9, c=0.9, d=0.6).
        let fl = |_t: f64, y: &[f64; 2]| [y[0] * (1.1 - 0.9 * y[1]), -y[1] * (0.9 - 0.6 * y[0])];
        for (p, trail) in &mut self.lv {
            let mut y = [p.x as f64, p.y as f64];
            for _ in 0..2 {
                y = rk4(&fl, 0.0, &y, h * 1.4);
            }
            *p = Vec2::new(y[0] as f32, y[1] as f32);
            trail.push(*p);
            if trail.len() > 2000 {
                trail.remove(0);
            }
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        scene.field = Some("ode.paper");
        match mode {
            0 => {
                let k = self.params.c("f");
                // The rule, everywhere: short slope ticks.
                let n = 23;
                for i in 0..=n {
                    for j in 0..=n {
                        let x = -3.8 + 7.6 * i as f64 / n as f64;
                        let y = -3.2 + 6.4 * j as f64 / n as f64;
                        let s = slope(k, x, y);
                        let d = Vec2::new(1.0, s as f32).normalize_or_zero() * 0.115;
                        let c = Vec3::new(x as f32, y as f32, 0.0);
                        scene.line(
                            LineLayer::World,
                            c - Vec3::new(d.x, d.y, 0.0),
                            c + Vec3::new(d.x, d.y, 0.0),
                            1.15,
                            with_alpha(TEAL, 0.35),
                        );
                    }
                }
                for (ci, curve) in self.curves.iter().enumerate() {
                    let hue = phase_color(ci as f32 * 0.75 + 0.3);
                    let pts: Vec<Vec3> = curve.iter().map(|p| Vec3::new(p.x, p.y, 0.02)).collect();
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.8, with_alpha(hue, 0.7));
                }
            }
            1 => {
                // Test equation: circular motion (x,y)' = (y, -x); exact circle.
                let ring: Vec<Vec3> = (0..=128)
                    .map(|i| {
                        let a = i as f32 / 128.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos() * 1.6, a.sin() * 1.6, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &ring, 1.3, with_alpha(BONE_O, 0.45));
                let h = self.params.f("h");
                let steps = ((t * 3.0) as usize).min(400);
                // Euler.
                let mut pe = [1.6f64, 0.0f64];
                let mut euler = vec![Vec3::new(1.6, 0.0, 0.0)];
                for _ in 0..steps {
                    let d = [pe[1], -pe[0]];
                    pe = [pe[0] + h * d[0], pe[1] + h * d[1]];
                    euler.push(Vec3::new(pe[0] as f32, pe[1] as f32, 0.01));
                    if pe[0].abs() > 8.0 {
                        break;
                    }
                }
                // RK4.
                let f = |_t: f64, y: &[f64; 2]| [y[1], -y[0]];
                let mut pr = [1.6f64, 0.0];
                let mut rk = vec![Vec3::new(1.6, 0.0, 0.0)];
                for _ in 0..steps {
                    pr = rk4(&f, 0.0, &pr, h);
                    rk.push(Vec3::new(pr[0] as f32, pr[1] as f32, 0.02));
                }
                scene.polyline(LineLayer::WorldAdd, &euler, 1.7, with_alpha(RED_E, 0.75));
                scene.polyline(LineLayer::WorldAdd, &rk, 1.9, with_alpha(TEAL, 0.9));
                if let (Some(&e), Some(&r)) = (euler.last(), rk.last()) {
                    scene.point(e, 4.0, RED_E, 1.8);
                    scene.point(r, 4.0, TEAL, 1.8);
                }
            }
            2 => {
                for (i, (p, trail)) in self.vdp.iter().enumerate() {
                    let hue = phase_color(i as f32 * 0.9);
                    let pts: Vec<Vec3> = trail.iter().map(|q| Vec3::new(q.x, q.y, 0.0)).collect();
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.4, with_alpha(hue, 0.35));
                    scene.point(Vec3::new(p.x, p.y, 0.02), 3.6, hue, 1.8);
                }
            }
            3 => {
                for (i, (p, trail)) in self.lv.iter().enumerate() {
                    let hue = phase_color(i as f32 * 1.1 + 0.5);
                    let pts: Vec<Vec3> = trail.iter().map(|q| Vec3::new(q.x, q.y, 0.0)).collect();
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.5, with_alpha(hue, 0.5));
                    scene.point(Vec3::new(p.x, p.y, 0.02), 4.0, hue, 2.0);
                }
                // Equilibrium.
                scene.point(
                    Vec3::new(1.5, 1.2222, 0.02),
                    3.4,
                    with_alpha(BONE_O, 0.9),
                    1.6,
                );
                scene.line(
                    LineLayer::World,
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(4.6, 0.0, 0.0),
                    1.1,
                    [0.5, 0.55, 0.5, 0.4],
                );
                scene.line(
                    LineLayer::World,
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(0.0, 3.4, 0.0),
                    1.1,
                    [0.5, 0.55, 0.5, 0.4],
                );
            }
            5 => {
                // SIR integrated fresh each frame up to the timeline clock.
                let r0 = self.params.f("r0");
                let gamma = 0.55;
                let beta = r0 * gamma;
                let f = move |_t: f64, y: &[f64; 3]| {
                    [
                        -beta * y[0] * y[1],
                        beta * y[0] * y[1] - gamma * y[1],
                        gamma * y[1],
                    ]
                };
                let t_end = (t / 12.0).clamp(0.0, 1.0) * 22.0;
                let mut y = [0.999, 0.001, 0.0];
                let n_st = 620;
                let (mut s_c, mut i_c, mut r_c) = (Vec::new(), Vec::new(), Vec::new());
                for i in 0..=n_st {
                    let tt = t_end * i as f64 / n_st as f64;
                    let x = (tt / 22.0 * 4.4) as f32;
                    s_c.push(Vec3::new(x, y[0] as f32, 0.0));
                    i_c.push(Vec3::new(x, y[1] as f32, 0.0));
                    r_c.push(Vec3::new(x, y[2] as f32, 0.0));
                    y = rk4(&f, tt, &y, t_end / n_st as f64);
                }
                scene.line(
                    LineLayer::World,
                    Vec3::ZERO,
                    Vec3::new(4.5, 0.0, 0.0),
                    1.1,
                    [0.5, 0.55, 0.5, 0.4],
                );
                scene.polyline(LineLayer::WorldAdd, &s_c, 1.9, with_alpha(TEAL, 0.85));
                scene.glow_polyline(&i_c, 2.2, RED_E, 2.6);
                scene.polyline(LineLayer::WorldAdd, &r_c, 1.9, with_alpha(BONE_O, 0.6));
                // Herd-immunity threshold on S.
                let hit = 1.0 / r0;
                scene.line(
                    LineLayer::Overlay,
                    Vec3::new(0.0, hit as f32, 0.0),
                    Vec3::new(4.5, hit as f32, 0.0),
                    0.9,
                    with_alpha(APRICOT, 0.4),
                );
            }
            _ => {
                let zeta = self.params.f("zeta");
                let om_d = self.params.f("omega");
                // Amplitude response |H| = 1/sqrt((1-w^2)^2 + (2 zeta w)^2).
                let resp: Vec<Vec3> = (0..=300)
                    .map(|i| {
                        let w = 0.05 + 2.6 * i as f64 / 300.0;
                        let h = 1.0 / (((1.0 - w * w).powi(2) + (2.0 * zeta * w).powi(2)).sqrt());
                        Vec3::new(w as f32, (h * 0.55) as f32, 0.0)
                    })
                    .collect();
                scene.glow_polyline(&resp, 2.0, TEAL, 2.6);
                let h_now =
                    1.0 / (((1.0 - om_d * om_d).powi(2) + (2.0 * zeta * om_d).powi(2)).sqrt());
                scene.line(
                    LineLayer::WorldAdd,
                    Vec3::new(om_d as f32, 0.0, 0.0),
                    Vec3::new(om_d as f32, (h_now * 0.55) as f32, 0.0),
                    1.8,
                    with_alpha(APRICOT, 0.8),
                );
                // The oscillator, breathing at its steady state.
                let phase = (om_d * t * 3.0).cos();
                let x = h_now * phase * 0.5;
                let bob = Vec3::new(3.4, 0.9 + x as f32, 0.0);
                scene.line(
                    LineLayer::World,
                    Vec3::new(3.4, 2.1, 0.0),
                    bob,
                    1.6,
                    with_alpha(BONE_O, 0.7),
                );
                scene.point(bob, 7.0, APRICOT, 2.4);
                scene.line(
                    LineLayer::World,
                    Vec3::new(3.1, 2.1, 0.0),
                    Vec3::new(3.7, 2.1, 0.0),
                    2.2,
                    with_alpha(BONE_O, 0.8),
                );
                scene.line(
                    LineLayer::World,
                    Vec3::ZERO,
                    Vec3::new(2.9, 0.0, 0.0),
                    1.1,
                    [0.5, 0.55, 0.5, 0.4],
                );
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let k = self.params.c("f");
        let h = self.params.f("h");
        let zeta = self.params.f("zeta");
        vec![
            title("dy/dx = f(x, y)"),
            formula(F_NAMES[k]),
            ftex("taylor.series", "solutions exist & are unique where f is Lipschitz"),
            value(format!("Euler error per step ~ h\u{00B2} = {:.3};  RK4 ~ h\u{2075} = {:.6}", h * h, h.powi(5))),
            value(format!("resonance peak at \u{03C9} \u{2248} \u{221A}(1\u{2212}2\u{03B6}\u{00B2}) = {:.3}", (1.0f64 - 2.0 * zeta * zeta).max(0.0).sqrt())),
            note("Van der Pol: the nonlinearity pumps small oscillations and damps large ones \u{2014} everything is herded onto one closed curve, the limit cycle."),
            note("Lotka\u{2013}Volterra orbits are level sets of a conserved quantity: the truce between fox and hare never settles, only circles."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("RK4 for threaded curves; slope ticks exact."),
            1 => Some("Both integrators shown honestly at the SAME step h \u{2014} the drift is Euler's real global error."),
            2 => Some("RK4, two steps of up to 0.033 per frame."),
            3 => Some("RK4, two steps of up to 0.046 per frame."),
            4 => Some("Closed-form steady-state response, with the time trace drawn from the same formula."),
            5 => Some("SIR by RK4, integrated fresh each frame from t = 0 up to the time line."),
            _ => None,
        }
    }
}
