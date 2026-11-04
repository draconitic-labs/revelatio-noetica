//! Differential forms and the generalized Stokes theorem:
//! the boundary already knows what the interior is doing.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const VERDIGRIS: [f32; 4] = [0.35, 0.80, 0.70, 1.0];
const COPPER: [f32; 4] = [0.88, 0.55, 0.28, 1.0];
const PALE: [f32; 4] = [0.90, 0.94, 1.0, 1.0];

const PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.0);
    color += vec3<f32>(0.10, 0.22, 0.18) * grid_glow(w, 0.7) * 0.12;
    return vec4<f32>(color * (1.0 - 0.28 * dot(ndc, ndc)), 1.0);
}
"#;

/// The wavy cap surface z = h(x,y) over the unit disk.
fn cap_height(x: f32, y: f32) -> f32 {
    0.85 - 0.45 * (x * x + y * y) + 0.16 * (2.6 * x).sin() * (2.2 * y).cos()
}

/// The ambient 3D field for the Stokes demonstration.
fn field3(p: Vec3) -> Vec3 {
    Vec3::new(-p.y * 0.9 + 0.3 * p.z, p.x * 0.9, 0.4 * (1.6 * p.x).sin())
}

fn curl3(p: Vec3) -> Vec3 {
    // Analytic curl of field3.
    let e = 1e-3;
    let dx = (field3(p + Vec3::X * e) - field3(p - Vec3::X * e)) / (2.0 * e);
    let dy = (field3(p + Vec3::Y * e) - field3(p - Vec3::Y * e)) / (2.0 * e);
    let dz = (field3(p + Vec3::Z * e) - field3(p - Vec3::Z * e)) / (2.0 * e);
    Vec3::new(dy.z - dz.y, dz.x - dx.z, dx.y - dy.x)
}

pub struct FormsOratory {
    params: Params,
    cap: Option<GpuMesh>,
    flux_val: f64,
    circ_val: f64,
    work_path: Vec<Vec2>,
}

impl FormsOratory {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.int("cells", "Cell subdivision", "Physics", 2, 12, 6);
        p.slider(
            "cancel",
            "Interior cancellation",
            "Observer",
            0.0,
            1.0,
            0.6,
            "",
        );
        p.toggle("autocancel", "Animate cancellation", "Observer", true);
        Self {
            params: p,
            cap: None,
            flux_val: 0.0,
            circ_val: 0.0,
            work_path: Vec::new(),
        }
    }

    fn compute_stokes(&mut self) {
        // Circulation around the boundary circle (z on the cap rim).
        let n = 1200;
        let mut circ = 0.0f64;
        for i in 0..n {
            let a = i as f64 / n as f64 * std::f64::consts::TAU;
            let a2 = (i + 1) as f64 / n as f64 * std::f64::consts::TAU;
            let p0 = rim_point(a as f32);
            let p1 = rim_point(a2 as f32);
            let mid = (p0 + p1) * 0.5;
            circ += field3(mid).dot(p1 - p0) as f64;
        }
        // Flux of curl through the cap (parameter-grid quadrature).
        let m = 90;
        let mut flux = 0.0f64;
        for i in 0..m {
            for j in 0..m {
                let u = (i as f32 + 0.5) / m as f32;
                let v = (j as f32 + 0.5) / m as f32;
                let r = u;
                let ang = v * std::f32::consts::TAU;
                let (x, y) = (r * ang.cos(), r * ang.sin());
                let p = Vec3::new(x, cap_height(x, y), y);
                // dS from parameter derivatives.
                let du = 1.0 / m as f32;
                let pu = cap_pt(u + du * 0.5, v) - cap_pt(u - du * 0.5, v);
                let pv = cap_pt(u, v + du * 0.5) - cap_pt(u, v - du * 0.5);
                let ds = pu.cross(pv);
                flux += curl3(p).dot(ds) as f64;
            }
        }
        self.circ_val = circ;
        self.flux_val = flux;
    }
}

fn cap_pt(u: f32, v: f32) -> Vec3 {
    let r = u.clamp(0.0, 1.0);
    let ang = v * std::f32::consts::TAU;
    let (x, y) = (r * ang.cos(), r * ang.sin());
    Vec3::new(x, cap_height(x, y), y)
}

fn rim_point(a: f32) -> Vec3 {
    Vec3::new(a.cos(), cap_height(a.cos(), a.sin()), a.sin())
}

impl ConceptModule for FormsOratory {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "forms.stokes",
            name: "The Boundary Oratory",
            tagline: "\u{222B}_M d\u{03C9} = \u{222B}_\u{2202}M \u{03C9}",
            branch: Branch::DifferentialForms,
            description: "One identity contains the fundamental theorem of calculus, Green, Gauss, and the classical Stokes: integrate a form's derivative over a region, or the form itself over the region's boundary \u{2014} the answer is the same. Watch interior circulations annihilate pairwise until only the rim survives.",
            epigraph: Some(("The boundary of a boundary is zero.", "\u{2202}\u{2202} = 0")),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit(
                "stokes",
                "Surface & Boundary",
                "curl F \u{00B7} dS on the cap balances F \u{00B7} dl around the rim",
                4.6,
                0.5,
                0.8,
            )
            .look_at(0.0, 0.5, 0.0),
            RenderMode::plane(
                "cancel",
                "Telescoping Cells",
                "Green's theorem: interior edges cancel in pairs; the rim remains",
                1.7,
            ),
            RenderMode::plane(
                "oneform",
                "1-Form as Level Stack",
                "\u{03C9} = dH counts contour crossings along any path",
                1.9,
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

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "forms.paper", PAPER, false);
        let d = MeshData::parametric(64, 96, |u, v| {
            let p = cap_pt(u, v);
            let k = curl3(p).dot(cap_normal(u, v));
            let heat = (k * 0.7).clamp(-1.0, 1.0);
            let color = if heat >= 0.0 {
                [0.20 + 0.55 * heat, 0.45 + 0.30 * heat, 0.40, 0.92]
            } else {
                [0.20, 0.35 + 0.25 * -heat, 0.55 + 0.35 * -heat, 0.92]
            };
            (p, color)
        });
        let mut m = d.upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, glam::Mat4::IDENTITY, [1.0; 4], [0.0, 0.40, 0.92, 0.0]);
        self.cap = Some(m);
        self.compute_stokes();
        // Wandering integration path for the 1-form view.
        let mut path = Vec::new();
        for i in 0..=260 {
            let s = i as f32 / 260.0;
            let a = s * std::f32::consts::TAU * 1.1 - 2.4;
            path.push(Vec2::new(
                -1.3 + 2.7 * s + 0.25 * (a * 2.4).sin(),
                -0.9 + 1.7 * s.powf(0.9) + 0.30 * (a * 1.7).cos() - 0.4,
            ));
        }
        self.work_path = path;
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, _dt: f64) {}

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        match mode {
            0 => {
                if let Some(m) = &self.cap {
                    scene.mesh(m);
                }
                // curl F normal spines on the cap.
                for i in 0..7 {
                    for j in 0..12 {
                        let u = (i as f32 + 0.6) / 7.4;
                        let v = j as f32 / 12.0;
                        let p = cap_pt(u, v);
                        let nrm = cap_normal(u, v);
                        let k = curl3(p).dot(nrm);
                        scene.vector(
                            p,
                            nrm * k * 0.22,
                            2.6,
                            if k >= 0.0 {
                                with_alpha(COPPER, 0.9)
                            } else {
                                with_alpha([0.3, 0.5, 0.95, 1.0], 0.9)
                            },
                        );
                    }
                }
                // The rim, with circulation runners.
                let rim: Vec<Vec3> = (0..=200)
                    .map(|i| rim_point(i as f32 / 200.0 * std::f32::consts::TAU))
                    .collect();
                scene.glow_polyline(&rim, 2.4, VERDIGRIS, 3.0);
                for k in 0..14 {
                    let a = (k as f32 / 14.0 + (t * 0.05) as f32) % 1.0 * std::f32::consts::TAU;
                    let p = rim_point(a);
                    let tan = (rim_point(a + 0.02) - p).normalize_or_zero();
                    let speed = field3(p).dot(tan);
                    scene.point(
                        p + Vec3::Y * 0.01,
                        3.0 + speed.abs() * 2.0,
                        with_alpha(PALE, 0.9),
                        1.8,
                    );
                    scene.vector(p, tan * speed * 0.30, 2.4, with_alpha(VERDIGRIS, 0.9));
                }
            }
            1 => {
                scene.field = Some("forms.paper");
                let n = self.params.i("cells") as usize;
                let cancel = if self.params.b("autocancel") {
                    let ph = (t / 12.0) * std::f64::consts::TAU;
                    (0.5 - 0.5 * ph.cos()) as f32
                } else {
                    self.params.f("cancel") as f32
                };
                let ext = 1.25f32;
                let cell = 2.0 * ext / n as f32;
                for i in 0..n {
                    for j in 0..n {
                        let x0 = -ext + i as f32 * cell;
                        let y0 = -ext + j as f32 * cell;
                        // Each cell: rotating circulation loop; interior edges fade
                        // with `cancel`, boundary edges stay.
                        let corners = [
                            (
                                Vec3::new(x0, y0, 0.0),
                                Vec3::new(x0 + cell, y0, 0.0),
                                j == 0,
                            ),
                            (
                                Vec3::new(x0 + cell, y0, 0.0),
                                Vec3::new(x0 + cell, y0 + cell, 0.0),
                                i == n - 1,
                            ),
                            (
                                Vec3::new(x0 + cell, y0 + cell, 0.0),
                                Vec3::new(x0, y0 + cell, 0.0),
                                j == n - 1,
                            ),
                            (
                                Vec3::new(x0, y0 + cell, 0.0),
                                Vec3::new(x0, y0, 0.0),
                                i == 0,
                            ),
                        ];
                        for (a, b, on_rim) in corners {
                            let alpha = if on_rim {
                                0.95
                            } else {
                                (1.0 - cancel) * 0.55 + 0.03
                            };
                            let col = if on_rim { COPPER } else { VERDIGRIS };
                            let mid = (a + b) * 0.5;
                            let dir = (b - a) * 0.5 * 0.72;
                            scene.vector(mid - dir, dir * 2.0, 2.4, with_alpha(col, alpha));
                        }
                    }
                }
            }
            _ => {
                scene.field = Some("forms.paper");
                // Level stack of H(x,y) = y + 0.35 sin(2x): omega = dH.
                for k in -8..=10 {
                    let level = k as f32 * 0.22;
                    let pts: Vec<Vec3> = (0..=200)
                        .map(|i| {
                            let x = -1.8 + 3.6 * i as f32 / 200.0;
                            Vec3::new(x, level - 0.35 * (2.0 * x).sin(), 0.0)
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &pts, 1.1, with_alpha(VERDIGRIS, 0.30));
                }
                // The path and its running integral: crossings light up.
                let s_end = ((t / 12.0).clamp(0.0, 1.0) * self.work_path.len() as f64) as usize;
                let done: Vec<Vec3> = self.work_path[..s_end.max(2)]
                    .iter()
                    .map(|p| Vec3::new(p.x, p.y, 0.05))
                    .collect();
                let rest: Vec<Vec3> = self.work_path[s_end.saturating_sub(1)..]
                    .iter()
                    .map(|p| Vec3::new(p.x, p.y, 0.05))
                    .collect();
                scene.glow_polyline(&done, 2.4, COPPER, 2.8);
                scene.polyline(LineLayer::World, &rest, 1.0, with_alpha(COPPER, 0.18));
                if let Some(p) = self.work_path.get(s_end.max(2) - 1) {
                    scene.point(Vec3::new(p.x, p.y, 0.06), 4.5, PALE, 2.0);
                }
            }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let mut out = vec![
            title("Generalized Stokes"),
            ftex("forms.stokes", "\u{222B}_M d\u{03C9} = \u{222B}_{\u{2202}M} \u{03C9}"),
            def("1-form \u{03C9} eats curves; 2-form d\u{03C9} eats surfaces"),
            value(format!("\u{222C}_S curl F \u{00B7} dS = {:+.4}", self.flux_val)),
            value(format!("\u{222E}_{{\u{2202}S}} F \u{00B7} dl   = {:+.4}", self.circ_val)),
            note("The two integrals agree to quadrature error. Deform the cap all you like \u{2014} only the rim matters."),
        ];
        if self.params.b("autocancel") {
            let ph = (t / 12.0) * std::f64::consts::TAU;
            out.insert(
                4,
                value(format!(
                    "interior cancellation: {:.0}%",
                    (0.5 - 0.5 * ph.cos()) * 100.0
                )),
            );
        }
        // 1-form running integral.
        let s_end = ((t / 12.0).clamp(0.0, 1.0) * self.work_path.len() as f64) as usize;
        if let (Some(p0), Some(p1)) = (self.work_path.first(), self.work_path.get(s_end.max(1) - 1))
        {
            let h = |p: &Vec2| p.y + 0.35 * (2.0 * p.x).sin();
            out.push(value(format!(
                "\u{222B}\u{03B3} dH = H(end)\u{2212}H(start) = {:+.3}",
                h(p1) - h(p0)
            )));
        }
        out
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Circulation: 1200-segment quadrature; flux: 90\u{00D7}90 parameter grid; curl by central differences."),
            _ => None,
        }
    }
}

fn cap_normal(u: f32, v: f32) -> Vec3 {
    let e = 0.004;
    let pu = cap_pt(u + e, v) - cap_pt(u - e, v);
    let pv = cap_pt(u, v + e) - cap_pt(u, v - e);
    let n = pu.cross(pv).normalize_or_zero();
    if n.y < 0.0 {
        -n
    } else {
        n
    }
}
