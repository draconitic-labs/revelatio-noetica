//! Manifolds: charts, tangent spaces, parallel transport, and curvature.

use glam::{Mat4, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const ICE_BLUE: [f32; 4] = [0.55, 0.72, 0.98, 1.0];
const SILVER: [f32; 4] = [0.85, 0.88, 0.95, 1.0];
const WARM: [f32; 4] = [0.95, 0.70, 0.35, 1.0];

const SURF_NAMES: [&str; 3] = ["Sphere", "Torus", "Saddle"];

/// Nearest-seed cells with luminous frontiers; seeds in U.p[2..8] (xy, zw).
const VORONOI_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var d1 = 1e9;
    var d2 = 1e9;
    var id = 0.0;
    for (var i = 0; i < 6; i++) {
        for (var half_i = 0; half_i < 2; half_i++) {
            let s = select(U.p[2 + i].xy, U.p[2 + i].zw, half_i == 1);
            let d = length(w - s);
            if (d < d1) {
                d2 = d1;
                d1 = d;
                id = f32(i * 2 + half_i);
            } else if (d < d2) {
                d2 = d;
            }
        }
    }
    // Black cells with luminous gold frontiers; a little of each cell's own hue
    // bleeds in from its edge and is gone well before the middle.
    let bleed = max(1.0 - (d2 - d1) / 0.5, 0.0);
    var color = phase_color(id * 0.55) * 0.09 * bleed * bleed;
    let edge = exp(-(d2 - d1) * (d2 - d1) * 260.0);
    color += vec3<f32>(0.92, 0.74, 0.36) * edge * 0.85;
    color += vec3<f32>(0.9, 0.95, 1.0) * exp(-d1 * d1 * 140.0) * 0.55;
    return vec4<f32>(color * (1.0 - 0.22 * dot(ndc, ndc)), 1.0);
}
"#;

fn surf(k: usize, u: f32, v: f32) -> Vec3 {
    match k {
        0 => {
            let th = v * std::f32::consts::PI;
            let ph = u * std::f32::consts::TAU;
            1.4 * Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin())
        }
        1 => {
            let (r_maj, r_min) = (1.25f32, 0.55f32);
            let a = u * std::f32::consts::TAU;
            let b = v * std::f32::consts::TAU;
            Vec3::new(
                (r_maj + r_min * b.cos()) * a.cos(),
                r_min * b.sin(),
                (r_maj + r_min * b.cos()) * a.sin(),
            )
        }
        _ => {
            let x = (u - 0.5) * 3.2;
            let z = (v - 0.5) * 3.2;
            Vec3::new(x, 0.42 * (x * x - z * z) * 0.5, z)
        }
    }
}

/// Gaussian curvature, analytic per surface.
fn gauss_k(k: usize, u: f32, v: f32) -> f32 {
    match k {
        0 => 1.0 / (1.4f32 * 1.4),
        1 => {
            let (r_maj, r_min) = (1.25f32, 0.55f32);
            let b = v * std::f32::consts::TAU;
            b.cos() / (r_min * (r_maj + r_min * b.cos()))
        }
        _ => {
            let x = (u - 0.5) * 3.2;
            let z = (v - 0.5) * 3.2;
            let hxx = 0.42;
            let hzz = -0.42;
            let hx = 0.42 * x;
            let hz = -0.42 * z;
            (hxx * hzz) / (1.0 + hx * hx + hz * hz).powi(2)
        }
    }
}

fn surf_du(k: usize, u: f32, v: f32) -> Vec3 {
    (surf(k, u + 0.002, v) - surf(k, u - 0.002, v)) / 0.004
}
fn surf_dv(k: usize, u: f32, v: f32) -> Vec3 {
    (surf(k, u, v + 0.002) - surf(k, u, v - 0.002)) / 0.004
}

pub struct ManifoldAtlas {
    params: Params,
    surface: Option<GpuMesh>,
    last_key: (usize, bool),
    /// Parallel-transported vector along the latitude loop (sphere mode).
    transported: Vec<(Vec3, Vec3)>,
}

impl ManifoldAtlas {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("surf", "Manifold", "Symbolic", SURF_NAMES.to_vec(), 0);
        p.slider(
            "lat",
            "Transport colatitude \u{03B8}\u{2080}",
            "Physics",
            0.15,
            1.35,
            0.75,
            "rad",
        );
        p.toggle("kheat", "Curvature heat", "Display", false);
        Self {
            params: p,
            surface: None,
            last_key: (usize::MAX, false),
            transported: Vec::new(),
        }
    }

    fn rebuild(&mut self, gpu: &GpuContext) {
        let k = self.params.c("surf");
        let heat = self.params.b("kheat");
        let d = MeshData::parametric(110, 110, |u, v| {
            let p = surf(k, u, v);
            let color = if heat {
                let kk = gauss_k(k, u, v);
                let m = (kk * 1.4).clamp(-1.0, 1.0);
                if m >= 0.0 {
                    [0.25 + 0.65 * m, 0.35 + 0.25 * m, 0.30, 0.95]
                } else {
                    [0.22, 0.30 + 0.25 * -m, 0.50 + 0.45 * -m, 0.95]
                }
            } else {
                [0.34, 0.42, 0.60, 0.92]
            };
            (p, color)
        });
        if let Some(m) = &self.surface {
            m.update_vertices(gpu, &d.vertices);
        } else {
            // created in init
        }
        // Parallel transport along the latitude circle (sphere only, but
        // computed for any k=0 latitude).
        self.transported.clear();
        if k == 0 {
            let th0 = self.params.f("lat") as f32;
            let n = 220;
            // Initial vector: pointing along e_theta (south), unit.
            let mut comp = glam::Vec2::new(1.0, 0.0); // components in (e_th, e_ph)
            for i in 0..=n {
                let ph = i as f32 / n as f32 * std::f32::consts::TAU;
                // On the unit sphere the transport equations along a latitude:
                // dA^th/dph = A^ph cos th ; dA^ph/dph = -A^th cos th   (angle rotates by -cos th dph)
                let p = 1.4 * Vec3::new(th0.sin() * ph.cos(), th0.cos(), th0.sin() * ph.sin());
                let e_th = Vec3::new(th0.cos() * ph.cos(), -th0.sin(), th0.cos() * ph.sin());
                let e_ph = Vec3::new(-ph.sin(), 0.0, ph.cos());
                let vect = (e_th * comp.x + e_ph * comp.y) * 0.5;
                self.transported.push((p, vect));
                let dph = std::f32::consts::TAU / n as f32;
                let rot = -th0.cos() * dph;
                comp = glam::Vec2::new(
                    comp.x * rot.cos() - comp.y * rot.sin(),
                    comp.x * rot.sin() + comp.y * rot.cos(),
                );
            }
        }
    }
}

impl ConceptModule for ManifoldAtlas {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "geometry.manifolds",
            name: "The Atlas Room",
            tagline: "Charts, tangent planes, and the memory of curvature",
            branch: Branch::Geometry,
            description: "A manifold is a space that is flat enough up close: every point owns a tangent plane, and charts cover it like map pages in an atlas. Curvature is what the pages cannot hide \u{2014} carry a vector around a loop without ever turning it, and it comes back rotated by exactly the curvature the loop enclosed.",
            epigraph: Some(Epigraph::quoted(
"Ubi materia, ibi geometria.",
"Johannes Kepler, De fundamentis astrologiae certioribus, 1602, Thesis XX",
)),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit("tangent", "Tangent Space", "The plane that best forgets the curvature at p(t)", 5.6, 0.35, 0.7),
            RenderMode::orbit("transport", "Parallel Transport", "Around the latitude loop; the holonomy angle is 2\u{03C0}(1\u{2212}cos\u{03B8}\u{2080})", 5.2, 0.5, 0.7),
            RenderMode::orbit("curvature", "Gaussian Curvature", "With curvature heat on, K > 0 is copper and K < 0 blue; the torus carries both", 5.6, 0.4, 0.7),
            RenderMode::plane("hyperbolic", "The Poincar\u{00E9} Disk", "Geodesics meet the rim at right angles; the boundary is infinitely far", 1.5),
            RenderMode::plane("voronoi", "Voronoi Bloom", "Twelve wandering seeds carve the plane into nearest-neighbour cells", 2.6),
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
        rend.register_field(gpu, "geo.voronoi", VORONOI_SHADE, false);
        let d = MeshData::parametric(110, 110, |u, v| (surf(0, u, v), [0.34, 0.42, 0.60, 0.92]));
        let mut m = d.upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, Mat4::IDENTITY, [1.0; 4], [0.0, 0.45, 0.90, 0.0]);
        self.surface = Some(m);
        self.rebuild(gpu);
    }

    fn update(&mut self, gpu: &GpuContext, _t: f64, _dt: f64) {
        let key = (self.params.c("surf"), self.params.b("kheat"));
        let lat_changed = true; // cheap enough to rebuild transport every frame
        if key != self.last_key || lat_changed {
            self.last_key = key;
            self.rebuild(gpu);
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        let k = self.params.c("surf");
        if mode <= 2 {
            if let Some(m) = &self.surface {
                scene.mesh(m);
            }
            // Engraved parameter grid.
            for i in 0..12 {
                let u = i as f32 / 12.0;
                let a: Vec<Vec3> = (0..=60)
                    .map(|j| surf(k, u, j as f32 / 60.0) * 1.001)
                    .collect();
                let b: Vec<Vec3> = (0..=60)
                    .map(|j| surf(k, j as f32 / 60.0, u) * 1.001)
                    .collect();
                scene.polyline(LineLayer::World, &a, 0.8, with_alpha(ICE_BLUE, 0.14));
                scene.polyline(LineLayer::World, &b, 0.8, with_alpha(ICE_BLUE, 0.14));
            }
        }

        match mode {
            3 => {
                // Poincare disk: boundary + a rotating pencil of geodesics.
                let rim: Vec<Vec3> = (0..=180)
                    .map(|i| {
                        let a = i as f32 / 180.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos() * 1.3, a.sin() * 1.3, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &rim, 1.8, with_alpha(SILVER, 0.7));
                let drift = t * 0.08;
                for g in 0..30 {
                    // Boundary endpoints of the geodesic (unit disk coords).
                    let t1 = drift + g as f64 * 0.6;
                    let t2 = t1 + 0.9 + 1.6 * ((g as f64 * 1.7).sin() * 0.5 + 0.5);
                    let a = glam::DVec2::new(t1.cos(), t1.sin());
                    let b = glam::DVec2::new(t2.cos(), t2.sin());
                    let denom = 1.0 + a.dot(b);
                    if denom.abs() < 1e-4 {
                        continue;
                    }
                    let c = (a + b) / denom; // centre of the orthogonal circle
                    let r = (c - a).length();
                    // Sample the arc inside the disk.
                    let a0 = (a - c).y.atan2((a - c).x);
                    let mut a1 = (b - c).y.atan2((b - c).x);
                    while a1 - a0 > std::f64::consts::PI {
                        a1 -= std::f64::consts::TAU;
                    }
                    while a0 - a1 > std::f64::consts::PI {
                        a1 += std::f64::consts::TAU;
                    }
                    let pts: Vec<Vec3> = (0..=60)
                        .map(|i| {
                            let aa = a0 + (a1 - a0) * i as f64 / 60.0;
                            Vec3::new(
                                ((c.x + r * aa.cos()) * 1.3) as f32,
                                ((c.y + r * aa.sin()) * 1.3) as f32,
                                0.0,
                            )
                        })
                        .collect();
                    let hue = crate::modules::util::phase_color(g as f32 * 0.42);
                    scene.polyline(LineLayer::WorldAdd, &pts, 1.2, with_alpha(hue, 0.30));
                }
            }
            4 => {
                scene.field = Some("geo.voronoi");
                for s in 0..6 {
                    let ph = s as f64 * 1.9;
                    let p1 = (
                        ((t * 0.21 + ph).sin() * 2.0) as f32,
                        ((t * 0.17 + ph * 1.3).cos() * 1.8) as f32,
                    );
                    let p2 = (
                        ((t * 0.13 + ph * 2.1).cos() * 2.1) as f32,
                        ((t * 0.19 + ph * 0.7).sin() * 1.9) as f32,
                    );
                    scene.params[2 + s] = [p1.0, p1.1, p2.0, p2.1];
                }
            }
            0 => {
                // A wandering point with its tangent frame.
                let s = (t / 12.0) as f32;
                let (u, v) = (
                    s + 0.13 * (s * 12.0).sin(),
                    0.35 + 0.22 * (s * std::f32::consts::TAU).sin(),
                );
                let (u, v) = (u.rem_euclid(1.0), v.clamp(0.05, 0.95));
                let p = surf(k, u, v);
                let du = surf_du(k, u, v).normalize_or_zero() * 0.7;
                let dv = surf_dv(k, u, v).normalize_or_zero() * 0.7;
                let nrm = du.cross(dv).normalize_or_zero() * 0.7;
                // Tangent plane quad as glowing outline.
                let quad = [
                    p + du + dv,
                    p + du - dv,
                    p - du - dv,
                    p - du + dv,
                    p + du + dv,
                ];
                scene.polyline(LineLayer::Overlay, &quad, 1.6, with_alpha(SILVER, 0.65));
                scene.line(
                    LineLayer::Overlay,
                    quad[0],
                    quad[2],
                    0.9,
                    with_alpha(SILVER, 0.18),
                );
                scene.line(
                    LineLayer::Overlay,
                    quad[1],
                    quad[3],
                    0.9,
                    with_alpha(SILVER, 0.18),
                );
                scene.vector(p, du, 3.6, WARM);
                scene.vector(p, dv, 3.6, ICE_BLUE);
                scene.vector(p, nrm, 3.0, SILVER);
                scene.point(p, 4.5, SILVER, 2.0);
                // The trajectory on the manifold.
                let trail: Vec<Vec3> = (0..=140)
                    .map(|i| {
                        let ss = s * i as f32 / 140.0;
                        let uu = (ss + 0.13 * (ss * 12.0).sin()).rem_euclid(1.0);
                        let vv =
                            (0.35 + 0.22 * (ss * std::f32::consts::TAU).sin()).clamp(0.05, 0.95);
                        surf(k, uu, vv) * 1.004
                    })
                    .collect();
                scene.polyline(LineLayer::World, &trail, 1.6, with_alpha(WARM, 0.55));
            }
            1 => {
                if k != 0 {
                    // Transport is staged on the sphere.
                }
                let frac = ((t / 12.0).clamp(0.0, 1.0) * self.transported.len() as f64) as usize;
                let shown = &self.transported[..frac.max(2).min(self.transported.len())];
                // Loop path.
                let path: Vec<Vec3> = shown.iter().map(|(p, _)| *p * 1.003).collect();
                scene.glow_polyline(&path, 2.0, WARM, 2.6);
                // Transported vectors along the way (sparse).
                for (i, (p, vv)) in shown.iter().enumerate().step_by(14) {
                    let fade = 0.25 + 0.75 * (i as f32 / shown.len().max(1) as f32);
                    scene.vector(*p, *vv, 3.0, with_alpha(SILVER, fade));
                }
                if let Some((p, vv)) = shown.last() {
                    scene.vector(*p, *vv, 4.2, SILVER);
                    scene.point(*p, 4.5, SILVER, 2.0);
                }
                // The initial vector, engraved for comparison.
                if let Some((p0, v0)) = self.transported.first() {
                    scene.vector(*p0, *v0, 3.0, with_alpha(WARM, 0.9));
                }
            }
            _ => { /* curvature heat is in the vertex colors */ }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let k = self.params.c("surf");
        let th0 = self.params.f("lat");
        let mut out = vec![
            title(format!("{} \u{2014} a 2-manifold in \u{211D}\u{00B3}", SURF_NAMES[k])),
            ftex("geometry.metric", "T\u{209A}M = span(\u{2202}u, \u{2202}v)     g\u{1D62}\u{2C7C} = \u{2202}\u{1D62}\u{00B7}\u{2202}\u{2C7C}"),
        ];
        match k {
            0 => out.push(value(format!(
                "K = 1/R\u{00B2} = {:.3} everywhere",
                1.0 / (1.4f64 * 1.4)
            ))),
            1 => out.push(value(
                "K > 0 outside, K < 0 inside, \u{222C}K dA = 0 = 2\u{03C0}\u{03C7}(T\u{00B2})"
                    .to_string(),
            )),
            _ => out.push(value(
                "K < 0 everywhere: a saddle at every point".to_string(),
            )),
        }
        if k == 0 {
            let hol = std::f64::consts::TAU * (1.0 - th0.cos());
            let frac = (t / 12.0).clamp(0.0, 1.0);
            out.push(ftex(
                "geometry.holonomy",
                "holonomy \u{03B1} = \u{222C} K dA = 2\u{03C0}(1 \u{2212} cos\u{03B8}\u{2080})",
            ));
            out.push(value(format!(
                "\u{03B8}\u{2080} = {th0:.2}   \u{03B1} = {:.3} rad = {:.1}\u{00B0}   (loop {:.0}% done)",
                hol,
                hol.to_degrees(),
                frac * 100.0
            )));
            out.push(note("The vector is never rotated locally \u{2014} each step keeps it as parallel as the sphere allows. The rotation that remains after the loop is pure curvature."));
        }
        out.push(note("Gauss's Theorema Egregium: K is intrinsic \u{2014} an ant with a ruler can measure it without ever leaving the surface."));
        out
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            1 => Some("Transport integrated in 220 steps along the latitude; holonomy exact to that resolution."),
            _ => None,
        }
    }
}
