//! Approximation: Taylor's telescope, Newton's fractal basins, and the
//! local linearity that makes calculus possible at all.

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

const SKY: [f32; 4] = [0.45, 0.75, 1.0, 1.0];
const FLAME: [f32; 4] = [1.0, 0.62, 0.30, 1.0];
const GHOST_T: [f32; 4] = [0.62, 0.66, 0.75, 0.40];

const FN_NAMES: [&str; 4] = [
    "sin x",
    "e^x \u{00B7} \u{00BD}",
    "ln(1+x)",
    "1/(1+x\u{00B2})",
];

/// Where the Microscope actually looks. The slider reaches x = -1.5, but ln(1 + x)
/// has no real value at or below x = -1, so its center stops short of that.
fn microscope_center(k: usize, x: f64) -> f64 {
    if k == 2 {
        x.max(-0.9)
    } else {
        x
    }
}

fn f_true(k: usize, x: f64) -> f64 {
    match k {
        0 => x.sin(),
        1 => x.exp() * 0.5,
        2 => (1.0 + x).ln(),
        _ => 1.0 / (1.0 + x * x),
    }
}

/// Taylor coefficient a_n about 0.
fn coeff(k: usize, n: usize) -> f64 {
    match k {
        0 => {
            // sin: 0,1,0,-1/3!,...
            if n.is_multiple_of(2) {
                0.0
            } else {
                let sign = if (n / 2).is_multiple_of(2) { 1.0 } else { -1.0 };
                sign / fact(n)
            }
        }
        1 => 0.5 / fact(n),
        2 => {
            if n == 0 {
                0.0
            } else {
                let sign = if n % 2 == 1 { 1.0 } else { -1.0 };
                sign / n as f64
            }
        }
        _ => {
            // 1/(1+x^2) = sum (-1)^m x^(2m)
            if n % 2 == 1 {
                0.0
            } else {
                if (n / 2).is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }
}

fn fact(n: usize) -> f64 {
    (1..=n).map(|v| v as f64).product::<f64>().max(1.0)
}

fn taylor_eval(k: usize, deg: usize, x: f64) -> f64 {
    let mut acc = 0.0;
    let mut xp = 1.0;
    for n in 0..=deg {
        acc += coeff(k, n) * xp;
        xp *= x;
        if acc.abs() > 1e6 {
            break;
        }
    }
    acc
}

/// Newton's method basins for z^3 - 1, colored by root and shaded by speed.
const NEWTON_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    var z = plane_pos(ndc);
    var it = 0.0;
    for (var i = 0; i < 42; i++) {
        // z <- z - (z^3-1)/(3z^2)
        let z2 = cmul(z, z);
        let z3 = cmul(z2, z);
        let num = z3 - vec2<f32>(1.0, 0.0);
        let den = 3.0 * z2;
        let step = cdiv(num, den);
        z = z - step;
        it = it + 1.0;
        if (dot(step, step) < 1e-10) { break; }
    }
    // Which cube root of unity?
    let ang = atan2(z.y, z.x);
    var color = phase_color(ang * 1.0 + 0.6);
    let speed = 1.0 - clamp(it / 42.0, 0.0, 1.0);
    color = color * (0.18 + 0.82 * pow(speed, 1.4));
    return vec4<f32>(color * (1.0 - 0.20 * dot(ndc, ndc)), 1.0);
}
"#;

const PAPER_T: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var color = vec3<f32>(0.0);
    color += vec3<f32>(0.10, 0.18, 0.24) * grid_glow(w, 0.7) * 0.12;
    return vec4<f32>(color * (1.0 - 0.28 * dot(ndc, ndc)), 1.0);
}
"#;

pub struct ApproximationAtelier {
    params: Params,
    chaos_pts: Vec<Vec3>,
    chaos_p: glam::Vec2,
    rng: u64,
    last_fractal: usize,
}

impl ApproximationAtelier {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("fn", "Function", "Symbolic", FN_NAMES.to_vec(), 0);
        p.int("deg", "Degree N", "Physics", 0, 24, 7);
        p.toggle("grow", "Grow N with time", "Observer", true);
        p.slider(
            "zoomx",
            "Microscope center x",
            "Observer",
            -1.5,
            1.5,
            0.6,
            "",
        );
        p.choice(
            "fractal",
            "Chaos-game target",
            "Symbolic",
            vec!["Sierpi\u{0144}ski", "Barnsley fern"],
            0,
        );
        Self {
            params: p,
            chaos_pts: Vec::new(),
            chaos_p: glam::Vec2::new(0.2, 0.3),
            rng: 0xC0FFEE123456789,
            last_fractal: usize::MAX,
        }
    }

    fn rand(&mut self) -> f64 {
        self.rng = self
            .rng
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.rng >> 11) as f64 / (1u64 << 53) as f64
    }

    fn deg_now(&self, t: f64) -> usize {
        if self.params.b("grow") {
            ((t / 12.0).clamp(0.0, 1.0) * 24.0) as usize
        } else {
            self.params.i("deg") as usize
        }
    }

    fn restart_chaos(&mut self) {
        self.chaos_pts.clear();
        self.chaos_p = glam::Vec2::new(0.2, 0.3);
    }
}

impl ConceptModule for ApproximationAtelier {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "analysis.taylor",
            name: "The Approximation Atelier",
            tagline: "Polynomials impersonating the transcendental",
            branch: Branch::Analysis,
            description: "Calculus runs on a single act of faith: up close, everything smooth is a line, and with more derivatives, a polynomial. Watch Taylor sums reach out from the origin \u{2014} and hit the invisible wall of the convergence radius where the function's complex singularities stand guard. Newton's root-finder, iterated, shatters the plane into fractal basins.",
            epigraph: Some(Epigraph::quoted(
"To divide each of the difficulties under examination into as many parts as possible, and as might be necessary for its adequate solution.",
"Ren\u{00E9} Descartes, Discourse on the Method, tr. Veitch",
)),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane("taylor", "Taylor Stage", "Partial sums reaching out from a = 0; the radius of convergence bites", 2.6)
                .look_at(0.0, 0.3, 0.0),
            RenderMode::plane("newton", "Newton Fractal", "Basins of z\u{00B3} = 1 under Newton iteration; the boundary is the Julia set of the Newton map", 1.9),
            RenderMode::plane("linear", "The Microscope", "Zoom into any smooth curve and find its tangent line waiting", 2.2)
                .look_at(0.0, 0.3, 0.0),
            RenderMode::plane("fresnel", "The Euler Spiral", "\u{222B}cos(t\u{00B2}), \u{222B}sin(t\u{00B2}): curvature growing linearly, coiling to a point", 1.15)
                .look_at(0.0, 0.0, 0.0),
            RenderMode::plane("chaosgame", "The Chaos Game", "Random dice, deterministic ghost: the attractor of an IFS", 1.5)
                .look_at(0.0, 1.2, 0.0),
            RenderMode::plane("hilbert", "The Hilbert Curve", "One line visiting every point of the square, order by order", 1.35),
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
        rend.register_field(gpu, "taylor.newton", NEWTON_SHADE, false);
        rend.register_field(gpu, "taylor.paper", PAPER_T, false);
    }

    fn rewind(&mut self) {
        self.restart_chaos();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, _dt: f64) {
        let kind = self.params.c("fractal");
        if kind != self.last_fractal {
            self.last_fractal = kind;
            self.restart_chaos();
        }
        // Feed the chaos game.
        for _ in 0..600 {
            if self.chaos_pts.len() >= 46000 {
                break;
            }
            let p = self.chaos_p;
            self.chaos_p = if kind == 0 {
                let verts = [
                    glam::Vec2::new(-1.3, 0.0),
                    glam::Vec2::new(1.3, 0.0),
                    glam::Vec2::new(0.0, 2.2),
                ];
                let v = verts[(self.rand() * 3.0) as usize % 3];
                (p + v) * 0.5
            } else {
                let r = self.rand();
                if r < 0.01 {
                    glam::Vec2::new(0.0, 0.16 * p.y)
                } else if r < 0.86 {
                    glam::Vec2::new(0.85 * p.x + 0.04 * p.y, -0.04 * p.x + 0.85 * p.y + 1.6)
                } else if r < 0.93 {
                    glam::Vec2::new(0.20 * p.x - 0.26 * p.y, 0.23 * p.x + 0.22 * p.y + 1.6)
                } else {
                    glam::Vec2::new(-0.15 * p.x + 0.28 * p.y, 0.26 * p.x + 0.24 * p.y + 0.44)
                }
            };
            let q = chaos_on_screen(kind, self.chaos_p);
            self.chaos_pts.push(Vec3::new(q.x, q.y, 0.0));
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        let k = self.params.c("fn");
        match mode {
            0 => {
                scene.field = Some("taylor.paper");
                scene.line(
                    LineLayer::World,
                    Vec3::new(-4.4, 0.0, 0.0),
                    Vec3::new(4.4, 0.0, 0.0),
                    1.1,
                    [0.5, 0.5, 0.55, 0.4],
                );
                // The function, true and patient.
                let truth: Vec<Vec3> = (0..=440)
                    .filter_map(|i| {
                        let x = -4.2 + 8.4 * i as f64 / 440.0;
                        if k == 2 && x <= -0.99 {
                            return None;
                        }
                        let y = f_true(k, x);
                        (y.abs() < 6.0).then(|| Vec3::new(x as f32, y as f32, 0.0))
                    })
                    .collect();
                scene.polyline(LineLayer::World, &truth, 1.3, GHOST_T);
                // Ghosts of earlier partial sums, then the current one bright.
                let deg = self.deg_now(t);
                for (d, alpha, w) in [
                    (deg.saturating_sub(2), 0.15f32, 1.2f32),
                    (deg.saturating_sub(1), 0.25, 1.4),
                ] {
                    if d < deg {
                        let pts = taylor_pts(k, d);
                        scene.polyline(LineLayer::World, &pts, w, with_alpha(SKY, alpha));
                    }
                }
                let pts = taylor_pts(k, deg);
                scene.glow_polyline(&pts, 2.2, FLAME, 2.6);
                // Radius of convergence, where finite.
                let radius = match k {
                    2 | 3 => Some(1.0f32),
                    _ => None,
                };
                if let Some(r) = radius {
                    for s in [-r, r] {
                        scene.line(
                            LineLayer::Overlay,
                            Vec3::new(s, -2.4, 0.0),
                            Vec3::new(s, 2.4, 0.0),
                            1.1,
                            [0.95, 0.4, 0.4, 0.4],
                        );
                    }
                }
                scene.point(Vec3::ZERO, 3.4, FLAME, 1.8);
            }
            1 => {
                scene.field = Some("taylor.newton");
                // The three roots of unity, engraved.
                for i in 0..3 {
                    let a = i as f32 / 3.0 * std::f32::consts::TAU;
                    scene.point(
                        Vec3::new(a.cos(), a.sin(), 0.02),
                        4.0,
                        with_alpha([1.0, 1.0, 1.0, 1.0], 0.9),
                        1.8,
                    );
                }
            }
            3 => {
                scene.field = Some("taylor.paper");
                // Fresnel integrals traced live: the clothoid of every highway.
                let reach = 0.5 + (t / 12.0).clamp(0.0, 1.0) * 9.0;
                let n = 2400;
                let (mut x, mut y) = (0.0f64, 0.0f64);
                let mut pts = vec![Vec3::new(-0.6267 * 1.6, -0.6267 * 1.6, 0.0)];
                let ds = reach / n as f64;
                for i in 0..n {
                    let s = i as f64 * ds;
                    x += (s * s).cos() * ds;
                    y += (s * s).sin() * ds;
                    pts.push(Vec3::new(
                        (x - 0.6267) as f32 * 1.6,
                        (y - 0.6267) as f32 * 1.6,
                        0.0,
                    ));
                }
                // Recentered on the limit point sqrt(pi/8)(1,1) ~ 0.6267.
                for i in 1..pts.len() {
                    let a = i as f32 / pts.len() as f32;
                    scene.line(
                        LineLayer::WorldAdd,
                        pts[i - 1],
                        pts[i],
                        1.6,
                        with_alpha(phase_color(a * 4.0), 0.20 + 0.45 * a),
                    );
                }
                scene.point(Vec3::ZERO, 3.0, with_alpha(FLAME, 0.9), 1.6);
                if let Some(&last) = pts.last() {
                    scene.point(last, 4.2, [0.95, 0.97, 1.0, 1.0], 2.0);
                }
            }
            4 => {
                let kind = self.params.c("fractal");
                let col = if kind == 0 {
                    [0.95f32, 0.72, 0.30, 0.5]
                } else {
                    [0.35, 0.88, 0.45, 0.5]
                };
                for p in &self.chaos_pts {
                    scene.point(*p, 0.75, col, 0.3);
                }
                let head = chaos_on_screen(kind, self.chaos_p);
                scene.point(
                    Vec3::new(head.x, head.y, 0.02),
                    4.0,
                    [0.95, 0.97, 1.0, 1.0],
                    2.0,
                );
            }
            5 => {
                scene.field = Some("taylor.paper");
                // Order grows with the loop: 1..6.
                let order = 1 + ((t / 12.0).clamp(0.0, 0.999) * 6.0) as u32;
                let n = 1u32 << order;
                let total = n * n;
                let mut pts = Vec::with_capacity(total as usize);
                for d in 0..total {
                    // Standard d2xy bit transform.
                    let (mut x, mut y) = (0u32, 0u32);
                    let mut td = d;
                    let mut s = 1u32;
                    while s < n {
                        let rx = 1 & (td / 2);
                        let ry = 1 & (td ^ rx);
                        if ry == 0 {
                            if rx == 1 {
                                x = s - 1 - x;
                                y = s - 1 - y;
                            }
                            std::mem::swap(&mut x, &mut y);
                        }
                        x += s * rx;
                        y += s * ry;
                        td /= 4;
                        s *= 2;
                    }
                    pts.push(Vec3::new(
                        (x as f32 / (n - 1).max(1) as f32 - 0.5) * 2.2,
                        (y as f32 / (n - 1).max(1) as f32 - 0.5) * 2.2,
                        0.0,
                    ));
                }
                for i in 1..pts.len() {
                    let hue = phase_color(i as f32 / pts.len() as f32 * 5.0);
                    scene.line(
                        LineLayer::WorldAdd,
                        pts[i - 1],
                        pts[i],
                        1.4,
                        with_alpha(hue, 0.55),
                    );
                }
            }
            _ => {
                scene.field = Some("taylor.paper");
                let x0 = microscope_center(k, self.params.f("zoomx"));
                // Breathing zoom driven by the loop.
                let breathe = 0.5 - 0.5 * ((t / 12.0) * std::f64::consts::TAU).cos();
                let span = 3.0 * (0.04f64 / 3.0).powf(breathe);
                let y0 = f_true(k, x0);
                let d = (f_true(k, x0 + 1e-6) - f_true(k, x0 - 1e-6)) / 2e-6;
                // Window-normalized rendering: map [x0-span, x0+span] to view.
                let n = 360;
                let curve: Vec<Vec3> = (0..=n)
                    .filter_map(|i| {
                        let x = x0 - span + 2.0 * span * i as f64 / n as f64;
                        // ln(1 + x) has no real value at x <= -1: leave a gap.
                        let y = f_true(k, x);
                        y.is_finite().then(|| {
                            Vec3::new(
                                ((x - x0) / span * 3.0) as f32,
                                ((y - y0) / span * 3.0) as f32,
                                0.0,
                            )
                        })
                    })
                    .collect();
                scene.glow_polyline(&curve, 2.2, SKY, 2.6);
                // The tangent line in the same window.
                let tangent: Vec<Vec3> = [-1.0f64, 1.0]
                    .iter()
                    .map(|&s| Vec3::new((s * 3.2) as f32, (d * s * 3.2) as f32, 0.0))
                    .collect();
                scene.polyline(LineLayer::World, &tangent, 1.2, with_alpha(FLAME, 0.75));
                scene.point(Vec3::ZERO, 4.0, FLAME, 2.0);
            }
        }
    }

    fn inspector(&self, t: f64) -> Vec<InsLine> {
        let k = self.params.c("fn");
        let deg = self.deg_now(t);
        let x0 = microscope_center(k, self.params.f("zoomx"));
        let d = (f_true(k, x0 + 1e-6) - f_true(k, x0 - 1e-6)) / 2e-6;
        let mut v = vec![
            title(format!("f(x) = {}", FN_NAMES[k])),
            ftex(
                "taylor.series",
                "f(x) \u{2248} \u{03A3} f\u{207D}\u{1D4F}\u{207E}(0)/k! \u{00B7} x\u{1D4F}",
            ),
            ftex(
                "taylor.newton",
                "z\u{2099}\u{208A}\u{2081} = z\u{2099} \u{2212} f(z\u{2099})/f'(z\u{2099})",
            ),
            value(format!("degree N = {deg}    f'({x0:.2}) = {d:.4}")),
        ];
        match k {
            2 => v.push(note("Radius of convergence 1: the singularity at x = \u{2212}1 is visible. Beyond +1 the series fails even where ln(1+x) is perfectly healthy \u{2014} the complex plane casts the veto.")),
            3 => v.push(note("Runge's ghost: 1/(1+x\u{00B2}) is smooth on all of \u{211D}, yet the series dies at |x| = 1 \u{2014} the poles at \u{00B1}i decide, invisibly from the real line.")),
            _ => v.push(note("Entire function: the series converges everywhere, and each new term extends the polynomial's reach.")),
        }
        v.push(note("Newton fractal: three basins meet not at lines but at a fractal common boundary \u{2014} every boundary point touches all three."));
        v
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Closed-form coefficients; sums clipped at |y| = 6 for display."),
            1 => Some("42 Newton iterations per pixel in fp32."),
            2 => Some("Central difference for f' (h = 10\u{207B}\u{2076}); zoom is genuine rescaling, not smoothing."),
            3 => Some("Fresnel integrals by a left Riemann sum of 2400 steps along the curve."),
            4 => Some("Random iterates of the affine maps, up to 46,000 points; the attractor itself is not computed."),
            5 => Some("Exact integer recursion, up to order 6; a finite order fills the square only approximately."),
            _ => None,
        }
    }
}

/// Where a chaos-game iterate is drawn: the fern is scaled down to fit the view.
fn chaos_on_screen(kind: usize, p: glam::Vec2) -> glam::Vec2 {
    if kind == 0 {
        p
    } else {
        glam::Vec2::new(p.x * 0.45, p.y * 0.45 - 0.1)
    }
}

fn taylor_pts(k: usize, deg: usize) -> Vec<Vec3> {
    (0..=440)
        .filter_map(|i| {
            let x = -4.2 + 8.4 * i as f64 / 440.0;
            let y = taylor_eval(k, deg, x);
            (y.abs() < 6.0).then(|| Vec3::new(x as f32, y as f32, 0.0))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_microscope_has_a_curve_and_a_slope_anywhere_on_its_slider() {
        for (k, name) in FN_NAMES.iter().enumerate() {
            for i in 0..=30 {
                let slider = -1.5 + 0.1 * i as f64;
                let x0 = microscope_center(k, slider);
                let (lo, hi) = (f_true(k, x0 - 1e-6), f_true(k, x0 + 1e-6));
                assert!(f_true(k, x0).is_finite(), "{name} at {slider}");
                assert!(
                    ((hi - lo) / 2e-6).is_finite(),
                    "{} at {slider}",
                    FN_NAMES[k]
                );
            }
        }
    }

    #[test]
    fn the_center_only_moves_when_the_function_has_no_value_there() {
        assert_eq!(microscope_center(0, -1.5), -1.5);
        assert_eq!(microscope_center(2, 0.5), 0.5);
        assert_eq!(microscope_center(2, -1.5), -0.9);
    }
}
