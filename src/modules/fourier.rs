//! Fourier decomposition: waveforms unwound into circles.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

const TEAL: [f32; 4] = [0.25, 0.78, 0.72, 1.0];
const ORANGE: [f32; 4] = [0.95, 0.55, 0.20, 1.0];
const GHOST: [f32; 4] = [0.55, 0.60, 0.70, 0.35];

const PAPER: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.0);
    color += vec3<f32>(0.10, 0.22, 0.22) * grid_glow(w, 0.7) * 0.14;
    let vig = 1.0 - 0.30 * dot(ndc, ndc);
    return vec4<f32>(color * vig, 1.0);
}
"#;

/// Fourier sine coefficient b_n of the chosen 2pi-periodic waveform.
fn coeff(wave: usize, n: usize) -> f64 {
    let nf = n as f64;
    match wave {
        // Square wave: 4/(pi n), odd n.
        0 => {
            if n % 2 == 1 {
                4.0 / (std::f64::consts::PI * nf)
            } else {
                0.0
            }
        }
        // Sawtooth 1 - x/pi on (0, 2 pi): 2/(pi n).
        1 => 2.0 / (std::f64::consts::PI * nf),
        // Triangle: 8/(pi^2 n^2), odd n, alternating.
        2 => {
            if n % 2 == 1 {
                let s = if (n / 2).is_multiple_of(2) { 1.0 } else { -1.0 };
                8.0 / (std::f64::consts::PI.powi(2) * nf * nf) * s
            } else {
                0.0
            }
        }
        // Pulse of width pi/4 and height 1 on a floor of -1/7: even, mean zero, so
        // a cosine series with a_n = (16/(7 pi n)) sin(n pi/8). See `phase`.
        _ => 16.0 / (7.0 * nf * std::f64::consts::PI) * (nf * std::f64::consts::PI / 8.0).sin(),
    }
}

fn target(wave: usize, x: f64) -> f64 {
    let p = x.rem_euclid(std::f64::consts::TAU);
    match wave {
        0 => {
            if p < std::f64::consts::PI {
                1.0
            } else {
                -1.0
            }
        }
        1 => 1.0 - p / std::f64::consts::PI,
        2 => {
            let q = p / std::f64::consts::TAU;
            if q < 0.25 {
                4.0 * q
            } else if q < 0.75 {
                2.0 - 4.0 * q
            } else {
                4.0 * q - 4.0
            }
        }
        _ => {
            let half = std::f64::consts::PI / 8.0;
            if p < half || p > std::f64::consts::TAU - half {
                1.0
            } else {
                -1.0 / 7.0
            }
        }
    }
}

/// Phase of every term: sines, except the even pulse, whose terms are cosines.
fn phase(wave: usize) -> f64 {
    if wave == 3 {
        std::f64::consts::FRAC_PI_2
    } else {
        0.0
    }
}

fn term(wave: usize, n: usize, x: f64) -> f64 {
    coeff(wave, n) * (n as f64 * x + phase(wave)).sin()
}

fn partial_sum(wave: usize, n_max: usize, x: f64) -> f64 {
    (1..=n_max).map(|n| term(wave, n, x)).sum()
}

/// Limit of the first peak of a square wave's partial sums: (2/pi) Si(pi). The
/// jump is 2, so this is 8.95% of the jump above the wave.
const GIBBS_PEAK: f64 = 1.178_979_744;

/// Rotating terms in the heart's Fourier series, at k = +-1 to +-4. The curve is
/// a trigonometric polynomial, so every other coefficient is zero.
#[cfg(test)]
const HEART_CIRCLES: usize = 8;

const WAVE_NAMES: [&str; 4] = ["Square", "Sawtooth", "Triangle", "Pulse train"];
const WAVE_SERIES: [&str; 4] = [
    "f = (4/\u{03C0}) \u{03A3} sin((2k\u{2212}1)x)/(2k\u{2212}1)",
    "f = (2/\u{03C0}) \u{03A3} sin(nx)/n",
    "f = (8/\u{03C0}\u{00B2}) \u{03A3} (\u{2212}1)\u{1D4F} sin((2k+1)x)/(2k+1)\u{00B2}",
    "f = \u{03A3} (16/7n\u{03C0}) sin(n\u{03C0}/8) cos(nx)",
];

pub struct FourierAtelier {
    params: Params,
    /// DFT of a closed figure: (signed frequency, amplitude, phase).
    draw_coefs: Vec<(i32, f64, f64)>,
}

impl FourierAtelier {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("wave", "Waveform", "Symbolic", WAVE_NAMES.to_vec(), 0);
        p.int("n", "Harmonics N", "Physics", 1, 64, 7);
        p.toggle("circles", "Show epicycles", "Display", true);
        p.toggle("ghost", "Show true waveform", "Display", true);
        Self {
            params: p,
            draw_coefs: Vec::new(),
        }
    }

    /// The figure the drawing machine reproduces: a heart, sampled closed.
    fn figure(s: f64) -> (f64, f64) {
        let th = s * std::f64::consts::TAU;
        let x = 16.0 * th.sin().powi(3);
        let y =
            13.0 * th.cos() - 5.0 * (2.0 * th).cos() - 2.0 * (3.0 * th).cos() - (4.0 * th).cos();
        (x * 0.085, y * 0.085 + 0.15)
    }

    fn build_draw_coefs(&mut self) {
        let n_s = 512usize;
        let mut coefs = Vec::new();
        for k in -60i32..=60 {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for i in 0..n_s {
                let s = i as f64 / n_s as f64;
                let (x, y) = Self::figure(s);
                let ang = -std::f64::consts::TAU * k as f64 * s;
                re += x * ang.cos() - y * ang.sin();
                im += x * ang.sin() + y * ang.cos();
            }
            re /= n_s as f64;
            im /= n_s as f64;
            let amp = (re * re + im * im).sqrt();
            if amp > 1e-4 {
                coefs.push((k, amp, im.atan2(re)));
            }
        }
        coefs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        coefs.truncate(70);
        self.draw_coefs = coefs;
    }
}

impl ConceptModule for FourierAtelier {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "analysis.fourier",
            name: "The Harmonic Loom",
            tagline: "Every waveform, woven from circles",
            branch: Branch::Analysis,
            description: "Fourier's claim scandalized his examiners: any periodic signal \u{2014} even a hard-edged square \u{2014} is a sum of smooth sinusoids. Watch the sum assemble as a chain of rotating circles, each turning at an integer multiple of the fundamental, each contributing its amplitude to the woven curve.",
            epigraph: Some(Epigraph::quoted(
"Profound study of nature is the most fertile source of mathematical discoveries.",
"Joseph Fourier, The Analytical Theory of Heat, tr. Freeman",
)),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "epicycles",
                "Epicycle Loom",
                "A chain of rotating circles traces the partial sum",
                2.9,
            )
            .look_at(1.6, 0.0, 0.0),
            RenderMode::plane(
                "stack",
                "Harmonic Stack",
                "Each harmonic alone, then their sum",
                3.4,
            )
            .look_at(3.1, 0.0, 0.0),
            RenderMode::plane(
                "spectrum",
                "Amplitude Spectrum",
                "|b\u{2099}| against n \u{2014} the recipe of the waveform",
                1.6,
            )
            .look_at(2.9, 0.9, 0.0),
            RenderMode::plane(
                "gibbs",
                "Gibbs Lens",
                "Overshoot at the jump refuses to die: \u{2248}8.95% forever",
                0.75,
            )
            .look_at(0.55, 0.85, 0.0),
            RenderMode::plane(
                "lissajous",
                "Lissajous Chamber",
                "x and y each hum one frequency; their ratio draws the figure",
                1.6,
            ),
            RenderMode::plane(
                "drawing",
                "The Drawing Machine",
                "Rotating circles chained tip-to-tip redraw a closed figure; this heart takes eight",
                1.7,
            ),
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
        rend.register_field(gpu, "fourier.paper", PAPER, false);
        self.build_draw_coefs();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, _dt: f64) {}

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        scene.field = Some("fourier.paper");
        let wave = self.params.c("wave");
        let n_max = self.params.i("n") as usize;
        let omega = 1.1;
        let th = t * omega;

        match mode {
            0 => {
                // Epicycle chain centered at (-2.2, 0); trace drawn to the right.
                let c0 = Vec3::new(-2.2, 0.0, 0.0);
                let mut tip = c0;
                let mut idx = 0usize;
                for n in 1..=n_max {
                    let b = coeff(wave, n);
                    if b.abs() < 1e-9 {
                        continue;
                    }
                    let r = b.abs() as f32;
                    let ang = (n as f64 * th + phase(wave)) * b.signum();
                    if self.params.b("circles") {
                        let ring: Vec<Vec3> = (0..=48)
                            .map(|i| {
                                let a = i as f32 / 48.0 * std::f32::consts::TAU;
                                tip + Vec3::new(a.cos() * r, a.sin() * r, 0.0)
                            })
                            .collect();
                        scene.polyline(LineLayer::World, &ring, 1.0, with_alpha(TEAL, 0.28));
                    }
                    let next = tip
                        + Vec3::new(
                            (ang.cos() * r as f64) as f32,
                            (ang.sin() * r as f64) as f32,
                            0.0,
                        );
                    let hue = phase_color(idx as f32 * 0.9);
                    scene.line(LineLayer::WorldAdd, tip, next, 2.0, with_alpha(hue, 0.85));
                    scene.point(next, 2.2, hue, 1.2);
                    tip = next;
                    idx += 1;
                }
                // The woven trace: y(t - delay) plotted rightward.
                let x0 = 0.6f32;
                let width = 5.6f32;
                let window = 10.0;
                let pts: Vec<Vec3> = (0..=440)
                    .map(|i| {
                        let back = window * i as f64 / 440.0;
                        let y = partial_sum(wave, n_max, (t - back) * omega);
                        Vec3::new(x0 + width * (back / window) as f32, y as f32, 0.0)
                    })
                    .collect();
                scene.glow_polyline(&pts, 2.2, ORANGE, 2.8);
                // Thread from the chain tip to the trace head.
                scene.line_soft(
                    LineLayer::Overlay,
                    tip,
                    Vec3::new(x0, tip.y, 0.0),
                    1.2,
                    with_alpha([0.9, 0.9, 0.95, 1.0], 0.25),
                );
                scene.line(
                    LineLayer::World,
                    Vec3::new(x0, -2.6, 0.0),
                    Vec3::new(x0, 2.6, 0.0),
                    1.0,
                    with_alpha(GHOST, 0.4),
                );
                scene.point(Vec3::new(x0, tip.y, 0.0), 4.0, ORANGE, 2.0);
            }
            1 => {
                // Stacked harmonics, offset vertically; sum at top.
                let x0 = 0.0f32;
                let width = 6.2f32;
                let shown: Vec<usize> = (1..=n_max)
                    .filter(|&n| coeff(wave, n).abs() > 1e-9)
                    .take(9)
                    .collect();
                let rows = shown.len().max(1);
                for (row, &n) in shown.iter().enumerate() {
                    let y_off = -2.9 + 4.4 * (rows - 1 - row) as f32 / rows.max(2) as f32;
                    let hue = phase_color(row as f32 * 0.9);
                    let pts: Vec<Vec3> = (0..=300)
                        .map(|i| {
                            let x = std::f64::consts::TAU * 2.0 * i as f64 / 300.0;
                            let y = term(wave, n, x + th);
                            Vec3::new(
                                x0 + width * i as f32 / 300.0,
                                y_off + (y * 0.42) as f32,
                                0.0,
                            )
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &pts, 1.4, with_alpha(hue, 0.55));
                }
                // The sum, raised and bright.
                let pts: Vec<Vec3> = (0..=440)
                    .map(|i| {
                        let x = std::f64::consts::TAU * 2.0 * i as f64 / 440.0;
                        let y = partial_sum(wave, n_max, x + th);
                        Vec3::new(x0 + width * i as f32 / 440.0, 2.35 + (y * 0.55) as f32, 0.0)
                    })
                    .collect();
                if self.params.b("ghost") {
                    let gp: Vec<Vec3> = (0..=440)
                        .map(|i| {
                            let x = std::f64::consts::TAU * 2.0 * i as f64 / 440.0;
                            Vec3::new(
                                x0 + width * i as f32 / 440.0,
                                2.35 + (target(wave, x + th) * 0.55) as f32,
                                0.0,
                            )
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &gp, 1.0, GHOST);
                }
                scene.glow_polyline(&pts, 2.2, ORANGE, 2.6);
            }
            2 => {
                // The full 32-slot rack; harmonics beyond N stand as ghosts.
                let bar_w = 5.4 / 32.0;
                for n in 1..=32 {
                    let b = coeff(wave, n);
                    let x = 0.2 + (n - 1) as f32 * bar_w * 1.05;
                    let h = (b.abs() * 1.6) as f32;
                    let hue = phase_color((n as f32) * 0.35);
                    let included = n <= n_max;
                    let live = ((n as f64 * th).sin() * 0.5 + 0.5) as f32;
                    let alpha = if included { 0.40 + 0.55 * live } else { 0.10 };
                    scene.line(
                        LineLayer::World,
                        Vec3::new(x, 0.0, 0.0),
                        Vec3::new(x, h.max(0.006), 0.0),
                        (bar_w * 190.0).max(3.0),
                        with_alpha(hue, alpha),
                    );
                    if included && b.abs() > 1e-9 {
                        scene.point(Vec3::new(x, h, 0.0), 2.2, hue, 1.6);
                    }
                }
                // The 1/n envelope of the square wave family.
                let env: Vec<Vec3> = (1..=64)
                    .map(|i| {
                        let nf = 1.0 + 31.0 * i as f64 / 64.0;
                        let x = 0.2 + (nf as f32 - 1.0) * bar_w * 1.05;
                        Vec3::new(x, (4.0 / (std::f64::consts::PI * nf) * 1.6) as f32, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::WorldAdd, &env, 1.2, with_alpha(ORANGE, 0.35));
                scene.line(
                    LineLayer::World,
                    Vec3::new(-0.1, 0.0, 0.0),
                    Vec3::new(6.0, 0.0, 0.0),
                    1.2,
                    GHOST,
                );
            }
            4 => {
                // Lissajous: integer ratio locks the figure; the phase drifts.
                let (fa, fb) = (3.0f64, 4.0f64);
                let phase = t * 0.35;
                let pts: Vec<Vec3> = (0..=1400)
                    .map(|i| {
                        let s = i as f64 / 1400.0 * std::f64::consts::TAU;
                        Vec3::new(
                            ((fa * s + phase).sin() * 1.25) as f32,
                            ((fb * s).sin() * 1.25) as f32,
                            0.0,
                        )
                    })
                    .collect();
                for i in 1..pts.len() {
                    let hue = phase_color(i as f32 / pts.len() as f32 * std::f32::consts::TAU);
                    scene.line(
                        LineLayer::WorldAdd,
                        pts[i - 1],
                        pts[i],
                        1.5,
                        with_alpha(hue, 0.30),
                    );
                }
                // The two generating oscillators, engraved on the margins.
                let px = ((fa * phase * 2.0 + phase).sin() * 1.25) as f32;
                let py = ((fb * phase * 2.0).sin() * 1.25) as f32;
                scene.point(Vec3::new(px, -1.5, 0.0), 3.0, with_alpha(TEAL, 0.9), 1.4);
                scene.point(Vec3::new(-1.5, py, 0.0), 3.0, with_alpha(ORANGE, 0.9), 1.4);
                scene.point(Vec3::new(px, py, 0.01), 4.5, [0.95, 0.97, 1.0, 1.0], 2.2);
            }
            5 => {
                let tau = (t / 12.0) % 1.0;
                // The target, engraved faintly beneath the reproduction.
                let ghost: Vec<Vec3> = (0..=200)
                    .map(|i| {
                        let (x, y) = Self::figure(i as f64 / 200.0);
                        Vec3::new(x as f32, y as f32, -0.01)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &ghost, 1.0, with_alpha(GHOST, 0.5));
                // The traced figure so far.
                let trace: Vec<Vec3> = (0..=((tau * 420.0) as usize).max(2))
                    .map(|i| {
                        let s = tau * i as f64 / (tau * 420.0).max(2.0);
                        let (mut x, mut y) = (0.0, 0.0);
                        for &(k, amp, ph) in &self.draw_coefs {
                            let a = std::f64::consts::TAU * k as f64 * s + ph;
                            x += amp * a.cos();
                            y += amp * a.sin();
                        }
                        Vec3::new(x as f32, y as f32, 0.01)
                    })
                    .collect();
                scene.glow_polyline(&trace, 2.2, ORANGE, 2.6);
                // The chain of epicycles at the pen's current instant.
                let (mut cx, mut cy) = (0.0f64, 0.0f64);
                for (ci, &(k, amp, ph)) in self.draw_coefs.iter().enumerate() {
                    if ci == 0 && k == 0 {
                        cx += amp * ph.cos() * 0.0; // DC handled like the rest below
                    }
                    let a = std::f64::consts::TAU * k as f64 * tau + ph;
                    let (nx, ny) = (cx + amp * a.cos(), cy + amp * a.sin());
                    if self.params.b("circles") && amp > 0.012 && k != 0 {
                        let ring: Vec<Vec3> = (0..=36)
                            .map(|i| {
                                let b = i as f64 / 36.0 * std::f64::consts::TAU;
                                Vec3::new(
                                    (cx + amp * b.cos()) as f32,
                                    (cy + amp * b.sin()) as f32,
                                    0.0,
                                )
                            })
                            .collect();
                        scene.polyline(LineLayer::World, &ring, 0.9, with_alpha(TEAL, 0.34));
                    }
                    if k != 0 {
                        scene.line(
                            LineLayer::WorldAdd,
                            Vec3::new(cx as f32, cy as f32, 0.0),
                            Vec3::new(nx as f32, ny as f32, 0.0),
                            1.4,
                            with_alpha(phase_color(ci as f32 * 0.5), 0.7),
                        );
                    }
                    cx = nx;
                    cy = ny;
                }
                scene.point(Vec3::new(cx as f32, cy as f32, 0.02), 4.5, ORANGE, 2.2);
            }
            _ => {
                // Gibbs: zoom at the square-like jump; N grows with time.
                let n_anim = 1 + ((t / 12.0).clamp(0.0, 1.0) * 60.0) as usize;
                let pts: Vec<Vec3> = (0..=700)
                    .map(|i| {
                        let x = -0.8 + 2.6 * i as f64 / 700.0;
                        let y = partial_sum(wave, n_anim, x);
                        Vec3::new(x as f32, y as f32, 0.0)
                    })
                    .collect();
                scene.glow_polyline(&pts, 1.8, ORANGE, 2.4);
                let gp: Vec<Vec3> = (0..=200)
                    .map(|i| {
                        let x = -0.8 + 2.6 * i as f64 / 200.0;
                        Vec3::new(x as f32, target(wave, x) as f32, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &gp, 1.0, GHOST);
                // The Wilbraham-Gibbs ceiling for the square wave: (2/pi) Si(pi).
                if wave == 0 {
                    let g = GIBBS_PEAK as f32;
                    scene.line(
                        LineLayer::Overlay,
                        Vec3::new(-0.8, g, 0.0),
                        Vec3::new(1.8, g, 0.0),
                        1.0,
                        with_alpha([0.95, 0.4, 0.4, 1.0], 0.5),
                    );
                }
            }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let wave = self.params.c("wave");
        let n = self.params.i("n");
        vec![
            title("Fourier series"),
            formula(WAVE_SERIES[wave]),
            if wave == 3 {
                formula("f(x) = \u{03A3}\u{2099} a\u{2099} cos(nx),   a\u{2099} = (1/\u{03C0}) \u{222B} f cos(nx) dx   (an even waveform)")
            } else {
                ftex("fourier.series", "f(x) = \u{03A3}\u{2099} b\u{2099} sin(nx),   b\u{2099} = (1/\u{03C0}) \u{222B} f sin(nx) dx")
            },
            value(format!("N = {n} harmonics    \u{03C9}t = {:.2}", t * 1.1)),
            value(format!(
                "{c}\u{2081} = {:.4}   {c}\u{2082} = {:.4}   {c}\u{2083} = {:.4}",
                coeff(wave, 1),
                coeff(wave, 2),
                coeff(wave, 3),
                c = if wave == 3 { "a" } else { "b" },
            )),
            note("At a jump the partial sums overshoot by \u{2248}8.95% of the step \u{2014} the Gibbs phenomenon \u{2014} no matter how many terms are taken."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Partial sum with exact closed-form coefficients; the chain is the sum, term by term."),
            3 => Some("N grows with the timeline; the overshoot height converges to the Wilbraham\u{2013}Gibbs constant."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{PI, TAU};

    /// Fourier coefficient of `target` against the wave's own basis.
    fn numeric_coefficient(wave: usize, n: usize) -> f64 {
        let steps = 200_000;
        let h = TAU / steps as f64;
        let sum: f64 = (0..steps)
            .map(|i| {
                let x = (i as f64 + 0.5) * h;
                target(wave, x) * (n as f64 * x + phase(wave)).sin()
            })
            .sum();
        sum * h / PI
    }

    #[test]
    fn stated_coefficients_are_the_coefficients_of_the_drawn_waveform() {
        for (wave, name) in WAVE_NAMES.iter().enumerate() {
            for n in 1..=12 {
                let (stated, numeric) = (coeff(wave, n), numeric_coefficient(wave, n));
                assert!(
                    (stated - numeric).abs() < 1e-4,
                    "{name} n = {n}: stated {stated}, numeric {numeric}"
                );
            }
        }
    }

    #[test]
    fn partial_sums_converge_to_the_waveform_away_from_jumps() {
        for (wave, x) in [(0, 1.0), (1, 1.0), (2, 1.0), (3, 0.1), (3, 2.0)] {
            let (sum, want) = (partial_sum(wave, 4001, x), target(wave, x));
            assert!(
                (sum - want).abs() < 2e-3,
                "{}: {sum} vs {want}",
                WAVE_NAMES[wave]
            );
        }
    }

    #[test]
    fn the_gibbs_line_sits_on_the_first_peak() {
        let n = 2001;
        let peak = (1..400)
            .map(|i| partial_sum(0, n, i as f64 * 0.2 * PI / n as f64))
            .fold(f64::MIN, f64::max);
        assert!((peak - GIBBS_PEAK).abs() < 1e-3, "peak {peak}");
        assert!(((GIBBS_PEAK - 1.0) / 2.0 - 0.0895).abs() < 1e-3);
    }

    #[test]
    fn the_heart_is_exactly_eight_circles() {
        let mut f = FourierAtelier::new();
        f.build_draw_coefs();
        let circles = f.draw_coefs.iter().filter(|c| c.0 != 0).count();
        assert_eq!(circles, HEART_CIRCLES, "{:?}", f.draw_coefs);
    }
}
