//! Vector calculus: fields, flow, divergence, curl, flux, and circulation.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const GREEN: [f32; 4] = [0.30, 0.85, 0.55, 1.0];
const BLUE: [f32; 4] = [0.40, 0.60, 0.98, 1.0];
const WHITE: [f32; 4] = [0.92, 0.96, 1.0, 1.0];

const FIELD_NAMES: [&str; 6] = [
    "Vortex",
    "Source & sink",
    "Saddle",
    "Shear",
    "Spiral sink",
    "Dipole",
];
const FIELD_EQNS: [&str; 6] = [
    "F = (\u{2212}y, x)",
    "F = ((x\u{2212}1)/r\u{2081}\u{00B2} \u{2212} (x+1)/r\u{2082}\u{00B2}, y/r\u{2081}\u{00B2} \u{2212} y/r\u{2082}\u{00B2})",
    "F = (x, \u{2212}y)",
    "F = (y, 0)",
    "F = (\u{2212}0.4x \u{2212} y, x \u{2212} 0.4y)",
    "F = dipole (source+sink fused)",
];

fn field(k: usize, p: Vec2) -> Vec2 {
    match k {
        0 => Vec2::new(-p.y, p.x),
        1 => {
            let a = p - Vec2::new(1.0, 0.0);
            let b = p + Vec2::new(1.0, 0.0);
            let ra = a.length_squared().max(0.05);
            let rb = b.length_squared().max(0.05);
            a / ra - b / rb
        }
        2 => Vec2::new(p.x, -p.y),
        3 => Vec2::new(p.y, 0.0),
        4 => Vec2::new(-0.4 * p.x - p.y, p.x - 0.4 * p.y),
        _ => {
            let r2 = p.length_squared().max(0.03);
            // 2D dipole oriented along +x.
            Vec2::new(
                (p.x * p.x - p.y * p.y) / (r2 * r2),
                2.0 * p.x * p.y / (r2 * r2),
            ) * 0.5
        }
    }
}

/// WGSL twin of `field`, plus divergence/curl heat shading.
const HEAT_SHADE: &str = r#"
fn field2(k: i32, p: vec2<f32>) -> vec2<f32> {
    switch k {
        case 0: { return vec2<f32>(-p.y, p.x); }
        case 1: {
            let a = p - vec2<f32>(1.0, 0.0);
            let b = p + vec2<f32>(1.0, 0.0);
            return a / max(dot(a, a), 0.05) - b / max(dot(b, b), 0.05);
        }
        case 2: { return vec2<f32>(p.x, -p.y); }
        case 3: { return vec2<f32>(p.y, 0.0); }
        case 4: { return vec2<f32>(-0.4 * p.x - p.y, p.x - 0.4 * p.y); }
        default: {
            let r2 = max(dot(p, p), 0.03);
            return vec2<f32>((p.x * p.x - p.y * p.y), 2.0 * p.x * p.y) * 0.5 / (r2 * r2);
        }
    }
}

fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let k = i32(U.p[0].x + 0.5);
    let show_curl = U.p[0].y > 0.5;
    let e = 0.01;
    let fx1 = field2(k, w + vec2<f32>(e, 0.0));
    let fx0 = field2(k, w - vec2<f32>(e, 0.0));
    let fy1 = field2(k, w + vec2<f32>(0.0, e));
    let fy0 = field2(k, w - vec2<f32>(0.0, e));
    let div = (fx1.x - fx0.x + fy1.y - fy0.y) / (2.0 * e);
    let curl = (fx1.y - fx0.y - (fy1.x - fy0.x)) / (2.0 * e);
    let q = select(div, curl, show_curl) * 0.55;
    let m = clamp(abs(q) / (1.0 + abs(q)), 0.0, 1.0);
    var color = vec3<f32>(0.0);
    if (q > 0.0) {
        color = mix(color, vec3<f32>(0.42, 0.19, 0.04), m * 0.34);
    } else {
        color = mix(color, vec3<f32>(0.04, 0.15, 0.42), m * 0.34);
    }
    // Woven texture along the flow keeps a flat field readable.
    let f = field2(k, w);
    let fl = max(length(f), 1e-4);
    let tang = f / fl;
    let s_coord = dot(w, vec2<f32>(-tang.y, tang.x));
    let stripe = sin(s_coord * 22.0) * 0.5 + 0.5;
    color *= 0.72 + 0.28 * stripe;
    color += vec3<f32>(0.1, 0.2, 0.16) * grid_glow(w, 0.7) * 0.10;
    return vec4<f32>(color * (1.0 - 0.25 * dot(ndc, ndc)), 1.0);
}
"#;

const PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.0);
    color += vec3<f32>(0.10, 0.22, 0.16) * grid_glow(w, 0.7) * 0.13;
    return vec4<f32>(color * (1.0 - 0.28 * dot(ndc, ndc)), 1.0);
}
"#;

const N_PART: usize = 2600;
const TRAIL: usize = 7;

pub struct VectorField {
    params: Params,
    /// Particle ring buffers: TRAIL past positions each.
    particles: Vec<[Vec2; TRAIL]>,
    seed: u64,
    last_k: usize,
}

impl VectorField {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("field", "Field F(x,y)", "Symbolic", FIELD_NAMES.to_vec(), 0);
        p.toggle("curlheat", "Heat: curl (else divergence)", "Display", true);
        p.slider("probe_x", "Probe center x", "Observer", -2.5, 2.5, 0.8, "");
        p.slider("probe_y", "Probe center y", "Observer", -2.5, 2.5, 0.4, "");
        p.slider("probe_r", "Probe radius", "Observer", 0.3, 1.8, 0.9, "");
        p.slider("flow", "Flow speed", "Physics", 0.0, 2.5, 1.0, "");
        Self {
            params: p,
            particles: Vec::new(),
            seed: 0x9E3779B97F4A7C15,
            last_k: usize::MAX,
        }
    }

    fn rand(&mut self) -> f32 {
        self.seed = self
            .seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.seed >> 32) as u32 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }

    fn respawn(&mut self, i: usize) {
        let p = Vec2::new(self.rand() * 3.4, self.rand() * 3.4);
        self.particles[i] = [p; TRAIL];
    }

    /// Green's theorem probe: (flux, circulation) around the circle, plus the
    /// area integrals of div and curl inside it (midpoint quadrature).
    fn probe_integrals(&self) -> (f64, f64, f64, f64) {
        let k = self.params.c("field");
        let c = Vec2::new(
            self.params.f("probe_x") as f32,
            self.params.f("probe_y") as f32,
        );
        let r = self.params.f("probe_r") as f32;
        let n = 720;
        let (mut flux, mut circ) = (0.0f64, 0.0f64);
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let nrm = Vec2::new(a.cos(), a.sin());
            let tan = Vec2::new(-a.sin(), a.cos());
            let f = field(k, c + nrm * r);
            let ds = (std::f32::consts::TAU * r / n as f32) as f64;
            flux += f.dot(nrm) as f64 * ds;
            circ += f.dot(tan) as f64 * ds;
        }
        // Area integrals on a polar grid.
        let (mut divi, mut curli) = (0.0f64, 0.0f64);
        let (nr, na) = (40, 90);
        let e = 0.008;
        for ir in 0..nr {
            let rr = r * (ir as f32 + 0.5) / nr as f32;
            for ia in 0..na {
                let a = ia as f32 / na as f32 * std::f32::consts::TAU;
                let p = c + Vec2::new(a.cos(), a.sin()) * rr;
                let fx1 = field(k, p + Vec2::new(e, 0.0));
                let fx0 = field(k, p - Vec2::new(e, 0.0));
                let fy1 = field(k, p + Vec2::new(0.0, e));
                let fy0 = field(k, p - Vec2::new(0.0, e));
                let div = (fx1.x - fx0.x + fy1.y - fy0.y) / (2.0 * e);
                let curl = (fx1.y - fx0.y - (fy1.x - fy0.x)) / (2.0 * e);
                let da = (r / nr as f32 * std::f32::consts::TAU * rr / na as f32) as f64;
                divi += div as f64 * da;
                curli += curl as f64 * da;
            }
        }
        (flux, circ, divi, curli)
    }
}

impl ConceptModule for VectorField {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "vector.fields",
            name: "The Field Chamber",
            tagline: "Flux, circulation, and the anatomy of flow",
            branch: Branch::VectorCalculus,
            description: "A vector field assigns an arrow to every point. Its local anatomy is captured by two derivatives: divergence, the rate at which flow is created, and curl, the rate at which it turns. Green's theorem says the boundary already knows what the interior is doing \u{2014} watch the numbers agree.",
            epigraph: None,
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "flow",
                "Field & Flow",
                "Comet arrows on the lattice; tracer particles ride the flow",
                3.4,
            ),
            RenderMode::plane(
                "heat",
                "Divergence \u{00B7} Curl Heat",
                "Sources burn orange, sinks glow blue; rotation likewise",
                3.4,
            ),
            RenderMode::plane(
                "probe",
                "Flux & Circulation Probe",
                "A circle interrogates the field: \u{222E}F\u{00B7}n ds and \u{222E}F\u{00B7}T ds",
                3.4,
            ),
            RenderMode::plane(
                "weave",
                "Streamline Weave",
                "Hundreds of integral curves woven into the field's fabric",
                3.4,
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

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "vector.heat", HEAT_SHADE, false);
        rend.register_field(gpu, "vector.paper", PAPER, false);
        self.particles = vec![[Vec2::ZERO; TRAIL]; N_PART];
        for i in 0..N_PART {
            self.respawn(i);
        }
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, dt: f64) {
        let k = self.params.c("field");
        if k != self.last_k {
            self.last_k = k;
            for i in 0..N_PART {
                self.respawn(i);
            }
        }
        let speed = self.params.f("flow") as f32;
        if speed <= 0.0 || dt == 0.0 {
            return;
        }
        let h = (dt as f32 * speed).clamp(0.0, 0.05);
        for i in 0..N_PART {
            let head = self.particles[i][0];
            // RK2 midpoint step.
            let k1 = field(k, head);
            let k2 = field(k, head + k1 * (h * 0.5));
            let new = head + k2 * h;
            let ring = &mut self.particles[i];
            for j in (1..TRAIL).rev() {
                ring[j] = ring[j - 1];
            }
            ring[0] = new;
            if !new.x.is_finite() || new.length() > 5.2 || k2.length() < 0.004 {
                self.respawn(i);
            }
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        let k = self.params.c("field");
        scene.p(0, k as f32);
        scene.p(1, if self.params.b("curlheat") { 1.0 } else { 0.0 });

        let arrows = |scene: &mut Scene, alpha: f32| {
            let n = 15;
            for i in 0..=n {
                for j in 0..=n {
                    let p = Vec2::new(
                        -3.2 + 6.4 * i as f32 / n as f32,
                        -3.2 + 6.4 * j as f32 / n as f32,
                    );
                    let f = field(k, p);
                    let mag = f.length();
                    if mag < 1e-4 {
                        continue;
                    }
                    let len = 0.34 * mag / (0.8 + mag);
                    let dir = f / mag;
                    let c = [
                        GREEN[0] + (BLUE[0] - GREEN[0]) * (mag / (1.5 + mag)),
                        GREEN[1] - 0.25 * (mag / (1.5 + mag)),
                        GREEN[2] + (BLUE[2] - GREEN[2]) * (mag / (1.5 + mag)),
                        alpha,
                    ];
                    scene.vector(
                        Vec3::new(p.x - dir.x * len * 0.5, p.y - dir.y * len * 0.5, 0.0),
                        Vec3::new(dir.x * len, dir.y * len, 0.0),
                        2.6,
                        c,
                    );
                }
            }
        };

        match mode {
            0 => {
                scene.field = Some("vector.paper");
                arrows(scene, 0.5);
                // Tracer trails.
                for ring in &self.particles {
                    for j in 0..TRAIL - 1 {
                        let a = ring[j];
                        let b = ring[j + 1];
                        if (a - b).length_squared() < 1e-10 {
                            continue;
                        }
                        let fade = (1.0 - j as f32 / TRAIL as f32) * 0.5;
                        scene.line(
                            LineLayer::WorldAdd,
                            Vec3::new(a.x, a.y, 0.05),
                            Vec3::new(b.x, b.y, 0.05),
                            1.5,
                            with_alpha(WHITE, fade * 0.5),
                        );
                    }
                }
            }
            1 => {
                scene.field = Some("vector.heat");
                arrows(scene, 0.65);
            }
            3 => {
                scene.field = Some("vector.paper");
                // Dense weave: streamlines from a seed lattice, both directions.
                let n_seed = 15;
                for si in 0..n_seed {
                    for sj in 0..n_seed {
                        let seed = Vec2::new(
                            -3.2 + 6.4 * si as f32 / (n_seed - 1) as f32,
                            -3.2 + 6.4 * sj as f32 / (n_seed - 1) as f32,
                        );
                        for dir in [1.0f32, -1.0] {
                            let mut p = seed;
                            let mut pts = vec![Vec3::new(p.x, p.y, 0.0)];
                            for _ in 0..110 {
                                let f = field(k, p);
                                let fl = f.length();
                                if fl < 1e-4 || p.length() > 5.0 {
                                    break;
                                }
                                p += f / fl * 0.045 * dir;
                                pts.push(Vec3::new(p.x, p.y, 0.0));
                            }
                            let hue = crate::modules::util::phase_color(
                                seed.y.atan2(seed.x) + seed.length() * 0.8,
                            );
                            scene.polyline(LineLayer::WorldAdd, &pts, 1.0, with_alpha(hue, 0.10));
                        }
                    }
                }
            }
            _ => {
                scene.field = Some("vector.paper");
                arrows(scene, 0.35);
                let c = Vec3::new(
                    self.params.f("probe_x") as f32,
                    self.params.f("probe_y") as f32,
                    0.0,
                );
                let r = self.params.f("probe_r") as f32;
                let ring: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let a = i as f32 / 96.0 * std::f32::consts::TAU;
                        c + Vec3::new(a.cos() * r, a.sin() * r, 0.06)
                    })
                    .collect();
                scene.glow_polyline(&ring, 2.0, WHITE, 2.6);
                // Normal and tangential samples on the boundary.
                for i in 0..28 {
                    let a = i as f32 / 28.0 * std::f32::consts::TAU;
                    let nrm = Vec2::new(a.cos(), a.sin());
                    let tan = Vec2::new(-a.sin(), a.cos());
                    let pos = Vec2::new(c.x, c.y) + nrm * r;
                    let f = field(k, pos);
                    let fn_ = f.dot(nrm);
                    let ft = f.dot(tan);
                    let ncol = if fn_ >= 0.0 {
                        [0.95, 0.55, 0.15, 0.9]
                    } else {
                        [0.25, 0.5, 0.95, 0.9]
                    };
                    scene.vector(
                        Vec3::new(pos.x, pos.y, 0.08),
                        Vec3::new(nrm.x * fn_ * 0.30, nrm.y * fn_ * 0.30, 0.0),
                        2.4,
                        ncol,
                    );
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(pos.x, pos.y, 0.07),
                        Vec3::new(pos.x + tan.x * ft * 0.30, pos.y + tan.y * ft * 0.30, 0.07),
                        1.8,
                        with_alpha(GREEN, 0.75),
                    );
                }
                // A pulse riding the boundary at the local tangential speed.
                let a = (t * 0.7) as f32 % std::f32::consts::TAU;
                let pos = Vec2::new(c.x, c.y) + Vec2::new(a.cos(), a.sin()) * r;
                scene.point(Vec3::new(pos.x, pos.y, 0.09), 4.0, WHITE, 2.0);
            }
        }
    }

    fn wants_pointer(&self, mode: usize) -> bool {
        mode == 2
    }

    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode == 2).then_some("drag to move the probe")
    }

    fn pointer(&mut self, _mode: usize, ev: crate::engine::module::PointerEvent) {
        use crate::engine::module::PointerEvent as PE;
        if let PE::Down(w) | PE::Drag(w) = ev {
            self.params.set_f("probe_x", (w.x as f64).clamp(-2.5, 2.5));
            self.params.set_f("probe_y", (w.y as f64).clamp(-2.5, 2.5));
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let k = self.params.c("field");
        let (flux, circ, divi, curli) = self.probe_integrals();
        vec![
            title("Vector field anatomy"),
            formula(FIELD_EQNS[k]),
            ftex("vector.divcurl", "div F = \u{2202}F\u{2093}/\u{2202}x + \u{2202}F\u{1D67}/\u{2202}y        curl F = \u{2202}F\u{1D67}/\u{2202}x \u{2212} \u{2202}F\u{2093}/\u{2202}y"),
            ftex("vector.green", "Green:  \u{222E} F\u{00B7}n ds = \u{222C} div F dA"),
            value(format!("\u{222E} F\u{00B7}n ds = {flux:+.4}      \u{222C} div F dA = {divi:+.4}")),
            value(format!("\u{222E} F\u{00B7}T ds = {circ:+.4}      \u{222C} curl F dA = {curli:+.4}")),
            note("The two columns agree to quadrature error \u{2014} the boundary already knows the interior."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Tracers: RK2 midpoint advection; arrows exact analytic."),
            1 => Some("div/curl by central differences (h = 0.01) per pixel."),
            2 => Some("Boundary integrals: 720-point quadrature; area integrals: 40\u{00D7}90 polar grid."),
            3 => Some("Euler steps of fixed length 0.045 along the unit field, 110 each way from each seed."),
            _ => None,
        }
    }
}
