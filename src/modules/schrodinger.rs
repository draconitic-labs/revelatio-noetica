//! Schrödinger wavefunctions: exact eigenstate superpositions in the
//! harmonic well. Amplitude, phase, probability, and the rain of measurement.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData, MeshVertex};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{c, phase_color, with_alpha, C};
use crate::theme::Branch;

const VIOLET: [f32; 4] = [0.62, 0.48, 0.95, 1.0];
const AMBER: [f32; 4] = [0.95, 0.72, 0.30, 1.0];
const GHOSTQ: [f32; 4] = [0.5, 0.55, 0.7, 0.30];

const NX: usize = 420;
const X_MAX: f64 = 6.0;
const N_STATES: usize = 14;

const PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.0);
    color += vec3<f32>(0.14, 0.10, 0.25) * grid_glow(w, 0.7) * 0.11;
    return vec4<f32>(color * (1.0 - 0.30 * dot(ndc, ndc)), 1.0);
}
"#;

const STATE_NAMES: [&str; 4] = [
    "Coherent packet  \u{03B1}=1.8",
    "Fock pair  (\u{03C6}\u{2080}+\u{03C6}\u{2083})/\u{221A}2",
    "Dipole  (\u{03C6}\u{2080}+\u{03C6}\u{2081})/\u{221A}2",
    "Breather  (\u{03C6}\u{2080}+\u{03C6}\u{2082})/\u{221A}2",
];

/// Harmonic oscillator eigenfunction values on the grid (Hermite recurrence).
fn eigenstates() -> Vec<[f64; N_STATES]> {
    let mut out = Vec::with_capacity(NX + 1);
    for i in 0..=NX {
        let x = -X_MAX + 2.0 * X_MAX * i as f64 / NX as f64;
        let g = (-x * x / 2.0).exp() / std::f64::consts::PI.powf(0.25);
        let mut h = [0.0; N_STATES];
        let mut h0 = 1.0;
        let mut h1 = 2.0 * x;
        for (n, h_n) in h.iter_mut().enumerate() {
            let hn = if n == 0 {
                h0
            } else if n == 1 {
                h1
            } else {
                let h2 = 2.0 * x * h1 - 2.0 * (n as f64 - 1.0) * h0;
                h0 = h1;
                h1 = h2;
                h2
            };
            // norm: 1/sqrt(2^n n!)
            let mut norm = 1.0f64;
            for k in 1..=n {
                norm *= 2.0 * k as f64;
            }
            *h_n = g * hn / norm.sqrt();
        }
        out.push(h);
    }
    out
}

fn coeffs(preset: usize) -> [f64; N_STATES] {
    let mut cs = [0.0; N_STATES];
    match preset {
        0 => {
            let alpha: f64 = 1.8;
            let mut fact = 1.0;
            for (n, c_n) in cs.iter_mut().enumerate() {
                if n > 0 {
                    fact *= n as f64;
                }
                *c_n = (-alpha * alpha / 2.0).exp() * alpha.powi(n as i32) / fact.sqrt();
            }
        }
        1 => {
            cs[0] = std::f64::consts::FRAC_1_SQRT_2;
            cs[3] = std::f64::consts::FRAC_1_SQRT_2;
        }
        2 => {
            cs[0] = std::f64::consts::FRAC_1_SQRT_2;
            cs[1] = std::f64::consts::FRAC_1_SQRT_2;
        }
        _ => {
            cs[0] = std::f64::consts::FRAC_1_SQRT_2;
            cs[2] = std::f64::consts::FRAC_1_SQRT_2;
        }
    }
    cs
}

pub struct SchrodingerHall {
    params: Params,
    basis: Vec<[f64; N_STATES]>,
    psi: Vec<C>,
    prob_mesh: Option<GpuMesh>,
    samples: Vec<(f64, f64)>, // (x, birth on `clock`)
    hist: Vec<u32>,
    /// Measurements owed, carried between frames.
    sample_debt: f64,
    /// Time since the last rewind. Unlike t it never wraps, so falling
    /// samples keep their age across a loop.
    clock: f64,
    last_state: usize,
    rng: u64,
}

impl SchrodingerHall {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice(
            "state",
            "Prepared state",
            "Symbolic",
            STATE_NAMES.to_vec(),
            0,
        );
        p.slider("rate", "Measurement rate", "Physics", 2.0, 60.0, 24.0, "/s");
        p.toggle("current", "Probability current", "Display", true);
        Self {
            params: p,
            basis: Vec::new(),
            psi: vec![C::ZERO; NX + 1],
            prob_mesh: None,
            samples: Vec::new(),
            hist: vec![0; 64],
            sample_debt: 0.0,
            clock: 0.0,
            last_state: 0,
            rng: 0x853c49e6748fea9b,
        }
    }

    fn rand(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn x_of(i: usize) -> f64 {
        -X_MAX + 2.0 * X_MAX * i as f64 / NX as f64
    }

    fn recompute_psi(&mut self, t: f64) {
        let cs = coeffs(self.params.c("state"));
        for i in 0..=NX {
            let mut acc = C::ZERO;
            for (n, &c_n) in cs.iter().enumerate() {
                if c_n.abs() < 1e-12 {
                    continue;
                }
                let e = n as f64 + 0.5;
                let ph = c(0.0, -e * t).exp();
                acc = acc + ph * (c_n * self.basis[i][n]);
            }
            self.psi[i] = acc;
        }
    }

    /// Draw one x from |psi|^2 by inverse CDF on the grid.
    fn sample_x(&mut self) -> f64 {
        let total: f64 = self.psi.iter().map(|p| p.abs().powi(2)).sum();
        let mut u = self.rand() * total;
        for i in 0..=NX {
            u -= self.psi[i].abs().powi(2);
            if u <= 0.0 {
                return Self::x_of(i);
            }
        }
        0.0
    }

    fn clear_measurements(&mut self) {
        self.samples.clear();
        self.hist.iter_mut().for_each(|h| *h = 0);
        self.sample_debt = 0.0;
    }
}

impl ConceptModule for SchrodingerHall {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "quantum.schrodinger",
            name: "The Amplitude Hall",
            tagline: "\u{03C8}: amplitude, phase, and chance",
            branch: Branch::Quantum,
            description: "A quantum state is not a particle smeared out \u{2014} it is a complex amplitude whose squared magnitude is the only thing experiment ever sees. Here exact eigenstates of the harmonic well are superposed: phase turns at rate E\u{2099}, interference makes the density slosh, and measurement \u{2014} the rain below \u{2014} samples what the amplitude permits.",
            epigraph: None,
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "psi",
                "Wavefunction",
                "\u{03C8}(x,t) colored by phase; envelope \u{00B1}|\u{03C8}|",
                2.4,
            )
            .look_at(0.0, 0.4, 0.0),
            RenderMode::plane(
                "prob",
                "Probability River",
                "|\u{03C8}|\u{00B2} with probability current",
                2.4,
            )
            .look_at(0.0, 0.6, 0.0),
            RenderMode::plane(
                "phasor",
                "Phase Clocks",
                "Each eigenstate turns at its own energy: e^{\u{2212}iE\u{2099}t}",
                2.2,
            )
            .look_at(0.0, 0.0, 0.0),
            RenderMode::plane(
                "measure",
                "Measurement Rain",
                "Samples from |\u{03C8}|\u{00B2} accumulate into the Born histogram",
                2.6,
            )
            .look_at(0.0, -0.2, 0.0),
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

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "quantum.paper", PAPER, false);
        self.basis = eigenstates();
        // Probability fill strip.
        let mut d = MeshData::new();
        for _ in 0..=NX {
            d.vertices.push(MeshVertex {
                pos: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
                color: [1.0; 4],
            });
            d.vertices.push(MeshVertex {
                pos: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
                color: [1.0; 4],
            });
        }
        for i in 0..NX as u32 {
            let a = i * 2;
            d.indices
                .extend_from_slice(&[a, a + 1, a + 2, a + 2, a + 1, a + 3]);
        }
        let mut m = d.upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, glam::Mat4::IDENTITY, [1.0; 4], [1.0, 0.0, 1.0, 0.0]);
        self.prob_mesh = Some(m);
    }

    fn rewind(&mut self) {
        self.clear_measurements();
        self.clock = 0.0;
    }

    fn update(&mut self, gpu: &GpuContext, t: f64, dt: f64) {
        // Samples of one state say nothing about another.
        let state = self.params.c("state");
        if state != self.last_state {
            self.last_state = state;
            self.clear_measurements();
        }
        self.clock += dt.abs();
        self.recompute_psi(t);
        // Probability fill vertices, phase-colored.
        let verts: Vec<MeshVertex> = (0..=NX)
            .flat_map(|i| {
                let x = Self::x_of(i) as f32;
                let p = self.psi[i];
                let dens = (p.abs().powi(2) * 2.2) as f32;
                let mut col = phase_color(p.arg() as f32);
                col[3] = 0.55;
                let base = [col[0] * 0.5, col[1] * 0.5, col[2] * 0.5, 0.10];
                [
                    MeshVertex {
                        pos: [x * 0.9, dens, -0.05],
                        normal: [0.0, 0.0, 1.0],
                        color: col,
                    },
                    MeshVertex {
                        pos: [x * 0.9, 0.0, -0.05],
                        normal: [0.0, 0.0, 1.0],
                        color: base,
                    },
                ]
            })
            .collect();
        if let Some(m) = &self.prob_mesh {
            m.update_vertices(gpu, &verts);
        }
        // Measurement sampling.
        let rate = self.params.f("rate");
        self.sample_debt += rate * dt.abs();
        while self.sample_debt >= 1.0 {
            self.sample_debt -= 1.0;
            let x = self.sample_x();
            self.samples.push((x, self.clock));
            self.hist[hist_bin(x)] += 1;
            if self.samples.len() > 800 {
                self.samples.remove(0);
            }
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        scene.field = Some("quantum.paper");
        let sx = 0.9f32; // world scale on x

        let well = |scene: &mut Scene, y0: f32| {
            let pts: Vec<Vec3> = (0..=120)
                .map(|i| {
                    let x = -X_MAX + 2.0 * X_MAX * i as f64 / 120.0;
                    Vec3::new(x as f32 * sx, y0 + (0.5 * x * x * 0.12) as f32, -0.1)
                })
                .collect();
            scene.polyline(LineLayer::World, &pts, 1.0, GHOSTQ);
        };

        match mode {
            0 => {
                well(scene, -1.8);
                scene.line(
                    LineLayer::World,
                    Vec3::new(-5.6, 0.0, 0.0),
                    Vec3::new(5.6, 0.0, 0.0),
                    1.0,
                    GHOSTQ,
                );
                // Envelope.
                let env: Vec<Vec3> = (0..=NX)
                    .map(|i| {
                        Vec3::new(
                            Self::x_of(i) as f32 * sx,
                            (self.psi[i].abs() * 1.5) as f32,
                            0.0,
                        )
                    })
                    .collect();
                let env2: Vec<Vec3> = env.iter().map(|p| Vec3::new(p.x, -p.y, 0.0)).collect();
                scene.polyline(LineLayer::World, &env, 1.0, with_alpha(VIOLET, 0.30));
                scene.polyline(LineLayer::World, &env2, 1.0, with_alpha(VIOLET, 0.30));
                // Re psi, phase-colored segment by segment.
                for i in 0..NX {
                    let x0 = Self::x_of(i) as f32 * sx;
                    let x1 = Self::x_of(i + 1) as f32 * sx;
                    let p0 = self.psi[i];
                    let p1 = self.psi[i + 1];
                    let colr = with_alpha(phase_color(p0.arg() as f32), 0.95);
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(x0, (p0.re * 1.5) as f32, 0.0),
                        Vec3::new(x1, (p1.re * 1.5) as f32, 0.0),
                        2.6,
                        colr,
                    );
                }
            }
            1 => {
                well(scene, 0.0);
                scene.line(
                    LineLayer::World,
                    Vec3::new(-5.6, 0.0, 0.0),
                    Vec3::new(5.6, 0.0, 0.0),
                    1.0,
                    GHOSTQ,
                );
                if let Some(m) = &self.prob_mesh {
                    scene.mesh(m);
                }
                // Probability current arrows: j = Im(psi* dpsi/dx).
                if self.params.b("current") {
                    for k in 0..21 {
                        let i = k * NX / 20;
                        if i == 0 || i >= NX {
                            continue;
                        }
                        let dx = 2.0 * X_MAX / NX as f64;
                        let dpsi = (self.psi[i + 1] - self.psi[i - 1]) * (1.0 / (2.0 * dx));
                        let j = (self.psi[i].conj() * dpsi).im;
                        if j.abs() < 1e-4 {
                            continue;
                        }
                        let x = Self::x_of(i) as f32 * sx;
                        scene.vector(
                            Vec3::new(x, -0.35, 0.0),
                            Vec3::new((j * 1.8) as f32, 0.0, 0.0),
                            3.0,
                            with_alpha(AMBER, 0.85),
                        );
                    }
                }
            }
            2 => {
                // Phase clocks for each occupied eigenstate.
                let cs = coeffs(self.params.c("state"));
                let occupied: Vec<usize> = (0..N_STATES).filter(|&n| cs[n].abs() > 0.02).collect();
                let count = occupied.len().max(1);
                for (slot, &n) in occupied.iter().enumerate() {
                    let cx = -4.4 + 8.8 * (slot as f32 + 0.5) / count as f32;
                    let r = (cs[n] * 1.5).max(0.18) as f32;
                    let ring: Vec<Vec3> = (0..=64)
                        .map(|i| {
                            let a = i as f32 / 64.0 * std::f32::consts::TAU;
                            Vec3::new(cx + a.cos() * r, a.sin() * r, 0.0)
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &ring, 1.2, with_alpha(VIOLET, 0.45));
                    let e = n as f64 + 0.5;
                    let ang = (-e * t) as f32;
                    let needle = Vec3::new(cx + ang.cos() * r, ang.sin() * r, 0.0);
                    let hue = phase_color(ang);
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(cx, 0.0, 0.0),
                        needle,
                        2.6,
                        hue,
                    );
                    scene.point(needle, 3.5, hue, 1.8);
                    // Energy rungs below.
                    scene.line(
                        LineLayer::World,
                        Vec3::new(cx - 0.5, -1.9, 0.0),
                        Vec3::new(cx + 0.5, -1.9, 0.0),
                        2.0,
                        with_alpha(AMBER, (0.25 + 0.7 * cs[n] * cs[n]) as f32),
                    );
                }
            }
            _ => {
                well(scene, 0.6);
                // |psi|^2 as the live target curve.
                let curve: Vec<Vec3> = (0..=NX)
                    .map(|i| {
                        Vec3::new(
                            Self::x_of(i) as f32 * sx,
                            0.6 + (self.psi[i].abs().powi(2) * 1.8) as f32,
                            0.0,
                        )
                    })
                    .collect();
                scene.polyline(LineLayer::World, &curve, 1.6, with_alpha(VIOLET, 0.8));
                // Falling samples.
                for &(x, birth) in &self.samples {
                    let age = (self.clock - birth) as f32;
                    let y = (1.9 - age * 2.4).max(-1.4);
                    let fade = if y <= -1.39 { 0.25 } else { 0.9 };
                    scene.point(
                        Vec3::new(x as f32 * sx, y, 0.05),
                        2.2,
                        with_alpha(AMBER, fade),
                        1.4,
                    );
                }
                // Histogram bars.
                let max_h = self.hist.iter().copied().max().unwrap_or(1).max(6) as f32;
                for (b, &n) in self.hist.iter().enumerate() {
                    if n == 0 {
                        continue;
                    }
                    let x = (-X_MAX + 2.0 * X_MAX * (b as f64 + 0.5) / 64.0) as f32 * sx;
                    let h = n as f32 / max_h * 1.3;
                    scene.line(
                        LineLayer::World,
                        Vec3::new(x, -1.5, 0.0),
                        Vec3::new(x, -1.5 + h, 0.0),
                        6.0,
                        with_alpha(AMBER, 0.55),
                    );
                }
                scene.line(
                    LineLayer::World,
                    Vec3::new(-5.6, -1.5, 0.0),
                    Vec3::new(5.6, -1.5, 0.0),
                    1.0,
                    GHOSTQ,
                );
            }
        }
    }

    fn wants_pointer(&self, mode: usize) -> bool {
        mode == 3
    }

    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode == 3).then_some("click to fire a burst of measurements")
    }

    fn pointer(&mut self, _mode: usize, ev: crate::engine::module::PointerEvent) {
        use crate::engine::module::PointerEvent as PE;
        if let PE::Down(_) = ev {
            for _ in 0..40 {
                let x = self.sample_x();
                self.samples.push((x, self.clock));
                self.hist[hist_bin(x)] += 1;
                if self.samples.len() > 800 {
                    self.samples.remove(0);
                }
            }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let cs = coeffs(self.params.c("state"));
        let occ: Vec<String> = (0..N_STATES)
            .filter(|&n| cs[n].abs() > 0.02)
            .take(5)
            .map(|n| format!("c{} = {:.3}", subscript(n), cs[n]))
            .collect();
        // <x> for the packet.
        let mut mean = 0.0;
        let mut norm = 0.0;
        for i in 0..=NX {
            let p2 = self.psi[i].abs().powi(2);
            mean += Self::x_of(i) * p2;
            norm += p2;
        }
        mean /= norm.max(1e-12);
        vec![
            title("Harmonic well  (\u{0127}=m=\u{03C9}=1)"),
            ftex("quantum.tdse", "i\u{2202}\u{209C}\u{03C8} = (\u{2212}\u{00BD}\u{2202}\u{2093}\u{2093} + \u{00BD}x\u{00B2})\u{03C8}"),
            ftex("quantum.super", "\u{03C8}(x,t) = \u{03A3} c\u{2099}\u{03C6}\u{2099}(x) e^{\u{2212}iE\u{2099}t},   E\u{2099} = n + \u{00BD}"),
            ftex("quantum.born", "Born rule:  P(x) dx = |\u{03C8}(x,t)|\u{00B2} dx"),
            value(occ.join("   ")),
            value(format!("\u{27E8}x\u{27E9} = {mean:+.3}   t = {t:.2}   samples = {}", self.samples.len())),
            note("No orbiting dots, no little planets: only amplitude, phase, and the statistics of many identical measurements."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            3 => Some("Sampling by inverse-CDF on the 420-point grid \u{2014} exact Born statistics up to grid resolution."),
            _ => Some("Exact eigenstate superposition (coherent state truncated at n = 13); no PDE discretization error."),
        }
    }
}

fn subscript(n: usize) -> char {
    char::from_u32(0x2080 + n as u32 % 10).unwrap_or('\u{2099}')
}

/// Which of the 64 equal histogram bars over [-X_MAX, X_MAX] a position falls in.
fn hist_bin(x: f64) -> usize {
    (((x + X_MAX) / (2.0 * X_MAX) * 64.0).floor().max(0.0) as usize).min(63)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_bins_are_equal_and_match_their_drawn_centres() {
        for b in 0..64 {
            let centre = -X_MAX + 2.0 * X_MAX * (b as f64 + 0.5) / 64.0;
            assert_eq!(hist_bin(centre), b);
        }
        assert_eq!(hist_bin(-X_MAX * 2.0), 0);
        assert_eq!(hist_bin(X_MAX * 2.0), 63);
    }

    #[test]
    fn every_state_is_normalized() {
        for (state, name) in STATE_NAMES.iter().enumerate() {
            let sum: f64 = coeffs(state).iter().map(|c| c * c).sum();
            assert!((sum - 1.0).abs() < 1e-3, "{name}: {sum}");
        }
    }
}
