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

/// Chladni figure of a square plate: nodal lines of a two-mode superposition.
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
        let dens = 0.5 * (ut * ut * 0.02 + (gx * gx + gy * gy) * 2.0);
        color = inferno(clamp(log(1.0 + dens * 6.0) * 0.30, 0.0, 1.0));
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
    last_reset_key: (usize, i64),
    last_timeline_t: f64,
    /// Doppler wavefronts: (emission point, emission time).
    rings: Vec<(glam::Vec2, f64)>,
    last_emit: f64,
}

fn doppler_src(t: f64, mach: f64) -> glam::Vec2 {
    // The source sweeps left-to-right and wraps.
    let c_wave = 1.1;
    let x = -3.4 + ((t * mach * c_wave) % 6.8);
    glam::Vec2::new(x as f32, 0.0)
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
            last_reset_key: (usize::MAX, 0),
            last_timeline_t: 0.0,
            rings: Vec::new(),
            last_emit: -1.0,
        }
    }

    fn reset(&mut self) {
        self.u.iter_mut().for_each(|v| *v = 0.0);
        self.up.iter_mut().for_each(|v| *v = 0.0);
        self.barrier.iter_mut().for_each(|v| *v = false);
        self.sim_t = 0.0;
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
                un[idx] = (2.0 * self.u[idx] - self.up[idx] + r2 * lap) * (1.0 - damp);
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
                "u(x,y,t): gold crests, indigo troughs",
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
                "Chladni Plate",
                "Sand settles where the plate never moves: the nodal figures of 1787",
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

    fn update(&mut self, gpu: &GpuContext, t: f64, dt: f64) {
        let key = (self.params.c("preset"), (self.params.f("c") * 100.0) as i64);
        if key != self.last_reset_key || t < self.last_timeline_t - 0.5 {
            self.last_reset_key = key;
            self.reset();
        }
        self.last_timeline_t = t;
        // Doppler emissions.
        let mach = self.params.f("mach");
        if t - self.last_emit > 0.14 || t < self.last_emit {
            self.last_emit = t;
            self.rings.push((doppler_src(t, mach), t));
            if self.rings.len() > 60 {
                self.rings.remove(0);
            }
        }
        // March the lattice to keep pace with the timeline (cap per frame).
        let steps = ((dt / DT).round() as usize).min(24);
        for _ in 0..steps {
            self.step();
        }
        if steps > 0 || self.sim_t == 0.0 {
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
                let c_wave = 1.1;
                let src = doppler_src(_t, mach);
                for &(center, birth) in &self.rings {
                    let r = ((_t - birth) * c_wave) as f32;
                    if r <= 0.0 || r > 7.0 {
                        continue;
                    }
                    let fade = (1.0 - r / 7.0).powf(1.5) * 0.85;
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
                let trail: Vec<Vec3> = (0..40)
                    .map(|i| {
                        let p = doppler_src(_t - i as f64 * 0.03, mach);
                        Vec3::new(p.x, p.y, 0.01)
                    })
                    .collect();
                scene.glow_polyline(&trail, 2.0, [0.95, 0.85, 0.55, 0.6], 2.4);
                if mach > 1.0 {
                    let half_angle = (1.0 / mach).asin() as f32;
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
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let c = self.params.f("c");
        let cfl = c * DT / DX;
        vec![
            title("The wave equation"),
            ftex("waves.eq", "\u{2202}\u{00B2}u/\u{2202}t\u{00B2} = c\u{00B2}(\u{2202}\u{00B2}u/\u{2202}x\u{00B2} + \u{2202}\u{00B2}u/\u{2202}y\u{00B2})"),
            def("Leapfrog: u\u{207F}\u{207A}\u{00B9} = 2u\u{207F} \u{2212} u\u{207F}\u{207B}\u{00B9} + (c\u{0394}t/\u{0394}x)\u{00B2}\u{2207}\u{00B2}u\u{207F}"),
            value(format!("lattice 200\u{00B2}   \u{0394}x = {DX:.3}   \u{0394}t = {DT}   sim t = {:.2}", self.sim_t)),
            value(format!("Courant number c\u{0394}t/\u{0394}x = {cfl:.3}  (stable < 0.707)")),
            note("Two slits, one wave: the fringes on the far side are spaced by \u{03BB}D/d \u{2014} interference, not addition of particles."),
        ]
    }

    fn approx(&self, _mode: usize) -> Option<&'static str> {
        Some("2nd-order leapfrog FDTD, reflecting walls with 14-cell sponge margins; not an exact solution.")
    }
}
