//! Linear transformations: the plane stretched, turned, and decomposed.

use glam::{Mat2, Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

const RED: [f32; 4] = [0.95, 0.35, 0.35, 1.0];
const STEEL: [f32; 4] = [0.45, 0.65, 0.95, 1.0];
const GOLDL: [f32; 4] = [0.92, 0.75, 0.35, 1.0];
const GRID_C: [f32; 4] = [0.35, 0.45, 0.65, 0.30];

const PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.012, 0.010, 0.013);
    color += vec3<f32>(0.22, 0.10, 0.12) * grid_glow(w, 0.65) * 0.10;
    return vec4<f32>(color * (1.0 - 0.28 * dot(ndc, ndc)), 1.0);
}
"#;

/// 2x2 eigenvalues: (re1, im1, re2, im2).
fn eigen2(m: Mat2) -> (f64, f64, f64, f64) {
    let a = m.col(0).x as f64;
    let b = m.col(1).x as f64;
    let c = m.col(0).y as f64;
    let d = m.col(1).y as f64;
    let tr = a + d;
    let det = a * d - b * c;
    let disc = tr * tr / 4.0 - det;
    if disc >= 0.0 {
        let s = disc.sqrt();
        (tr / 2.0 + s, 0.0, tr / 2.0 - s, 0.0)
    } else {
        let s = (-disc).sqrt();
        (tr / 2.0, s, tr / 2.0, -s)
    }
}

/// Real eigenvector for eigenvalue `l` (if any).
fn eigvec(m: Mat2, l: f32) -> Option<Vec2> {
    let a = m.col(0).x - l;
    let b = m.col(1).x;
    let c = m.col(0).y;
    let d = m.col(1).y - l;
    let v1 = Vec2::new(-b, a);
    let v2 = Vec2::new(-d, c);
    let v = if v1.length_squared() > v2.length_squared() { v1 } else { v2 };
    (v.length_squared() > 1e-9).then(|| v.normalize())
}

/// SVD of 2x2: returns (angle_u, s1, s2, angle_v) with A = R(u) S R(v)^T.
fn svd2(m: Mat2) -> (f32, f32, f32, f32) {
    let a = m.col(0).x;
    let b = m.col(1).x;
    let c = m.col(0).y;
    let d = m.col(1).y;
    let e = (a + d) * 0.5;
    let f = (a - d) * 0.5;
    let g = (c + b) * 0.5;
    let h = (c - b) * 0.5;
    let q = (e * e + h * h).sqrt();
    let r = (f * f + g * g).sqrt();
    let s1 = q + r;
    let s2 = q - r;
    let a1 = g.atan2(f);
    let a2 = h.atan2(e);
    let theta = (a2 - a1) * 0.5;
    let phi = (a2 + a1) * 0.5;
    (phi, s1, s2, theta)
}

pub struct LinearAtelier {
    params: Params,
}

impl LinearAtelier {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider("a", "a  (col 1, x)", "Symbolic", -2.5, 2.5, 1.2, "");
        p.slider("c", "c  (col 1, y)", "Symbolic", -2.5, 2.5, 0.4, "");
        p.slider("b", "b  (col 2, x)", "Symbolic", -2.5, 2.5, 0.6, "");
        p.slider("d", "d  (col 2, y)", "Symbolic", -2.5, 2.5, 0.9, "");
        p.toggle("animate", "Animate I \u{2192} A", "Observer", true);
        p.slider("blend", "Interpolation s", "Observer", 0.0, 1.0, 1.0, "");
        Self { params: p }
    }

    fn matrix(&self) -> Mat2 {
        Mat2::from_cols(
            Vec2::new(self.params.f("a") as f32, self.params.f("c") as f32),
            Vec2::new(self.params.f("b") as f32, self.params.f("d") as f32),
        )
    }

    fn blend(&self, t: f64) -> f32 {
        if self.params.b("animate") {
            let ph = (t / 12.0) * std::f64::consts::TAU;
            (0.5 - 0.5 * ph.cos()) as f32
        } else {
            self.params.f("blend") as f32
        }
    }
}

impl ConceptModule for LinearAtelier {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "linalg.transform",
            name: "The Shear Atelier",
            tagline: "Matrices as motions of the plane",
            branch: Branch::LinearAlgebra,
            description: "A matrix is not a grid of numbers but a motion: it carries every vector to a new one, linearly. The unit square becomes a parallelogram whose signed area is the determinant; special directions \u{2014} eigenvectors \u{2014} are merely stretched; and every matrix whatsoever factors as rotation \u{00B7} stretch \u{00B7} rotation.",
            epigraph: None,
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane("grid", "Grid Deformation", "The lattice carried by (1\u{2212}s)I + sA; det = signed area", 3.0),
            RenderMode::plane("eigen", "Eigen-directions", "Rays that only stretch; everything else turns", 3.0),
            RenderMode::plane("svd", "SVD Stage", "A = U \u{03A3} V\u{1D40}: rotate, stretch, rotate", 3.0),
        ]
    }

    fn params(&mut self) -> &mut Params {
        &mut self.params
    }
    fn params_ref(&self) -> &Params {
        &self.params
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "linalg.paper", PAPER, false);
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, _dt: f64) {}

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        scene.field = Some("linalg.paper");
        let a = self.matrix();
        let s = self.blend(t);
        let m = Mat2::from_cols(
            Vec2::new(1.0 + (a.col(0).x - 1.0) * s, a.col(0).y * s),
            Vec2::new(a.col(1).x * s, 1.0 + (a.col(1).y - 1.0) * s),
        );
        let det = m.determinant();

        let map = |p: Vec2| -> Vec3 {
            let q = m * p;
            Vec3::new(q.x, q.y, 0.0)
        };

        match mode {
            0 => {
                // Deformed lattice.
                let n = 13;
                let ext = 3.0f32;
                for i in 0..=n {
                    let x = -ext + 2.0 * ext * i as f32 / n as f32;
                    let mut va = Vec::new();
                    let mut vb = Vec::new();
                    for j in 0..=48 {
                        let y = -ext + 2.0 * ext * j as f32 / 48.0;
                        va.push(map(Vec2::new(x, y)));
                        vb.push(map(Vec2::new(y, x)));
                    }
                    let is_axis = i == n / 2 && n % 2 == 0;
                    let _ = is_axis;
                    scene.polyline(LineLayer::World, &va, 1.1, GRID_C);
                    scene.polyline(LineLayer::World, &vb, 1.1, GRID_C);
                }
                // Unit square -> parallelogram, tinted by det sign.
                let sq = [Vec2::ZERO, Vec2::X, Vec2::ONE, Vec2::Y, Vec2::ZERO];
                let poly: Vec<Vec3> = sq.iter().map(|&p| map(p) + Vec3::new(0.0, 0.0, 0.05)).collect();
                let fill_c = if det >= 0.0 { with_alpha(GOLDL, 0.85) } else { with_alpha(RED, 0.9) };
                scene.polyline(LineLayer::WorldAdd, &poly, 2.4, fill_c);
                // Basis vectors: the columns of M.
                scene.vector(Vec3::ZERO, Vec3::new(m.col(0).x, m.col(0).y, 0.1), 4.5, RED);
                scene.vector(Vec3::ZERO, Vec3::new(m.col(1).x, m.col(1).y, 0.1), 4.5, STEEL);
                // Ghost of the identity basis.
                scene.line(LineLayer::World, Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0), 1.0, with_alpha(RED, 0.25));
                scene.line(LineLayer::World, Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), 1.0, with_alpha(STEEL, 0.25));
            }
            1 => {
                // Ring of unit vectors and their images; eigen rays highlighted.
                for i in 0..48 {
                    let ang = i as f32 / 48.0 * std::f32::consts::TAU;
                    let u = Vec2::new(ang.cos(), ang.sin());
                    let v = m * u;
                    scene.line(LineLayer::World, Vec3::new(u.x, u.y, 0.0), Vec3::new(v.x, v.y, 0.0), 1.2, with_alpha(STEEL, 0.30));
                    scene.point(Vec3::new(v.x, v.y, 0.0), 1.8, with_alpha(GOLDL, 0.8), 1.0);
                }
                let circle: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let ang = i as f32 / 96.0 * std::f32::consts::TAU;
                        Vec3::new(ang.cos(), ang.sin(), 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &circle, 1.0, with_alpha([0.7, 0.75, 0.85, 1.0], 0.3));
                let (r1, i1, r2, _) = eigen2(m);
                if i1.abs() < 1e-9 {
                    for (l, c) in [(r1 as f32, RED), (r2 as f32, GOLDL)] {
                        if let Some(v) = eigvec(m, l) {
                            scene.line(LineLayer::WorldAdd, Vec3::new(-v.x * 8.0, -v.y * 8.0, 0.05), Vec3::new(v.x * 8.0, v.y * 8.0, 0.05), 1.8, with_alpha(c, 0.5));
                            scene.vector(Vec3::ZERO, Vec3::new(v.x * l, v.y * l, 0.08), 4.0, c);
                        }
                    }
                } else {
                    // Complex pair: draw the rotation spiral of iterates.
                    let mut p = Vec2::new(1.3, 0.0);
                    let mut spiral = vec![Vec3::new(p.x, p.y, 0.05)];
                    for _ in 0..60 {
                        p = m * p * 0.999;
                        spiral.push(Vec3::new(p.x, p.y, 0.05));
                        if p.length() > 6.0 || p.length() < 0.01 {
                            break;
                        }
                    }
                    scene.polyline(LineLayer::WorldAdd, &spiral, 1.6, with_alpha(RED, 0.6));
                }
            }
            _ => {
                // SVD in three acts, staged over the loop.
                let (phi, s1, s2, theta) = svd2(a);
                let stage = (t / 12.0).clamp(0.0, 0.9999) * 3.0;
                let (w1, w2, w3) = (
                    (stage).clamp(0.0, 1.0) as f32,
                    (stage - 1.0).clamp(0.0, 1.0) as f32,
                    (stage - 2.0).clamp(0.0, 1.0) as f32,
                );
                let rot = |ang: f32| Mat2::from_angle(ang);
                let cur = rot(phi * w3.max(0.0))
                    * Mat2::from_cols(Vec2::new(1.0 + (s1 - 1.0) * w2, 0.0), Vec2::new(0.0, 1.0 + (s2 - 1.0) * w2))
                    * rot(-theta * w1);
                let mapc = |p: Vec2| -> Vec3 {
                    let q = cur * p;
                    Vec3::new(q.x, q.y, 0.0)
                };
                // The untouched unit circle, engraved for reference.
                let unit: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let ang = i as f32 / 96.0 * std::f32::consts::TAU;
                        Vec3::new(ang.cos(), ang.sin(), -0.01)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &unit, 1.0, with_alpha([0.7, 0.74, 0.84, 1.0], 0.30));
                // Spokes: every marked direction and where it lands.
                for i in 0..24 {
                    let ang = i as f32 / 24.0 * std::f32::consts::TAU;
                    let u = Vec2::new(ang.cos(), ang.sin());
                    let q = mapc(u);
                    let hue = phase_color(ang);
                    scene.line(LineLayer::World, Vec3::new(u.x, u.y, 0.0), q, 1.0, with_alpha(hue, 0.30));
                    scene.point(q, 2.4, with_alpha(hue, 0.95), 1.3);
                }
                // Circle -> ellipse, luminous.
                let ell: Vec<Vec3> = (0..=160)
                    .map(|i| {
                        let ang = i as f32 / 160.0 * std::f32::consts::TAU;
                        mapc(Vec2::new(ang.cos(), ang.sin()))
                    })
                    .collect();
                scene.glow_polyline(&ell, 2.6, GOLDL, 3.0);
                scene.vector(Vec3::ZERO, mapc(Vec2::X), 4.5, RED);
                scene.vector(Vec3::ZERO, mapc(Vec2::Y), 4.5, STEEL);
                // Principal semi-axes of the final ellipse.
                let u1 = rot(phi) * Vec2::new(s1, 0.0);
                let u2 = rot(phi) * Vec2::new(0.0, s2);
                scene.vector(Vec3::ZERO, Vec3::new(u1.x, u1.y, 0.05), 3.0, with_alpha(GOLDL, 0.75));
                scene.vector(Vec3::ZERO, Vec3::new(u2.x, u2.y, 0.05), 3.0, with_alpha(GOLDL, 0.75));
            }
        }
    }

    fn wants_pointer(&self, mode: usize) -> bool {
        mode == 0
    }

    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode == 0).then_some("drag the basis vectors to sculpt A")
    }

    fn pointer(&mut self, _mode: usize, ev: crate::engine::module::PointerEvent) {
        use crate::engine::module::PointerEvent as PE;
        if let PE::Down(w) | PE::Drag(w) = ev {
            // Grab whichever transformed basis tip is nearer; freeze the morph.
            self.params.set_f("blend", 1.0);
            let m = self.matrix();
            let d1 = (Vec2::new(w.x, w.y) - m.col(0)).length_squared();
            let d2 = (Vec2::new(w.x, w.y) - m.col(1)).length_squared();
            let (ax, ay) = if d1 <= d2 { ("a", "c") } else { ("b", "d") };
            self.params.set_f(ax, (w.x as f64).clamp(-2.5, 2.5));
            self.params.set_f(ay, (w.y as f64).clamp(-2.5, 2.5));
            if self.params.b("animate") {
                // Dragging implies direct control.
                self.params.load_json(&serde_json::json!({"animate": false}));
            }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let a = self.matrix();
        let det = a.determinant();
        let (r1, i1, r2, i2) = eigen2(a);
        let (_, s1, s2, _) = svd2(a);
        let eig_str = if i1.abs() < 1e-9 {
            format!("\u{03BB}\u{2081} = {r1:.3}    \u{03BB}\u{2082} = {r2:.3}")
        } else {
            format!("\u{03BB} = {r1:.3} \u{00B1} {:.3}i  (rotation-like)", i1.abs())
        };
        let _ = i2;
        vec![
            title("A : \u{211D}\u{00B2} \u{2192} \u{211D}\u{00B2}"),
            formula(format!(
                "A = \u{23A1} {:+.2}  {:+.2} \u{23A4}\n      \u{23A3} {:+.2}  {:+.2} \u{23A6}",
                a.col(0).x, a.col(1).x, a.col(0).y, a.col(1).y
            )),
            ftex("linalg.svd", "Av = \u{03BB}v on eigen-directions;  A = U\u{03A3}V\u{1D40} always"),
            value(format!("det A = {det:.4}   tr A = {:.4}", a.col(0).x + a.col(1).y)),
            value(eig_str),
            value(format!("\u{03C3}\u{2081} = {s1:.3}   \u{03C3}\u{2082} = {s2:.3}   \u{03BA} = {:.2}", (s1 / s2.max(1e-6)).abs())),
            value(format!("s = {:.2}", self.blend(t))),
            note("Negative determinant flips orientation \u{2014} the parallelogram turns red. Complex eigenvalues mean no direction survives: everything rotates."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Linear interpolation (1\u{2212}s)I + sA; not a rotation path \u{2014} intermediate frames may collapse."),
            2 => Some("Exact closed-form 2\u{00D7}2 SVD; stages eased linearly in time."),
            _ => None,
        }
    }
}
