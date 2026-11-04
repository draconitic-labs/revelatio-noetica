//! Number theory made visible: prime constellations, modular string art,
//! and a walk along the critical line.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

const PRIME_GREEN: [f32; 4] = [0.65, 0.95, 0.50, 1.0];
const GOLD_N: [f32; 4] = [0.90, 0.76, 0.42, 1.0];

const N_ULAM: usize = 40_000;

fn sieve(n: usize) -> Vec<bool> {
    let mut is_p = vec![true; n + 1];
    is_p[0] = false;
    if n >= 1 {
        is_p[1] = false;
    }
    let mut i = 2;
    while i * i <= n {
        if is_p[i] {
            let mut j = i * i;
            while j <= n {
                is_p[j] = false;
                j += i;
            }
        }
        i += 1;
    }
    is_p
}

/// Ulam spiral coordinate of n (1 at origin, counter-clockwise square spiral).
fn ulam_xy(n: usize) -> (i32, i32) {
    if n == 1 {
        return (0, 0);
    }
    let m = n as i64 - 1;
    let k = (((m as f64).sqrt() as i64 + 1) / 2).max(1);
    let side = 2 * k;
    let start = (2 * k - 1) * (2 * k - 1); // last number of previous ring
    let off = m + 1 - start - 1; // 0-based position along the ring
    let leg = off / side;
    let pos = off % side;
    let (x, y) = match leg {
        0 => (k, -k + 1 + pos),
        1 => (k - 1 - pos, k),
        2 => (-k, k - 1 - pos),
        _ => (-k + 1 + pos, -k),
    };
    (x as i32, y as i32)
}

pub struct ArithmeticRotunda {
    params: Params,
    primes: Vec<bool>,
    ulam_pts: Vec<(Vec3, bool)>,
}

impl ArithmeticRotunda {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.int("modn", "Circle points N", "Symbolic", 40, 400, 210);
        p.slider("mulk", "Multiplier k", "Symbolic", 2.0, 7.0, 2.0, "");
        p.toggle("sweepk", "Sweep k with time", "Observer", true);
        p.slider(
            "zeta_t",
            "Critical-line start t",
            "Symbolic",
            0.0,
            120.0,
            0.0,
            "",
        );
        Self {
            params: p,
            primes: Vec::new(),
            ulam_pts: Vec::new(),
        }
    }

    fn k_now(&self, t: f64) -> f64 {
        if self.params.b("sweepk") {
            2.0 + (t / 12.0) * 3.0
        } else {
            self.params.f("mulk")
        }
    }
}

impl ConceptModule for ArithmeticRotunda {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "number.rotunda",
            name: "The Arithmetic Rotunda",
            tagline: "The integers, seen from far away",
            branch: Branch::NumberTheory,
            description: "Individually the integers are trivial; collectively they conspire. Primes fall into diagonal constellations no one predicted; multiplication tables wound on a circle draw cardioids; and the partial sums of \u{03B6}(\u{00BD}+it) trace a drunken spiral along the critical line, where the zeros that guard the primes' secret are found.",
            epigraph: Some(Epigraph::unquoted(
"Mathematics is the queen of the sciences, and arithmetic the queen of mathematics.",
"Gauss's own words, as reported by Sartorius von Waltershausen, 1856, translated",
)),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "ulam",
                "Ulam Spiral",
                "40,000 integers coiled; primes glow, odd composites stay faint, even ones are left out",
                105.0,
            ),
            RenderMode::plane(
                "cardioid",
                "Modular Strings",
                "Chords n \u{2192} kn mod N; k = 2 draws the cardioid",
                1.35,
            ),
            RenderMode::plane(
                "zeta",
                "Zeta Walk",
                "Partial sums of \u{03A3} n^{\u{2212}(\u{00BD}+it)} on the critical line",
                2.6,
            )
            .look_at(0.8, 0.9, 0.0),
            RenderMode::plane(
                "staircase",
                "The Prime Staircase",
                "\u{03C0}(x) against x/ln x: chaos up close, law from afar",
                1.35,
            )
            .look_at(2.4, 1.1, 0.0),
            RenderMode::plane(
                "phyllo",
                "Phyllotaxis",
                "Seeds placed one golden angle apart \u{2014} the sunflower's theorem",
                2.4,
            ),
            RenderMode::plane(
                "collatz",
                "Collatz Coral",
                "Hailstone paths bending by parity; nobody knows if the coral is finite",
                2.6,
            )
            .look_at(0.0, 1.4, 0.0),
            RenderMode::plane(
                "primepolar",
                "The Prime Galaxy",
                "Each prime p at angle p radians and radius proportional to \u{221A}p: curved arms, then patches",
                2.6,
            ),
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

    fn init(&mut self, _gpu: &GpuContext, _rend: &mut Renderers) {
        self.primes = sieve(N_ULAM);
        self.ulam_pts = (1..=N_ULAM)
            .map(|n| {
                let (x, y) = ulam_xy(n);
                (Vec3::new(x as f32, y as f32, 0.0), self.primes[n])
            })
            .collect();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, _dt: f64) {}

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        match mode {
            0 => {
                // Reveal outward with time.
                let reveal = ((t / 12.0).clamp(0.02, 1.0) * N_ULAM as f64) as usize;
                for (i, (p, is_prime)) in self.ulam_pts.iter().enumerate().take(reveal) {
                    if *is_prime {
                        scene.point(*p, 1.6, with_alpha(PRIME_GREEN, 0.85), 1.2);
                    } else if i % 2 == 0 {
                        scene.point(*p, 0.7, [0.20, 0.24, 0.20, 0.16], 0.0);
                    }
                }
                scene.point(Vec3::ZERO, 3.0, GOLD_N, 1.6);
            }
            1 => {
                let n = self.params.i("modn") as usize;
                let k = self.k_now(t);
                // The circle.
                let ring: Vec<Vec3> = (0..=180)
                    .map(|i| {
                        let a = i as f32 / 180.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos(), a.sin(), 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &ring, 1.3, [0.45, 0.50, 0.42, 0.5]);
                let pt = |idx: f64| -> Vec3 {
                    let a = idx / n as f64 * std::f64::consts::TAU - std::f64::consts::FRAC_PI_2;
                    Vec3::new(a.cos() as f32, -a.sin() as f32, 0.0)
                };
                for i in 0..n {
                    let from = pt(i as f64);
                    let to = pt((i as f64 * k) % n as f64);
                    let hue = phase_color(i as f32 / n as f32 * std::f32::consts::TAU);
                    scene.line(LineLayer::WorldAdd, from, to, 1.0, with_alpha(hue, 0.22));
                }
            }
            3 => {
                // pi(x) staircase, revealed with time, vs x/ln x.
                let x_max = 40.0 + (t / 12.0).clamp(0.0, 1.0) * 1960.0;
                let sc_x = 4.6 / x_max;
                let sc_y = 2.2 / (x_max / (x_max.ln().max(1.5)));
                let mut count = 0u32;
                let mut pts = vec![Vec3::new(0.0, 0.0, 0.0)];
                for n in 2..=(x_max as usize) {
                    if self.primes[n] {
                        let x = n as f64 * sc_x;
                        let y0 = count as f64 * sc_y;
                        count += 1;
                        pts.push(Vec3::new(x as f32, y0 as f32, 0.0));
                        pts.push(Vec3::new(x as f32, (count as f64 * sc_y) as f32, 0.0));
                    }
                }
                scene.polyline(
                    LineLayer::WorldAdd,
                    &pts,
                    1.5,
                    with_alpha(PRIME_GREEN, 0.85),
                );
                // The smooth law.
                let law: Vec<Vec3> = (2..=220)
                    .map(|i| {
                        let x = x_max * i as f64 / 220.0;
                        Vec3::new((x * sc_x) as f32, (x / x.ln().max(1.2) * sc_y) as f32, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &law, 1.3, with_alpha(GOLD_N, 0.75));
                scene.line(
                    LineLayer::World,
                    Vec3::ZERO,
                    Vec3::new(4.8, 0.0, 0.0),
                    1.0,
                    [0.4, 0.45, 0.4, 0.5],
                );
            }
            4 => {
                // Golden-angle placement; deviation animates around 137.508.
                let dev = ((t / 12.0) * std::f64::consts::TAU).sin() * 0.25;
                let alpha = (137.50776 + dev).to_radians();
                let n_seeds = 1100;
                for n_i in 1..=n_seeds {
                    let r = 0.055 * (n_i as f64).sqrt();
                    let th = n_i as f64 * alpha;
                    let hue = phase_color((n_i % 34) as f32 / 34.0 * std::f32::consts::TAU);
                    scene.point(
                        Vec3::new((r * th.cos()) as f32, (r * th.sin()) as f32, 0.0),
                        1.4 + 0.9 * (n_i as f32 / n_seeds as f32),
                        with_alpha(hue, 0.75),
                        0.8,
                    );
                }
            }
            5 => {
                // Hailstone paths as turtle walks: odd turns left, even right.
                let reveal = (t / 12.0).clamp(0.05, 1.0);
                for s in 0..46 {
                    let mut n: u64 = 27 + s * 173;
                    let mut pos = glam::Vec2::new(0.0, -1.2);
                    let mut ang = std::f32::consts::FRAC_PI_2;
                    let mut pts = vec![Vec3::new(pos.x, pos.y, 0.0)];
                    for _ in 0..(220.0 * reveal) as usize {
                        if n <= 1 {
                            break;
                        }
                        if n.is_multiple_of(2) {
                            n /= 2;
                            ang -= 0.16;
                        } else {
                            n = 3 * n + 1;
                            ang += 0.27;
                        }
                        pos += glam::Vec2::new(ang.cos(), ang.sin()) * 0.035;
                        pts.push(Vec3::new(pos.x, pos.y, 0.0));
                    }
                    let hue = phase_color(s as f32 * 0.4);
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.1, with_alpha(hue, 0.22));
                }
            }
            6 => {
                let reveal = ((t / 12.0).clamp(0.02, 1.0) * N_ULAM as f64) as usize;
                for n in 2..reveal.min(N_ULAM) {
                    if !self.primes[n] {
                        continue;
                    }
                    let r = (n as f64).sqrt() * 0.0122;
                    let th = n as f64;
                    scene.point(
                        Vec3::new((r * th.cos()) as f32, (r * th.sin()) as f32, 0.0),
                        1.1,
                        with_alpha(PRIME_GREEN, 0.6),
                        0.6,
                    );
                }
                scene.point(Vec3::ZERO, 2.6, GOLD_N, 1.4);
            }
            _ => {
                // Partial-sum walk of zeta on the critical line.
                let tt = self.params.f("zeta_t") + t * 2.0;
                let n_terms = 700usize;
                let mut z = (0.0f64, 0.0f64);
                let mut pts = Vec::with_capacity(n_terms);
                for n in 1..=n_terms {
                    let nf = n as f64;
                    let mag = 1.0 / nf.sqrt();
                    let ang = -tt * nf.ln();
                    z.0 += mag * ang.cos();
                    z.1 += mag * ang.sin();
                    pts.push(Vec3::new(z.0 as f32 * 0.85, z.1 as f32 * 0.85, 0.0));
                }
                // Keep the walk centred: pan everything by its own centroid.
                let centroid = pts.iter().fold(Vec3::ZERO, |a, &p| a + p) / pts.len().max(1) as f32;
                for p in pts.iter_mut() {
                    *p -= centroid;
                }
                let origin = -centroid;
                for i in 1..pts.len() {
                    let age = i as f32 / pts.len() as f32;
                    let hue = phase_color(age * 4.5);
                    scene.line(
                        LineLayer::WorldAdd,
                        pts[i - 1],
                        pts[i],
                        1.4,
                        with_alpha(hue, 0.30 + 0.35 * (1.0 - age)),
                    );
                }
                scene.point(origin, 3.4, GOLD_N, 1.8);
                if let Some(&last) = pts.last() {
                    scene.point(last, 4.0, [0.95, 0.97, 1.0, 1.0], 2.0);
                    scene.line(
                        LineLayer::Overlay,
                        origin,
                        last,
                        1.0,
                        [0.9, 0.85, 0.6, 0.25],
                    );
                }
            }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let n_primes = self.primes.iter().filter(|&&p| p).count();
        let k = self.k_now(t);
        let tt = self.params.f("zeta_t") + t * 2.0;
        vec![
            title("Structures in \u{2115}"),
            ftex("number.zeta", "\u{03B6}(s) = \u{03A3} n^{\u{2212}s},  s = \u{00BD} + it"),
            ftex("number.cardioid", "n \u{21A6} kn mod N"),
            value(format!("primes below {N_ULAM}: {n_primes}  (density ~ 1/ln n)")),
            value(format!("k = {k:.2}   \u{00B7}   critical line at t = {tt:.1}")),
            note("k = 2 draws the cardioid, k = 3 the nephroid: the same envelopes as light caustics in a teacup \u{2014} multiplication is a folding map."),
            note("The series does not converge on the critical line, so these partial sums only hint at \u{03B6}: at a zero such as t \u{2248} 14.13 the walk need not return to the origin. The Riemann hypothesis, that all nontrivial zeros lie on this line, is unproved; no exception has been found."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Sieve of Eratosthenes to 40,000; every dot is a genuine integer."),
            2 => Some("700-term partial sums; the true \u{03B6} adds the analytic continuation this walk only hints at."),
            _ => None,
        }
    }
}
