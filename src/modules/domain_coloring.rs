//! Complex Analysis — domain coloring, analytic landscape, conformal grid
//! flow, and the Riemann sphere.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{c, phase_color, with_alpha, C};
use crate::theme::Branch;

const FN_LABELS: [&str; 9] = [
    "z\u{00B2}",
    "z\u{00B3} \u{2212} 1",
    "1/z",
    "(z\u{00B2}\u{2212}1)(z\u{2212}2\u{2212}i)\u{00B2} / (z\u{00B2}+2+2i)",
    "e\u{1D22B}",
    "sin z",
    "Log z",
    "(z\u{2212}a)/(1\u{2212}\u{0101}z),  a = 0.7\u{00B7}e^{it/2}",
    "e^{1/z}",
];

const FN_NOTES: [&str; 9] = [
    "Double zero at 0: phase winds twice around it.",
    "Three simple zeros at the cube roots of unity.",
    "Simple pole at 0 — phase winds backwards around poles.",
    "Wegert's example: zeros at \u{00B1}1, a double zero at 2+i, two poles.",
    "No zeros anywhere; period 2\u{03C0}i visible as horizontal banding.",
    "Zeros at k\u{03C0} along the real axis; growth e^{|Im z|} above and below.",
    "Branch cut along the negative real axis; zero at z = 1.",
    "Blaschke factor: automorphism of the disk; |B(z)| = 1 on the unit circle.",
    "Essential singularity at 0: every value hit infinitely often near it (Picard).",
];

// WGSL: shared complex-function evaluator + enhanced domain coloring.
const FEVAL_WGSL: &str = r#"
fn feval(z: vec2<f32>) -> vec2<f32> {
    let k = i32(U.p[0].x + 0.5);
    let a = vec2<f32>(U.p[0].z, U.p[0].w);
    switch k {
        case 0: { return cmul(z, z); }
        case 1: { return cmul(cmul(z, z), z) - vec2<f32>(1.0, 0.0); }
        case 2: { return cinv(z); }
        case 3: {
            let z2 = cmul(z, z);
            let n1 = z2 - vec2<f32>(1.0, 0.0);
            let d1 = z - vec2<f32>(2.0, 1.0);
            let num = cmul(n1, cmul(d1, d1));
            let den = z2 + vec2<f32>(2.0, 2.0);
            return cdiv(num, den);
        }
        case 4: { return cexp(z); }
        case 5: { return csin(z); }
        case 6: { return clog(z); }
        case 7: {
            let num = z - a;
            let den = vec2<f32>(1.0, 0.0) - cmul(vec2<f32>(a.x, -a.y), z);
            return cdiv(num, den);
        }
        default: { return cexp(cinv(z)); }
    }
}

fn dc_color(fz: vec2<f32>, contours: f32, plines: f32) -> vec3<f32> {
    let r = length(fz);
    let th = atan2(fz.y, fz.x);
    var color = phase_color(th);
    let t = r / (1.0 + r);
    let dark = smoothstep(0.0, 0.13, t);
    let lite = smoothstep(0.91, 0.9995, t);
    color = color * dark;
    color = mix(color, vec3<f32>(0.96, 0.97, 1.0), lite * lite * 0.92);
    let m = log2(max(r, 1e-24));
    color = color * mix(1.0, 0.62 + 0.38 * fract(m), contours);
    color = color * mix(1.0, 0.82 + 0.18 * fract(th * 6.0 / TAU), plines);
    return color * 0.92;
}
"#;

const DC_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let fz = feval(w);
    var color = dc_color(fz, U.p[1].x, U.p[1].y);
    // Faint engraved unit lattice of the input plane.
    let g = grid_glow(w, 0.9) * 0.028;
    color += vec3<f32>(g);
    return vec4<f32>(color, 1.0);
}
"#;

const SPHERE_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let ro = U.eye.xyz;
    let rd = ray_dir(ndc);
    let b = dot(ro, rd);
    let cc = dot(ro, ro) - 1.0;
    let disc = b * b - cc;
    if (disc < 0.0) {
        // The void: vignette + sparse stars.
        let hs2 = hash21(floor(ndc * U.res.xy * 0.5));
        let star = step(0.9992, hs2) * (hs2 - 0.999) * 400.0;
        let vig = 0.012 * (1.0 - 0.6 * length(ndc));
        return vec4<f32>(vec3<f32>(vig) + vec3<f32>(0.7, 0.8, 1.0) * star * 0.3, 1.0);
    }
    let tt = -b - sqrt(disc);
    if (tt < 0.0) {
        return vec4<f32>(vec3<f32>(0.01), 1.0);
    }
    let p = ro + rd * tt;
    // Stereographic projection from the north pole (0,1,0).
    let denom = max(1.0 - p.y, 1e-4);
    let z = vec2<f32>(p.x, p.z) / denom;
    let fz = feval(z);
    var color = dc_color(fz, U.p[1].x, U.p[1].y);
    // Instrument lighting.
    let n = p;
    let l = normalize(vec3<f32>(0.5, 0.8, 0.35));
    let vdir = -rd;
    let diff = 0.45 + 0.62 * max(dot(n, l), 0.0);
    let fres = pow(1.0 - max(dot(n, vdir), 0.0), 3.0);
    color = color * diff + vec3<f32>(0.35, 0.45, 0.65) * fres * 0.35;
    // Engrave the real & imaginary meridians and the unit-circle equator.
    let eq = exp(-abs(p.y) * 60.0) * 0.35;
    color += vec3<f32>(1.0, 0.95, 0.8) * eq;
    return vec4<f32>(color, 1.0);
}
"#;

/// Julia set of z^2 + c with smooth escape-time shading; c in U.p[2].xy.
const JULIA_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    var z = plane_pos(ndc);
    let c = vec2<f32>(U.p[2].x, U.p[2].y);
    var it = 0.0;
    var escaped = false;
    for (var i = 0; i < 160; i++) {
        z = cmul(z, z) + c;
        it += 1.0;
        if (dot(z, z) > 64.0) { escaped = true; break; }
    }
    var color = vec3<f32>(0.006, 0.006, 0.010);
    if (escaped) {
        let smooth_i = it - log2(max(log2(dot(z, z)) * 0.5, 1e-6));
        let tt = clamp(smooth_i / 90.0, 0.0, 1.0);
        color = mix(aurum(pow(tt, 0.55)), phase_color(smooth_i * 0.23), 0.30);
        // Fast-escaping exterior falls to black; the frontier keeps the fire.
        color *= 0.9 * pow(tt, 0.6);
    }
    return vec4<f32>(color, 1.0);
}
"#;

/// Mandelbrot set, camera-zoomable, smooth-shaded.
const MANDEL_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let c = plane_pos(ndc);
    var z = vec2<f32>(0.0);
    var it = 0.0;
    var escaped = false;
    for (var i = 0; i < 220; i++) {
        z = cmul(z, z) + c;
        it += 1.0;
        if (dot(z, z) > 64.0) { escaped = true; break; }
    }
    var color = vec3<f32>(0.004, 0.004, 0.008);
    if (escaped) {
        let smooth_i = it - log2(max(log2(dot(z, z)) * 0.5, 1e-6));
        let tt = clamp(smooth_i / 110.0, 0.0, 1.0);
        color = mix(aurum(pow(tt, 0.5)), phase_color(smooth_i * 0.19 + 2.0), 0.25) * 0.92;
    }
    return vec4<f32>(color, 1.0);
}
"#;

pub struct DomainColoring {
    params: Params,
    landscape: Option<GpuMesh>,
    grid_cache: Vec<Vec<Vec3>>,
    last_key: (usize, i64, u64),
    a: C,
    morph: f64,
}

impl DomainColoring {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice(
            "fn",
            "Function f(z)",
            "Symbolic",
            vec![
                "z\u{00B2}",
                "z\u{00B3} \u{2212} 1",
                "1/z",
                "Wegert rational",
                "exp z",
                "sin z",
                "Log z",
                "Blaschke a(t)",
                "exp(1/z)",
            ],
            3,
        );
        p.slider("contours", "Modulus contours", "Display", 0.0, 1.0, 0.75, "");
        p.slider("plines", "Phase isochromes", "Display", 0.0, 1.0, 0.45, "");
        p.toggle("animate", "Animate parameter / flow", "Observer", true);
        p.slider("morph", "Grid morph z \u{2192} f(z)", "Observer", 0.0, 1.0, 1.0, "");
        p.int("grid_n", "Grid lines", "Display", 5, 41, 17);
        p.slider("hscale", "Landscape height", "Display", 0.2, 2.5, 1.0, "");
        Self {
            params: p,
            landscape: None,
            grid_cache: Vec::new(),
            last_key: (usize::MAX, 0, 0),
            a: C::ZERO,
            morph: 1.0,
        }
    }

    fn feval(&self, k: usize, z: C) -> C {
        match k {
            0 => z * z,
            1 => z * z * z - C::ONE,
            2 => C::ONE / z,
            3 => {
                let z2 = z * z;
                ((z2 - C::ONE) * (z - c(2.0, 1.0)).powi(2)) / (z2 + c(2.0, 2.0))
            }
            4 => z.exp(),
            5 => z.sin(),
            6 => z.ln(),
            7 => (z - self.a) / (C::ONE - self.a.conj() * z),
            _ => (C::ONE / z).exp(),
        }
    }

    fn landscape_data(&self) -> MeshData {
        let k = self.params.c("fn");
        let hs = self.params.f("hscale");
        MeshData::parametric(150, 150, |u, v| {
            let z = c((u as f64 - 0.5) * 4.6, (v as f64 - 0.5) * 4.6);
            let fz = self.feval(k, z);
            let h = 3.1 * ((1.0 + fz.abs()).ln() * hs / 3.1).tanh();
            let mut color = phase_color(fz.arg() as f32);
            let m = (fz.abs().max(1e-20)).log2();
            let shade = 0.72 + 0.28 * (m - m.floor()) as f32;
            color = crate::modules::util::scale_rgb(color, shade);
            (Vec3::new(z.re as f32, h as f32, z.im as f32), color)
        })
    }

    fn rebuild_grid(&mut self) {
        let k = self.params.c("fn");
        let n = self.params.i("grid_n") as usize;
        let s = self.morph;
        let mut lines = Vec::new();
        let samples = 120;
        let ext = 2.3;
        let mut push_line = |f: &dyn Fn(f64) -> C| {
            let mut pts = Vec::with_capacity(samples + 1);
            for i in 0..=samples {
                let z = f(i as f64 / samples as f64);
                let fz = self.feval(k, z);
                let mut p = c(
                    z.re * (1.0 - s) + fz.re * s,
                    z.im * (1.0 - s) + fz.im * s,
                );
                let r = p.abs();
                if r > 6.5 {
                    p = p * (6.5 / r);
                }
                pts.push(Vec3::new(p.re as f32, p.im as f32, 0.0));
            }
            lines.push(pts);
        };
        for i in 0..n {
            let x = -ext + 2.0 * ext * i as f64 / (n - 1) as f64;
            push_line(&move |t| c(x, -ext + 2.0 * ext * t));
            push_line(&move |t| c(-ext + 2.0 * ext * t, x));
        }
        self.grid_cache = lines;
    }
}

impl ConceptModule for DomainColoring {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "complex.domain",
            name: "Chromatic Plane",
            tagline: "Domain coloring of complex functions",
            branch: Branch::ComplexAnalysis,
            description: "A complex function assigns to every point of the plane another point of the plane. Color makes the assignment visible: hue is the phase of f(z), shading carries its modulus. Zeros appear as dark stars where every hue meets; poles as white ones.",
            epigraph: Some((
                "The shortest path between two truths in the real domain passes through the complex domain.",
                "Jacques Hadamard",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane("dc", "Domain Coloring", "Hue = arg f(z), shading = |f(z)| with log-spaced contours", 2.3),
            RenderMode::orbit("landscape", "Analytic Landscape", "Height = ln(1+|f|), color = phase", 10.0, 0.50, 1.05)
                .look_at(0.0, 1.1, 0.0),
            RenderMode::plane("conformal", "Conformal Grid Flow", "The Cartesian grid carried from z to f(z); angles survive", 2.8),
            RenderMode::orbit("sphere", "Riemann Sphere", "The plane wrapped stereographically; \u{221E} becomes a point", 3.6, 0.35, 0.7),
            RenderMode::plane("julia", "Julia Frontier", "z \u{2192} z\u{00B2}+c; c circles the parameter space and the frontier storms", 1.7),
            RenderMode::plane("mandel", "Mandelbrot Atlas", "The catalogue of every Julia set; zoom is exact re-evaluation", 1.3)
                .look_at(-0.6, 0.0, 0.0),
            RenderMode::plane("moebius", "M\u{00F6}bius Ballet", "Circles remain circles under (az+b)/(cz+d); the dance is loxodromic", 2.6),
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
        rend.register_field(gpu, "complex.dc", &format!("{FEVAL_WGSL}{DC_SHADE}"), false);
        rend.register_field(gpu, "complex.sphere", &format!("{FEVAL_WGSL}{SPHERE_SHADE}"), false);
        rend.register_field(gpu, "complex.julia", JULIA_SHADE, false);
        rend.register_field(gpu, "complex.mandel", MANDEL_SHADE, false);
        let mut m = self.landscape_data().upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, glam::Mat4::IDENTITY, [1.0; 4], [0.0, 0.30, 1.0, 0.0]);
        self.landscape = Some(m);
        self.rebuild_grid();
    }

    fn update(&mut self, gpu: &GpuContext, t: f64, _dt: f64) {
        let animate = self.params.b("animate");
        self.a = if animate {
            C::from_polar(0.7, 0.5 * t)
        } else {
            c(0.55, 0.35)
        };
        self.morph = if animate {
            let phase = (t / 12.0) * std::f64::consts::TAU;
            0.5 - 0.5 * phase.cos()
        } else {
            self.params.f("morph")
        };
        let key = (
            self.params.c("fn"),
            (self.params.f("hscale") * 1000.0) as i64,
            if animate && self.params.c("fn") == 7 { (t * 60.0) as u64 } else { 0 },
        );
        if key != self.last_key {
            self.last_key = key;
            let data = self.landscape_data();
            if let Some(m) = &self.landscape {
                m.update_vertices(gpu, &data.vertices);
            }
        }
        self.rebuild_grid();
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, _t: f64, _cam: &Camera, scene: &mut Scene) {
        scene.p(0, self.params.c("fn") as f32);
        scene.p(2, self.a.re as f32);
        scene.p(3, self.a.im as f32);
        scene.p(4, self.params.f("contours") as f32);
        scene.p(5, self.params.f("plines") as f32);

        let axis = [0.55, 0.62, 0.78, 0.5];
        match mode {
            0 => {
                scene.field = Some("complex.dc");
                scene.line(LineLayer::Overlay, Vec3::new(-40.0, 0.0, 0.1), Vec3::new(40.0, 0.0, 0.1), 1.2, axis);
                scene.line(LineLayer::Overlay, Vec3::new(0.0, -40.0, 0.1), Vec3::new(0.0, 40.0, 0.1), 1.2, axis);
            }
            1 => {
                scene.clear = [0.008, 0.009, 0.015, 1.0];
                if let Some(m) = &self.landscape {
                    scene.mesh(m);
                }
                // Base plane axes.
                let y = 0.005;
                scene.line(LineLayer::World, Vec3::new(-2.4, y, 0.0), Vec3::new(2.4, y, 0.0), 1.4, axis);
                scene.line(LineLayer::World, Vec3::new(0.0, y, -2.4), Vec3::new(0.0, y, 2.4), 1.4, axis);
            }
            2 => {
                scene.clear = [0.008, 0.010, 0.017, 1.0];
                let k = self.params.c("fn");
                let _ = k;
                for pts in &self.grid_cache {
                    // Color each polyline by the hue of its starting point.
                    let z0 = pts.first().copied().unwrap_or(Vec3::ZERO);
                    let hue = z0.y.atan2(z0.x);
                    let color = with_alpha(phase_color(hue), 0.85);
                    scene.polyline(LineLayer::World, pts, 1.6, color);
                }
                // Unit circle reference.
                let circle: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let a = i as f32 / 96.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos(), a.sin(), 0.05)
                    })
                    .collect();
                scene.polyline(LineLayer::Overlay, &circle, 1.0, [0.9, 0.8, 0.5, 0.35]);
            }
            3 => {
                scene.field = Some("complex.sphere");
            }
            4 => {
                // Julia: c orbits just outside the main cardioid.
                let ang = 0.35 * _t;
                let cj = C::from_polar(0.7885, ang);
                scene.p(8, cj.re as f32);
                scene.p(9, cj.im as f32);
                scene.field = Some("complex.julia");
            }
            5 => {
                scene.field = Some("complex.mandel");
            }
            _ => {
                // Moebius ballet on the Steiner net: the invariant grid of a
                // transformation with fixed points +-1, plus tracer dancers.
                scene.clear = [0.008, 0.010, 0.017, 1.0];
                // Net, family one: circles through BOTH fixed points.
                for k_c in -5i32..=5 {
                    if k_c == 0 {
                        scene.line(LineLayer::World, Vec3::new(-6.0, 0.0, 0.0), Vec3::new(6.0, 0.0, 0.0), 0.9, [0.45, 0.50, 0.62, 0.22]);
                        continue;
                    }
                    let cy = 1.35f64 / k_c as f64;
                    let r = (1.0 + cy * cy).sqrt();
                    let pts: Vec<Vec3> = (0..=120)
                        .map(|i| {
                            let a = i as f64 / 120.0 * std::f64::consts::TAU;
                            Vec3::new((r * a.cos()) as f32, (cy + r * a.sin()) as f32, 0.0)
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &pts, 0.9, [0.45, 0.50, 0.62, 0.18]);
                }
                // Family two: the orthogonal Apollonian circles around each pole.
                for k_c in 1..=5 {
                    let s = 1.18f64.powi(k_c * 2);
                    let cx_m = (s + 1.0) / (s - 1.0);
                    let r = (cx_m * cx_m - 1.0).abs().sqrt();
                    for sign in [1.0f64, -1.0] {
                        let pts: Vec<Vec3> = (0..=120)
                            .map(|i| {
                                let a = i as f64 / 120.0 * std::f64::consts::TAU;
                                Vec3::new((sign * cx_m + r * a.cos()) as f32, (r * a.sin()) as f32, 0.0)
                            })
                            .collect();
                        scene.polyline(LineLayer::World, &pts, 0.9, [0.45, 0.50, 0.62, 0.18]);
                    }
                }
                // Dancers: tracer points iterated by a fixed loxodromic map,
                // spiralling from the blue pole to the gold along the net.
                let lam = C::from_polar((0.10f64).exp(), 0.35);
                let moeb = |z: C| -> C {
                    let w = (z - C::ONE) / (z + C::ONE);
                    let w2 = lam * w;
                    (C::ONE + w2) / (C::ONE - w2)
                };
                for d in 0..26 {
                    let a0 = d as f64 / 26.0 * std::f64::consts::TAU;
                    let mut z = C::from_polar(0.55, a0) + c(0.0, 1.2);
                    let steps = ((_t * 1.2) as usize % 22) + 8;
                    let mut path = vec![Vec3::new(z.re as f32, z.im as f32, 0.0)];
                    for _ in 0..steps {
                        z = moeb(z);
                        if z.abs() > 8.0 {
                            break;
                        }
                        path.push(Vec3::new(z.re as f32, z.im as f32, 0.0));
                    }
                    let hue = phase_color(d as f32 * 0.35);
                    for i in 1..path.len() {
                        let age = i as f32 / path.len() as f32;
                        scene.line(LineLayer::WorldAdd, path[i - 1], path[i], 1.4, with_alpha(hue, 0.06 + 0.30 * age));
                    }
                    if let Some(&last) = path.last() {
                        scene.point(last, 3.2, hue, 1.6);
                    }
                }
                // The two poles of the dance.
                scene.point(Vec3::new(1.0, 0.0, 0.02), 4.5, [0.95, 0.75, 0.35, 1.0], 2.2);
                scene.point(Vec3::new(-1.0, 0.0, 0.02), 4.5, [0.45, 0.75, 0.95, 1.0], 2.2);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let k = self.params.c("fn");
        let mut out = vec![
            title("Holomorphic map  f : \u{2102} \u{2192} \u{2102}"),
            formula(format!("f(z) = {}", FN_LABELS[k])),
            def("hue(w) = arg f(w)     shading = |f(w)|"),
            ftex("complex.cr", "Cauchy\u{2013}Riemann:  \u{2202}u/\u{2202}x = \u{2202}v/\u{2202}y,   \u{2202}u/\u{2202}y = \u{2212}\u{2202}v/\u{2202}x"),
            note(FN_NOTES[k]),
        ];
        if k == 7 {
            out.push(value(format!("a(t) = {:.3} + {:.3}i,  |a| = 0.7", self.a.re, self.a.im)));
        }
        out.push(note(
            "Winding of hue around a point = order of the zero (counter-clockwise) or pole (clockwise).",
        ));
        out
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 | 3 => Some("Exact pointwise evaluation in fp32 per pixel; contours are display shading, not data."),
            1 => Some("Landscape sampled on a 150\u{00D7}150 grid; height compressed by ln(1+|f|)."),
            2 => Some("Grid images clamped to |w| \u{2264} 6.5 near poles for display."),
            _ => None,
        }
    }

    fn overlay(&self, painter: &egui::Painter, rect: egui::Rect, cam: &Camera, mode: usize, _t: f64) {
        if mode == 0 || mode == 2 {
            if let Some(p) = to_screen(cam, rect, Vec3::new(2.05, 0.08, 0.0)) {
                painter.text(p, egui::Align2::LEFT_BOTTOM, "Re", crate::theme::math_font(13.0), crate::theme::OVERLAY_DIM);
            }
            if let Some(p) = to_screen(cam, rect, Vec3::new(0.06, 2.05, 0.0)) {
                painter.text(p, egui::Align2::LEFT_BOTTOM, "Im", crate::theme::math_font(13.0), crate::theme::OVERLAY_DIM);
            }
        }
    }
}
