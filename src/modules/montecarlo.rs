//! Monte Carlo: computing with dice. Estimates converge as 1/sqrt(N),
//! indifferent to dimension — chance as a numerical method.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const HIT: [f32; 4] = [0.95, 0.72, 0.30, 1.0];
const MISS: [f32; 4] = [0.35, 0.50, 0.85, 1.0];
const CROSS: [f32; 4] = [0.95, 0.42, 0.38, 1.0];

fn mc_f(x: f64) -> f64 {
    ((3.0 * x).sin().powi(2) * (-x * x).exp() + 0.15) * 1.4
}

pub struct MonteCarloCasino {
    params: Params,
    darts: Vec<(Vec2, bool)>,
    needles: Vec<(Vec2, f32, bool)>,
    integ: Vec<(Vec2, bool)>,
    n_in: u64,
    n_tot: u64,
    n_cross: u64,
    n_under: u64,
    exact_area: f64,
    rng: u64,
    last_t: f64,
    /// Galton balls: (row-progress clock, path of L/R choices) + bins.
    balls: Vec<(f64, u32)>,
    bins: Vec<u32>,
}

const G_ROWS: usize = 13;

impl MonteCarloCasino {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider("rate", "Samples per second", "Physics", 20.0, 2000.0, 400.0, "");
        // Quasi-exact reference by fine Simpson.
        let n = 4000;
        let mut acc = 0.0;
        for i in 0..=n {
            let x = 3.0 * i as f64 / n as f64;
            let w = if i == 0 || i == n { 1.0 } else if i % 2 == 1 { 4.0 } else { 2.0 };
            acc += w * mc_f(x);
        }
        let exact = acc * (3.0 / n as f64) / 3.0;
        Self {
            params: p,
            darts: Vec::new(),
            needles: Vec::new(),
            integ: Vec::new(),
            n_in: 0,
            n_tot: 0,
            n_cross: 0,
            n_under: 0,
            exact_area: exact,
            rng: 0xB5297A4D2C4979A3,
            last_t: 0.0,
            balls: Vec::new(),
            bins: vec![0; G_ROWS + 1],
        }
    }

    fn rand(&mut self) -> f64 {
        self.rng = self.rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn reset(&mut self) {
        self.darts.clear();
        self.needles.clear();
        self.integ.clear();
        self.n_in = 0;
        self.n_tot = 0;
        self.n_cross = 0;
        self.n_under = 0;
        self.balls.clear();
        self.bins = vec![0; G_ROWS + 1];
    }

    /// World position of a Galton ball from its clock and choice bits.
    fn ball_pos(clock: f64, path: u32) -> Vec2 {
        let row_f = clock.min(G_ROWS as f64);
        let row = row_f as usize;
        let frac = (row_f - row as f64) as f32;
        let mut x = 0.0f32;
        for r in 0..row.min(G_ROWS) {
            x += if (path >> r) & 1 == 1 { 0.115 } else { -0.115 };
        }
        let x_next = if row < G_ROWS {
            x + if (path >> row) & 1 == 1 { 0.115 } else { -0.115 }
        } else {
            x
        };
        let xi = x + (x_next - x) * frac;
        Vec2::new(xi, 2.0 - row_f as f32 * 0.27)
    }
}

impl ConceptModule for MonteCarloCasino {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "statmech.montecarlo",
            name: "The Monte Carlo Casino",
            tagline: "Computing with dice",
            branch: Branch::StatMech,
            description: "Ulam, recovering from illness and playing solitaire, wondered: why compute the odds when you can just deal the cards many times? Every dart here is a random trial; the estimate converges as 1/\u{221A}N regardless of dimension \u{2014} which is why the method that measures a circle badly measures a thousand-dimensional integral better than anything else we have.",
            epigraph: Some((
                "The first thoughts and attempts I made were suggested by a question which occurred to me while playing solitaires.",
                "Stanis\u{0142}aw Ulam",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane("pi", "Dartboard \u{03C0}", "In-circle fraction \u{00D7} 4 \u{2192} \u{03C0}, one dart at a time", 1.5),
            RenderMode::plane("buffon", "Buffon's Needle", "1777: drop needles on floorboards, read off \u{03C0} from the crossings", 2.2),
            RenderMode::plane("integrate", "Integration by Rain", "Area under a curve nobody wants to antidifferentiate", 1.35)
                .look_at(1.5, 0.85, 0.0),
            RenderMode::plane("galton", "The Galton Board", "Coin flips falling into a bell curve \u{2014} the central limit theorem, 1893", 2.5)
                .look_at(0.0, -0.4, 0.0),
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

    fn init(&mut self, _gpu: &GpuContext, _rend: &mut Renderers) {
        self.reset();
    }

    fn update(&mut self, _gpu: &GpuContext, t: f64, dt: f64) {
        if t < self.last_t - 0.5 {
            self.reset();
        }
        self.last_t = t;
        let n = (self.params.f("rate") * dt).round() as usize;
        for _ in 0..n.min(4000) {
            // Dartboard.
            let p = Vec2::new(self.rand() as f32 * 2.0 - 1.0, self.rand() as f32 * 2.0 - 1.0);
            let inside = p.length_squared() <= 1.0;
            self.n_tot += 1;
            if inside {
                self.n_in += 1;
            }
            if self.darts.len() < 26000 {
                self.darts.push((p, inside));
            }
            // Buffon: spacing 1, needle length 0.5.
            let y = self.rand() as f32 * 4.0 - 2.0;
            let x = self.rand() as f32 * 5.6 - 2.8;
            let a = self.rand() as f32 * std::f32::consts::PI;
            let half = 0.25 * a.sin();
            let crosses = (y - half).floor() != (y + half).floor();
            if crosses {
                self.n_cross += 1;
            }
            if self.needles.len() < 9000 {
                self.needles.push((Vec2::new(x, y), a, crosses));
            }
            // Integration rain over [0,3] x [0,1.7].
            let q = Vec2::new(self.rand() as f32 * 3.0, self.rand() as f32 * 1.7);
            let under = (q.y as f64) <= mc_f(q.x as f64);
            if under {
                self.n_under += 1;
            }
            if self.integ.len() < 26000 {
                self.integ.push((q, under));
            }
        }
        // Galton: spawn a few balls per frame, march their clocks.
        for _ in 0..2 {
            if self.balls.len() < 140 {
                let path = (self.rand() * (1u64 << 31) as f64) as u32;
                self.balls.push((0.0, path));
            }
        }
        let speed = 6.5 * dt;
        let mut done = Vec::new();
        for (bi, (clock, path)) in self.balls.iter_mut().enumerate() {
            *clock += speed;
            if *clock >= G_ROWS as f64 + 1.5 {
                let bin = (0..G_ROWS).filter(|r| (*path >> r) & 1 == 1).count();
                done.push((bi, bin));
            }
        }
        for (bi, bin) in done.into_iter().rev() {
            self.bins[bin] += 1;
            self.balls.remove(bi);
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, _t: f64, _cam: &Camera, scene: &mut Scene) {
        scene.clear = [0.008, 0.009, 0.012, 1.0];
        match mode {
            0 => {
                // Square + inscribed circle.
                let sq = [
                    Vec3::new(-1.0, -1.0, 0.0),
                    Vec3::new(1.0, -1.0, 0.0),
                    Vec3::new(1.0, 1.0, 0.0),
                    Vec3::new(-1.0, 1.0, 0.0),
                    Vec3::new(-1.0, -1.0, 0.0),
                ];
                scene.polyline(LineLayer::Overlay, &sq, 1.4, [0.6, 0.63, 0.7, 0.6]);
                let ring: Vec<Vec3> = (0..=128)
                    .map(|i| {
                        let a = i as f32 / 128.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos(), a.sin(), 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::Overlay, &ring, 1.6, with_alpha(HIT, 0.8));
                for &(p, inside) in &self.darts {
                    scene.point(
                        Vec3::new(p.x, p.y, 0.0),
                        1.1,
                        if inside { with_alpha(HIT, 0.5) } else { with_alpha(MISS, 0.5) },
                        0.5,
                    );
                }
            }
            1 => {
                // Floorboards.
                for k in -2..=2 {
                    scene.line(
                        LineLayer::World,
                        Vec3::new(-2.9, k as f32, -0.01),
                        Vec3::new(2.9, k as f32, -0.01),
                        1.6,
                        [0.55, 0.58, 0.65, 0.55],
                    );
                }
                for &(c, a, crosses) in &self.needles {
                    let d = Vec3::new(a.cos(), a.sin(), 0.0) * 0.25;
                    let p = Vec3::new(c.x, c.y, 0.0);
                    let col = if crosses { with_alpha(CROSS, 0.75) } else { [0.75, 0.78, 0.85, 0.30] };
                    scene.line(LineLayer::WorldAdd, p - d, p + d, 1.3, col);
                }
            }
            3 => {
                // Pegs.
                for r in 0..G_ROWS {
                    for k in 0..=r {
                        let x = (k as f32 - r as f32 / 2.0) * 0.23;
                        scene.point(Vec3::new(x, 2.0 - (r as f32 + 0.5) * 0.27, 0.0), 1.6, [0.65, 0.67, 0.72, 0.8], 0.6);
                    }
                }
                // Falling balls.
                for &(clock, path) in &self.balls {
                    let p = Self::ball_pos(clock, path);
                    scene.point(Vec3::new(p.x, p.y, 0.02), 3.2, HIT, 1.8);
                }
                // Bins + the binomial's Gaussian shadow.
                let maxb = *self.bins.iter().max().unwrap_or(&1).max(&8) as f32;
                let total: u32 = self.bins.iter().sum();
                for (b, &n) in self.bins.iter().enumerate() {
                    let x = (b as f32 - G_ROWS as f32 / 2.0) * 0.23;
                    let h = n as f32 / maxb * 1.5;
                    scene.line(LineLayer::World, Vec3::new(x, -2.2, 0.0), Vec3::new(x, -2.2 + h.max(0.004), 0.0), 9.0, with_alpha(HIT, 0.55));
                }
                if total > 40 {
                    let sigma = (G_ROWS as f32 * 0.25).sqrt() * 0.23;
                    let pts: Vec<Vec3> = (0..=120)
                        .map(|i| {
                            let x = -1.8 + 3.6 * i as f32 / 120.0;
                            let g = (-x * x / (2.0 * sigma * sigma)).exp();
                            Vec3::new(x, -2.2 + g * 1.5, 0.01)
                        })
                        .collect();
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.6, with_alpha(MISS, 0.7));
                }
            }
            _ => {
                // The curve and its box.
                let curve: Vec<Vec3> = (0..=300)
                    .map(|i| {
                        let x = 3.0 * i as f64 / 300.0;
                        Vec3::new(x as f32, mc_f(x) as f32, 0.01)
                    })
                    .collect();
                scene.glow_polyline(&curve, 2.0, [0.35, 0.85, 0.75, 1.0], 2.4);
                scene.line(LineLayer::World, Vec3::ZERO, Vec3::new(3.0, 0.0, 0.0), 1.2, [0.5, 0.5, 0.55, 0.5]);
                for &(p, under) in &self.integ {
                    scene.point(
                        Vec3::new(p.x, p.y, 0.0),
                        1.0,
                        if under { with_alpha(HIT, 0.45) } else { with_alpha(MISS, 0.28) },
                        0.4,
                    );
                }
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let pi_est = if self.n_tot > 0 { 4.0 * self.n_in as f64 / self.n_tot as f64 } else { 0.0 };
        let buffon = if self.n_cross > 0 {
            // P(cross) = 2 l / (pi d), l = 0.5, d = 1.
            self.n_tot as f64 / self.n_cross as f64
        } else {
            0.0
        };
        let area = if self.n_tot > 0 {
            3.0 * 1.7 * self.n_under as f64 / self.n_tot as f64
        } else {
            0.0
        };
        vec![
            title("The law of large numbers, live"),
            formula("estimate error \u{223C} \u{03C3}/\u{221A}N \u{2014} independent of dimension"),
            value(format!("N = {}", self.n_tot)),
            value(format!("\u{03C0} by darts   = {pi_est:.5}   (err {:+.5})", pi_est - std::f64::consts::PI)),
            value(format!("\u{03C0} by needles = {buffon:.5}   (err {:+.5})", buffon - std::f64::consts::PI)),
            value(format!("\u{222B}f = {area:.5}   vs Simpson {:.5}", self.exact_area)),
            note("Buffon's needle, 1777: probability of crossing = 2\u{2113}/\u{03C0}d. With \u{2113} = d/2 the crossing fraction is exactly 1/\u{03C0} \u{2014} geometry paying out through chance."),
            note("Ten times the accuracy costs a hundred times the darts. For one integral that is terrible; for a sixty-dimensional one it is the only game in town."),
        ]
    }

    fn approx(&self, _mode: usize) -> Option<&'static str> {
        Some("True pseudo-random trials (PCG-style LCG); estimates and errors update live in the inspector.")
    }
}
