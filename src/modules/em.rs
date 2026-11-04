//! Electromagnetism: charges you can hold, the fields they cast,
//! and the light an oscillating dipole throws off.

use glam::{Vec2, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::with_alpha;
use crate::theme::Branch;

const CYAN: [f32; 4] = [0.35, 0.80, 1.0, 1.0];
const MAGENTA: [f32; 4] = [1.0, 0.45, 0.75, 1.0];
const GOLDE: [f32; 4] = [0.95, 0.78, 0.35, 1.0];

/// Potential heat + equipotential bands for up to 6 charges in U.p[2..8].
const POTENTIAL_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    var phi = 0.0;
    let n = i32(U.p[0].x + 0.5);
    for (var i = 0; i < n; i++) {
        let cx = U.p[2 + i].x;
        let cy = U.p[2 + i].y;
        let q = U.p[2 + i].z;
        let r = max(length(w - vec2<f32>(cx, cy)), 0.045);
        phi += q / r;
    }
    var color = vec3<f32>(0.0);
    let m = clamp(abs(phi) / (2.5 + abs(phi)), 0.0, 1.0);
    if (phi > 0.0) {
        color = vec3<f32>(0.85, 0.30, 0.45) * m * m * 0.62;
    } else {
        color = vec3<f32>(0.15, 0.45, 0.90) * m * m * 0.62;
    }
    // Equipotential bands.
    let band = abs(fract(phi * 1.4) - 0.5) * 2.0;
    color *= 0.72 + 0.28 * smoothstep(0.0, 0.35, band);
    color += vec3<f32>(0.10, 0.16, 0.20) * grid_glow(w, 0.7) * 0.10;
    return vec4<f32>(color * (1.0 - 0.25 * dot(ndc, ndc)), 1.0);
}
"#;

/// Hertzian dipole radiation: level bands of the exact field-line function
/// psi = sin^2(theta) [cos(kr - wt) - sin(kr - wt)/(kr)].
const DIPOLE_SHADE: &str = r#"
fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let w = plane_pos(ndc);
    let t = U.res.z;
    let kk = U.p[0].y;
    let r = max(length(w), 0.05);
    let s2 = w.x * w.x / max(dot(w, w), 1e-6);
    let ph = kk * r - 2.2 * t;
    let psi = s2 * (cos(ph) - sin(ph) / (kk * r));
    var color = vec3<f32>(0.0);
    // Field lines: glowing near integer levels of psi.
    let lv = psi * 3.0;
    let d = abs(fract(lv) - 0.5) * 2.0;
    let line = exp(-(1.0 - d) * (1.0 - d) * 34.0);
    let fade = 1.0 / (1.0 + r * 0.35);
    let sgn = clamp(psi * 2.0, -1.0, 1.0);
    let warm = vec3<f32>(0.95, 0.62, 0.25) * max(sgn, 0.0);
    let cool = vec3<f32>(0.25, 0.55, 0.95) * max(-sgn, 0.0);
    color += (warm + cool + vec3<f32>(0.20)) * line * fade * 1.4;
    // The near-zone glow around the dipole itself.
    color += vec3<f32>(0.9, 0.75, 0.4) * exp(-r * r * 6.0) * (0.5 + 0.5 * sin(2.2 * t));
    return vec4<f32>(color * (1.0 - 0.22 * dot(ndc, ndc)), 1.0);
}
"#;

pub struct FieldForge {
    params: Params,
    charges: Vec<(Vec2, f32)>,
    drag_idx: Option<usize>,
    /// Cyclotron dancers: (pos, vel, charge sign, trail).
    cyc: Vec<(Vec2, Vec2, f32, Vec<Vec2>)>,
}

/// Uniform fields of the cyclotron view: B out of the plane, E along +x.
const CYC_B: f32 = 1.4;
const CYC_E: f32 = 0.22;
/// Half the height of the view, plus a little. Charges drift along -y at E/B
/// forever, so once a whole orbit has passed below this it is moved to just above.
const CYC_VIEW: f32 = 3.4;

/// Where a charge may stand. Every way of placing one agrees, so what a click
/// puts on screen is what a saved scene brings back.
fn on_stage(w: Vec2) -> Vec2 {
    w.clamp(Vec2::splat(-3.4), Vec2::splat(3.4))
}

/// One Boris step of dv/dt = q(E + v x B), unit mass. The magnetic part is an exact
/// rotation, so B does no work. A plain Euler kick lengthens v at every step and
/// the charges would slowly heat up.
fn boris_step(p: Vec2, v: Vec2, q: f32, h: f32) -> (Vec2, Vec2) {
    let half_kick = Vec2::new(CYC_E, 0.0) * q * (h * 0.5);
    let minus = v + half_kick;
    let t = q * CYC_B * h * 0.5;
    let s = 2.0 * t / (1.0 + t * t);
    let prime = Vec2::new(minus.x + minus.y * t, minus.y - minus.x * t);
    let plus = Vec2::new(minus.x + prime.y * s, minus.y - prime.x * s);
    let v = plus + half_kick;
    (p + v * h, v)
}

fn seed_cyc() -> Vec<(Vec2, Vec2, f32, Vec<Vec2>)> {
    (0..10)
        .map(|i| {
            let a = i as f32 / 10.0 * std::f32::consts::TAU;
            let q = if i % 2 == 0 { 1.0 } else { -1.0 };
            (
                Vec2::new(a.cos() * 1.4, a.sin() * 1.4),
                Vec2::new(-a.sin(), a.cos()) * (0.6 + 0.13 * i as f32),
                q,
                Vec::new(),
            )
        })
        .collect()
}

impl FieldForge {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.slider("k", "Dipole wavenumber k", "Physics", 2.0, 9.0, 4.5, "");
        p.toggle("lines", "Field lines", "Display", true);
        Self {
            params: p,
            charges: vec![
                (Vec2::new(-1.1, 0.0), 1.0),
                (Vec2::new(1.1, 0.0), -1.0),
                (Vec2::new(0.0, 1.4), 1.0),
                (Vec2::new(0.0, -1.4), -1.0),
            ],
            drag_idx: None,
            cyc: seed_cyc(),
        }
    }

    fn efield(&self, p: Vec2) -> Vec2 {
        let mut e = Vec2::ZERO;
        for &(c, q) in &self.charges {
            let d = p - c;
            let r2 = d.length_squared().max(0.002);
            e += d * (q / (r2 * r2.sqrt()));
        }
        e
    }

    fn advance_charges(&mut self, h: f32) {
        for (p, v, q, trail) in &mut self.cyc {
            for _ in 0..4 {
                (*p, *v) = boris_step(*p, *v, *q, h / 4.0);
            }
            // Measured from the center of the gyration, with the drift taken out of
            // the velocity, so the charge moves only while its orbit is out of sight.
            let center_y = p.y - v.x / (*q * CYC_B);
            let radius = (*v - Vec2::new(0.0, -CYC_E / CYC_B)).length() / CYC_B;
            let edge = CYC_VIEW + radius;
            if center_y < -edge {
                p.y += 2.0 * edge;
                trail.clear();
            }
            trail.push(*p);
            if trail.len() > 260 {
                trail.remove(0);
            }
        }
    }
}

impl ConceptModule for FieldForge {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "em.fields",
            name: "The Field Forge",
            tagline: "Charge, field, and the birth of light",
            branch: Branch::Electromagnetism,
            description: "Faraday's great invention was not a device but a picture: the field line. Here charges are things you can hold and drag while their lines re-weave themselves; and when a charge is shaken \u{2014} the Hertzian dipole \u{2014} the lines pinch off and leave as ripples of light.",
            epigraph: None,
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::plane(
                "lines",
                "Charges & Field Lines",
                "Drag the four charges; the lines re-weave live",
                3.0,
            ),
            RenderMode::plane(
                "dipole",
                "Hertzian Dipole",
                "Exact field lines of an oscillating dipole: light, leaving",
                4.4,
            ),
            RenderMode::orbit(
                "wave",
                "The Plane Wave",
                "E and B, perpendicular, in phase, self-propelling",
                8.0,
                0.30,
                0.55,
            ),
            RenderMode::plane(
                "cyclotron",
                "Cyclotron Dance",
                "qv\u{00D7}B curls charges into circles; E adds the drift",
                3.2,
            ),
            RenderMode::plane(
                "caustic",
                "The Teacup Caustic",
                "Parallel light reflecting in a cup envelopes a nephroid \u{2014} check your mug",
                1.6,
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
        (mode == 0).then_some("drag the charges")
    }
    fn pointer(&mut self, _mode: usize, ev: PointerEvent) {
        match ev {
            PointerEvent::Down(w) => {
                self.drag_idx = self
                    .charges
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| {
                        (a.0 - w)
                            .length_squared()
                            .partial_cmp(&(b.0 - w).length_squared())
                            .unwrap()
                    })
                    .filter(|(_, c)| (c.0 - w).length() < 0.8)
                    .map(|(i, _)| i);
                if let Some(i) = self.drag_idx {
                    self.charges[i].0 = on_stage(w);
                }
            }
            PointerEvent::Drag(w) => {
                if let Some(i) = self.drag_idx {
                    self.charges[i].0 = on_stage(w);
                }
            }
            PointerEvent::Up => self.drag_idx = None,
        }
    }

    fn init(&mut self, gpu: &GpuContext, rend: &mut Renderers) {
        rend.register_field(gpu, "em.potential", POTENTIAL_SHADE, false);
        rend.register_field(gpu, "em.dipole", DIPOLE_SHADE, false);
    }

    fn rewind(&mut self) {
        self.cyc = seed_cyc();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, dt: f64) {
        let h = (dt.abs() as f32).min(0.033);
        if h > 0.0 {
            self.advance_charges(h);
        }
    }

    fn render(&mut self, _gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        match mode {
            0 => {
                scene.field = Some("em.potential");
                scene.p(0, self.charges.len() as f32);
                for (i, &(c, q)) in self.charges.iter().enumerate() {
                    scene.params[2 + i] = [c.x, c.y, q, 0.0];
                }
                // Field lines launched from positive charges (and mirrored
                // backwards from negatives so isolated - charges get lines too).
                if self.params.b("lines") {
                    for &(c, q) in &self.charges {
                        let n_lines = 14;
                        for k in 0..n_lines {
                            let a = k as f32 / n_lines as f32 * std::f32::consts::TAU;
                            let mut p = c + Vec2::new(a.cos(), a.sin()) * 0.09;
                            let mut pts = vec![Vec3::new(p.x, p.y, 0.02)];
                            let dir_sign = q.signum();
                            for _ in 0..340 {
                                let e = self.efield(p);
                                let el = e.length();
                                if el < 1e-4 {
                                    break;
                                }
                                p += e / el * 0.028 * dir_sign;
                                pts.push(Vec3::new(p.x, p.y, 0.02));
                                if p.length() > 5.4 {
                                    break;
                                }
                                // Stop when swallowed by another charge.
                                if self
                                    .charges
                                    .iter()
                                    .any(|&(c2, _)| (p - c2).length() < 0.085)
                                {
                                    pts.push(Vec3::new(p.x, p.y, 0.02));
                                    break;
                                }
                            }
                            let col = if q > 0.0 {
                                with_alpha(GOLDE, 0.34)
                            } else {
                                with_alpha(CYAN, 0.22)
                            };
                            scene.polyline(LineLayer::WorldAdd, &pts, 1.25, col);
                        }
                    }
                }
                for &(c, q) in &self.charges {
                    let col = if q > 0.0 { MAGENTA } else { CYAN };
                    scene.point(Vec3::new(c.x, c.y, 0.05), 7.0, col, 2.4);
                    // Engrave the sign.
                    let s = 0.085;
                    scene.line(
                        LineLayer::Overlay,
                        Vec3::new(c.x - s, c.y, 0.06),
                        Vec3::new(c.x + s, c.y, 0.06),
                        1.8,
                        [0.0, 0.0, 0.0, 0.85],
                    );
                    if q > 0.0 {
                        scene.line(
                            LineLayer::Overlay,
                            Vec3::new(c.x, c.y - s, 0.06),
                            Vec3::new(c.x, c.y + s, 0.06),
                            1.8,
                            [0.0, 0.0, 0.0, 0.85],
                        );
                    }
                }
            }
            1 => {
                scene.field = Some("em.dipole");
                scene.p(1, self.params.f("k") as f32);
                // The dipole itself: a trembling vertical stub.
                let osc = (2.2 * t).sin() as f32 * 0.16;
                scene.line_soft(
                    LineLayer::WorldAdd,
                    Vec3::new(0.0, -0.2 - osc * 0.4, 0.05),
                    Vec3::new(0.0, 0.2 + osc * 0.4, 0.05),
                    3.4,
                    with_alpha(GOLDE, 0.9),
                );
            }
            3 => {
                // Uniform-B backdrop: faint symbols of field-out-of-plane.
                for gx in -4..=4 {
                    for gy in -3..=3 {
                        let p = Vec3::new(gx as f32 * 0.9, gy as f32 * 0.9, -0.05);
                        scene.point(p, 1.2, [0.35, 0.55, 0.75, 0.25], 0.4);
                    }
                }
                for (p, v, q, trail) in &self.cyc {
                    let col = if *q > 0.0 { MAGENTA } else { CYAN };
                    let n = trail.len();
                    for i in 1..n {
                        let age = i as f32 / n as f32;
                        scene.line(
                            LineLayer::WorldAdd,
                            Vec3::new(trail[i - 1].x, trail[i - 1].y, 0.0),
                            Vec3::new(trail[i].x, trail[i].y, 0.0),
                            1.5,
                            with_alpha(col, 0.04 + 0.5 * age * age),
                        );
                    }
                    scene.point(Vec3::new(p.x, p.y, 0.02), 4.0, col, 2.0);
                    scene.vector(
                        Vec3::new(p.x, p.y, 0.02),
                        Vec3::new(v.x, v.y, 0.0) * 0.35,
                        2.6,
                        with_alpha(GOLDE, 0.8),
                    );
                }
            }
            4 => {
                // The cup.
                let rim: Vec<Vec3> = (0..=180)
                    .map(|i| {
                        let a = i as f32 / 180.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos() * 1.4, a.sin() * 1.4, 0.0)
                    })
                    .collect();
                scene.polyline(LineLayer::World, &rim, 1.8, [0.7, 0.72, 0.78, 0.7]);
                // Sunlight direction slowly circles the cup.
                let sun = (t * 0.15) as f32;
                let dir = glam::Vec2::new(sun.cos(), sun.sin());
                for k in 0..110 {
                    // Impact parameter across the cup.
                    let b = -1.38 + 2.76 * k as f32 / 109.0;
                    let perp = glam::Vec2::new(-dir.y, dir.x);
                    // The ray crosses the cup and strikes the FAR inner wall.
                    let along = (1.4f32 * 1.4 - b * b).max(0.0).sqrt();
                    let hit = perp * b + dir * along;
                    let n = hit.normalize_or_zero();
                    let refl = dir - n * 2.0 * dir.dot(n);
                    let start = hit - dir * 3.2;
                    let h3 = Vec3::new(hit.x, hit.y, 0.0);
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(start.x, start.y, 0.0),
                        h3,
                        1.0,
                        [0.85, 0.75, 0.45, 0.06],
                    );
                    // Clip the reflected ray at its second rim crossing.
                    let s2 = (-2.0 * hit.dot(refl)).max(0.05);
                    let out = hit + refl * s2;
                    scene.line(
                        LineLayer::WorldAdd,
                        h3,
                        Vec3::new(out.x, out.y, 0.0),
                        1.1,
                        [0.95, 0.78, 0.35, 0.16],
                    );
                }
            }
            _ => {
                // Propagation axis.
                scene.line(
                    LineLayer::World,
                    Vec3::new(-6.4, 0.0, 0.0),
                    Vec3::new(6.4, 0.0, 0.0),
                    1.2,
                    [0.5, 0.5, 0.55, 0.5],
                );
                let k = 1.6f64;
                let om = 2.2f64;
                let n = 96;
                let mut e_env = Vec::with_capacity(n + 1);
                let mut b_env = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let z = -6.0 + 12.0 * i as f64 / n as f64;
                    let a = (k * z - om * t).cos() as f32;
                    let zf = z as f32;
                    e_env.push(Vec3::new(zf, a * 1.7, 0.0));
                    b_env.push(Vec3::new(zf, 0.0, a * 1.7));
                    if i % 3 == 0 {
                        scene.vector(
                            Vec3::new(zf, 0.0, 0.0),
                            Vec3::new(0.0, a * 1.7, 0.0),
                            2.6,
                            with_alpha(GOLDE, 0.75),
                        );
                        scene.vector(
                            Vec3::new(zf, 0.0, 0.0),
                            Vec3::new(0.0, 0.0, a * 1.7),
                            2.6,
                            with_alpha(CYAN, 0.75),
                        );
                    }
                }
                scene.glow_polyline(&e_env, 1.8, GOLDE, 2.4);
                scene.glow_polyline(&b_env, 1.8, CYAN, 2.4);
            }
        }
    }

    fn placed(&self) -> Option<serde_json::Value> {
        let positions: Vec<[f32; 2]> = self.charges.iter().map(|(c, _)| [c.x, c.y]).collect();
        Some(serde_json::json!(positions))
    }

    fn restore_placed(&mut self, v: &serde_json::Value) {
        let Some(list) = v.as_array() else { return };
        for (charge, saved) in self.charges.iter_mut().zip(list) {
            let (Some(x), Some(y)) = (saved[0].as_f64(), saved[1].as_f64()) else {
                continue;
            };
            let at = Vec2::new(x as f32, y as f32);
            if at.is_finite() {
                charge.0 = on_stage(at);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let dip: f32 = self.charges.iter().map(|&(c, q)| c.x * q).sum();
        vec![
            title("Maxwell's electrostatics & radiation"),
            ftex("em.field", "E(r) = \u{03A3} q\u{1D62} r\u{0302}\u{1D62}/r\u{1D62}\u{00B2}     \u{2207}\u{00D7}E = 0 (statics)"),
            ftex("em.dipole", "\u{03C8}(r,\u{03B8},t) = sin\u{00B2}\u{03B8}[cos(kr\u{2212}\u{03C9}t) \u{2212} sin(kr\u{2212}\u{03C9}t)/kr]"),
            def("field lines leave + and land on \u{2212}; equipotentials cross them at right angles"),
            value(format!("charges: {}    p\u{2093} = {dip:+.2}", self.charges.len())),
            note("The near zone ends around r \u{2248} \u{03BB}/2\u{03C0}: inside it the field breathes with the source; beyond it, closed loops of field travel outward as radiation."),
            note("E \u{22A5} B \u{22A5} k, |E| = c|B|, in phase: the plane wave carries itself."),
        ]
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some(
                "Lines integrated by 340 Euler steps along E\u{0302}; potential exact per pixel.",
            ),
            1 => Some("Exact Hertzian-dipole field-line function; amplitude fade is display only."),
            2 => Some("Closed-form plane wave, in arbitrary units."),
            3 => Some("Boris integrator, 4 substeps per frame, under uniform E and B. A charge that drifts off the bottom re-enters at the top."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_at_the_edge_saves_what_it_shows() {
        let mut f = FieldForge::new();
        f.pointer(0, PointerEvent::Down(Vec2::new(1.1, 0.0)));
        f.pointer(0, PointerEvent::Drag(Vec2::new(9.0, 0.0)));
        f.pointer(0, PointerEvent::Up);
        // Within reach of the charge now at the edge, but outside the stage itself.
        f.pointer(0, PointerEvent::Down(Vec2::new(3.9, 0.0)));
        f.pointer(0, PointerEvent::Up);
        let saved = f.placed().unwrap();
        let mut restored = FieldForge::new();
        restored.restore_placed(&saved);
        assert_eq!(restored.placed().unwrap(), saved);
    }

    /// The field-line function the dipole shader contours, as a Rust twin.
    fn psi(k: f64, wt: f64, r: f64, th: f64) -> f64 {
        let ph = k * r - wt;
        th.sin().powi(2) * (ph.cos() - ph.sin() / (k * r))
    }

    /// The exact field of a Hertzian dipole, p = p0 sin(wt - kr), with c = 1.
    fn field(k: f64, wt: f64, r: f64, th: f64) -> (f64, f64) {
        let phi = wt - k * r;
        let p = phi.sin();
        let pdot = k * phi.cos();
        let pddot = -k * k * phi.sin();
        let near = p / r.powi(3) + pdot / (r * r);
        (2.0 * th.cos() * near, th.sin() * (near + pddot / r))
    }

    #[test]
    fn charges_stay_in_the_frame_long_after_they_would_have_drifted_out() {
        let mut f = FieldForge::new();
        // Three minutes at 60 frames a second, far past t = 34 s.
        for frame in 0..(180 * 60) {
            f.advance_charges(1.0 / 60.0);
            if frame % 60 == 0 {
                for (p, _, _, _) in &f.cyc {
                    assert!(p.y.abs() < CYC_VIEW + 3.0, "frame {frame}: {p:?}");
                    assert!(p.x.abs() < 6.0, "frame {frame}: {p:?}");
                }
            }
        }
    }

    #[test]
    fn the_integrator_does_not_heat_the_charges() {
        // In the frame drifting at E/B the field is pure magnetic, so speed there
        // is constant. Forward Euler took the top speed from 1.77 to 3.9 in 3 min.
        let drift = Vec2::new(0.0, -CYC_E / CYC_B);
        for (p0, v0, q, _) in seed_cyc() {
            let (mut p, mut v) = (p0, v0);
            let start = (v - drift).length();
            for _ in 0..(180 * 60 * 4) {
                (p, v) = boris_step(p, v, q, 1.0 / 240.0);
            }
            let end = (v - drift).length();
            assert!((end - start).abs() < 0.05 * start, "{start} -> {end}");
        }
    }

    #[test]
    fn re_entry_happens_only_with_the_whole_orbit_out_of_sight() {
        let mut f = FieldForge::new();
        for _ in 0..(180 * 60) {
            let before: Vec<Vec2> = f.cyc.iter().map(|c| c.0).collect();
            f.advance_charges(1.0 / 60.0);
            for (old, now) in before.iter().zip(&f.cyc) {
                if (now.0.y - old.y).abs() > 1.0 {
                    assert!(
                        old.y < -CYC_VIEW + 0.1,
                        "jumped from y = {} in plain sight",
                        old.y
                    );
                    assert!(
                        now.0.y > CYC_VIEW - 0.1,
                        "landed at y = {} in plain sight",
                        now.0.y
                    );
                }
            }
        }
    }

    #[test]
    fn both_signs_drift_at_e_over_b() {
        for q in [1.0f32, -1.0] {
            let (mut p, mut v) = (Vec2::ZERO, Vec2::new(0.0, 0.8));
            let period = std::f32::consts::TAU / CYC_B;
            let steps = 4000;
            let turns = 8.0;
            for _ in 0..steps {
                (p, v) = boris_step(p, v, q, turns * period / steps as f32);
            }
            let expected = -CYC_E / CYC_B * turns * period;
            assert!(
                (p.y - expected).abs() < 0.05,
                "q = {q}: {} vs {expected}",
                p.y
            );
        }
    }

    #[test]
    fn dipole_field_lines_really_are_level_sets_of_psi() {
        let (k, h) = (1.3, 1e-5);
        for (wt, r, th) in [
            (0.4, 1.5, 0.7),
            (2.0, 3.0, 1.1),
            (4.0, 0.9, 2.3),
            (5.5, 6.0, 0.4),
        ] {
            let (er, eth) = field(k, wt, r, th);
            let dr = (psi(k, wt, r + h, th) - psi(k, wt, r - h, th)) / (2.0 * h);
            let dth = (psi(k, wt, r, th + h) - psi(k, wt, r, th - h)) / (2.0 * h) / r;
            let along = er * dr + eth * dth;
            let size = (er * er + eth * eth).sqrt() * (dr * dr + dth * dth).sqrt();
            assert!(along.abs() < 1e-6 * size, "E.grad(psi) = {along} of {size}");
        }
        assert!(DIPOLE_SHADE.contains("cos(ph) - sin(ph) / (kk * r)"));
    }
}
