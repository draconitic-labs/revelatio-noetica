//! Phase space and Hamiltonian mechanics: the pendulum's portrait,
//! its energy landscape, and the double pendulum's descent into chaos.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, rk4, with_alpha};
use crate::theme::Branch;

const CORAL: [f32; 4] = [0.98, 0.55, 0.32, 1.0];
const SEA: [f32; 4] = [0.40, 0.85, 0.78, 1.0];
const BONE: [f32; 4] = [0.92, 0.94, 0.98, 1.0];

/// Energy heat + separatrix glow for the pendulum, H = w^2/2 - k cos(theta).
const PHASE_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let k = U.p[0].x;
    let h = 0.5 * w.y * w.y - k * cos(w.x);
    var color = vec3<f32>(0.0);
    // Level lines of the Hamiltonian, on black: an orbit runs along them.
    let band = fract(h * 0.55);
    let wide = fwidth(h * 0.55) * 1.5 + 0.01;
    let edge = 1.0 - smoothstep(0.0, wide, min(band, 1.0 - band));
    let depth = clamp((h + k) / (4.0 * k + 4.0), 0.0, 1.0);
    color += aurum(depth * 0.45 + 0.35) * 0.30 * edge;
    // The separatrix H = k, burning.
    let d = abs(h - k);
    color += vec3<f32>(1.0, 0.55, 0.25) * exp(-d * d * 60.0) * 0.26;
    color += vec3<f32>(0.20, 0.16, 0.10) * grid_glow(w, 0.7) * 0.12;
    return vec4<f32>(color * (1.0 - 0.25 * dot(ndc, ndc)), 1.0);
}
"#;

const MAX_ORBITS: usize = 26;
const N_DP: usize = 150;
/// Theta wraps at these saddles, which an orbit only ever passes through. The
/// wells at 0 and plus or minus 2 pi stay interior, so a damped orbit can settle in any of them.
const THETA_WRAP: f64 = 3.0 * std::f64::consts::PI;
/// The pendulum wave repeats exactly every 12 s, the default time span.
const WAVE_CYCLE: f64 = 12.0;
const WAVE_BOBS: usize = 30;

/// f_i = (24 + i) / T: every bob completes a whole number of swings per cycle.
fn wave_frequency(i: usize) -> f64 {
    (24.0 + i as f64) / WAVE_CYCLE
}

/// Exact small-angle phase of bob `i` at time `t`.
fn wave_phase(i: usize, t: f64) -> f64 {
    (std::f64::consts::TAU * wave_frequency(i) * t).cos()
}

pub struct DynamicsHall {
    params: Params,
    /// Live pendulum trajectories in phase space: (state, trail).
    orbits: Vec<((f64, f64), Vec<Vec2>)>,
    /// Double pendulums: (th1, w1, th2, w2) each.
    dp: Vec<[f64; 4]>,
    dp_trails: Vec<Vec<Vec2>>,
    surface: Option<GpuMesh>,
    last_k: i64,
    last_spread: i64,
}

impl DynamicsHall {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider(
            "k",
            "Gravity stiffness g/\u{2113}",
            "Physics",
            0.4,
            3.0,
            1.2,
            "",
        );
        p.slider("damp", "Damping", "Physics", 0.0, 0.30, 0.0, "");
        p.slider("rate", "Flow rate", "Observer", 0.2, 4.0, 1.4, "\u{00D7}");
        p.slider(
            "spread",
            "Fan spread \u{03B4}\u{2080}",
            "Physics",
            -7.0,
            -2.0,
            -4.0,
            "10^",
        );
        Self {
            params: p,
            orbits: Vec::new(),
            dp: Vec::new(),
            dp_trails: Vec::new(),
            surface: None,
            last_k: 0,
            last_spread: 0,
        }
    }

    fn seed_orbits(&mut self) {
        self.orbits.clear();
        // A graceful default family: librations and rotations.
        for i in 0..9 {
            let w0 = -3.2 + i as f64 * 0.8;
            self.orbits.push(((0.0, w0), Vec::new()));
        }
    }

    fn spread_key(&self) -> i64 {
        (self.params.f("spread") * 100.0) as i64
    }

    fn seed_dp(&mut self) {
        self.last_spread = self.spread_key();
        let d0 = 10f64.powf(self.params.f("spread"));
        self.dp.clear();
        self.dp_trails.clear();
        for i in 0..N_DP {
            let e = (i as f64 - N_DP as f64 / 2.0) * d0;
            self.dp.push([2.1 + e, 0.0, 2.6, 0.0]);
            self.dp_trails.push(Vec::new());
        }
    }

    fn build_surface(&mut self, gpu: &GpuContext) {
        let k = self.params.f("k");
        if let Some(m) = &self.surface {
            let data = surface_data(k);
            m.update_vertices(gpu, &data.vertices);
        }
    }

    /// A new starting spread needs a new fan. Gravity does not: the same states
    /// simply carry on under the new g.
    fn sync_fan_spread(&mut self) {
        if self.spread_key() != self.last_spread {
            self.seed_dp();
        }
    }

    /// Advance the pendulum orbits and the double pendulums by `h`.
    fn step(&mut self, h: f64) {
        let k = self.params.f("k");
        let damp = self.params.f("damp");
        let f = move |_t: f64, y: &[f64; 2]| -> [f64; 2] { [y[1], -k * y[0].sin() - damp * y[1]] };
        for ((th, w), trail) in &mut self.orbits {
            let mut y = [*th, *w];
            for _ in 0..3 {
                y = rk4(&f, 0.0, &y, h / 3.0);
            }
            if y[0].abs() > THETA_WRAP {
                y[0] -= (2.0 * THETA_WRAP).copysign(y[0]);
                trail.clear();
            }
            *th = y[0];
            *w = y[1];
            trail.push(Vec2::new(*th as f32, *w as f32));
            if trail.len() > 900 {
                trail.remove(0);
            }
        }
        let g = k;
        let fdp = move |_t: f64, y: &[f64; 4]| dp_deriv(g, y);
        for (i, s) in self.dp.iter_mut().enumerate() {
            let mut y = *s;
            for _ in 0..4 {
                y = rk4(&fdp, 0.0, &y, h / 4.0);
            }
            *s = y;
            let tip = Vec2::new(
                (y[0].sin() + y[2].sin()) as f32,
                (-y[0].cos() - y[2].cos()) as f32,
            );
            let tr = &mut self.dp_trails[i];
            tr.push(tip);
            if tr.len() > 40 {
                tr.remove(0);
            }
        }
    }
}

fn surface_data(k: f64) -> MeshData {
    MeshData::parametric(120, 120, |u, v| {
        let th = (u as f64 - 0.5) * 2.0 * THETA_WRAP;
        let w = (v as f64 - 0.5) * 7.0;
        let h = 0.5 * w * w - k * th.cos();
        let hn = ((h + k) / (4.0 * k + 5.0)).clamp(0.0, 1.0) as f32;
        // Deep aurum by energy, with engraved contour banding.
        let band = 0.55 + 0.45 * ((hn * 22.0).fract() - 0.5).abs() * 2.0;
        let base = [
            (0.02 + hn.powi(3) * 0.95) * band,
            (0.015 + hn.powi(4) * 0.60) * band,
            (0.03 + hn.powi(2) * 0.13) * band,
            1.0,
        ];
        (
            Vec3::new(th as f32 * 0.55, h as f32 * 0.42, w as f32 * 0.55),
            base,
        )
    })
}

/// Double pendulum equations (m1=m2=l1=l2=1, g param).
fn dp_deriv(g: f64, y: &[f64; 4]) -> [f64; 4] {
    let (t1, w1, t2, w2) = (y[0], y[1], y[2], y[3]);
    let d = t1 - t2;
    let den = 2.0 - d.cos() * d.cos();
    let a1 = (-w1 * w1 * d.sin() * d.cos() - w2 * w2 * d.sin() - 2.0 * g * t1.sin()
        + g * t2.sin() * d.cos())
        / den;
    let a2 = (w1 * w1 * d.sin() * 2.0 + w2 * w2 * d.sin() * d.cos() + 2.0 * g * t1.sin() * d.cos()
        - 2.0 * g * t2.sin())
        / den;
    [w1, a1, w2, a2]
}

impl ConceptModule for DynamicsHall {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "dynamics.phase",
            name: "The Phase Portrait Hall",
            tagline: "Every possible history, drawn at once",
            branch: Branch::Dynamics,
            description: "Phase space turns mechanics into geometry: each point is a complete state, each curve a possible life of the system. The pendulum's portrait is ruled by its separatrix \u{2014} the burning frontier between swinging and tumbling \u{2014} and every undamped orbit is a level set of the Hamiltonian. Two pendulums coupled, and prediction itself unravels.",
            epigraph: Some((
                "It may happen that small differences in the initial conditions produce very great ones in the final phenomena.",
                "Henri Poincar\u{00E9}, 1908",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "portrait",
                "Phase Portrait",
                "(\u{03B8}, \u{03C9})-space: librations inside the separatrix, rotations beyond",
                4.4,
            ),
            RenderMode::plane("swing", "The Swing", "The first trajectory, embodied", 2.1)
                .look_at(0.0, 0.55, 0.0),
            RenderMode::orbit(
                "energy",
                "Energy Landscape",
                "H(\u{03B8},\u{03C9}); undamped trajectories are its contour lines",
                9.0,
                0.55,
                0.9,
            )
            .look_at(0.0, 0.6, 0.0),
            RenderMode::plane(
                "fan",
                "Double-Pendulum Fan",
                "150 pendulums, neighbours \u{03B4}\u{2080} apart: the butterfly in slow motion",
                2.7,
            )
            .look_at(0.0, -0.6, 0.0),
            RenderMode::plane(
                "wave",
                "The Infinite Pendulum Wave",
                "Lengths tuned so the whole row realigns every 12 s",
                1.55,
            )
            .look_at(0.0, 0.75, 0.0),
        ]
    }

    fn default_modes(&self) -> Vec<usize> {
        vec![0, 3]
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
        (mode == 0).then_some("click to drop a trajectory")
    }
    fn pointer(&mut self, _mode: usize, ev: PointerEvent) {
        if let PointerEvent::Down(w) = ev {
            if self.orbits.len() >= MAX_ORBITS {
                self.orbits.remove(0);
            }
            self.orbits.push(((w.x as f64, w.y as f64), Vec::new()));
        }
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "dyn.phase", PHASE_SHADE, false);
        let mut m = surface_data(self.params.f("k")).upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, glam::Mat4::IDENTITY, [1.0; 4], [1.0, 0.0, 1.0, 0.0]);
        self.surface = Some(m);
        self.seed_orbits();
        self.seed_dp();
    }

    fn update(&mut self, gpu: &GpuContext, _t: f64, dt: f64) {
        let kq = (self.params.f("k") * 1000.0) as i64;
        if kq != self.last_k {
            self.last_k = kq;
            self.build_surface(gpu);
        }
        self.sync_fan_spread();
        let h = (dt.abs() * self.params.f("rate")).min(0.08);
        if h > 0.0 {
            self.step(h);
        }
    }

    fn rewind(&mut self) {
        self.seed_orbits();
        self.seed_dp();
    }

    fn render(
        &mut self,
        _gpu: &GpuContext,
        mode: usize,
        _t: f64,
        _cam: &Camera,
        scene: &mut Scene,
    ) {
        let k = self.params.f("k") as f32;
        match mode {
            0 => {
                scene.field = Some("dyn.phase");
                scene.p(0, k);
                // Axes.
                scene.line(
                    LineLayer::World,
                    Vec3::new(-7.0, 0.0, 0.0),
                    Vec3::new(7.0, 0.0, 0.0),
                    1.1,
                    [0.5, 0.5, 0.55, 0.4],
                );
                scene.line(
                    LineLayer::World,
                    Vec3::new(0.0, -4.4, 0.0),
                    Vec3::new(0.0, 4.4, 0.0),
                    1.1,
                    [0.5, 0.5, 0.55, 0.4],
                );
                // Fixed points.
                scene.point(Vec3::ZERO, 3.0, with_alpha(SEA, 0.9), 1.5);
                for s in [-1.0f32, 1.0] {
                    scene.point(
                        Vec3::new(s * std::f32::consts::PI, 0.0, 0.0),
                        3.0,
                        with_alpha(CORAL, 0.9),
                        1.5,
                    );
                }
                for (i, ((th, w), trail)) in self.orbits.iter().enumerate() {
                    let hue = phase_color(i as f32 * 0.7 + 0.4);
                    let pts: Vec<Vec3> = trail.iter().map(|p| Vec3::new(p.x, p.y, 0.02)).collect();
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.7, with_alpha(hue, 0.55));
                    scene.point(Vec3::new(*th as f32, *w as f32, 0.03), 3.4, hue, 1.8);
                }
            }
            1 => {
                // The first trajectory embodied as a swinging rod.
                if let Some(((th, w), trail)) = self.orbits.first() {
                    let pivot = Vec3::new(0.0, 1.0, 0.0);
                    let arm = 1.45f32;
                    let bob_of =
                        |theta: f32| pivot + Vec3::new(theta.sin(), -theta.cos(), 0.0) * arm;
                    let bob = bob_of(*th as f32);
                    // The full swing circle, engraved.
                    let ring: Vec<Vec3> = (0..=96)
                        .map(|i| bob_of(i as f32 / 96.0 * std::f32::consts::TAU))
                        .collect();
                    scene.polyline(LineLayer::World, &ring, 0.9, [0.45, 0.44, 0.40, 0.22]);
                    // Phosphor trail of the recent motion, from the phase history.
                    let n = trail.len();
                    for (i, ph) in trail.iter().rev().take(70).enumerate() {
                        let fade = (1.0 - i as f32 / 70.0) * 0.30;
                        scene.point(bob_of(ph.x), 2.4, with_alpha(CORAL, fade), 0.9);
                    }
                    let _ = n;
                    scene.line(
                        LineLayer::World,
                        Vec3::new(-1.2, 1.0, 0.0),
                        Vec3::new(1.2, 1.0, 0.0),
                        2.2,
                        [0.4, 0.38, 0.35, 0.85],
                    );
                    scene.line_soft(LineLayer::WorldAdd, pivot, bob, 3.0, with_alpha(BONE, 0.85));
                    scene.point(bob, 8.0, CORAL, 2.6);
                    scene.point(pivot, 2.5, with_alpha(BONE, 0.8), 1.0);
                    // Velocity, tangent to the swing.
                    let tangent = Vec3::new(th.cos() as f32, th.sin() as f32, 0.0);
                    scene.vector(
                        bob,
                        tangent * (*w as f32) * 0.35,
                        3.4,
                        with_alpha(SEA, 0.95),
                    );
                }
            }
            2 => {
                if let Some(m) = &self.surface {
                    scene.mesh(m);
                }
                // Trajectories live on the surface: (th, H, w).
                for (i, ((_, _), trail)) in self.orbits.iter().enumerate() {
                    let hue = phase_color(i as f32 * 0.7 + 0.4);
                    let pts: Vec<Vec3> = trail
                        .iter()
                        .map(|p| {
                            let h = 0.5 * p.y * p.y - k * p.x.cos();
                            Vec3::new(p.x * 0.55, h * 0.42 + 0.035, p.y * 0.55)
                        })
                        .collect();
                    scene.polyline(LineLayer::WorldAdd, &pts, 2.3, with_alpha(hue, 0.95));
                    if let Some(&head) = pts.last() {
                        scene.point(head, 3.6, hue, 1.8);
                    }
                }
            }
            4 => {
                // The pendulum wave: every bob swings a whole number of times per
                // WAVE_CYCLE, so the row realigns at t = 0, T, 2T, ...
                let amp = 0.55f64;
                let pivot_y = 1.9f32;
                scene.line(
                    LineLayer::World,
                    Vec3::new(-3.4, pivot_y, 0.0),
                    Vec3::new(3.4, pivot_y, 0.0),
                    2.2,
                    [0.45, 0.42, 0.38, 0.8],
                );
                for k_i in 0..WAVE_BOBS {
                    let f = wave_frequency(k_i);
                    let x0 = -3.1 + 6.2 * k_i as f64 / (WAVE_BOBS - 1) as f64;
                    // Length rendered to match frequency (l ~ 1/f^2), scaled.
                    let l = (1.0 / (f * f)) * 8.7;
                    let bob_at = |phase: f64| {
                        let sway = amp * phase;
                        Vec3::new(
                            (x0 + sway * l * 0.55) as f32,
                            pivot_y - (l * (1.0 - 0.5 * sway * sway * 0.3)) as f32,
                            0.0,
                        )
                    };
                    let bob = bob_at(wave_phase(k_i, _t));
                    let hue = phase_color(k_i as f32 / WAVE_BOBS as f32 * std::f32::consts::TAU);
                    scene.line(
                        LineLayer::World,
                        Vec3::new(x0 as f32, pivot_y, 0.0),
                        bob,
                        1.0,
                        with_alpha([0.85, 0.87, 0.92, 1.0], 0.30),
                    );
                    scene.point(bob, 4.6, hue, 2.0);
                    // Phosphor echo of each bob's recent arc.
                    for e in 1..=5 {
                        let be = bob_at(wave_phase(k_i, _t - e as f64 * 0.04));
                        scene.point(be, 2.6, with_alpha(hue, 0.18 / e as f32 + 0.02), 0.8);
                    }
                }
            }
            _ => {
                // The fan of fates.
                let pivot = Vec3::new(0.0, 0.9, 0.0);
                for (i, s) in self.dp.iter().enumerate() {
                    let hue = phase_color(i as f32 / N_DP as f32 * std::f32::consts::TAU * 0.8);
                    let alpha = 0.10 + 0.06 * (i % 5) as f32;
                    let j1 = pivot + Vec3::new(s[0].sin() as f32, -s[0].cos() as f32, 0.0) * 0.85;
                    let j2 = j1 + Vec3::new(s[2].sin() as f32, -s[2].cos() as f32, 0.0) * 0.85;
                    scene.line(LineLayer::WorldAdd, pivot, j1, 1.2, with_alpha(hue, alpha));
                    scene.line(
                        LineLayer::WorldAdd,
                        j1,
                        j2,
                        1.2,
                        with_alpha(hue, alpha + 0.05),
                    );
                    let tr = &self.dp_trails[i];
                    if tr.len() > 2 && i % 3 == 0 {
                        let pts: Vec<Vec3> = tr
                            .iter()
                            .map(|p| pivot + Vec3::new(p.x, p.y, 0.0) * 0.85)
                            .collect();
                        scene.polyline(LineLayer::WorldAdd, &pts, 1.0, with_alpha(hue, 0.10));
                    }
                }
                scene.point(pivot, 3.0, with_alpha(BONE, 0.9), 1.4);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let k = self.params.f("k");
        // Divergence of the double-pendulum fan.
        let (mut lo, mut hi) = (f64::MAX, f64::MIN);
        for s in &self.dp {
            lo = lo.min(s[2]);
            hi = hi.max(s[2]);
        }
        let spread = (hi - lo).abs();
        vec![
            title("Hamiltonian flow"),
            ftex("dynamics.hamilton", "q\u{0307} = \u{2202}H/\u{2202}p,   p\u{0307} = \u{2212}\u{2202}H/\u{2202}q"),
            ftex("dynamics.pendulum", "H(\u{03B8},\u{03C9}) = \u{00BD}\u{03C9}\u{00B2} \u{2212} (g/\u{2113})cos\u{03B8}"),
            def("separatrix: H = g/\u{2113} \u{2014} infinite-period boundary between swing and tumble"),
            value(format!("g/\u{2113} = {k:.2}    orbits alive: {}", self.orbits.len())),
            value(format!("fan width in \u{03B8}\u{2082}: {spread:.2e}  (neighbours began 10^{:.1} apart in \u{03B8}\u{2081})", self.params.f("spread"))),
            note("Liouville: the flow preserves phase-space area \u{2014} ensembles shear and fold but never compress. With damping on, they spiral into the attracting well."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 | 2 => Some("RK4, 3 substeps per frame; energy bands exact analytic."),
            1 => Some("The first portrait orbit, RK4 with 3 substeps per frame."),
            3 => Some("150 double pendulums, RK4 \u{00D7}4 substeps; divergence is genuine dynamics, not noise."),
            4 => Some("Exact small-angle cosine for each bob; rod length follows 1/f\u{00B2} up to a display scale, and the arc shape is schematic."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    fn energy(k: f64, th: f64, w: f64) -> f64 {
        0.5 * w * w - k * th.cos()
    }

    fn hall_with_orbit(th: f64, w: f64, damp: f64) -> DynamicsHall {
        let mut d = DynamicsHall::new();
        d.params.set_f("damp", damp);
        d.orbits = vec![((th, w), Vec::new())];
        d
    }

    #[test]
    fn undamped_orbit_keeps_its_energy_even_across_the_wrap() {
        let mut d = hall_with_orbit(0.0, 3.2, 0.0);
        let k = d.params.f("k");
        let start = energy(k, 0.0, 3.2);
        let mut wrapped = false;
        for _ in 0..4000 {
            d.step(0.02);
            let (th, w) = d.orbits[0].0;
            assert!(th.abs() <= THETA_WRAP);
            assert!((energy(k, th, w) - start).abs() < 1e-5);
            wrapped |= th < -PI;
        }
        assert!(wrapped, "a rotating orbit should reach the wrap");
    }

    #[test]
    fn damped_orbit_loses_energy_at_every_step() {
        let mut d = hall_with_orbit(1.0, 1.0, 0.2);
        let k = d.params.f("k");
        let mut last = energy(k, 1.0, 1.0);
        for _ in 0..500 {
            d.step(0.02);
            let (th, w) = d.orbits[0].0;
            let now = energy(k, th, w);
            assert!(now < last);
            last = now;
        }
    }

    #[test]
    fn damped_orbit_settles_in_the_well_at_two_pi_without_jumping() {
        let mut d = hall_with_orbit(TAU - 0.5, 0.0, 0.3);
        for _ in 0..6000 {
            d.step(0.02);
            let (th, _) = d.orbits[0].0;
            assert!((th - TAU).abs() < 0.6, "orbit jumped to theta = {th}");
        }
        let (th, w) = d.orbits[0].0;
        assert!((th - TAU).abs() < 1e-3 && w.abs() < 1e-3);
    }

    #[test]
    fn double_pendulum_conserves_its_energy() {
        // m = l = 1: T = w1^2 + w2^2/2 + w1 w2 cos(t1 - t2), V = -2g cos t1 - g cos t2.
        let g = 1.7;
        let e = |y: &[f64; 4]| {
            y[1] * y[1] + 0.5 * y[3] * y[3] + y[1] * y[3] * (y[0] - y[2]).cos()
                - 2.0 * g * y[0].cos()
                - g * y[2].cos()
        };
        let f = move |_t: f64, y: &[f64; 4]| dp_deriv(g, y);
        let mut y = [2.1, 0.0, 2.6, 0.0];
        let start = e(&y);
        for _ in 0..4000 {
            y = rk4(&f, 0.0, &y, 0.005);
        }
        assert!((e(&y) - start).abs() < 1e-6);
    }

    #[test]
    fn pendulum_wave_realigns_after_one_cycle_and_splits_at_half() {
        for i in 0..WAVE_BOBS {
            assert!((wave_phase(i, 0.0) - 1.0).abs() < 1e-12);
            assert!((wave_phase(i, WAVE_CYCLE) - 1.0).abs() < 1e-9);
            let alternating = if i % 2 == 0 { 1.0 } else { -1.0 };
            assert!((wave_phase(i, WAVE_CYCLE / 2.0) - alternating).abs() < 1e-9);
        }
    }

    #[test]
    fn only_the_spread_restarts_the_fan() {
        let mut d = DynamicsHall::new();
        d.seed_dp();
        for _ in 0..50 {
            d.step(0.02);
        }
        let moved = d.dp[0];
        assert!(moved[1] != 0.0);
        d.params.set_f("k", 2.5);
        d.sync_fan_spread();
        assert_eq!(d.dp[0], moved, "gravity must not restart the fan");
        d.params.set_f("spread", -6.0);
        d.sync_fan_spread();
        assert_eq!(d.dp[0][1], 0.0, "a new spread is a new fan");
    }
}
