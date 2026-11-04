//! Electromagnetism: charges you can hold, the fields they cast,
//! and the light an oscillating dipole throws off.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const CYAN: [f32; 4] = [0.35, 0.80, 1.0, 1.0];
const MAGENTA: [f32; 4] = [1.0, 0.45, 0.75, 1.0];
const GOLDE: [f32; 4] = [0.95, 0.78, 0.35, 1.0];

/// Potential heat + equipotential bands for up to 6 charges in U.p[2..8].
const POTENTIAL_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var phi = 0.0;
    let n = i32(U.p[0].x + 0.5);
    for (var i = 0; i < n; i++) {
        let cx = U.p[2 + i].x;
        let cy = U.p[2 + i].y;
        let q = U.p[2 + i].z;
        let r = max(length(w - vec2<f32>(cx, cy)), 0.045);
        phi += q / r;
    }
    var color = vec3<f32>(0.0);
    let m = clamp(abs(phi) / (2.5 + abs(phi)), 0.0, 1.0);
    if (phi > 0.0) {
        color = mix(color, vec3<f32>(0.85, 0.30, 0.45), m * 0.75);
    } else {
        color = mix(color, vec3<f32>(0.15, 0.45, 0.90), m * 0.75);
    }
    // Equipotential bands.
    let band = abs(fract(phi * 1.4) - 0.5) * 2.0;
    color *= 0.72 + 0.28 * smoothstep(0.0, 0.35, band);
    color += vec3<f32>(0.10, 0.16, 0.20) * grid_glow(w, 0.7) * 0.10;
    return vec4<f32>(color * (1.0 - 0.25 * dot(ndc, ndc)), 1.0);
}
"#;

/// Hertzian dipole radiation: level bands of the exact field-line function
/// psi = sin^2(theta) [cos(kr - wt) + sin(kr - wt)/(kr)].
const DIPOLE_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let t = U.res.z;
    let kk = U.p[0].y;
    let r = max(length(w), 0.05);
    let s2 = w.x * w.x / max(dot(w, w), 1e-6);
    let ph = kk * r - 2.2 * t;
    let psi = s2 * (cos(ph) + sin(ph) / (kk * r));
    var color = vec3<f32>(0.0);
    // Field lines: glowing near integer levels of psi.
    let lv = psi * 3.0;
    let d = abs(fract(lv) - 0.5) * 2.0;
    let line = exp(-(1.0 - d) * (1.0 - d) * 34.0);
    let fade = 1.0 / (1.0 + r * 0.35);
    let sgn = clamp(psi * 2.0, -1.0, 1.0);
    let warm = vec3<f32>(0.95, 0.62, 0.25) * max(sgn, 0.0);
    let cool = vec3<f32>(0.25, 0.55, 0.95) * max(-sgn, 0.0);
    color += (warm + cool + vec3<f32>(0.20)) * line * fade * 1.4;
    // The near-zone glow around the dipole itself.
    color += vec3<f32>(0.9, 0.75, 0.4) * exp(-r * r * 6.0) * (0.5 + 0.5 * sin(2.2 * t));
    return vec4<f32>(color * (1.0 - 0.22 * dot(ndc, ndc)), 1.0);
}
"#;

pub struct FieldForge {
    params: Params,
    charges: Vec<(Vec2, f32)>,
    drag_idx: Option<usize>,
    /// Cyclotron dancers: (pos, vel, charge sign, trail).
    cyc: Vec<(Vec2, Vec2, f32, Vec<Vec2>)>,
    last_t: f64,
}

fn seed_cyc() -> Vec<(Vec2, Vec2, f32, Vec<Vec2>)> {
    (0..10)
        .map(|i| {
            let a = i as f32 / 10.0 * std::f32::consts::TAU;
            let q = if i % 2 == 0 { 1.0 } else { -1.0 };
            (
                Vec2::new(a.cos() * 1.4, a.sin() * 1.4),
                Vec2::new(-a.sin(), a.cos()) * (0.6 + 0.13 * i as f32),
                q,
                Vec::new(),
            )
        })
        .collect()
}

impl FieldForge {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider("k", "Dipole wavenumber k", "Physics", 2.0, 9.0, 4.5, "");
        p.toggle("lines", "Field lines", "Display", true);
        Self {
            params: p,
            charges: vec![
                (Vec2::new(-1.1, 0.0), 1.0),
                (Vec2::new(1.1, 0.0), -1.0),
                (Vec2::new(0.0, 1.4), 1.0),
                (Vec2::new(0.0, -1.4), -1.0),
            ],
            drag_idx: None,
            cyc: seed_cyc(),
            last_t: 0.0,
        }
    }

    fn efield(&self, p: Vec2) -> Vec2 {
        let mut e = Vec2::ZERO;
        for &(c, q) in &self.charges {
            let d = p - c;
            let r2 = d.length_squared().max(0.002);
            e += d * (q / (r2 * r2.sqrt()));
        }
        e
    }
}

impl ConceptModule for FieldForge {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "em.fields",
            name: "The Field Forge",
            tagline: "Charge, field, and the birth of light",
            branch: Branch::Electromagnetism,
            description: "Faraday's great invention was not a device but a picture: the field line. Here charges are things you can hold and drag while their lines re-weave themselves; and when a charge is shaken \u{2014} the Hertzian dipole \u{2014} the lines pinch off and leave as ripples of light, at exactly the rate Larmor demands.",
            epigraph: Some((
                "The most fascinating subject at the time I was a student was Maxwell's theory.",
                "Albert Einstein",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "lines",
                "Charges & Field Lines",
                "Drag the four charges; the lines re-weave live",
                3.0,
            ),
            RenderMode::plane(
                "dipole",
                "Hertzian Dipole",
                "Exact field lines of an oscillating dipole: light, leaving",
                4.4,
            ),
            RenderMode::orbit(
                "wave",
                "The Plane Wave",
                "E and B, perpendicular, in phase, self-propelling",
                8.0,
                0.30,
                0.55,
            ),
            RenderMode::plane(
                "cyclotron",
                "Cyclotron Dance",
                "qv\u{00D7}B curls charges into circles; E adds the drift",
                3.2,
            ),
            RenderMode::plane(
                "caustic",
                "The Teacup Caustic",
                "Parallel light reflecting in a cup envelopes a nephroid \u{2014} check your mug",
                1.6,
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

    fn wants_pointer(&self, mode: usize) -> bool {
        mode == 0
    }
    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode == 0).then_some("drag the charges")
    }
    fn pointer(&mut self, _mode: usize, ev: PointerEvent) {
        match ev {
            PointerEvent::Down(w) => {
                self.drag_idx = self
                    .charges
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        (a.0 - w)
                            .length_squared()
                            .partial_cmp(&(b.0 - w).length_squared())
                            .unwrap()
                    })
                    .filter(|(_, c)| (c.0 - w).length() < 0.8)
                    .map(|(i, _)| i);
                if let Some(i) = self.drag_idx {
                    self.charges[i].0 = w;
                }
            }
            PointerEvent::Drag(w) => {
                if let Some(i) = self.drag_idx {
                    self.charges[i].0 = w.clamp(Vec2::splat(-3.4), Vec2::splat(3.4));
                }
            }
            PointerEvent::Up => self.drag_idx = None,
        }
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "em.potential", POTENTIAL_SHADE, false);
        rend.register_field(gpu, "em.dipole", DIPOLE_SHADE, false);
    }

    fn update(&mut self, _gpu: &GpuContext, t: f64, dt: f64) {
        if t < self.last_t - 0.5 {
            self.cyc = seed_cyc();
        }
        self.last_t = t;
        // Lorentz force: B = 1.4 out of plane, E = 0.22 along +x.
        let h = (dt as f32).clamp(0.0, 0.033);
        let (bz, ex) = (1.4f32, 0.22f32);
        for (p, v, q, trail) in &mut self.cyc {
            for _ in 0..4 {
                let f = Vec2::new(ex + v.y * bz, -v.x * bz) * *q;
                *v += f * (h / 4.0);
                *p += *v * (h / 4.0);
            }
            trail.push(*p);
            if trail.len() > 260 {
                trail.remove(0);
            }
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        match mode {
            0 => {
                scene.field = Some("em.potential");
                scene.p(0, self.charges.len() as f32);
                for (i, &(c, q)) in self.charges.iter().enumerate() {
                    scene.params[2 + i] = [c.x, c.y, q, 0.0];
                }
                // Field lines launched from positive charges (and mirrored
                // backwards from negatives so isolated - charges get lines too).
                if self.params.b("lines") {
                    for &(c, q) in &self.charges {
                        let n_lines = 14;
                        for k in 0..n_lines {
                            let a = k as f32 / n_lines as f32 * std::f32::consts::TAU;
                            let mut p = c + Vec2::new(a.cos(), a.sin()) * 0.09;
                            let mut pts = vec![Vec3::new(p.x, p.y, 0.02)];
                            let dir_sign = q.signum();
                            for _ in 0..340 {
                                let e = self.efield(p);
                                let el = e.length();
                                if el < 1e-4 {
                                    break;
                                }
                                p += e / el * 0.028 * dir_sign;
                                pts.push(Vec3::new(p.x, p.y, 0.02));
                                if p.length() > 5.4 {
                                    break;
                                }
                                // Stop when swallowed by another charge.
                                if self
                                    .charges
                                    .iter()
                                    .any(|&(c2, _)| (p - c2).length() < 0.085)
                                {
                                    pts.push(Vec3::new(p.x, p.y, 0.02));
                                    break;
                                }
                            }
                            let col = if q > 0.0 {
                                with_alpha(GOLDE, 0.34)
                            } else {
                                with_alpha(CYAN, 0.22)
                            };
                            scene.polyline(LineLayer::WorldAdd, &pts, 1.25, col);
                        }
                    }
                }
                for &(c, q) in &self.charges {
                    let col = if q > 0.0 { MAGENTA } else { CYAN };
                    scene.point(Vec3::new(c.x, c.y, 0.05), 7.0, col, 2.4);
                    // Engrave the sign.
                    let s = 0.085;
                    scene.line(
                        LineLayer::Overlay,
                        Vec3::new(c.x - s, c.y, 0.06),
                        Vec3::new(c.x + s, c.y, 0.06),
                        1.8,
                        [0.0, 0.0, 0.0, 0.85],
                    );
                    if q > 0.0 {
                        scene.line(
                            LineLayer::Overlay,
                            Vec3::new(c.x, c.y - s, 0.06),
                            Vec3::new(c.x, c.y + s, 0.06),
                            1.8,
                            [0.0, 0.0, 0.0, 0.85],
                        );
                    }
                }
            }
            1 => {
                scene.field = Some("em.dipole");
                scene.p(1, self.params.f("k") as f32);
                // The dipole itself: a trembling vertical stub.
                let osc = (2.2 * t).sin() as f32 * 0.16;
                scene.line_soft(
                    LineLayer::WorldAdd,
                    Vec3::new(0.0, -0.2 - osc * 0.4, 0.05),
                    Vec3::new(0.0, 0.2 + osc * 0.4, 0.05),
                    3.4,
                    with_alpha(GOLDE, 0.9),
                );
            }
            3 => {
                // Uniform-B backdrop: faint symbols of field-out-of-plane.
                for gx in -4..=4 {
                    for gy in -3..=3 {
                        let p = Vec3::new(gx as f32 * 0.9, gy as f32 * 0.9, -0.05);
                        scene.point(p, 1.2, [0.35, 0.55, 0.75, 0.25], 0.4);
                    }
                }
                for (p, v, q, trail) in &self.cyc {
                    let col = if *q > 0.0 { MAGENTA } else { CYAN };
                    let n = trail.len();
                    for i in 1..n {
                        let age = i as f32 / n as f32;
                        scene.line(
                            LineLayer::WorldAdd,
                            Vec3::new(trail[i - 1].x, trail[i - 1].y, 0.0),
                            Vec3::new(trail[i].x, trail[i].y, 0.0),
                            1.5,
                            with_alpha(col, 0.04 + 0.5 * age * age),
                        );
                    }
                    scene.point(Vec3::new(p.x, p.y, 0.02), 4.0, col, 2.0);
                    scene.vector(
                        Vec3::new(p.x, p.y, 0.02),
                        Vec3::new(v.x, v.y, 0.0) * 0.35,
                        2.6,
                        with_alpha(GOLDE, 0.8),
                    );
                }
            }
            4 => {
                // The cup.
                let rim: Vec<Vec3> = (0..=180)
                    .map(|i| {
                        let a = i as f32 / 180.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos() * 1.4, a.sin() * 1.4, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &rim, 1.8, [0.7, 0.72, 0.78, 0.7]);
                // Sunlight direction slowly circles the cup.
                let sun = (t * 0.15) as f32;
                let dir = glam::Vec2::new(sun.cos(), sun.sin());
                for k in 0..110 {
                    // Impact parameter across the cup.
                    let b = -1.38 + 2.76 * k as f32 / 109.0;
                    let perp = glam::Vec2::new(-dir.y, dir.x);
                    // The ray crosses the cup and strikes the FAR inner wall.
                    let along = (1.4f32 * 1.4 - b * b).max(0.0).sqrt();
                    let hit = perp * b + dir * along;
                    let n = hit.normalize_or_zero();
                    let refl = dir - n * 2.0 * dir.dot(n);
                    let start = hit - dir * 3.2;
                    let h3 = Vec3::new(hit.x, hit.y, 0.0);
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(start.x, start.y, 0.0),
                        h3,
                        1.0,
                        [0.85, 0.75, 0.45, 0.06],
                    );
                    // Clip the reflected ray at its second rim crossing.
                    let s2 = (-2.0 * hit.dot(refl)).max(0.05);
                    let out = hit + refl * s2;
                    scene.line(
                        LineLayer::WorldAdd,
                        h3,
                        Vec3::new(out.x, out.y, 0.0),
                        1.1,
                        [0.95, 0.78, 0.35, 0.16],
                    );
                }
            }
            _ => {
                // Propagation axis.
                scene.line(
                    LineLayer::World,
                    Vec3::new(-6.4, 0.0, 0.0),
                    Vec3::new(6.4, 0.0, 0.0),
                    1.2,
                    [0.5, 0.5, 0.55, 0.5],
                );
                let k = 1.6f64;
                let om = 2.2f64;
                let n = 96;
                let mut e_env = Vec::with_capacity(n + 1);
                let mut b_env = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let z = -6.0 + 12.0 * i as f64 / n as f64;
                    let a = (k * z - om * t).cos() as f32;
                    let zf = z as f32;
                    e_env.push(Vec3::new(zf, a * 1.7, 0.0));
                    b_env.push(Vec3::new(zf, 0.0, a * 1.7));
                    if i % 3 == 0 {
                        scene.vector(
                            Vec3::new(zf, 0.0, 0.0),
                            Vec3::new(0.0, a * 1.7, 0.0),
                            2.6,
                            with_alpha(GOLDE, 0.75),
                        );
                        scene.vector(
                            Vec3::new(zf, 0.0, 0.0),
                            Vec3::new(0.0, 0.0, a * 1.7),
                            2.6,
                            with_alpha(CYAN, 0.75),
                        );
                    }
                }
                scene.glow_polyline(&e_env, 1.8, GOLDE, 2.4);
                scene.glow_polyline(&b_env, 1.8, CYAN, 2.4);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let dip: f32 = self.charges.iter().map(|&(c, q)| c.x * q).sum();
        vec![
            title("Maxwell's electrostatics & radiation"),
            ftex("em.field", "E(r) = \u{03A3} q\u{1D62} r\u{0302}\u{1D62}/r\u{1D62}\u{00B2}     \u{2207}\u{00D7}E = 0 (statics)"),
            ftex("em.dipole", "\u{03C8}(r,\u{03B8},t) = sin\u{00B2}\u{03B8}[cos(kr\u{2212}\u{03C9}t) + sin(kr\u{2212}\u{03C9}t)/kr]"),
            def("field lines leave + and land on \u{2212}; equipotentials cross them at right angles"),
            value(format!("charges: {}    p\u{2093} = {dip:+.2}", self.charges.len())),
            note("In the dipole view the lines detach at r \u{2248} \u{03BB}/2\u{03C0}: inside that radius the field breathes with the source; beyond it, the kinks are photons on their way out."),
            note("E \u{22A5} B \u{22A5} k, |E| = c|B|, in phase: the plane wave carries itself."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some(
                "Lines integrated by 340 Euler steps along E\u{0302}; potential exact per pixel.",
            ),
            1 => Some("Exact Hertzian-dipole field-line function; amplitude fade is display only."),
            _ => None,
        }
    }
}
