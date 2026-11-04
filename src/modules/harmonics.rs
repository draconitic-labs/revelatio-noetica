//! Spherical harmonics: the eigenmodes of the sphere, breathing.

use glam::{Mat4, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

/// Real spherical harmonic (unnormalized shape), l <= 4, 0 <= m <= l.
fn ylm_real(l: u32, m: u32, ct: f64, phi: f64) -> f64 {
    let s = (1.0 - ct * ct).max(0.0).sqrt();
    let p = match (l, m) {
        (0, 0) => 1.0,
        (1, 0) => ct,
        (1, 1) => -s,
        (2, 0) => 0.5 * (3.0 * ct * ct - 1.0),
        (2, 1) => -3.0 * ct * s,
        (2, 2) => 3.0 * s * s,
        (3, 0) => 0.5 * ct * (5.0 * ct * ct - 3.0),
        (3, 1) => -1.5 * (5.0 * ct * ct - 1.0) * s,
        (3, 2) => 15.0 * ct * s * s,
        (3, 3) => -15.0 * s * s * s,
        (4, 0) => 0.125 * (35.0 * ct.powi(4) - 30.0 * ct * ct + 3.0),
        (4, 1) => -2.5 * ct * (7.0 * ct * ct - 3.0) * s,
        (4, 2) => 7.5 * (7.0 * ct * ct - 1.0) * s * s,
        (4, 3) => -105.0 * ct * s * s * s,
        _ => 105.0 * s.powi(4),
    };
    // Normalize the shape to roughly unit peak for display.
    let peak = match (l, m) {
        (0, 0) => 1.0,
        (1, _) => 1.0,
        (2, 0) => 1.0,
        (2, 1) => 1.5,
        (2, 2) => 3.0,
        (3, 0) => 1.0,
        (3, 1) => 3.35,
        (3, 2) => 5.8,
        (3, 3) => 15.0,
        (4, 0) => 1.0,
        (4, 1) => 4.7,
        (4, 2) => 11.1,
        (4, 3) => 40.4,
        _ => 105.0,
    };
    p / peak * (m as f64 * phi).cos()
}

pub struct SphericalChoir {
    params: Params,
    sphere: Option<GpuMesh>,
    last_key: (i64, i64),
}

impl SphericalChoir {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.int("l", "Degree \u{2113}", "Symbolic", 0, 4, 3);
        p.int("m", "Order m", "Symbolic", 0, 4, 2);
        p.slider(
            "amp",
            "Deformation amplitude",
            "Display",
            0.0,
            0.7,
            0.35,
            "",
        );
        p.slider("omega", "Mode frequency", "Physics", 0.4, 4.0, 1.6, "");
        Self {
            params: p,
            sphere: None,
            last_key: (-1, -1),
        }
    }

    fn lm(&self) -> (u32, u32) {
        let l = self.params.i("l").clamp(0, 4) as u32;
        let m = (self.params.i("m").max(0) as u32).min(l);
        (l, m)
    }

    fn build(&self, t: f64, breathing: bool) -> MeshData {
        let (l, m) = self.lm();
        let amp = self.params.f("amp");
        let om = self.params.f("omega");
        let osc = if breathing { (om * t).cos() } else { 1.0 };
        MeshData::parametric(130, 130, |u, v| {
            let phi = u as f64 * std::f64::consts::TAU;
            let th = v as f64 * std::f64::consts::PI;
            let ct = th.cos();
            let y = ylm_real(l, m, ct, phi);
            let r = 1.35 * (1.0 + amp * y * osc);
            let pos = Vec3::new(
                (r * th.sin() * phi.cos()) as f32,
                (r * ct) as f32,
                (r * th.sin() * phi.sin()) as f32,
            );
            let val = (y * osc) as f32;
            let a = val.abs().min(1.0).powf(0.7);
            // Gold swells, indigo hollows, charcoal at the nodes.
            let color = if val >= 0.0 {
                [0.10 + 0.80 * a, 0.08 + 0.52 * a, 0.06 + 0.12 * a, 1.0]
            } else {
                [0.06 + 0.10 * a, 0.08 + 0.22 * a, 0.14 + 0.78 * a, 1.0]
            };
            (pos, color)
        })
    }
}

impl ConceptModule for SphericalChoir {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "waves.harmonics",
            name: "The Spherical Choir",
            tagline: "The eigenmodes of the sphere",
            branch: Branch::Waves,
            description: "Ask what shapes a sphere can vibrate in and you are asking for the eigenfunctions of its Laplacian: the spherical harmonics Y\u{2113}\u{2098}. They are the multipoles of electromagnetism, the angular anatomy of atomic orbitals, the basis in which the cosmic microwave background is scored \u{2014} one family of functions, singing everywhere.",
            epigraph: None,
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit("breathe", "Breathing Mode", "r = 1 + A\u{00B7}Y\u{2113}\u{2098}\u{00B7}cos \u{03C9}t \u{2014} the sphere sings one note", 5.6, 0.35, 0.7),
            RenderMode::orbit("nodal", "Nodal Atlas", "Where the mode never moves: \u{2113}\u{2212}m parallels, 2m meridians", 5.6, 0.35, 0.7),
        ]
    }

    fn params(&mut self) -> &mut Params {
        &mut self.params
    }
    fn params_ref(&self) -> &Params {
        &self.params
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        let mut m = self.build(0.0, true).upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, Mat4::IDENTITY, [1.0; 4], [0.0, 0.22, 0.95, 0.0]);
        self.sphere = Some(m);
    }

    fn update(&mut self, gpu: &GpuContext, t: f64, _dt: f64) {
        let key = (self.params.i("l"), self.params.i("m"));
        let _ = key == self.last_key; // mesh rebuilt every frame while breathing
        self.last_key = key;
        let data = self.build(t, true);
        if let Some(m) = &self.sphere {
            m.update_vertices(gpu, &data.vertices);
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
        if let Some(m) = &self.sphere {
            scene.mesh(m);
        }
        if mode == 1 {
            let (l, m_ord) = self.lm();
            // Nodal parallels: zeros of P_lm(cos theta).
            let mut zeros = Vec::new();
            let mut prev_v = ylm_real(l, m_ord, 0.999, 0.0);
            for i in 1..=400 {
                let ct = 0.999 - 1.998 * i as f64 / 400.0;
                let v = ylm_real(l, m_ord, ct, 0.0);
                if prev_v * v < 0.0 {
                    zeros.push(ct);
                }
                prev_v = v;
            }
            for ct in zeros {
                let th = ct.acos();
                let ring: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let a = i as f64 / 96.0 * std::f64::consts::TAU;
                        Vec3::new(
                            (1.42 * th.sin() * a.cos()) as f32,
                            (1.42 * ct) as f32,
                            (1.42 * th.sin() * a.sin()) as f32,
                        )
                    })
                    .collect();
                scene.polyline(
                    LineLayer::World,
                    &ring,
                    1.8,
                    with_alpha([0.95, 0.85, 0.5, 1.0], 0.8),
                );
            }
            // Nodal meridians: cos(m phi) = 0.
            for j in 0..(2 * m_ord) {
                let phi = (std::f64::consts::PI / 2.0 + j as f64 * std::f64::consts::PI)
                    / m_ord.max(1) as f64;
                let mer: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let th = i as f64 / 96.0 * std::f64::consts::PI;
                        Vec3::new(
                            (1.42 * th.sin() * phi.cos()) as f32,
                            (1.42 * th.cos()) as f32,
                            (1.42 * th.sin() * phi.sin()) as f32,
                        )
                    })
                    .collect();
                scene.polyline(
                    LineLayer::World,
                    &mer,
                    1.8,
                    with_alpha([0.95, 0.85, 0.5, 1.0], 0.8),
                );
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let (l, m) = self.lm();
        vec![
            title(format!("Y\u{2113}\u{2098}  \u{2014}  \u{2113} = {l}, m = {m}")),
            ftex("harmonics.ylm", "\u{2207}\u{00B2}_{S\u{00B2}} Y\u{2113}\u{2098} = \u{2212}\u{2113}(\u{2113}+1) Y\u{2113}\u{2098}"),
            def("2\u{2113}+1 modes per degree; nodal lines: \u{2113}\u{2212}m parallels + 2m meridian arcs"),
            value(format!("eigenvalue \u{2212}\u{2113}(\u{2113}+1) = \u{2212}{}", l * (l + 1))),
            note("These are the angular factors of the hydrogen orbitals in the Quantum wing, the multipole moments of the Field Forge, and the basis of the CMB power spectrum \u{2014} the atlas rhymes."),
        ]
    }

    fn approx(&self, _mode: usize) -> Option<&'static str> {
        Some("Exact Y\u{2113}\u{2098} on a 130\u{00B2} lattice, rebuilt each frame; the overall scale is set for display, so peak displacement differs by mode.")
    }
}
