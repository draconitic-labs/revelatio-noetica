//! The wave equation: disturbance, propagated. FDTD on a 200x200 lattice.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const N: usize = 200;
const WORLD: f32 = 8.0; // simulation spans [-4, 4]^2
const DX: f64 = (WORLD as f64) / N as f64;
const DT: f64 = 0.010;

const PRESETS: [&str; 4] = [
    "Gaussian pluck",
    "Point oscillator",
    "Double slit",
    "Two sources",
];

/// Nodal lines of a two-mode superposition on a fixed-edge square, drawn as
/// sand in the manner of Chladni's figures. These are membrane modes; a free
/// plate's modes are different functions.
const CHLADNI_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    if (abs(w.x) > 3.2 || abs(w.y) > 3.2) {
        return vec4<f32>(vec3<f32>(0.0), 1.0);
    }
    let p = (w + vec2<f32>(3.2)) / 6.4 * PI;
    let n = U.p[0].x;
    let m = U.p[0].y;
    let amp = sin(n * p.x) * sin(m * p.y) + sin(m * p.x) * sin(n * p.y);
    let osc = cos(U.res.z * 2.4);
    var color = vec3<f32>(0.0);
    // The plate shimmer, deep indigo.
    let sh = abs(amp * osc);
    color += vec3<f32>(0.05, 0.09, 0.20) * sh * 0.55;
    // Sand gathers on the nodal lines - but the clamped rim holds none.
    let edge = min(min(p.x, PI - p.x), min(p.y, PI - p.y)) / PI;
    let rim_fade = smoothstep(0.0, 0.05, edge);
    var sand = exp(-abs(amp) * abs(amp) * 46.0) * rim_fade;
    // Granular texture so it reads as sand, not neon.
    let grain = 0.65 + 0.35 * hash21(floor(w * 190.0));
    sand *= grain;
    color += vec3<f32>(0.93, 0.80, 0.48) * sand * 1.05;
    return vec4<f32>(color * (1.0 - 0.20 * dot(ndc, ndc)), 1.0);
}
"#;

/// u (R) and u_prev (G) sampled from the state texture.
const FIELD_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let tex_uv = (w + vec2<f32>(4.0, 4.0)) / 8.0;
    if (tex_uv.x < 0.0 || tex_uv.x > 1.0 || tex_uv.y < 0.0 || tex_uv.y > 1.0) {
        return vec4<f32>(vec3<f32>(0.0), 1.0);
    }
    let s = textureSample(t_field, s_field, vec2<f32>(tex_uv.x, 1.0 - tex_uv.y));
    let u = s.r;
    let mode = i32(U.p[0].x + 0.5);
    var color = vec3<f32>(0.0);
    if (mode == 0) {
        // Amplitude: gold above, deep-water blue below.
        let a = clamp(abs(u) * 1.8, 0.0, 1.0);
        if (u > 0.0) {
            color = mix(color, vec3<f32>(0.90, 0.62, 0.18), a);
        } else {
            color = mix(color, vec3<f32>(0.10, 0.38, 0.80), a);
        }
    } else if (mode == 1) {
        // Energy density from u_t and |grad u|.
        let e = 1.0 / 200.0;
        let sx1 = textureSample(t_field, s_field, vec2<f32>(tex_uv.x + e, 1.0 - tex_uv.y)).r;
        let sx0 = textureSample(t_field, s_field, vec2<f32>(tex_uv.x - e, 1.0 - tex_uv.y)).r;
        let sy1 = textureSample(t_field, s_field, vec2<f32>(tex_uv.x, 1.0 - tex_uv.y + e)).r;
        let sy0 = textureSample(t_field, s_field, vec2<f32>(tex_uv.x, 1.0 - tex_uv.y - e)).r;
        let ut = (s.r - s.g) / 0.01;
        let gx = (sx1 - sx0) / (2.0 * e * 8.0);
        let gy = (sy1 - sy0) / (2.0 * e * 8.0);
        let c = U.p[0].z;
        let dens = 0.5 * (ut * ut + c * c * (gx * gx + gy * gy));
        color = inferno(clamp(log(1.0 + dens * 1.5) * 0.30, 0.0, 1.0));
    } else {
        // Wavefront bands: |u| contour ridges.
        let a = abs(u);
        let band = smoothstep(0.05, 0.09, a) * (1.0 - smoothstep(0.10, 0.22, a));
        let crest = smoothstep(0.20, 0.55, a);
        color += vec3<f32>(0.15, 0.55, 0.65) * band * 1.2;
        color += vec3<f32>(0.95, 0.80, 0.40) * crest;
    }
    // Barrier overlay.
    if (U.p[0].y > 0.5) {
        let bx = abs(w.x - 0.0);
        if (bx < 0.10) {
            let gap1 = abs(w.y - 0.55);
            let gap2 = abs(w.y + 0.55);
            if (gap1 > 0.22 && gap2 > 0.22) {
                color = mix(color, vec3<f32>(0.45, 0.42, 0.40), 0.85);
            }
        }
    }
    return vec4<f32>(color * (1.0 - 0.22 * dot(ndc, ndc)), 1.0);
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

pub struct WaveTable {
    params: Params,
    u: Vec<f64>,
    up: Vec<f64>,
    barrier: Vec<bool>,
    tex: Option<wgpu::Texture>,
    tex_view: Option<wgpu::TextureView>,
    relief: Option<GpuMesh>,
    sim_t: f64,
    last_scenario: usize,
    /// Timeline seconds not yet turned into lattice steps.
    step_debt: f64,
    /// The field changed outside a step and has yet to reach the GPU.
    dirty: bool,
}

/// Speed of the Doppler wavefronts, how often the source emits one, and the
/// radius at which they have faded out.
const DOPPLER_C: f64 = 1.1;
const DOPPLER_EVERY: f64 = 0.14;
const DOPPLER_MAX_R: f64 = 7.0;

fn doppler_src(t: f64, mach: f64) -> glam::Vec2 {
    // The source sweeps left-to-right and wraps.
    let x = -3.4 + ((t * mach * DOPPLER_C) % 6.8);
    glam::Vec2::new(x as f32, 0.0)
}

/// Every wavefront alive at time `t`, as (center, radius). A pure function of
/// time, so any view can draw it at any moment and the scrubber just works.
fn doppler_rings(t: f64, mach: f64) -> Vec<(glam::Vec2, f32)> {
    let newest = (t / DOPPLER_EVERY).floor() as i64;
    let oldest = (((t - DOPPLER_MAX_R / DOPPLER_C) / DOPPLER_EVERY).ceil() as i64).max(0);
    (oldest..=newest)
        .map(|k| {
            let birth = k as f64 * DOPPLER_EVERY;
            (doppler_src(birth, mach), ((t - birth) * DOPPLER_C) as f32)
        })
        .filter(|&(_, r)| r > 0.0)
        .collect()
}

/// The source's recent track, newest first, split where it wrapped around.
fn doppler_trail(t: f64, mach: f64) -> Vec<Vec<Vec3>> {
    let mut runs = vec![Vec::new()];
    let mut last: Option<glam::Vec2> = None;
    for i in 0..40 {
        let ti = t - i as f64 * 0.03;
        if ti < 0.0 {
            break;
        }
        let p = doppler_src(ti, mach);
        if last.is_some_and(|q| (q.x - p.x).abs() > 3.0) {
            runs.push(Vec::new());
        }
        if let Some(run) = runs.last_mut() {
            run.push(Vec3::new(p.x, p.y, 0.01));
        }
        last = Some(p);
    }
    runs
}

/// Half-angle of the Mach cone for a supersonic source.
fn cone_half_angle(mach: f64) -> f64 {
    (1.0 / mach).asin()
}

impl WaveTable {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("preset", "Scenario", "Symbolic", PRESETS.to_vec(), 2);
        p.slider("c", "Wave speed c", "Physics", 0.5, 2.4, 1.5, "");
        p.slider("freq", "Source frequency", "Physics", 0.5, 4.0, 1.8, "Hz");
        p.slider("damp", "Damping", "Physics", 0.0, 0.02, 0.001, "");
        p.int("chl_n", "Chladni mode n", "Physics", 1, 9, 3);
        p.int("chl_m", "Chladni mode m", "Physics", 1, 9, 5);
        p.slider("mach", "Source speed (Mach)", "Physics", 0.2, 1.8, 0.75, "");
        Self {
            params: p,
            u: vec![0.0; N * N],
            up: vec![0.0; N * N],
            barrier: vec![false; N * N],
            tex: None,
            tex_view: None,
            relief: None,
            sim_t: 0.0,
            last_scenario: usize::MAX,
            step_debt: 0.0,
            dirty: true,
        }
    }

    fn reset(&mut self) {
        self.u.iter_mut().for_each(|v| *v = 0.0);
        self.up.iter_mut().for_each(|v| *v = 0.0);
        self.barrier.iter_mut().for_each(|v| *v = false);
        self.sim_t = 0.0;
        self.step_debt = 0.0;
        self.dirty = true;
        let preset = self.params.c("preset");
        if preset == 0 {
            // Gaussian pluck, off-center.
            for j in 0..N {
                for i in 0..N {
                    let x = (i as f64 / N as f64 - 0.30) * 8.0;
                    let y = (j as f64 / N as f64 - 0.5) * 8.0;
                    let g = (-(x * x + y * y) / 0.14).exp();
                    self.u[j * N + i] = g;
                    self.up[j * N + i] = g;
                }
            }
        }
        if preset == 2 {
            // Barrier at x = 0 with two slits at y = +-0.55, half-width 0.22.
            let bi = N / 2;
            for j in 0..N {
                let y = (j as f64 / N as f64 - 0.5) * 8.0;
                if (y - 0.55).abs() > 0.22 && (y + 0.55).abs() > 0.22 {
                    self.barrier[j * N + bi] = true;
                    self.barrier[j * N + bi + 1] = true;
                }
            }
        }
    }

    fn step(&mut self) {
        let c = self.params.f("c");
        let damp = self.params.f("damp");
        let r2 = (c * DT / DX).powi(2);
        let preset = self.params.c("preset");
        let om = self.params.f("freq") * std::f64::consts::TAU;
        let mut un = vec![0.0f64; N * N];
        for j in 1..N - 1 {
            for i in 1..N - 1 {
                let idx = j * N + i;
                if self.barrier[idx] {
                    continue;
                }
                let lap = self.u[idx - 1] + self.u[idx + 1] + self.u[idx - N] + self.u[idx + N]
                    - 4.0 * self.u[idx];
                // Damping acts on the velocity alone. Scaling the whole update
                // would add a restoring term and block low frequencies.
                un[idx] = self.u[idx] + (1.0 - damp) * (self.u[idx] - self.up[idx]) + r2 * lap;
            }
        }
        // Sponge margins to approximate open boundaries.
        let sponge = 14;
        for j in 0..N {
            for i in 0..N {
                let d = i.min(N - 1 - i).min(j).min(N - 1 - j);
                if d < sponge {
                    let f = 0.985 + 0.015 * (d as f64 / sponge as f64);
                    un[j * N + i] *= f;
                }
            }
        }
        // Sources.
        let drive = (om * self.sim_t).sin() * (1.0 - (-self.sim_t * 2.0).exp());
        match preset {
            1 => {
                let (si, sj) = (N / 4, N / 2);
                un[sj * N + si] = drive * 1.2;
            }
            2 => {
                // Plane-wave line source left of the barrier.
                let si = N / 8;
                for j in sponge..N - sponge {
                    un[j * N + si] = drive * 0.9;
                }
            }
            3 => {
                let (si, sj1, sj2) = (N / 3, N / 2 - N / 8, N / 2 + N / 8);
                un[sj1 * N + si] = drive * 1.2;
                un[sj2 * N + si] = drive * 1.2;
            }
            _ => {}
        }
        self.up.copy_from_slice(&self.u);
        self.u = un;
        self.sim_t += DT;
    }

    /// A new scenario needs a new membrane. Wave speed does not: the field
    /// carries on under the new c, so the slider can be swept.
    fn sync_scenario(&mut self) {
        let scenario = self.params.c("preset");
        if scenario != self.last_scenario {
            self.last_scenario = scenario;
            self.reset();
        }
    }

    /// How many lattice steps this much timeline is worth. The remainder
    /// carries over, so slow playback still advances and the lattice keeps
    /// pace at any frame rate. Reversed time still marches forward.
    fn steps_due(&mut self, dt: f64) -> usize {
        self.step_debt += dt.abs();
        let steps = (self.step_debt / DT).floor();
        self.step_debt -= steps * DT;
        if steps > 24.0 {
            self.step_debt = 0.0;
            24
        } else {
            steps as usize
        }
    }

    fn upload(&mut self, gpu: &GpuContext) {
        let mut data = Vec::with_capacity(N * N * 2);
        for j in 0..N {
            for i in 0..N {
                data.push(f16_bits(self.u[j * N + i] as f32));
                data.push(f16_bits(self.up[j * N + i] as f32));
            }
        }
        if let Some(tex) = &self.tex {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&data),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some((N * 4) as u32),
                    rows_per_image: Some(N as u32),
                },
                wgpu::Extent3d {
                    width: N as u32,
                    height: N as u32,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
}

impl ConceptModule for WaveTable {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "waves.membrane",
            name: "The Singing Membrane",
            tagline: "\u{2202}\u{00B2}u/\u{2202}t\u{00B2} = c\u{00B2}\u{2207}\u{00B2}u, live",
            branch: Branch::Waves,
            description: "One equation carries sound, light, and the surface of a struck drum. Here it runs live on a 200\u{00D7}200 lattice: pluck it, drive it, or send a plane wave through two slits and watch interference build the fringes that convinced physics light was a wave.",
            epigraph: None,
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "amplitude",
                "Amplitude Field",
                "u(x,y,t): gold crests, blue troughs",
                4.2,
            ),
            RenderMode::orbit(
                "relief",
                "Standing Relief",
                "The membrane in the round; height = u",
                11.0,
                0.55,
                0.9,
            ),
            RenderMode::plane(
                "energy",
                "Energy Density",
                "\u{00BD}(u\u{209C}\u{00B2} + c\u{00B2}|\u{2207}u|\u{00B2})",
                4.2,
            ),
            RenderMode::plane(
                "fronts",
                "Wavefronts",
                "Ridge bands of |u|: the geometry of Huygens",
                4.2,
            ),
            RenderMode::plane(
                "chladni",
                "Chladni Figure",
                "Nodal lines of a fixed-edge square, drawn as sand after Chladni's figures of 1787",
                3.6,
            ),
            RenderMode::plane(
                "doppler",
                "Doppler Wake",
                "Wavefronts crowd ahead of the source; past Mach 1, they pile into a cone",
                3.6,
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
        rend.register_field(gpu, "wave.field", FIELD_SHADE, true);
        rend.register_field(gpu, "wave.chladni", CHLADNI_SHADE, false);
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("wave-state"),
            size: wgpu::Extent3d {
                width: N as u32,
                height: N as u32,
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
        // Relief mesh: 100x100 decimated height field.
        let d = MeshData::parametric(100, 100, |u, v| {
            (Vec3::new((u - 0.5) * 8.0, 0.0, (v - 0.5) * 8.0), [1.0; 4])
        });
        let mut m = d.upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, glam::Mat4::IDENTITY, [1.0; 4], [0.0, 0.40, 1.0, 0.0]);
        self.relief = Some(m);
        self.reset();
    }

    fn rewind(&mut self) {
        self.reset();
    }

    fn update(&mut self, gpu: &GpuContext, _t: f64, dt: f64) {
        self.sync_scenario();
        // March the lattice to keep pace with the timeline (cap per frame).
        let steps = self.steps_due(dt);
        for _ in 0..steps {
            self.step();
        }
        if steps > 0 || self.dirty {
            self.dirty = false;
            self.upload(gpu);
            // Relief vertices from the current field.
            if let Some(mesh) = &self.relief {
                let d = MeshData::parametric(100, 100, |uu, vv| {
                    let i = ((uu * (N - 1) as f32) as usize).min(N - 1);
                    let j = ((vv * (N - 1) as f32) as usize).min(N - 1);
                    let u_val = self.u[j * N + i] as f32;
                    let h = u_val * 1.6;
                    let a = (u_val.abs() * 1.8).clamp(0.0, 1.0);
                    let color = if u_val > 0.0 {
                        [0.08 + 0.82 * a, 0.06 + 0.56 * a, 0.03 + 0.15 * a, 1.0]
                    } else {
                        [0.03 + 0.07 * a, 0.05 + 0.33 * a, 0.10 + 0.70 * a, 1.0]
                    };
                    (Vec3::new((uu - 0.5) * 8.0, h, (vv - 0.5) * 8.0), color)
                });
                mesh.update_vertices(gpu, &d.vertices);
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
        let preset = self.params.c("preset");
        match mode {
            4 => {
                scene.field = Some("wave.chladni");
                scene.p(0, self.params.i("chl_n") as f32);
                scene.p(1, self.params.i("chl_m") as f32);
            }
            5 => {
                let mach = self.params.f("mach");
                let src = doppler_src(_t, mach);
                for (center, r) in doppler_rings(_t, mach) {
                    let fade = (1.0 - r / DOPPLER_MAX_R as f32).max(0.0).powf(1.5) * 0.85;
                    let ring: Vec<Vec3> = (0..=90)
                        .map(|i| {
                            let a = i as f32 / 90.0 * std::f32::consts::TAU;
                            Vec3::new(center.x + a.cos() * r, center.y + a.sin() * r, 0.0)
                        })
                        .collect();
                    let col = if mach > 1.0 {
                        [0.95f32, 0.55, 0.25, fade]
                    } else {
                        [0.40, 0.78, 0.98, fade]
                    };
                    scene.polyline(LineLayer::WorldAdd, &ring, 1.9, col);
                }
                // The source, its wake, and (supersonic) the Mach cone.
                for run in doppler_trail(_t, mach) {
                    scene.glow_polyline(&run, 2.0, [0.95, 0.85, 0.55, 0.6], 2.4);
                }
                if mach > 1.0 {
                    let half_angle = cone_half_angle(mach) as f32;
                    for s in [-1.0f32, 1.0] {
                        let dirc = glam::Vec2::new(-(half_angle.cos()), s * half_angle.sin());
                        let far = src + dirc * 8.0;
                        scene.line_soft(
                            LineLayer::WorldAdd,
                            Vec3::new(src.x, src.y, 0.02),
                            Vec3::new(far.x, far.y, 0.02),
                            3.2,
                            [1.0, 0.62, 0.25, 0.5],
                        );
                    }
                }
                scene.point(
                    Vec3::new(src.x, src.y, 0.03),
                    6.0,
                    [0.98, 0.92, 0.78, 1.0],
                    2.6,
                );
            }
            1 => {
                if let Some(m) = &self.relief {
                    scene.mesh(m);
                }
                // Rim of the membrane.
                let rim: Vec<Vec3> = (0..=4)
                    .map(|i| {
                        let c = [
                            (-4.0, -4.0),
                            (4.0, -4.0),
                            (4.0, 4.0),
                            (-4.0, 4.0),
                            (-4.0, -4.0),
                        ][i];
                        Vec3::new(c.0, 0.0, c.1)
                    })
                    .collect();
                scene.polyline(
                    LineLayer::World,
                    &rim,
                    1.2,
                    with_alpha([0.5, 0.55, 0.7, 1.0], 0.35),
                );
                if preset == 2 {
                    // The double-slit barrier, standing.
                    for (y0, y1) in [(-4.0f32, -0.77f32), (-0.33, 0.33), (0.77, 4.0)] {
                        scene.line(
                            LineLayer::World,
                            Vec3::new(0.0, 0.35, y0),
                            Vec3::new(0.0, 0.35, y1),
                            4.0,
                            with_alpha([0.55, 0.5, 0.45, 1.0], 0.8),
                        );
                    }
                }
            }
            m => {
                scene.field = Some("wave.field");
                scene.field_tex = self.tex_view.clone();
                scene.p(
                    0,
                    match m {
                        0 => 0.0,
                        2 => 1.0,
                        _ => 2.0,
                    },
                );
                scene.p(1, if preset == 2 { 1.0 } else { 0.0 });
                scene.p(2, self.params.f("c") as f32);
            }
        }
    }

    fn wants_pointer(&self, mode: usize) -> bool {
        mode != 1 && mode < 4
    }

    fn pointer_hint(&self, mode: usize) -> Option<&'static str> {
        (mode != 1 && mode < 4).then_some("click or drag to strike the membrane")
    }

    fn pointer(&mut self, _mode: usize, ev: crate::engine::module::PointerEvent) {
        use crate::engine::module::PointerEvent as PE;
        let w = match ev {
            PE::Down(w) | PE::Drag(w) => w,
            PE::Up => return,
        };
        if w.x.abs() > 3.9 || w.y.abs() > 3.9 {
            return;
        }
        // Strike: add a Gaussian displacement at the point.
        let ci = ((w.x / 8.0 + 0.5) as f64 * N as f64) as i64;
        let cj = ((0.5 - w.y / 8.0) as f64 * N as f64) as i64;
        let rad = 5i64;
        for dj in -rad..=rad {
            for di in -rad..=rad {
                let (i, j) = (ci + di, cj + dj);
                if i < 1 || j < 1 || i >= N as i64 - 1 || j >= N as i64 - 1 {
                    continue;
                }
                let idx = j as usize * N + i as usize;
                if self.barrier[idx] {
                    continue;
                }
                let r2 = (di * di + dj * dj) as f64 / (rad * rad) as f64;
                let bump = 1.1 * (-r2 * 3.5).exp();
                self.u[idx] += bump;
                self.up[idx] += bump;
            }
        }
        self.dirty = true;
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let c = self.params.f("c");
        let cfl = c * DT / DX;
        vec![
            title("The wave equation"),
            ftex("waves.eq", "\u{2202}\u{00B2}u/\u{2202}t\u{00B2} = c\u{00B2}(\u{2202}\u{00B2}u/\u{2202}x\u{00B2} + \u{2202}\u{00B2}u/\u{2202}y\u{00B2})"),
            def("Leapfrog with damping d: u\u{207F}\u{207A}\u{00B9} = u\u{207F} + (1\u{2212}d)(u\u{207F} \u{2212} u\u{207F}\u{207B}\u{00B9}) + (c\u{0394}t/\u{0394}x)\u{00B2}\u{2207}\u{00B2}u\u{207F}"),
            value(format!("lattice 200\u{00B2}   \u{0394}x = {DX:.3}   \u{0394}t = {DT}   sim t = {:.2}", self.sim_t)),
            value(format!("Courant number c\u{0394}t/\u{0394}x = {cfl:.3}  (stable < 0.707)")),
            note("Two slits, one wave: in the far field at small angles the fringes are spaced by \u{03BB}D/d. That is interference, not addition of particles."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0..=3 => Some("2nd-order leapfrog FDTD, reflecting walls with 14-cell sponge margins; not an exact solution."),
            4 => Some("Closed-form nodal pattern of a fixed-edge square membrane, not the modes of a free plate; no sand is simulated."),
            5 => Some("Closed-form circular fronts from a source in uniform motion; not a lattice solution."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::module::PointerEvent;
    use crate::engine::params::PValue;

    fn table(scenario: usize, c: f64, freq: f64, damp: f64) -> WaveTable {
        let mut w = WaveTable::new();
        w.params.set("preset", PValue::C(scenario));
        w.params.set("c", PValue::F(c));
        w.params.set("freq", PValue::F(freq));
        w.params.set("damp", PValue::F(damp));
        w.sync_scenario();
        w
    }

    /// The discrete energy leapfrog conserves between steps n and n + 1:
    /// sum of (1/2)((u1 - u0)/dt)^2 + (1/2)c^2 grad(u1).grad(u0).
    fn lattice_energy(w: &WaveTable, c: f64) -> f64 {
        let mut e = 0.0;
        for j in 0..N - 1 {
            for i in 0..N - 1 {
                let idx = j * N + i;
                let ut = (w.u[idx] - w.up[idx]) / DT;
                let (gx1, gy1) = ((w.u[idx + 1] - w.u[idx]) / DX, (w.u[idx + N] - w.u[idx]) / DX);
                let (gx0, gy0) = (
                    (w.up[idx + 1] - w.up[idx]) / DX,
                    (w.up[idx + N] - w.up[idx]) / DX,
                );
                e += 0.5 * ut * ut + 0.5 * c * c * (gx1 * gx0 + gy1 * gy0);
            }
        }
        e * DX * DX
    }

    #[test]
    fn undamped_leapfrog_conserves_its_energy() {
        let mut w = table(0, 1.5, 1.8, 0.0);
        let start = lattice_energy(&w, 1.5);
        assert!(start > 1.0);
        // Before the pluck reaches the sponge, 46 cells away at x = -3.44.
        for _ in 0..80 {
            w.step();
        }
        let end = lattice_energy(&w, 1.5);
        assert!((end - start).abs() < 1e-6 * start, "{start} -> {end}");
    }

    #[test]
    fn damping_only_removes_energy() {
        let mut w = table(0, 1.5, 1.8, 0.01);
        let mut prev = lattice_energy(&w, 1.5);
        for _ in 0..80 {
            w.step();
            let e = lattice_energy(&w, 1.5);
            assert!(e <= prev * (1.0 + 1e-12), "{prev} -> {e}");
            prev = e;
        }
    }

    #[test]
    fn damping_does_not_pull_a_displaced_membrane_back() {
        let mut w = table(0, 1.5, 1.8, 0.02);
        w.u.fill(1.0);
        w.up.fill(1.0);
        // The walls are 86 cells from the center and news travels one per step.
        for _ in 0..50 {
            w.step();
        }
        let centre = w.u[N / 2 * N + N / 2];
        assert!((centre - 1.0).abs() < 1e-9, "centre moved to {centre}");
    }

    #[test]
    fn pluck_ring_travels_at_the_wave_speed() {
        for c in [1.0, 2.0] {
            let mut w = table(0, c, 1.8, 0.0);
            for _ in 0..100 {
                w.step();
            }
            // Along the +x ray from the pluck at cell (60, 100).
            let (mut best, mut at) = (0.0, 0);
            for i in 60..N {
                let v = w.u[100 * N + i].abs();
                if v > best {
                    (best, at) = (v, i);
                }
            }
            let r = (at - 60) as f64 * DX;
            assert!((r - c).abs() < 0.2, "c = {c}: ring at r = {r}");
        }
    }

    /// Time-averaged u^2 along the line x = 3, as (y, value) at its peaks.
    fn fringes(c: f64, freq: f64) -> Vec<f64> {
        let mut w = table(2, c, freq, 0.001);
        let ix = ((3.0 / 8.0 + 0.5) * N as f64) as usize;
        let mut power = vec![0.0; N];
        for s in 0..1100 {
            w.step();
            if s >= 700 {
                for (j, p) in power.iter_mut().enumerate() {
                    *p += w.u[j * N + ix].powi(2);
                }
            }
        }
        let top = power.iter().cloned().fold(0.0, f64::max);
        (20..N - 20)
            .filter(|&j| {
                power[j] > 0.15 * top && power[j] > power[j - 1] && power[j] >= power[j + 1]
            })
            .map(|j| (j as f64 / N as f64 - 0.5) * 8.0)
            .collect()
    }

    #[test]
    fn double_slit_shows_a_central_fringe_and_shorter_waves_crowd_them() {
        let near = |ys: &[f64], y: f64, tol: f64| ys.iter().any(|v| (v - y).abs() < tol);
        let short = fringes(1.0, 3.0);
        assert!(near(&short, 0.0, 0.1), "{short:?}");
        assert!(near(&short, 0.92, 0.12), "{short:?}");
        assert!(near(&short, -0.92, 0.12), "{short:?}");
        // The first side fringe moves inward as the frequency rises.
        let side = |ys: &[f64]| {
            ys.iter()
                .map(|y| y.abs())
                .filter(|y| *y > 0.3)
                .fold(f64::MAX, f64::min)
        };
        assert!(side(&fringes(1.5, 3.5)) < side(&fringes(1.5, 1.8)));
    }

    #[test]
    fn wave_speed_keeps_the_field_but_a_new_scenario_starts_over() {
        let mut w = table(0, 1.5, 1.8, 0.001);
        for _ in 0..10 {
            w.step();
        }
        let before = w.u.clone();
        w.params.set("c", PValue::F(2.0));
        w.sync_scenario();
        assert_eq!(w.u, before);
        w.params.set("preset", PValue::C(1));
        w.sync_scenario();
        assert!(w.u.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn slow_playback_still_steps_and_fast_playback_is_capped() {
        let mut w = table(0, 1.5, 1.8, 0.001);
        let total: usize = (0..250).map(|_| w.steps_due(0.004)).sum();
        assert!((99..=101).contains(&total), "{total}");
        // Reversed time still marches forward.
        let back: usize = (0..250).map(|_| w.steps_due(-0.004)).sum();
        assert!((99..=101).contains(&back), "{back}");
        assert_eq!(w.steps_due(10.0), 24);
        assert_eq!(w.steps_due(0.0), 0, "a capped frame leaves no backlog");
    }

    #[test]
    fn a_strike_marks_the_field_for_upload() {
        let mut w = table(0, 1.5, 1.8, 0.001);
        w.dirty = false;
        let before = w.u[N / 2 * N + N / 2];
        w.pointer(0, PointerEvent::Down(glam::Vec2::ZERO));
        assert!(w.dirty);
        assert!(w.u[N / 2 * N + N / 2] > before + 0.5);
    }

    #[test]
    fn mach_cone_is_tangent_to_every_ring_of_the_pass() {
        let mach = 1.5;
        let t = 3.0; // Still on the first pass: x = -3.4 + 1.65 t.
        let src = doppler_src(t, mach);
        let a = cone_half_angle(mach) as f32;
        let dir = glam::Vec2::new(-a.cos(), a.sin());
        let rings = doppler_rings(t, mach);
        assert!(rings.len() > 10);
        for (center, r) in rings {
            let d = center - src;
            let dist = (d.x * dir.y - d.y * dir.x).abs();
            assert!((dist - r).abs() < 2e-3, "ring r = {r} is {dist} from the cone");
        }
    }

    #[test]
    fn rings_are_gone_at_time_zero_and_fade_out_by_their_limit() {
        assert!(doppler_rings(0.0, 0.75).is_empty());
        let rings = doppler_rings(11.0, 0.75);
        assert!(rings.iter().all(|&(_, r)| r > 0.0 && r <= DOPPLER_MAX_R as f32));
        // One ring per emission over the time a ring takes to reach the limit.
        let expected = (DOPPLER_MAX_R / DOPPLER_C / DOPPLER_EVERY) as i64;
        assert!((rings.len() as i64 - expected).abs() <= 1, "{}", rings.len());
    }

    #[test]
    fn the_source_trail_breaks_where_the_source_wraps() {
        let mach = 1.0;
        // The source wraps when t * mach * c reaches 6.8.
        let wrap = 6.8 / (mach * DOPPLER_C);
        let runs = doppler_trail(wrap + 0.3, mach);
        assert_eq!(runs.len(), 2);
        for run in runs {
            for pair in run.windows(2) {
                assert!((pair[0].x - pair[1].x).abs() < 0.1);
            }
        }
        assert_eq!(doppler_trail(wrap - 2.0, mach).len(), 1);
    }
}
