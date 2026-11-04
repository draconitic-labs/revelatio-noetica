//! Strange attractors: the geometry that deterministic chaos settles onto.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const N_SWARM: usize = 26000;

/// Chenciner-Montgomery figure-eight initial conditions (G = m = 1).
const TB_INIT: [[f64; 4]; 3] = [
    [0.97000436, -0.24308753, 0.466203685, 0.43236573],
    [-0.97000436, 0.24308753, 0.466203685, 0.43236573],
    [0.0, 0.0, -0.93240737, -0.86473146],
];

fn tb_accel(tb: &[[f64; 4]; 3], i: usize) -> (f64, f64) {
    let (mut ax, mut ay) = (0.0, 0.0);
    for j in 0..3 {
        if j == i {
            continue;
        }
        let dx = tb[j][0] - tb[i][0];
        let dy = tb[j][1] - tb[i][1];
        let r2 = (dx * dx + dy * dy).max(1e-6);
        let inv_r3 = 1.0 / (r2 * r2.sqrt());
        ax += dx * inv_r3;
        ay += dy * inv_r3;
    }
    (ax, ay)
}

fn lorenz(sigma: f64, rho: f64, beta: f64, p: Vec3) -> Vec3 {
    Vec3::new(
        (sigma * (p.y as f64 - p.x as f64)) as f32,
        (p.x as f64 * (rho - p.z as f64) - p.y as f64) as f32,
        (p.x as f64 * p.y as f64 - beta * p.z as f64) as f32,
    )
}

fn rossler(a: f64, b: f64, c: f64, p: Vec3) -> Vec3 {
    Vec3::new(
        (-(p.y as f64) - p.z as f64) as f32,
        (p.x as f64 + a * p.y as f64) as f32,
        (b + p.z as f64 * (p.x as f64 - c)) as f32,
    )
}

pub struct ChaosObservatory {
    params: Params,
    swarm: Vec<Vec3>,
    /// Twin trajectories for the divergence ribbon.
    twin_a: Vec<Vec3>,
    twin_b: Vec<Vec3>,
    rng: u64,
    last_sys: usize,
    last_t: f64,
    logistic: Vec<Vec3>,
    /// Figure-eight choreography: (pos, vel) x3 + trails.
    tb: [[f64; 4]; 3],
    tb_trails: Vec<Vec<Vec3>>,
}

impl ChaosObservatory {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice(
            "system",
            "System",
            "Symbolic",
            vec!["Lorenz", "R\u{00F6}ssler"],
            0,
        );
        p.slider("rho", "Lorenz \u{03C1}", "Physics", 14.0, 45.0, 28.0, "");
        p.slider(
            "speed",
            "Integration rate",
            "Observer",
            0.2,
            4.0,
            1.0,
            "\u{00D7}",
        );
        Self {
            params: p,
            swarm: Vec::new(),
            twin_a: Vec::new(),
            twin_b: Vec::new(),
            rng: 0x2545F4914F6CDD1D,
            last_sys: usize::MAX,
            last_t: 0.0,
            logistic: Vec::new(),
            tb: TB_INIT,
            tb_trails: vec![Vec::new(); 3],
        }
    }

    fn rand(&mut self) -> f32 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.rng >> 33) as u32 as f64 / u32::MAX as f64) as f32 * 2.0 - 1.0
    }

    fn seed(&mut self) {
        self.swarm.clear();
        for _ in 0..N_SWARM {
            let (a, b, c) = (self.rand(), self.rand(), self.rand());
            self.swarm
                .push(Vec3::new(a * 18.0, b * 22.0, c * 18.0 + 22.0));
        }
        self.twin_a = vec![Vec3::new(1.0, 1.0, 20.0)];
        self.twin_b = vec![Vec3::new(1.0, 1.0, 20.00002)];
    }

    fn deriv(&self, p: Vec3) -> Vec3 {
        if self.params.c("system") == 0 {
            lorenz(10.0, self.params.f("rho"), 8.0 / 3.0, p)
        } else {
            rossler(0.2, 0.2, 5.7, p) * 3.0
        }
    }

    /// Map system space into view space.
    fn view_of(&self, p: Vec3) -> Vec3 {
        if self.params.c("system") == 0 {
            Vec3::new(p.x * 0.16, (p.z - 25.0) * 0.16, p.y * 0.16)
        } else {
            Vec3::new(p.x * 0.28, (p.z - 4.0) * 0.28, p.y * 0.28)
        }
    }
}

impl ConceptModule for ChaosObservatory {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "dynamics.attractor",
            name: "The Strange Attractor Observatory",
            tagline: "Where chaos keeps its shape",
            branch: Branch::Dynamics,
            description: "Deterministic equations, three variables, no randomness anywhere \u{2014} yet prediction fails, and what survives is a shape: the attractor, a fractal object every trajectory visits forever without repeating. Twenty-six thousand particles ride the flow; individually lost, collectively they ARE the butterfly.",
            epigraph: Some((
                "Chaos: When the present determines the future, but the approximate present does not approximately determine the future.",
                "Edward Lorenz",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit(
                "swarm",
                "Particle Swarm",
                "26,000 states advected by the flow; density reveals the attractor",
                11.0,
                0.15,
                0.7,
            ),
            RenderMode::orbit(
                "ribbon",
                "Twin Trajectories",
                "Two histories 2\u{00D7}10\u{207B}\u{2075} apart \u{2014} watch them part ways",
                11.0,
                0.15,
                0.7,
            ),
            RenderMode::plane(
                "logistic",
                "Logistic Bifurcation",
                "x \u{2192} rx(1\u{2212}x): period doubling cascading into chaos",
                0.62,
            )
            .look_at(1.65, 0.55, 0.0),
            RenderMode::plane(
                "threebody",
                "The Figure-Eight Waltz",
                "Three equal suns sharing one orbit \u{2014} the choreography of 2000",
                1.6,
            ),
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
        self.seed();
        // The bifurcation diagram, computed once.
        self.logistic.clear();
        for col in 0..950 {
            let r = 2.5 + 1.5 * col as f64 / 949.0;
            let mut x = 0.412;
            for _ in 0..220 {
                x = r * x * (1.0 - x);
            }
            for _ in 0..130 {
                x = r * x * (1.0 - x);
                self.logistic
                    .push(Vec3::new(((r - 2.5) * 2.2) as f32, (x * 1.1) as f32, 0.0));
            }
        }
    }

    fn update(&mut self, _gpu: &GpuContext, t: f64, dt: f64) {
        let sys = self.params.c("system");
        if sys != self.last_sys {
            self.last_sys = sys;
            self.seed();
        }
        if t < self.last_t - 0.5 && self.params.c("system") == 9 {
            // (attractors persist across loops by design)
        }
        self.last_t = t;
        let h = (dt * self.params.f("speed") * 0.9).clamp(0.0, 0.03) as f32;
        if h <= 0.0 {
            return;
        }
        // Midpoint advection for the swarm (cheap, stable enough at this h).
        for i in 0..self.swarm.len() {
            let p = self.swarm[i];
            let k1 = self.deriv(p);
            let k2 = self.deriv(p + k1 * (h * 0.5));
            let np = p + k2 * h;
            self.swarm[i] = if np.is_finite() && np.length() < 400.0 {
                np
            } else {
                Vec3::new(self.rand() * 10.0, self.rand() * 10.0, 25.0)
            };
        }
        // Twins: finer steps for the ribbons.
        let rho = self.params.f("rho");
        let is_lorenz = sys == 0;
        let dv = move |p: Vec3| {
            if is_lorenz {
                lorenz(10.0, rho, 8.0 / 3.0, p)
            } else {
                rossler(0.2, 0.2, 5.7, p) * 3.0
            }
        };
        for _ in 0..2 {
            for tw in [&mut self.twin_a, &mut self.twin_b] {
                let p = *tw.last().unwrap();
                let k1 = dv(p);
                let k2 = dv(p + k1 * (h * 0.25));
                tw.push(p + k2 * (h * 0.5));
                if tw.len() > 2600 {
                    tw.remove(0);
                }
            }
        }
        // Figure-eight waltz: leapfrog keeps the choreography closed.
        let hb = (dt * 0.6).clamp(0.0, 0.02);
        for _ in 0..6 {
            for i in 0..3 {
                let (ax, ay) = tb_accel(&self.tb, i);
                self.tb[i][2] += ax * hb * 0.5;
                self.tb[i][3] += ay * hb * 0.5;
            }
            for i in 0..3 {
                self.tb[i][0] += self.tb[i][2] * hb;
                self.tb[i][1] += self.tb[i][3] * hb;
            }
            for i in 0..3 {
                let (ax, ay) = tb_accel(&self.tb, i);
                self.tb[i][2] += ax * hb * 0.5;
                self.tb[i][3] += ay * hb * 0.5;
            }
        }
        for i in 0..3 {
            let p = Vec3::new(self.tb[i][0] as f32 * 1.3, self.tb[i][1] as f32 * 1.3, 0.0);
            let tr = &mut self.tb_trails[i];
            if tr.last().map_or(true, |q| (*q - p).length() > 0.01) {
                tr.push(p);
                if tr.len() > 700 {
                    tr.remove(0);
                }
            }
        }
    }

    fn render(
        &mut self,
        _gpu: &GpuContext,
        mode: usize,
        _t: f64,
        _cam: &Camera,
        scene: &mut Scene,
    ) {
        scene.clear = [0.005, 0.005, 0.008, 1.0];
        match mode {
            0 => {
                for (i, &p) in self.swarm.iter().enumerate() {
                    let v = self.view_of(p);
                    let speed = self.deriv(p).length();
                    let heat = (speed / (speed + 90.0)).clamp(0.0, 1.0);
                    let c = [
                        0.25 + 0.75 * heat,
                        0.30 + 0.35 * heat,
                        0.85 - 0.45 * heat,
                        0.30,
                    ];
                    let _ = i;
                    scene.point(v, 1.05, c, 0.7);
                }
            }
            2 => {
                scene.clear = [0.006, 0.006, 0.009, 1.0];
                for p in &self.logistic {
                    scene.point(*p, 0.55, [0.95, 0.70, 0.30, 0.30], 0.0);
                }
                // Feigenbaum landmarks.
                for r in [3.0f32, 3.44949, 3.56995] {
                    let x = (r - 2.5) * 2.2;
                    scene.line(
                        LineLayer::Overlay,
                        Vec3::new(x, 0.0, 0.0),
                        Vec3::new(x, 1.1, 0.0),
                        0.8,
                        [0.4, 0.6, 0.9, 0.25],
                    );
                }
            }
            3 => {
                scene.clear = [0.005, 0.005, 0.008, 1.0];
                let cols = [
                    [0.95f32, 0.72, 0.30, 1.0],
                    [0.40, 0.75, 0.95, 1.0],
                    [0.95, 0.45, 0.60, 1.0],
                ];
                for i in 0..3 {
                    let tr = &self.tb_trails[i];
                    for k in 1..tr.len() {
                        let age = k as f32 / tr.len() as f32;
                        scene.line(
                            LineLayer::WorldAdd,
                            tr[k - 1],
                            tr[k],
                            1.5,
                            with_alpha(cols[i], 0.04 + 0.4 * age * age),
                        );
                    }
                    let p = Vec3::new(self.tb[i][0] as f32 * 1.3, self.tb[i][1] as f32 * 1.3, 0.01);
                    scene.point(p, 6.5, cols[i], 2.4);
                }
            }
            _ => {
                for (tw, base) in [
                    (&self.twin_a, [0.95, 0.72, 0.30f32]),
                    (&self.twin_b, [0.35, 0.75, 0.95]),
                ] {
                    let pts: Vec<Vec3> = tw.iter().map(|&p| self.view_of(p)).collect();
                    let n = pts.len();
                    for i in 1..n {
                        let age = i as f32 / n as f32;
                        scene.line(
                            LineLayer::WorldAdd,
                            pts[i - 1],
                            pts[i],
                            1.6,
                            [base[0], base[1], base[2], 0.04 + 0.5 * age * age],
                        );
                    }
                    if let Some(&last) = pts.last() {
                        scene.point(last, 4.0, [base[0], base[1], base[2], 1.0], 2.0);
                    }
                }
                // The gap, made visible.
                if let (Some(&a), Some(&b)) = (self.twin_a.last(), self.twin_b.last()) {
                    scene.line(
                        LineLayer::Overlay,
                        self.view_of(a),
                        self.view_of(b),
                        1.2,
                        with_alpha([1.0, 1.0, 1.0, 1.0], 0.5),
                    );
                }
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let rho = self.params.f("rho");
        let sep = match (self.twin_a.last(), self.twin_b.last()) {
            (Some(&a), Some(&b)) => (a - b).length(),
            _ => 0.0,
        };
        vec![
            title(if self.params.c("system") == 0 { "Lorenz system" } else { "R\u{00F6}ssler system" }),
            ftex("chaos.lorenz", "x\u{0307} = \u{03C3}(y\u{2212}x),  y\u{0307} = x(\u{03C1}\u{2212}z)\u{2212}y,  z\u{0307} = xy \u{2212} \u{03B2}z"),
            def("\u{03C3} = 10,  \u{03B2} = 8/3;  chaos onsets near \u{03C1} \u{2248} 24.74"),
            value(format!("\u{03C1} = {rho:.1}")),
            value(format!("twin separation |\u{03B4}| = {sep:.4}  (born at 2\u{00D7}10\u{207B}\u{2075})")),
            note("The attractor has fractal dimension \u{2248} 2.06 at \u{03C1} = 28: more than a surface, less than a volume. No trajectory ever crosses itself \u{2014} determinism forbids it."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Midpoint advection, h \u{2264} 0.03; density is a live ensemble, not a precomputed texture."),
            _ => Some("Both twins integrated identically; their parting is the Lyapunov exponent at work."),
        }
    }
}
