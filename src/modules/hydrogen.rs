//! Hydrogen: exact Coulomb eigenfunctions rendered as volumetric amplitude
//! clouds, nodal chambers, radial distributions, and Born-rule point clouds.
//! No planetary electrons anywhere.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

// Balmer-inspired accents.
const H_ALPHA: [f32; 4] = [0.94, 0.35, 0.30, 1.0];
const H_BETA: [f32; 4] = [0.35, 0.80, 0.90, 1.0];

const PSI_WGSL: &str = r#"
fn fact(n: i32) -> f32 {
    var r = 1.0;
    for (var i = 2; i <= n; i++) { r *= f32(i); }
    return r;
}

fn laguerre(k: i32, a: i32, x: f32) -> f32 {
    if (k <= 0) { return 1.0; }
    var lm2 = 1.0;
    var lm1 = 1.0 + f32(a) - x;
    if (k == 1) { return lm1; }
    var l = 0.0;
    for (var i = 2; i <= k; i++) {
        l = ((f32(2 * i - 1 + a) - x) * lm1 - f32(i - 1 + a) * lm2) / f32(i);
        lm2 = lm1;
        lm1 = l;
    }
    return l;
}

fn assoc_legendre(l: i32, m: i32, ct: f32) -> f32 {
    let s = sqrt(max(1.0 - ct * ct, 0.0));
    switch (l * 10 + m) {
        case 0:  { return 1.0; }
        case 10: { return ct; }
        case 11: { return -s; }
        case 20: { return 0.5 * (3.0 * ct * ct - 1.0); }
        case 21: { return -3.0 * ct * s; }
        case 22: { return 3.0 * s * s; }
        case 30: { return 0.5 * ct * (5.0 * ct * ct - 3.0); }
        case 31: { return -1.5 * (5.0 * ct * ct - 1.0) * s; }
        case 32: { return 15.0 * ct * s * s; }
        case 33: { return -15.0 * s * s * s; }
        default: { return 0.0; }
    }
}

// psi_nlm(r, theta, phi) as complex (re, im); lengths in Bohr radii.
fn psi_h(n: i32, l: i32, m: i32, r: f32, ct: f32, phi: f32) -> vec2<f32> {
    let rho = 2.0 * r / f32(n);
    let nr = sqrt(pow(2.0 / f32(n), 3.0) * fact(n - l - 1) / (2.0 * f32(n) * fact(n + l)));
    let rad = nr * exp(-rho * 0.5) * pow(max(rho, 1e-6), f32(l)) * laguerre(n - l - 1, 2 * l + 1, rho);
    let am = abs(m);
    let ny = sqrt(f32(2 * l + 1) / (4.0 * PI) * fact(l - am) / fact(l + am));
    let p = assoc_legendre(l, am, ct);
    let ang = f32(m) * phi;
    return rad * ny * p * vec2<f32>(cos(ang), sin(ang));
}
"#;

const CLOUD_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let n = i32(U.p[0].x + 0.5);
    let l = i32(U.p[0].y + 0.5);
    let m = i32(U.p[0].z + 0.5) - 4;
    let dens_scale = U.p[0].w;
    let nodal = U.p[1].x > 0.5;
    let cutaway = U.p[1].y > 0.5;

    let ro = U.eye.xyz;
    let rd = ray_dir(ndc);
    let r_max = f32(n * n) * 2.0 + 8.0;

    // Ray-sphere entry/exit for the bounding sphere.
    let b = dot(ro, rd);
    let cc = dot(ro, ro) - r_max * r_max;
    let disc = b * b - cc;
    var color = vec3<f32>(0.0);
    if (disc < 0.0) {
        return vec4<f32>(color, 1.0);
    }
    let t0 = max(-b - sqrt(disc), 0.0);
    let t1 = -b + sqrt(disc);
    let steps = 110;
    let ds = (t1 - t0) / f32(steps);
    var trans = 1.0;
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < steps; i++) {
        let tt = t0 + (f32(i) + 0.5) * ds;
        let p = ro + rd * tt;
        if (cutaway && p.z < 0.0) { continue; }
        let r = length(p);
        let ct = p.y / max(r, 1e-5);
        let phi = atan2(p.z, p.x);
        let ps = psi_h(n, l, m, r, ct, phi);
        var dens = dot(ps, ps) * dens_scale;
        var hue = phase_color(atan2(ps.y, ps.x));
        if (nodal) {
            // Sign chambers of Re(psi e^{-im phi}) = R * P: gold vs indigo.
            let sgn = ps.x * cos(f32(m) * phi) + ps.y * sin(f32(m) * phi);
            if (sgn >= 0.0) { hue = vec3<f32>(0.92, 0.70, 0.25); }
            else { hue = vec3<f32>(0.30, 0.35, 0.95); }
        }
        let a = 1.0 - exp(-dens * ds);
        acc += trans * a * hue * 1.6;
        trans *= 1.0 - a;
        if (trans < 0.02) { break; }
    }
    color = color * trans + acc;
    return vec4<f32>(color, 1.0);
}
"#;

/// CPU radial function R_nl(r) for the plane views and the sampler.
fn radial(n: u32, l: u32, r: f64) -> f64 {
    let nf = n as f64;
    let rho = 2.0 * r / nf;
    let fact = |k: u32| -> f64 { (1..=k).map(|v| v as f64).product::<f64>().max(1.0) };
    let norm = ((2.0 / nf).powi(3) * fact(n - l - 1) / (2.0 * nf * fact(n + l))).sqrt();
    // Associated Laguerre by recurrence.
    let k = (n - l - 1) as i64;
    let a = (2 * l + 1) as f64;
    let lag = if k == 0 {
        1.0
    } else {
        let mut lm2 = 1.0;
        let mut lm1 = 1.0 + a - rho;
        if k == 1 {
            lm1
        } else {
            let mut l_val = 0.0;
            for i in 2..=k {
                let fi = i as f64;
                l_val = ((2.0 * fi - 1.0 + a - rho) * lm1 - (fi - 1.0 + a) * lm2) / fi;
                lm2 = lm1;
                lm1 = l_val;
            }
            l_val
        }
    };
    norm * (-rho / 2.0).exp() * rho.powi(l as i32) * lag
}

fn legendre(l: u32, m: u32, ct: f64) -> f64 {
    let s = (1.0 - ct * ct).max(0.0).sqrt();
    match (l, m) {
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
        _ => 0.0,
    }
}

const N_WALKERS: usize = 24000;

pub struct HydrogenSanctum {
    params: Params,
    samples: Vec<Vec3>,
    /// Metropolis walkers: positions + cached |psi|^2, re-jittered every frame
    /// so the cloud trembles while its density stays exactly Born-rule.
    walkers: Vec<(Vec3, f64)>,
    rng: u64,
    last_key: (i64, i64, i64),
    /// Largest value of the rejection weight for the current state.
    peak_weight: f64,
}

/// The sampling weight |psi|^2 r^2 at (r, cos theta), for a uniform proposal.
fn sampling_weight(n: u32, l: u32, m: i32, r: f64, ct: f64) -> f64 {
    let am = m.unsigned_abs();
    let fact = |k: u32| -> f64 { (1..=k).map(|v| v as f64).product::<f64>().max(1.0) };
    let ny =
        ((2 * l + 1) as f64 / (4.0 * std::f64::consts::PI) * fact(l - am) / fact(l + am)).sqrt();
    (radial(n, l, r) * ny * legendre(l, am, ct)).powi(2) * r * r
}

/// Scan for the weight's maximum, because a bound below it would clip the
/// densest lobes and bias the sampled cloud toward a flat one.
fn find_peak_weight(n: u32, l: u32, m: i32) -> f64 {
    let r_max = (n * n) as f64 * 2.0 + 8.0;
    let mut peak = 0.0f64;
    for i in 1..=400 {
        let r = r_max * i as f64 / 400.0;
        for j in 0..=200 {
            let ct = -1.0 + 2.0 * j as f64 / 200.0;
            peak = peak.max(sampling_weight(n, l, m, r, ct));
        }
    }
    peak * 1.02
}

impl HydrogenSanctum {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.int("n", "Principal n", "Symbolic", 1, 4, 3);
        p.int("l", "Azimuthal \u{2113}", "Symbolic", 0, 3, 2);
        p.int("m", "Magnetic m", "Symbolic", -3, 3, 1);
        p.slider("dens", "Cloud density", "Display", 0.2, 4.0, 1.0, "");
        p.toggle("cut", "Cutaway (reveal interior)", "Display", false);
        Self {
            params: p,
            samples: Vec::new(),
            walkers: Vec::new(),
            rng: 0xda3e39cb94b95bdb,
            last_key: (9, 9, 9),
            peak_weight: 1.0,
        }
    }

    /// |psi_nlm|^2 at a point (CPU twin of the shader).
    fn psi2(&self, n: u32, l: u32, m: i32, p: Vec3) -> f64 {
        let r = (p.length() as f64).max(1e-9);
        let ct = p.y as f64 / r;
        let am = m.unsigned_abs();
        let fact = |k: u32| -> f64 { (1..=k).map(|v| v as f64).product::<f64>().max(1.0) };
        let ny = ((2 * l + 1) as f64 / (4.0 * std::f64::consts::PI) * fact(l - am) / fact(l + am))
            .sqrt();
        let a = radial(n, l, r) * ny * legendre(l, am, ct);
        a * a
    }

    /// One Metropolis sweep: every walker proposes a Gaussian-ish step and
    /// accepts with min(1, psi2'/psi2). Detailed balance keeps the cloud
    /// distributed exactly as |psi|^2 while every dot vibrates.
    fn jitter_walkers(&mut self) {
        let (n, l, m) = self.nlm();
        let sigma = 0.16 * (n * n) as f32 * 0.35 + 0.10;
        for i in 0..self.walkers.len() {
            let (pos, p2) = self.walkers[i];
            let step = Vec3::new(
                (self.rand() as f32 - 0.5) * 2.0,
                (self.rand() as f32 - 0.5) * 2.0,
                (self.rand() as f32 - 0.5) * 2.0,
            ) * sigma;
            let cand = pos + step;
            let p2c = self.psi2(n, l, m, cand);
            if p2c > p2 || self.rand() < p2c / p2.max(1e-300) {
                self.walkers[i] = (cand, p2c);
            }
        }
    }

    fn seed_walkers(&mut self) {
        let (n, l, m) = self.nlm();
        self.walkers.clear();
        let spread = (n * n) as f32 * 1.2 + 1.0;
        while self.walkers.len() < N_WALKERS {
            let p = Vec3::new(
                (self.rand() as f32 - 0.5) * 2.0,
                (self.rand() as f32 - 0.5) * 2.0,
                (self.rand() as f32 - 0.5) * 2.0,
            ) * spread;
            let p2 = self.psi2(n, l, m, p);
            self.walkers.push((p, p2.max(1e-300)));
        }
        // Burn-in so the cloud settles into the orbital before it is shown.
        for _ in 0..30 {
            self.jitter_walkers();
        }
    }

    fn nlm(&self) -> (u32, u32, i32) {
        let n = self.params.i("n").clamp(1, 4) as u32;
        let l = (self.params.i("l").max(0) as u32).min(n - 1);
        let m = self.params.i("m").clamp(-(l as i64), l as i64) as i32;
        (n, l, m)
    }

    fn rand(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Rejection-sample a point from |psi|^2.
    fn sample_point(&mut self) -> Option<Vec3> {
        let (n, l, m) = self.nlm();
        let r_max = (n * n) as f64 * 2.0 + 8.0;
        for _ in 0..200 {
            let r = self.rand() * r_max;
            let ct = self.rand() * 2.0 - 1.0;
            let phi = self.rand() * std::f64::consts::TAU;
            let w = sampling_weight(n, l, m, r, ct);
            if self.rand() * self.peak_weight < w {
                let st = (1.0 - ct * ct).max(0.0).sqrt();
                return Some(Vec3::new(
                    (r * st * phi.cos()) as f32,
                    (r * ct) as f32,
                    (r * st * phi.sin()) as f32,
                ));
            }
        }
        None
    }
}

impl ConceptModule for HydrogenSanctum {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "quantum.hydrogen",
            name: "The Hydrogen Sanctum",
            tagline: "One proton, one amplitude, all of chemistry",
            branch: Branch::Quantum,
            description: "The hydrogen atom solved exactly: \u{03C8}\u{2099}\u{2113}\u{2098} = R\u{2099}\u{2113}(r)\u{00B7}Y\u{2113}\u{2098}(\u{03B8},\u{03C6}). What is rendered is the amplitude itself \u{2014} its density as luminous fog, its phase as hue turning m times around the axis, its nodes as dark chambers. The electron is not a planet; it is the statistics of this cloud.",
            epigraph: Some((
                "An atom is not a solar system. It is a resonance.",
                "\u{2014} the moral of 1926",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit("cloud", "Amplitude Cloud", "Volumetric |\u{03C8}|\u{00B2}, hue = arg \u{03C8} (turns m times about the axis)", 42.0, 0.25, 0.6),
            RenderMode::orbit("walkers", "Electron Cloud (Trembling)", "24,000 Metropolis walkers, always moving; their long-run density is |\u{03C8}|\u{00B2}", 42.0, 0.25, 0.6),
            RenderMode::orbit("nodal", "Nodal Chambers", "Sign of R(r)P(cos \u{03B8}), \u{03C8} without its phase: gold against indigo, dark node walls between", 42.0, 0.25, 0.6),
            RenderMode::plane("radial", "Radial Distribution", "P(r) = r\u{00B2}R\u{2099}\u{2113}\u{00B2}(r): shells, nodes, and \u{27E8}r\u{27E9}", 14.0)
                .look_at(16.0, 9.0, 0.0),
            RenderMode::orbit("sample", "Born Accumulation", "Measurements pile up one by one into the orbital", 42.0, 0.25, 0.6),
        ]
    }

    fn default_modes(&self) -> Vec<usize> {
        vec![1, 0]
    }
    fn params(&mut self) -> &mut Params {
        &mut self.params
    }
    fn params_ref(&self) -> &Params {
        &self.params
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(
            gpu,
            "hydrogen.cloud",
            &format!("{PSI_WGSL}{CLOUD_SHADE}"),
            false,
        );
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, _dt: f64) {
        let (n, l, m) = self.nlm();
        let key = (n as i64, l as i64, m as i64);
        if key != self.last_key {
            self.last_key = key;
            self.peak_weight = find_peak_weight(n, l, m);
            self.samples.clear();
            self.seed_walkers();
        }
        if self.walkers.is_empty() {
            self.seed_walkers();
        }
        // The trembling: one Metropolis sweep per frame.
        self.jitter_walkers();
        // Grow the accumulation cloud steadily.
        for _ in 0..40 {
            if self.samples.len() >= 6000 {
                break;
            }
            if let Some(p) = self.sample_point() {
                self.samples.push(p);
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
        let (n, l, m) = self.nlm();
        scene.p(0, n as f32);
        scene.p(1, l as f32);
        scene.p(2, (m + 4) as f32);
        // Density autoscale ~ n^6 to keep clouds visible across n.
        scene.p(3, self.params.f("dens") as f32 * (n as f32).powi(6) * 2.2);
        scene.p(4, if mode == 2 { 1.0 } else { 0.0 });
        scene.p(
            5,
            if self.params.b("cut") || mode == 2 {
                1.0
            } else {
                0.0
            },
        );

        match mode {
            0 | 2 => {
                scene.field = Some("hydrogen.cloud");
                // Quantization axis, engraved.
                let ax = (n * n) as f32 * 2.0 + 6.0;
                scene.line(
                    LineLayer::Overlay,
                    Vec3::new(0.0, -ax, 0.0),
                    Vec3::new(0.0, ax, 0.0),
                    1.0,
                    [0.5, 0.55, 0.7, 0.18],
                );
            }
            1 => {
                // The trembling pointillist cloud: every dot is a walker.
                for (p, _) in &self.walkers {
                    let phi = p.z.atan2(p.x);
                    let hue = phase_color(m as f32 * phi);
                    scene.point(*p, 1.15, with_alpha(hue, 0.42), 0.9);
                }
                let ax = (n * n) as f32 * 2.0 + 6.0;
                scene.line(
                    LineLayer::Overlay,
                    Vec3::new(0.0, -ax, 0.0),
                    Vec3::new(0.0, ax, 0.0),
                    1.0,
                    [0.5, 0.55, 0.7, 0.18],
                );
                scene.point(Vec3::ZERO, 3.0, [1.0, 0.9, 0.75, 1.0], 2.0);
            }
            3 => {
                let r_max = (n * n) as f64 * 2.4 + 6.0;
                let sx = 32.0 / r_max; // fit into view width
                                       // P(r) curve.
                let pts: Vec<Vec3> = (0..=400)
                    .map(|i| {
                        let r = r_max * i as f64 / 400.0;
                        let p = (r * radial(n, l, r)).powi(2);
                        Vec3::new((r * sx) as f32, (p * 18.0 * (n as f64).powi(2)) as f32, 0.0)
                    })
                    .collect();
                scene.glow_polyline(&pts, 2.2, H_BETA, 2.8);
                scene.line(
                    LineLayer::World,
                    Vec3::ZERO,
                    Vec3::new(33.0, 0.0, 0.0),
                    1.2,
                    [0.45, 0.5, 0.6, 0.4],
                );
                // <r> = (1/2)[3n^2 - l(l+1)].
                let mean_r = 0.5 * (3.0 * (n * n) as f64 - (l * (l + 1)) as f64);
                scene.line(
                    LineLayer::WorldAdd,
                    Vec3::new((mean_r * sx) as f32, 0.0, 0.0),
                    Vec3::new((mean_r * sx) as f32, 15.0, 0.0),
                    1.6,
                    with_alpha(H_ALPHA, 0.5),
                );
                // Bohr-shell ghost rings on the axis for comparison.
                for shell in 1..=n {
                    let rb = (shell * shell) as f64;
                    scene.point(
                        Vec3::new((rb * sx) as f32, -0.7, 0.0),
                        2.5,
                        with_alpha(H_ALPHA, 0.5),
                        1.2,
                    );
                }
            }
            _ => {
                // Born cloud: pure samples, phase-hued by azimuth.
                for p in &self.samples {
                    let phi = p.z.atan2(p.x);
                    let hue = phase_color(m as f32 * phi);
                    scene.point(*p, 1.6, with_alpha(hue, 0.75), 1.2);
                }
                let ax = (n * n) as f32 * 2.0 + 6.0;
                scene.line(
                    LineLayer::Overlay,
                    Vec3::new(0.0, -ax, 0.0),
                    Vec3::new(0.0, ax, 0.0),
                    1.0,
                    [0.5, 0.55, 0.7, 0.18],
                );
                scene.point(Vec3::ZERO, 3.0, [1.0, 0.9, 0.75, 1.0], 2.0);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let (n, l, m) = self.nlm();
        let e = -1.0 / (2.0 * (n * n) as f64);
        let mean_r = 0.5 * (3.0 * (n * n) as f64 - (l * (l + 1)) as f64);
        let radial_nodes = n - l - 1;
        vec![
            title(format!("\u{03C8}\u{2099}\u{2113}\u{2098}  \u{2014}  n={n}, \u{2113}={l}, m={m:+}")),
            ftex("hydrogen.psi", "\u{03C8} = R\u{2099}\u{2113}(r) \u{00B7} Y\u{2113}\u{2098}(\u{03B8},\u{03C6})     E\u{2099} = \u{2212}1/2n\u{00B2}"),
            ftex("hydrogen.radial", "R\u{2099}\u{2113} \u{221D} e^{\u{2212}r/n} (2r/n)^\u{2113} L\u{2099}\u{208B}\u{2113}\u{208B}\u{2081}^{2\u{2113}+1}(2r/n)   (a\u{2080} = 1)"),
            value(format!("E = {e:.4} Ha = {:.2} eV", e * 27.2114)),
            value(format!("\u{27E8}r\u{27E9} = {mean_r:.1} a\u{2080}    radial nodes: {radial_nodes}    angular nodes: {l}")),
            value(format!("phase winds m = {m:+} times around the z-axis")),
            note("Hue is the quantum phase of \u{03C8} \u{2014} unobservable alone, but its winding number m is real physics: angular momentum L\u{1D22} = m\u{0127}."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 | 2 => Some("Exact \u{03C8}\u{2099}\u{2113}\u{2098} in fp32; emission integration, 110 steps; brightness is tuned, geometry is not."),
            1 => Some("24,000 Metropolis\u{2013}Hastings walkers, one sweep per frame; stationary density is exactly |\u{03C8}|\u{00B2}."),
            4 => Some("Rejection sampling from exact |\u{03C8}|\u{00B2} inside a ball of radius 2n\u{00B2} + 8; every dot is one Born-rule draw."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set_state(h: &mut HydrogenSanctum, n: i64, l: i64, m: i64) {
        use crate::engine::params::PValue;
        h.params.set("n", PValue::I(n));
        h.params.set("l", PValue::I(l));
        h.params.set("m", PValue::I(m));
        let (n, l, m) = h.nlm();
        h.peak_weight = find_peak_weight(n, l, m);
    }

    /// <r> = (3n^2 - l(l + 1)) / 2 in atomic units, for every hydrogen state.
    #[test]
    fn sampled_mean_radius_matches_the_exact_expectation() {
        let mut h = HydrogenSanctum::new();
        for (n, l, m) in [(1, 0, 0), (2, 1, 0), (3, 2, 1), (4, 3, 3), (4, 0, 0)] {
            set_state(&mut h, n, l, m);
            let mut sum = 0.0f64;
            let mut count = 0.0f64;
            while count < 20_000.0 {
                if let Some(p) = h.sample_point() {
                    sum += f64::from(p.length());
                    count += 1.0;
                }
            }
            let exact = (3 * n * n - l * (l + 1)) as f64 / 2.0;
            let mean = sum / count;
            assert!(
                (mean - exact).abs() < 0.03 * exact,
                "n={n} l={l} m={m}: mean {mean}, exact {exact}"
            );
        }
    }

    #[test]
    fn the_radial_plot_stays_inside_its_view() {
        for (n, l) in [(1, 0), (2, 0), (2, 1), (3, 0), (3, 2), (4, 0), (4, 3)] {
            let r_max = (n * n) as f64 * 2.0 + 8.0;
            let peak = (0..=2000)
                .map(|i| {
                    let r = r_max * i as f64 / 2000.0;
                    (r * radial(n, l, r)).powi(2) * 18.0 * (n as f64).powi(2)
                })
                .fold(0.0f64, f64::max);
            assert!(peak < 23.0, "n={n} l={l}: peak height {peak}");
        }
    }
}
