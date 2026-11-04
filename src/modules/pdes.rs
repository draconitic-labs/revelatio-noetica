//! Partial differential equations: heat, harmonic equilibrium, shocks,
//! eigenmodes, and the chemistry of pattern.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

const NG: usize = 128; // 2D grids (Laplace, Gray-Scott)
const NX1: usize = 400; // 1D grids (heat, Burgers)

const TEAL_P: [f32; 4] = [0.35, 0.85, 0.75, 1.0];
const APRICOT_P: [f32; 4] = [1.0, 0.66, 0.38, 1.0];
const GHOST_P: [f32; 4] = [0.6, 0.65, 0.72, 0.35];

/// 2D field display: r channel through a palette chosen by U.p[0].x.
const FIELD2D_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let tex_uv = (w + vec2<f32>(3.2, 3.2)) / 6.4;
    if (tex_uv.x < 0.0 || tex_uv.x > 1.0 || tex_uv.y < 0.0 || tex_uv.y > 1.0) {
        return vec4<f32>(vec3<f32>(0.0), 1.0);
    }
    let s = textureSample(t_field, s_field, vec2<f32>(tex_uv.x, 1.0 - tex_uv.y)).r;
    var color: vec3<f32>;
    if (U.p[0].x < 0.5) {
        color = inferno(clamp(s, 0.0, 1.0));
    } else if (U.p[0].x < 1.5) {
        // Gray-Scott: v in [0, ~0.4]; teal life on black.
        let v = clamp(s * 2.6, 0.0, 1.0);
        color = mix(vec3<f32>(0.0), vec3<f32>(0.20, 0.85, 0.72), v);
        color += aurum(v) * 0.15;
    } else {
        // Potential plot: inferno heat + engraved equipotential rings.
        color = inferno(clamp(s, 0.0, 1.0)) * 0.88;
        let band = abs(fract(s * 14.0) - 0.5) * 2.0;
        color += vec3<f32>(0.85, 0.70, 0.38) * exp(-(1.0 - band) * (1.0 - band) * 60.0) * 0.35;
    }
    return vec4<f32>(color * (1.0 - 0.20 * dot(ndc, ndc)), 1.0);
}
"#;

fn f16_bits(x: f32) -> u16 {
    let b = x.to_bits();
    let s = ((b >> 16) & 0x8000) as u16;
    let e = ((b >> 23) & 0xff) as i32 - 127 + 15;
    let m = b & 0x007f_ffff;
    if e <= 0 {
        return s;
    }
    if e >= 31 {
        return s | 0x7c00;
    }
    s | ((e as u16) << 10) | ((m >> 13) as u16)
}

/// Resting state of the FitzHugh-Nagumo cell: the fixed point of its equations.
/// Starting elsewhere lets the whole sheet relax and kills the wavefront.
const FHN_REST: (f32, f32) = (-1.08005, -0.6601);

/// Heat mesh ratio per unit of diffusivity, chosen so the slider's top value,
/// alpha = 2, sits at r = 0.45, inside the explicit scheme's stability limit.
const HEAT_R_PER_ALPHA: f64 = 0.225;

/// Burgers advances the same time every frame whatever nu is: this returns a step
/// short enough for both stability limits and how many of them make that time.
fn burgers_steps(dx: f64, nu: f64) -> (f64, usize) {
    let per_frame = 6.0 * 0.30 * dx;
    let limit = (0.30 * dx).min(0.35 * dx * dx / nu.max(1e-6));
    let n = (per_frame / limit).ceil() as usize;
    (per_frame / n as f64, n)
}

/// Viscosity spends the wave for good, and a timeline loop does not restart a
/// simulation, so the profile starts over when its peak has decayed this far.
const BURGERS_RESTART_PEAK: f64 = 0.2;

/// A smooth sine that wants to break.
fn burgers_start() -> Vec<f64> {
    (0..NX1)
        .map(|i| {
            let x = i as f64 / NX1 as f64;
            (std::f64::consts::TAU * x).sin() * 0.9
        })
        .collect()
}

fn peak_magnitude(u: &[f64]) -> f64 {
    u.iter().fold(0.0, |peak, v| v.abs().max(peak))
}

pub struct PdeHall {
    params: Params,
    heat: Vec<f64>,
    heat_snaps: Vec<Vec<f64>>,
    burgers: Vec<f64>,
    lap: Vec<f32>,
    gs_u: Vec<f32>,
    gs_v: Vec<f32>,
    fhn_u: Vec<f32>,
    fhn_v: Vec<f32>,
    tex: Option<wgpu::Texture>,
    tex_view: Option<wgpu::TextureView>,
    sim_t: f64,
}

impl PdeHall {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider(
            "alpha",
            "Diffusivity \u{03B1}",
            "Physics",
            0.2,
            2.0,
            1.0,
            "",
        );
        p.slider(
            "nu",
            "Burgers viscosity \u{03BD}",
            "Physics",
            0.001,
            0.02,
            0.004,
            "",
        );
        p.int("modes", "String modes N", "Physics", 1, 24, 8);
        p.choice(
            "gs",
            "Gray\u{2013}Scott regime",
            "Symbolic",
            vec!["Spots (mitosis)", "Stripes (coral)", "U-skate parameters"],
            0,
        );
        Self {
            params: p,
            heat: Vec::new(),
            heat_snaps: Vec::new(),
            burgers: Vec::new(),
            lap: Vec::new(),
            gs_u: Vec::new(),
            gs_v: Vec::new(),
            fhn_u: Vec::new(),
            fhn_v: Vec::new(),
            tex: None,
            tex_view: None,
            sim_t: 0.0,
        }
    }

    fn reset(&mut self) {
        // Heat: two hot blocks on a cold rod.
        self.heat = (0..NX1)
            .map(|i| {
                let x = i as f64 / NX1 as f64;
                if (0.18..0.32).contains(&x) || (0.55..0.62).contains(&x) {
                    1.0
                } else {
                    0.0
                }
            })
            .collect();
        self.heat_snaps.clear();
        self.burgers = burgers_start();
        // Laplace: warm-start the interior by blending the boundaries so the
        // relaxation is visibly refining, not filling black.
        self.lap = (0..NG * NG)
            .map(|idx| {
                let (i, j) = (idx % NG, idx / NG);
                let fx = i as f32 / (NG - 1) as f32;
                let fy = j as f32 / (NG - 1) as f32;
                let left = 1.0 * (1.0 - fx);
                let top = ((i as f32 / NG as f32 * std::f32::consts::TAU * 1.5).sin() * 0.5 + 0.5)
                    * (1.0 - fy);
                let bot = (1.0
                    - ((i as f32 / NG as f32 * std::f32::consts::TAU * 1.5).sin() * 0.5 + 0.5))
                    * fy;
                (left + top + bot) * 0.5
            })
            .collect();
        // Gray-Scott: u=1, v=0 with a seeded square.
        self.gs_u = vec![1.0; NG * NG];
        self.gs_v = vec![0.0; NG * NG];
        let mut seed = 0x9E37u64;
        for _ in 0..14 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let cx = 20 + (seed >> 20) as usize % (NG - 40);
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let cy = 20 + (seed >> 20) as usize % (NG - 40);
            for j in 0..6 {
                for i in 0..6 {
                    let idx = (cy + j) * NG + cx + i;
                    self.gs_u[idx] = 0.5;
                    self.gs_v[idx] = 0.25;
                }
            }
        }
        // FitzHugh-Nagumo: start from rest. A broken wavefront curls into a spiral.
        self.fhn_u = vec![FHN_REST.0; NG * NG];
        self.fhn_v = vec![FHN_REST.1; NG * NG];
        for j in 0..NG {
            for i in 0..NG {
                let idx = j * NG + i;
                if i < NG / 2 && (NG / 2 - 6..NG / 2 + 6).contains(&j) {
                    self.fhn_u[idx] = 1.0;
                }
                if i < NG / 2 && j < NG / 2 - 6 {
                    self.fhn_v[idx] = 0.8;
                }
            }
        }
        self.sim_t = 0.0;
    }

    fn step_fhn(&mut self, n_steps: usize) {
        let (eps, beta_f, gamma_f, du, dt) = (0.06f32, 0.75f32, 0.5f32, 0.20f32, 0.28f32);
        for _ in 0..n_steps {
            let u0 = self.fhn_u.clone();
            for j in 0..NG {
                for i in 0..NG {
                    let idx = j * NG + i;
                    let lap = u0[j * NG + (i + 1) % NG]
                        + u0[j * NG + (i + NG - 1) % NG]
                        + u0[((j + 1) % NG) * NG + i]
                        + u0[((j + NG - 1) % NG) * NG + i]
                        - 4.0 * u0[idx];
                    let (u, v) = (u0[idx], self.fhn_v[idx]);
                    self.fhn_u[idx] = u + dt * (u - u * u * u / 3.0 - v + du * lap);
                    self.fhn_v[idx] = v + dt * eps * (u + beta_f - gamma_f * v);
                }
            }
        }
    }

    fn upload(&self, gpu: &GpuContext, data: &[f32]) {
        let Some(tex) = &self.tex else { return };
        let mut buf = Vec::with_capacity(NG * NG * 2);
        for &v in data {
            buf.push(f16_bits(v));
            buf.push(0u16);
        }
        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytemuck::cast_slice(&buf),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some((NG * 4) as u32),
                rows_per_image: Some(NG as u32),
            },
            wgpu::Extent3d {
                width: NG as u32,
                height: NG as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    /// One frame of Burgers: upwind plus viscosity, dt capped by both stability limits.
    fn step_burgers(&mut self) {
        let nu = self.params.f("nu");
        let dx = 1.0 / NX1 as f64;
        let (dtb, burgers_steps) = burgers_steps(dx, nu);
        for _ in 0..burgers_steps {
            let u0 = self.burgers.clone();
            for i in 0..NX1 {
                let im = (i + NX1 - 1) % NX1;
                let ip = (i + 1) % NX1;
                let u = u0[i];
                let dudx = if u >= 0.0 {
                    (u - u0[im]) / dx
                } else {
                    (u0[ip] - u) / dx
                };
                let visc = nu * (u0[ip] - 2.0 * u + u0[im]) / (dx * dx);
                self.burgers[i] = u + dtb * (-u * dudx + visc);
            }
        }
        if peak_magnitude(&self.burgers) < BURGERS_RESTART_PEAK {
            self.burgers = burgers_start();
        }
    }

    fn step_gray_scott(&mut self, n_steps: usize) {
        let (f, k) = match self.params.c("gs") {
            0 => (0.0367, 0.0649),
            1 => (0.0545, 0.0620),
            _ => (0.0620, 0.0609),
        };
        let (du, dv, dt) = (0.16f32, 0.08f32, 1.0f32);
        for _ in 0..n_steps {
            let u0 = self.gs_u.clone();
            let v0 = self.gs_v.clone();
            for j in 0..NG {
                for i in 0..NG {
                    let idx = j * NG + i;
                    let l = |g: &Vec<f32>| -> f32 {
                        g[j * NG + (i + 1) % NG]
                            + g[j * NG + (i + NG - 1) % NG]
                            + g[((j + 1) % NG) * NG + i]
                            + g[((j + NG - 1) % NG) * NG + i]
                            - 4.0 * g[idx]
                    };
                    let (u, v) = (u0[idx], v0[idx]);
                    let uvv = u * v * v;
                    self.gs_u[idx] = u + dt * (du * l(&u0) - uvv + f * (1.0 - u));
                    self.gs_v[idx] = v + dt * (dv * l(&v0) + uvv - (f + k as f32) * v);
                }
            }
        }
    }

    fn relax_laplace(&mut self, sweeps: usize) {
        // Boundary: left edge hot, right cold, top/bottom sinusoidal.
        for j in 0..NG {
            self.lap[j * NG] = 1.0;
            self.lap[j * NG + NG - 1] = 0.0;
        }
        for i in 0..NG {
            let s = (i as f32 / NG as f32 * std::f32::consts::TAU * 1.5).sin() * 0.5 + 0.5;
            self.lap[i] = s;
            self.lap[(NG - 1) * NG + i] = 1.0 - s;
        }
        for _ in 0..sweeps {
            for j in 1..NG - 1 {
                for i in 1..NG - 1 {
                    let idx = j * NG + i;
                    self.lap[idx] = 0.25
                        * (self.lap[idx - 1]
                            + self.lap[idx + 1]
                            + self.lap[idx - NG]
                            + self.lap[idx + NG]);
                }
            }
        }
    }
}

impl ConceptModule for PdeHall {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "diffeq.pdes",
            name: "The Field Equations Hall",
            tagline: "Laws that speak at every point at once",
            branch: Branch::DiffEq,
            description: "An ODE governs a quantity; a PDE governs a field \u{2014} infinitely many coupled unknowns, one at every point of space. Here the great archetypes run live: heat forgetting its past, Laplace's instant diplomacy of boundaries, a smooth wave breaking into a Burgers shock, a string resolving into its eigenmodes, and two chemicals inventing biology-shaped patterns.",
            epigraph: Some(Epigraph::quoted(
"Profound study of nature is the most fertile source of mathematical discoveries.",
"Joseph Fourier, The Analytical Theory of Heat, tr. Freeman",
)),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane("heat", "The Cooling Rod", "u\u{209C} = \u{03B1}u\u{2093}\u{2093}: sharp edges melt first; the past is forgotten", 1.6)
                .look_at(2.0, 0.65, 0.0),
            RenderMode::plane("laplace", "Laplace Equilibrium", "\u{2207}\u{00B2}u = 0 relaxing live: every point the average of its neighbours", 3.4),
            RenderMode::plane("burgers", "Burgers Shock", "u\u{209C} + u u\u{2093} = \u{03BD}u\u{2093}\u{2093}: the wave overtakes itself", 1.6)
                .look_at(2.0, 0.0, 0.0),
            RenderMode::plane("string", "String Eigenmodes", "The pluck resolved into sines, each humming its own frequency", 1.9)
                .look_at(2.0, 0.0, 0.0),
            RenderMode::plane("grayscott", "Gray\u{2013}Scott Garden", "Reaction + diffusion = pattern: spots divide like cells", 3.4),
            RenderMode::plane("fhn", "Spiral Hearts", "FitzHugh\u{2013}Nagumo excitable medium: the rotors of cardiac arrhythmia", 3.4),
        ]
    }

    fn default_modes(&self) -> Vec<usize> {
        vec![4, 2]
    }
    fn params(&mut self) -> &mut Params {
        &mut self.params
    }
    fn params_ref(&self) -> &Params {
        &self.params
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "pde.field2d", FIELD2D_SHADE, true);
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("pde-field"),
            size: wgpu::Extent3d {
                width: NG as u32,
                height: NG as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rg16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.tex_view = Some(tex.create_view(&Default::default()));
        self.tex = Some(tex);
        self.reset();
    }

    fn rewind(&mut self) {
        self.reset();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, dt: f64) {
        let h = dt.abs().min(0.033);
        if h <= 0.0 {
            return;
        }
        self.sim_t += h;
        self.relax_laplace(26);
        self.step_fhn(6);
        self.step_gray_scott(9);
        // Heat (explicit, stable: alpha dt/dx^2 < 0.5).
        let alpha = self.params.f("alpha");
        let steps = 6;
        for _ in 0..steps {
            let u0 = self.heat.clone();
            let r = alpha * HEAT_R_PER_ALPHA;
            for i in 1..NX1 - 1 {
                self.heat[i] = u0[i] + r * (u0[i + 1] - 2.0 * u0[i] + u0[i - 1]);
            }
        }
        // Snapshot ghosts of the cooling.
        if self.heat_snaps.len() < 9 && self.sim_t > 0.35 * (self.heat_snaps.len() + 1) as f64 {
            self.heat_snaps.push(self.heat.clone());
        }
        self.step_burgers();
    }

    fn render(&mut self, gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        match mode {
            0 => {
                scene.line(
                    LineLayer::World,
                    Vec3::ZERO,
                    Vec3::new(4.0, 0.0, 0.0),
                    1.2,
                    [0.5, 0.5, 0.55, 0.5],
                );
                for (si, snap) in self.heat_snaps.iter().enumerate() {
                    let a = 0.10 + 0.03 * si as f32;
                    let pts: Vec<Vec3> = snap
                        .iter()
                        .enumerate()
                        .map(|(i, &u)| Vec3::new(4.0 * i as f32 / NX1 as f32, u as f32 * 1.3, 0.0))
                        .collect();
                    scene.polyline(LineLayer::World, &pts, 1.0, with_alpha(GHOST_P, a));
                }
                let pts: Vec<Vec3> = self
                    .heat
                    .iter()
                    .enumerate()
                    .map(|(i, &u)| Vec3::new(4.0 * i as f32 / NX1 as f32, u as f32 * 1.3, 0.0))
                    .collect();
                scene.glow_polyline(&pts, 2.2, APRICOT_P, 2.6);
            }
            1 => {
                self.upload(gpu, &self.lap.clone());
                scene.field = Some("pde.field2d");
                scene.field_tex = self.tex_view.clone();
                scene.p(0, 2.0);
            }
            2 => {
                scene.line(
                    LineLayer::World,
                    Vec3::new(0.0, 0.0, 0.0),
                    Vec3::new(4.0, 0.0, 0.0),
                    1.1,
                    [0.5, 0.5, 0.55, 0.5],
                );
                // Colored by initial position: watch the fast catch the slow.
                for i in 1..NX1 {
                    let x0 = 4.0 * (i - 1) as f32 / NX1 as f32;
                    let x1 = 4.0 * i as f32 / NX1 as f32;
                    let hue = phase_color(i as f32 / NX1 as f32 * std::f32::consts::TAU);
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(x0, self.burgers[i - 1] as f32 * 0.9, 0.0),
                        Vec3::new(x1, self.burgers[i] as f32 * 0.9, 0.0),
                        2.2,
                        with_alpha(hue, 0.8),
                    );
                }
            }
            3 => {
                let n_modes = self.params.i("modes") as usize;
                let l = 4.0f64;
                // Plucked-triangle coefficients: b_n = 8/(pi^2 n^2) sin(n pi / 2)-like.
                let mut sum = vec![0.0f64; 220];
                for n in 1..=n_modes {
                    let bn = 8.0 / (std::f64::consts::PI.powi(2) * (n * n) as f64)
                        * ((n as f64) * std::f64::consts::PI / 2.0).sin();
                    if bn.abs() < 1e-9 {
                        continue;
                    }
                    let om = n as f64 * std::f64::consts::PI / l * 2.2;
                    let pts: Vec<Vec3> = (0..220)
                        .map(|i| {
                            let x = l * i as f64 / 219.0;
                            let y = bn
                                * (n as f64 * std::f64::consts::PI * x / l).sin()
                                * (om * t).cos();
                            sum[i] += y;
                            Vec3::new(x as f32, (y * 0.8) as f32 - 1.1, 0.0)
                        })
                        .collect();
                    let hue = phase_color(n as f32 * 0.7);
                    scene.polyline(LineLayer::World, &pts, 1.1, with_alpha(hue, 0.35));
                }
                let spts: Vec<Vec3> = sum
                    .iter()
                    .enumerate()
                    .map(|(i, &y)| {
                        Vec3::new(l as f32 * i as f32 / 219.0, (y * 0.8) as f32 + 0.7, 0.0)
                    })
                    .collect();
                scene.glow_polyline(&spts, 2.2, TEAL_P, 2.6);
                scene.line(
                    LineLayer::World,
                    Vec3::new(0.0, 0.7, 0.0),
                    Vec3::new(4.0, 0.7, 0.0),
                    0.9,
                    [0.5, 0.5, 0.55, 0.3],
                );
            }
            5 => {
                let disp: Vec<f32> = self.fhn_u.iter().map(|&u| (u + 2.0) / 4.0).collect();
                self.upload(gpu, &disp);
                scene.field = Some("pde.field2d");
                scene.field_tex = self.tex_view.clone();
                scene.p(0, 0.0);
            }
            _ => {
                self.upload(gpu, &self.gs_v.clone());
                scene.field = Some("pde.field2d");
                scene.field_tex = self.tex_view.clone();
                scene.p(0, 1.0);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        vec![
            title("The archetypes of PDE"),
            ftex("statmech.diffusion", "heat:  \u{2202}u/\u{2202}t = \u{03B1}\u{2207}\u{00B2}u"),
            ftex("waves.eq", "wave:  \u{2202}\u{00B2}u/\u{2202}t\u{00B2} = c\u{00B2}\u{2207}\u{00B2}u"),
            formula("Laplace: \u{2207}\u{00B2}u = 0        Burgers: u\u{209C} + u u\u{2093} = \u{03BD}u\u{2093}\u{2093}"),
            formula("Gray\u{2013}Scott: u\u{209C} = D\u{1D64}\u{2207}\u{00B2}u \u{2212} uv\u{00B2} + F(1\u{2212}u),   v\u{209C} = D\u{1D65}\u{2207}\u{00B2}v + uv\u{00B2} \u{2212} (F+k)v"),
            value(format!("sim t = {:.1}   Gray\u{2013}Scott regime: {}", self.sim_t, ["spots", "stripes", "U-skate parameters"][self.params.c("gs")])),
            value(format!("Burgers peak |u| = {:.2}, restarts from the sine at {BURGERS_RESTART_PEAK:.2}", peak_magnitude(&self.burgers))),
            note("Parabolic (heat) smooths and forgets; hyperbolic (wave) remembers; viscous Burgers is parabolic too, but at small \u{03BD} steepens toward the shock its hyperbolic limit forms; elliptic (Laplace) has no time at all \u{2014} only agreement. The three temperaments of physics."),
            note("Turing's 1952 mechanism is a uniform state that diffusion destabilizes. Gray\u{2013}Scott is different: its uniform state (u, v) = (1, 0) is linearly stable, with Jacobian diag(\u{2212}F, \u{2212}(F+k)), so these patterns grow only from finite seeds."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Explicit FTCS, r \u{2264} 0.45; ghost curves are true earlier states."),
            1 => Some("In-place Gauss\u{2013}Seidel relaxation, 26 sweeps/frame \u{2014} you are watching the solver converge."),
            2 => Some("Upwind + viscosity \u{03BD}; at small \u{03BD} the front is a few cells wide, thickened by the scheme's own numerical diffusion. The sine restarts when peak |u| decays to 0.2."),
            3 => Some("Exact eigenmode superposition of the plucked string \u{2014} no discretization at all."),
            4 => Some("Gray\u{2013}Scott, periodic 128\u{00B2}, 9 Euler steps/frame."),
            5 => Some("FitzHugh\u{2013}Nagumo, periodic 128\u{00B2}, 6 Euler steps/frame."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::params::PValue;

    #[test]
    fn gray_scott_needs_finite_seeds_in_every_regime() {
        // The note says the uniform state is linearly stable. A small disturbance must
        // die out, while the app's finite seeds must not.
        for regime in 0..3 {
            let mut p = PdeHall::new();
            p.reset();
            p.params.set("gs", PValue::C(regime));
            p.step_gray_scott(400);
            let seeded = p.gs_v.iter().cloned().fold(0.0f32, f32::max);
            assert!(
                seeded > 0.1,
                "regime {regime}: seeds died, max v = {seeded}"
            );

            let mut state = 0x9E37u64;
            for i in 0..NG * NG {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                p.gs_u[i] = 1.0;
                p.gs_v[i] = 0.01 * (state >> 40) as f32 / (1u64 << 24) as f32;
            }
            p.step_gray_scott(400);
            let small = p.gs_v.iter().cloned().fold(0.0f32, f32::max);
            assert!(small < 1e-6, "regime {regime}: disturbance grew to {small}");
        }
    }

    #[test]
    fn the_regime_readout_names_the_regime() {
        let mut p = PdeHall::new();
        p.params.set("gs", PValue::C(2));
        let lines = p.inspector(0.0);
        let line = lines.iter().find(|l| l.text.contains("regime")).unwrap();
        assert!(line.text.contains("U-skate"), "{}", line.text);
    }

    #[test]
    fn fhn_rest_is_a_fixed_point_of_the_cell() {
        let (eps, beta, gamma) = (0.06f32, 0.75f32, 0.5f32);
        let (u, v) = FHN_REST;
        assert!((u - u * u * u / 3.0 - v).abs() < 1e-3);
        assert!((eps * (u + beta - gamma * v)).abs() < 1e-3);
    }

    #[test]
    fn the_broken_wavefront_stays_alive() {
        let mut p = PdeHall::new();
        p.reset();
        p.step_fhn(1500);
        let excited = p.fhn_u.iter().filter(|&&u| u > 0.0).count();
        assert!(excited > 50, "only {excited} excited cells remain");
    }

    #[test]
    fn diffusivity_keeps_mattering_up_to_the_top_of_the_slider() {
        let (lo, hi) = (1.5 * HEAT_R_PER_ALPHA, 2.0 * HEAT_R_PER_ALPHA);
        assert!(lo < hi && hi <= 0.5, "stable and strictly increasing");
    }

    /// Run Burgers at 60 frames a second, checking the peak on every frame.
    fn run_burgers(p: &mut PdeHall, seconds: usize) {
        for _ in 0..seconds * 60 {
            p.step_burgers();
            let peak = peak_magnitude(&p.burgers);
            assert!(peak >= BURGERS_RESTART_PEAK, "peak fell to {peak}");
        }
    }

    fn steepest(u: &[f64]) -> f64 {
        let dx = 1.0 / NX1 as f64;
        u.windows(2)
            .fold(0.0, |s, w| ((w[1] - w[0]) / dx).abs().max(s))
    }

    #[test]
    fn burgers_is_still_worth_watching_after_a_long_run() {
        let mut p = PdeHall::new();
        p.reset();
        let mut lowest = f64::MAX;
        let mut restarts = 0;
        let mut last = peak_magnitude(&p.burgers);
        for _ in 0..60 * 60 * 5 {
            p.step_burgers();
            let peak = peak_magnitude(&p.burgers);
            lowest = lowest.min(peak);
            restarts += usize::from(peak > last);
            last = peak;
        }
        assert!(lowest >= BURGERS_RESTART_PEAK, "peak fell to {lowest}");
        assert!(
            restarts > 10,
            "restarted only {restarts} times in 5 minutes"
        );
    }

    #[test]
    fn the_guides_viscosity_steps_show_a_ramp_and_a_step_after_a_long_run() {
        let mut p = PdeHall::new();
        p.reset();
        run_burgers(&mut p, 60);
        p.params.set_f("nu", 0.02);
        run_burgers(&mut p, 5);
        let ramp = steepest(&p.burgers);
        p.params.set_f("nu", 0.001);
        run_burgers(&mut p, 5);
        let step = steepest(&p.burgers);
        assert!(step > 5.0 * ramp, "ramp {ramp}, step {step}");
    }

    #[test]
    fn burgers_covers_the_same_time_at_any_viscosity() {
        let dx = 1.0 / NX1 as f64;
        let time = |nu: f64| {
            let (dt, n) = burgers_steps(dx, nu);
            assert!(dt <= 0.30 * dx + 1e-12 && dt <= 0.35 * dx * dx / nu + 1e-12);
            dt * n as f64
        };
        assert!((time(0.001) - time(0.05)).abs() < 1e-12);
    }
}
