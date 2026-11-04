//! Statistical mechanics, live: the Ising lattice at its phase transition,
//! a gas discovering Maxwell-Boltzmann, and diffusion emerging from coin flips.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const NI: usize = 128; // Ising lattice
const N_GAS: usize = 900;
const N_WALK: usize = 7000;

const HEAT: [f32; 4] = [1.0, 0.70, 0.28, 1.0];
const COOL: [f32; 4] = [0.55, 0.65, 1.0, 1.0];

/// Three-level display: 0 void, ~0.35 dim bronze, 1 bright teal.
const LEVELS_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let tex_uv = (w + vec2<f32>(3.2, 3.2)) / 6.4;
    if (tex_uv.x < 0.0 || tex_uv.x > 1.0 || tex_uv.y < 0.0 || tex_uv.y > 1.0) {
        return vec4<f32>(vec3<f32>(0.0), 1.0);
    }
    let s = textureSample(t_field, s_field, vec2<f32>(tex_uv.x, 1.0 - tex_uv.y)).r;
    var color = vec3<f32>(0.0);
    if (s > 0.7) {
        color = vec3<f32>(0.22, 0.85, 0.72);
    } else if (s > 0.15) {
        color = vec3<f32>(0.17, 0.12, 0.06);
    }
    return vec4<f32>(color * (1.0 - 0.18 * dot(ndc, ndc)), 1.0);
}
"#;

const ISING_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let tex_uv = (w + vec2<f32>(3.2, 3.2)) / 6.4;
    if (tex_uv.x < 0.0 || tex_uv.x > 1.0 || tex_uv.y < 0.0 || tex_uv.y > 1.0) {
        return vec4<f32>(vec3<f32>(0.0), 1.0);
    }
    let s = textureSample(t_field, s_field, vec2<f32>(tex_uv.x, 1.0 - tex_uv.y)).r;
    var color: vec3<f32>;
    if (s > 0.0) {
        color = vec3<f32>(0.88, 0.62, 0.20) * (0.75 + 0.25 * s);
    } else {
        color = vec3<f32>(0.10, 0.20, 0.48) * (0.75 - 0.25 * s);
    }
    return vec4<f32>(color * (1.0 - 0.18 * dot(ndc, ndc)), 1.0);
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

pub struct EnsembleGallery {
    params: Params,
    spins: Vec<i8>,
    tex: Option<wgpu::Texture>,
    tex_view: Option<wgpu::TextureView>,
    gas_p: Vec<Vec2>,
    gas_v: Vec<Vec2>,
    walkers: Vec<Vec2>,
    /// Sum of each walker's squared steps so far, the same for every walker.
    walk_square: f64,
    rng: u64,
    perc: Vec<f32>,
    last_pfill: i64,
    life: Vec<u8>,
    life_clock: f64,
}

impl EnsembleGallery {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider(
            "temp",
            "Ising temperature T",
            "Physics",
            1.0,
            4.5,
            2.27,
            "J/k_B",
        );
        p.slider("sweeps", "Sweeps per frame", "Physics", 1.0, 12.0, 5.0, "");
        p.slider("dstep", "Walker step", "Physics", 0.004, 0.030, 0.012, "");
        p.slider("pfill", "Percolation p", "Physics", 0.35, 0.75, 0.5927, "");
        Self {
            params: p,
            spins: Vec::new(),
            tex: None,
            tex_view: None,
            gas_p: Vec::new(),
            gas_v: Vec::new(),
            walkers: Vec::new(),
            walk_square: 0.0,
            rng: 0x9E3779B97F4A7C15,
            perc: Vec::new(),
            last_pfill: -1,
            life: Vec::new(),
            life_clock: 0.0,
        }
    }

    /// Occupy sites with probability p; mark the top-spanning cluster bright.
    fn build_percolation(&mut self) {
        let p = self.params.f("pfill");
        let mut occ = vec![false; NI * NI];
        for c in occ.iter_mut() {
            *c = self.rand() < p;
        }
        // BFS from every occupied top-row site.
        let mut mark = vec![false; NI * NI];
        let mut queue: Vec<usize> = (0..NI).filter(|&i| occ[i]).collect();
        for &q in &queue {
            mark[q] = true;
        }
        while let Some(idx) = queue.pop() {
            let (i, j) = (idx % NI, idx / NI);
            for (di, dj) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                let (ni, nj) = (i as i32 + di, j as i32 + dj);
                if ni < 0 || nj < 0 || ni >= NI as i32 || nj >= NI as i32 {
                    continue;
                }
                let n = nj as usize * NI + ni as usize;
                if occ[n] && !mark[n] {
                    mark[n] = true;
                    queue.push(n);
                }
            }
        }
        self.perc = (0..NI * NI)
            .map(|i| {
                if mark[i] {
                    1.0
                } else if occ[i] {
                    0.35
                } else {
                    0.0
                }
            })
            .collect();
    }

    fn seed_life(&mut self) {
        self.life = (0..NI * NI).map(|_| (self.rand() < 0.14) as u8).collect();
    }

    fn step_life(&mut self) {
        let old = self.life.clone();
        for j in 0..NI {
            for i in 0..NI {
                let mut n = 0;
                for dj in [NI - 1, 0, 1] {
                    for di in [NI - 1, 0, 1] {
                        if di == 0 && dj == 0 {
                            continue;
                        }
                        n += old[((j + dj) % NI) * NI + (i + di) % NI];
                    }
                }
                let idx = j * NI + i;
                self.life[idx] = match (old[idx], n) {
                    (1, 2) | (1, 3) | (0, 3) => 1,
                    _ => 0,
                };
            }
        }
    }

    fn rand(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn reset_all(&mut self) {
        self.spins = (0..NI * NI)
            .map(|_| if self.rand() < 0.5 { 1i8 } else { -1 })
            .collect();
        // Gas: all particles share one speed; direction random. M-B emerges.
        self.gas_p.clear();
        self.gas_v.clear();
        for _ in 0..N_GAS {
            let p = Vec2::new(
                self.rand() as f32 * 5.6 - 2.8,
                self.rand() as f32 * 5.6 - 2.8,
            );
            let a = self.rand() as f32 * std::f32::consts::TAU;
            self.gas_p.push(p);
            self.gas_v.push(Vec2::new(a.cos(), a.sin()) * 1.35);
        }
        self.walkers = vec![Vec2::ZERO; N_WALK];
        self.walk_square = 0.0;
    }

    fn ising_sweeps(&mut self, n_sweeps: usize) {
        let beta = 1.0 / self.params.f("temp");
        for _ in 0..n_sweeps {
            for _ in 0..NI * NI {
                let i = (self.rand() * NI as f64) as usize % NI;
                let j = (self.rand() * NI as f64) as usize % NI;
                let idx = j * NI + i;
                let s = self.spins[idx] as i32;
                let nb = self.spins[j * NI + (i + 1) % NI] as i32
                    + self.spins[j * NI + (i + NI - 1) % NI] as i32
                    + self.spins[((j + 1) % NI) * NI + i] as i32
                    + self.spins[((j + NI - 1) % NI) * NI + i] as i32;
                let de = 2.0 * s as f64 * nb as f64;
                if de <= 0.0 || self.rand() < (-beta * de).exp() {
                    self.spins[idx] = -self.spins[idx];
                }
            }
        }
    }

    fn upload_spins(&self, gpu: &GpuContext) {
        let Some(tex) = &self.tex else { return };
        let mut data = Vec::with_capacity(NI * NI * 2);
        for &s in &self.spins {
            data.push(f16_bits(s as f32));
            data.push(0u16);
        }
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
                bytes_per_row: Some((NI * 4) as u32),
                rows_per_image: Some(NI as u32),
            },
            wgpu::Extent3d {
                width: NI as u32,
                height: NI as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    fn gas_step(&mut self, h: f32) {
        let box_h = 2.9f32;
        // Cell-hash elastic collisions (equal masses: swap velocity components
        // along the line of centres).
        let r = 0.055f32;
        for i in 0..N_GAS {
            let v = self.gas_v[i];
            self.gas_p[i] += v * h;
            let p = &mut self.gas_p[i];
            if p.x < -box_h + r {
                p.x = -box_h + r;
                self.gas_v[i].x = self.gas_v[i].x.abs();
            }
            if p.x > box_h - r {
                p.x = box_h - r;
                self.gas_v[i].x = -self.gas_v[i].x.abs();
            }
            if p.y < -box_h + r {
                p.y = -box_h + r;
                self.gas_v[i].y = self.gas_v[i].y.abs();
            }
            if p.y > box_h - r {
                p.y = box_h - r;
                self.gas_v[i].y = -self.gas_v[i].y.abs();
            }
        }
        // Spatial grid.
        let cell = 4.0 * r;
        let dim = ((2.0 * box_h) / cell).ceil() as i32;
        let mut grid: std::collections::HashMap<(i32, i32), Vec<usize>> =
            std::collections::HashMap::new();
        for i in 0..N_GAS {
            let c = (
                ((self.gas_p[i].x + box_h) / cell) as i32,
                ((self.gas_p[i].y + box_h) / cell) as i32,
            );
            grid.entry(c).or_default().push(i);
        }
        let dmin = 2.0 * r;
        for (&(cx, cy), items) in &grid {
            for n_off in [(0, 0), (1, 0), (0, 1), (1, 1), (1, -1)] {
                let key = (cx + n_off.0, cy + n_off.1);
                if key.0 < 0 || key.1 < 0 || key.0 > dim || key.1 > dim {
                    continue;
                }
                let Some(other) = grid.get(&key) else {
                    continue;
                };
                for (ai, &a) in items.iter().enumerate() {
                    let start = if key == (cx, cy) { ai + 1 } else { 0 };
                    for &b in &other[start..] {
                        let d = self.gas_p[b] - self.gas_p[a];
                        let dist2 = d.length_squared();
                        if dist2 < dmin * dmin && dist2 > 1e-9 {
                            let nrm = d / dist2.sqrt();
                            let rel = (self.gas_v[a] - self.gas_v[b]).dot(nrm);
                            if rel > 0.0 {
                                self.gas_v[a] -= nrm * rel;
                                self.gas_v[b] += nrm * rel;
                                // Separate overlap.
                                let push = nrm * (dmin - dist2.sqrt()) * 0.5;
                                self.gas_p[a] -= push;
                                self.gas_p[b] += push;
                            }
                        }
                    }
                }
            }
        }
    }
}

impl ConceptModule for EnsembleGallery {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "statmech.ensemble",
            name: "The Ensemble Gallery",
            tagline: "Order out of chance",
            branch: Branch::StatMech,
            description: "Nothing here is choreographed. Spins flip by coin-toss weighted with Boltzmann's factor and organize into continents at the critical temperature; a gas started at one single speed collides its way into Maxwell's curve; drunkards' walks add up to the diffusion equation. Statistics is the physics of not knowing \u{2014} and it works.",
            epigraph: Some((
                "S = k log W",
                "carved on Boltzmann's grave",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "ising",
                "Ising Lattice",
                "128\u{00B2} spins under Metropolis; criticality at T \u{2248} 2.269",
                3.4,
            ),
            RenderMode::plane(
                "gas",
                "Gas in a Box",
                "900 elastic discs; one speed becomes Maxwell\u{2013}Boltzmann",
                3.6,
            )
            .look_at(0.0, -0.3, 0.0),
            RenderMode::plane(
                "walk",
                "The Drunkard's Cloud",
                "7,000 random walks; \u{221A}t written in light",
                3.2,
            ),
            RenderMode::plane(
                "perc",
                "Percolation Threshold",
                "At p \u{2248} 0.5927 a path suddenly spans the world (teal: connected to the top row)",
                3.4,
            ),
            RenderMode::plane(
                "life",
                "Conway's Colony",
                "Three rules, no physics \u{2014} and still: gliders, pulsars, memory",
                3.4,
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
        (mode == 0).then_some("paint spins up with the pointer")
    }
    fn pointer(&mut self, _mode: usize, ev: PointerEvent) {
        use PointerEvent as PE;
        if let PE::Down(w) | PE::Drag(w) = ev {
            let ci = ((w.x + 3.2) / 6.4 * NI as f32) as i64;
            let cj = ((3.2 - w.y) / 6.4 * NI as f32) as i64;
            let rad = 6i64;
            for dj in -rad..=rad {
                for di in -rad..=rad {
                    if di * di + dj * dj > rad * rad {
                        continue;
                    }
                    let (i, j) = (ci + di, cj + dj);
                    if i >= 0 && j >= 0 && (i as usize) < NI && (j as usize) < NI {
                        self.spins[j as usize * NI + i as usize] = 1;
                    }
                }
            }
        }
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "sm.ising", ISING_SHADE, true);
        rend.register_field(gpu, "sm.levels", LEVELS_SHADE, true);
        let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ising-spins"),
            size: wgpu::Extent3d {
                width: NI as u32,
                height: NI as u32,
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
        self.reset_all();
        self.build_percolation();
        self.seed_life();
    }

    fn rewind(&mut self) {
        self.reset_all();
        self.build_percolation();
        self.seed_life();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, dt: f64) {
        let pf = (self.params.f("pfill") * 10000.0) as i64;
        if pf != self.last_pfill {
            self.last_pfill = pf;
            self.build_percolation();
        }
        self.life_clock += dt;
        if self.life_clock > 0.09 {
            self.life_clock = 0.0;
            self.step_life();
        }
        if dt > 0.0 {
            self.ising_sweeps(self.params.f("sweeps") as usize);
            let h = (dt as f32).clamp(0.0, 0.033);
            for _ in 0..2 {
                self.gas_step(h * 0.5);
            }
            // Walkers.
            let step = self.params.f("dstep") as f32 * 3.0;
            for i in 0..self.walkers.len() {
                let a = self.rand() as f32 * std::f32::consts::TAU;
                self.walkers[i] += Vec2::new(a.cos(), a.sin()) * step;
            }
            self.walk_square += f64::from(step * step);
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
        match mode {
            0 => {
                // The texture is shared by three views, so each uploads its own
                // lattice before drawing. Paint made while paused shows at once.
                self.upload_spins(_gpu);
                scene.field = Some("sm.ising");
                scene.field_tex = self.tex_view.clone();
            }
            3 => {
                // Percolation upload happens here so panes can coexist.
                let mut data = Vec::with_capacity(NI * NI * 2);
                for &v in &self.perc {
                    data.push(f16_bits(v));
                    data.push(0u16);
                }
                if let Some(tex) = &self.tex {
                    _gpu.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytemuck::cast_slice(&data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some((NI * 4) as u32),
                            rows_per_image: Some(NI as u32),
                        },
                        wgpu::Extent3d {
                            width: NI as u32,
                            height: NI as u32,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                scene.field = Some("sm.levels");
                scene.field_tex = self.tex_view.clone();
            }
            4 => {
                let mut data = Vec::with_capacity(NI * NI * 2);
                for &v in &self.life {
                    data.push(f16_bits(v as f32));
                    data.push(0u16);
                }
                if let Some(tex) = &self.tex {
                    _gpu.queue.write_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        bytemuck::cast_slice(&data),
                        wgpu::TexelCopyBufferLayout {
                            offset: 0,
                            bytes_per_row: Some((NI * 4) as u32),
                            rows_per_image: Some(NI as u32),
                        },
                        wgpu::Extent3d {
                            width: NI as u32,
                            height: NI as u32,
                            depth_or_array_layers: 1,
                        },
                    );
                }
                scene.field = Some("sm.levels");
                scene.field_tex = self.tex_view.clone();
            }
            1 => {
                // The box.
                let b = 2.9f32;
                let frame = [
                    Vec3::new(-b, -b, 0.0),
                    Vec3::new(b, -b, 0.0),
                    Vec3::new(b, b, 0.0),
                    Vec3::new(-b, b, 0.0),
                    Vec3::new(-b, -b, 0.0),
                ];
                scene.polyline(LineLayer::World, &frame, 1.6, [0.55, 0.58, 0.66, 0.6]);
                let vmax = 3.2f32;
                for i in 0..N_GAS {
                    let sp = self.gas_v[i].length();
                    let heat = (sp / vmax).clamp(0.0, 1.0);
                    let c = [
                        0.20 + 0.80 * heat,
                        0.35 + 0.35 * heat,
                        0.95 - 0.70 * heat,
                        0.9,
                    ];
                    scene.point(
                        Vec3::new(self.gas_p[i].x, self.gas_p[i].y, 0.0),
                        2.0,
                        c,
                        0.8,
                    );
                }
                // Speed histogram below the box + Maxwell-Boltzmann overlay.
                let bins = 44;
                let mut hist = vec![0u32; bins];
                let mut mean_v2 = 0.0f64;
                for v in &self.gas_v {
                    let s = v.length();
                    mean_v2 += (s * s) as f64;
                    let b_i = ((s / vmax) * bins as f32) as usize;
                    if b_i < bins {
                        hist[b_i] += 1;
                    }
                }
                mean_v2 /= N_GAS as f64;
                let max_h = *hist.iter().max().unwrap_or(&1) as f32;
                let y0 = -b - 1.35;
                for (bi, &hv) in hist.iter().enumerate() {
                    if hv == 0 {
                        continue;
                    }
                    let x = -b + (bi as f32 + 0.5) / bins as f32 * 2.0 * b;
                    let hgt = hv as f32 / max_h * 1.0;
                    scene.line(
                        LineLayer::World,
                        Vec3::new(x, y0, 0.0),
                        Vec3::new(x, y0 + hgt, 0.0),
                        5.0,
                        with_alpha(HEAT, 0.55),
                    );
                }
                // 2D M-B: f(v) ~ v exp(-v^2/(2 sigma^2)), sigma^2 = <v^2>/2.
                let sig2 = (mean_v2 / 2.0) as f32;
                let norm: f32 = {
                    // peak of v exp(-v^2/2s2) at v = sqrt(s2)
                    let vp = sig2.sqrt();
                    vp * (-vp * vp / (2.0 * sig2)).exp()
                };
                let pts: Vec<Vec3> = (0..=120)
                    .map(|i| {
                        let v = vmax * i as f32 / 120.0;
                        let f = v * (-v * v / (2.0 * sig2)).exp() / norm;
                        Vec3::new(-b + v / vmax * 2.0 * b, y0 + f * 1.0, 0.01)
                    })
                    .collect();
                scene.glow_polyline(&pts, 1.8, COOL, 2.4);
            }
            _ => {
                for w in &self.walkers {
                    scene.point(Vec3::new(w.x, w.y, 0.0), 1.2, with_alpha(HEAT, 0.35), 0.8);
                }
                // Rings at the root-mean-square radius and twice it. A walk's mean
                // squared distance is the sum of its squared steps.
                let sigma = self.walk_square.sqrt() as f32;
                for (mult, alpha) in [(1.0f32, 0.5), (2.0, 0.25)] {
                    let ring: Vec<Vec3> = (0..=96)
                        .map(|i| {
                            let a = i as f32 / 96.0 * std::f32::consts::TAU;
                            Vec3::new(a.cos() * sigma * mult, a.sin() * sigma * mult, 0.02)
                        })
                        .collect();
                    scene.polyline(LineLayer::Overlay, &ring, 1.3, with_alpha(COOL, alpha));
                }
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let temp = self.params.f("temp");
        let mag: f64 = self.spins.iter().map(|&s| s as f64).sum::<f64>() / (NI * NI) as f64;
        let mean_v2: f64 = self
            .gas_v
            .iter()
            .map(|v| (v.length() * v.length()) as f64)
            .sum::<f64>()
            / N_GAS as f64;
        vec![
            title("Boltzmann's gamble"),
            ftex("statmech.ising", "E = \u{2212}J \u{03A3} s\u{1D62}s\u{2C7C},   P \u{221D} e^{\u{2212}E/k_BT}"),
            ftex("statmech.maxwell", "f(v) \u{221D} v e^{\u{2212}mv\u{00B2}/2k_BT}  (2D)"),
            ftex("statmech.diffusion", "\u{2202}\u{03C1}/\u{2202}t = D\u{2207}\u{00B2}\u{03C1},   \u{03C3} = \u{221A}(2Dt)"),
            value(format!("T = {temp:.2}  (T_c = 2/ln(1+\u{221A}2) \u{2248} 2.269)    m = {mag:+.3}", )),
            value(format!("gas \u{27E8}v\u{00B2}\u{27E9} = {mean_v2:.3}  (conserved by elastic collisions)")),
            note("Below T_c the lattice chooses a side and magnetizes; at T_c the fluctuations have no scale. Paint a patch of spins and watch the temperature decide its fate."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Metropolis single-spin flips, periodic boundary; sweeps/frame set by the slider."),
            1 => Some("Equal-mass hard-disc collisions on a spatial hash; energy conserved to float precision."),
            2 => Some("Isotropic fixed-step walk, one step per frame; rings mark the exact root-mean-square radius of those steps, and twice it."),
            3 => Some("Site percolation on a 128\u{00B2} lattice; teal marks sites joined to the top row by occupied neighbours."),
            4 => Some("Conway's rules, exact, on a 128\u{00B2} periodic lattice."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::params::PValue;

    fn magnetization(g: &EnsembleGallery) -> f64 {
        let sum: f64 = g.spins.iter().map(|&s| f64::from(s)).sum();
        (sum / g.spins.len() as f64).abs()
    }

    fn after_sweeps_from_order(temp: f64) -> f64 {
        let mut g = EnsembleGallery::new();
        g.params.set("temp", PValue::F(temp));
        g.spins = vec![1; NI * NI];
        for _ in 0..200 {
            g.ising_sweeps(5);
        }
        magnetization(&g)
    }

    #[test]
    fn ising_orders_below_the_critical_temperature_and_melts_above_it() {
        assert!(
            after_sweeps_from_order(1.5) > 0.9,
            "cold lattice lost its order"
        );
        assert!(
            after_sweeps_from_order(4.0) < 0.2,
            "hot lattice kept its order"
        );
    }
}
