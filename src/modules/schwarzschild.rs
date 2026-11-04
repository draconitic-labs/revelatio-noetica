//! Schwarzschild geometry: lensing, precessing geodesics, the effective
//! potential, and Flamm's paraboloid. Geometrized units, M = 1 (r_s = 2).

use glam::Vec3;

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{rk4, with_alpha};
use crate::theme::Branch;

const EMBER: [f32; 4] = [0.95, 0.48, 0.16, 1.0];
const ICE: [f32; 4] = [0.55, 0.70, 0.95, 1.0];

/// Null-geodesic raymarcher with thin disk and gravitationally lensed stars.
/// Units: r_s = 1 here (photon accel kernel -1.5 h^2 r / r^5).
const LENSING_SHADE: &str = r#"
fn stars(rd: vec3<f32>) -> vec3<f32> {
    // Hash the direction on a cube grid for stable stars.
    let a = abs(rd);
    var uv2: vec2<f32>;
    if (a.x >= a.y && a.x >= a.z) { uv2 = rd.yz / rd.x; }
    else if (a.y >= a.z) { uv2 = rd.xz / rd.y + vec2<f32>(7.0, 3.0); }
    else { uv2 = rd.xy / rd.z + vec2<f32>(13.0, 11.0); }
    let cell = floor(uv2 * 96.0);
    let h = hash21(cell);
    var c = vec3<f32>(0.0);
    if (h > 0.9962) {
        let mag = (h - 0.9962) / 0.0038;
        // Round star core within its cell.
        let f = fract(uv2 * 96.0) - vec2<f32>(0.5);
        let core = exp(-dot(f, f) * 30.0);
        let tint = mix(vec3<f32>(0.75, 0.82, 1.0), vec3<f32>(1.0, 0.88, 0.70), hash21(cell + 4.7));
        c = tint * mag * core * 1.2;
    }
    return c;
}

fn disk_color(r: f32, phi: f32, side: f32) -> vec3<f32> {
    let t = U.res.z;
    // Keplerian-ish angular pattern, drifting.
    let swirl = sin(phi * 9.0 - 26.0 / max(r, 1.2) + t * 0.7) * 0.5 + 0.5;
    let heat = clamp((4.2 - r) / 3.2, 0.0, 1.0);
    var c = aurum(0.35 + heat * 0.5 + swirl * 0.18);
    // Doppler beaming: approaching side brighter and bluer.
    let dop = 1.0 + side * 0.75 / sqrt(max(r, 1.5));
    c = c * pow(max(dop, 0.15), 3.0);
    c = mix(c, vec3<f32>(0.85, 0.9, 1.0) * length(c), clamp(side * 0.35, 0.0, 0.6));
    let fade = smoothstep(4.8, 3.4, r) * 0.4 + 0.6;
    return c * fade * 1.6;
}

fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32> {
    let ro = U.eye.xyz;
    var rd = ray_dir(ndc);
    var p = ro;
    var v = rd;
    let h = cross(p, v);
    let h2 = dot(h, h);
    var color = vec3<f32>(0.0);
    var trans = 1.0;
    var captured = false;
    // Adaptive leapfrog integration of the null geodesic.
    for (var i = 0; i < 320; i++) {
        let r = length(p);
        if (r < 0.52) { captured = true; break; }
        if (r > 42.0 && dot(p, v) > 0.0) { break; }
        let dt = clamp(r * 0.11, 0.02, 0.9);
        let a = -1.5 * h2 * p / pow(dot(p, p), 2.5);
        let vn = v + a * dt;
        let pn = p + vn * dt;
        // Thin disk in the y=0 plane, 1.7 < r < 4.6 (units of r_s... scaled).
        if (p.y * pn.y < 0.0) {
            let f = p.y / (p.y - pn.y);
            let hit = mix(p, pn, f);
            let hr = length(hit.xz);
            if (hr > 1.55 && hr < 4.6) {
                let phi = atan2(hit.z, hit.x);
                // Approaching side: disk rotates +phi; side factor from view.
                let tangent = normalize(vec3<f32>(-hit.z, 0.0, hit.x));
                let side = dot(tangent, normalize(v));
                color += trans * disk_color(hr, phi, side);
                trans *= 0.12;
                if (trans < 0.02) { break; }
            }
        }
        p = pn;
        v = vn;
    }
    if (!captured && trans > 0.02) {
        color += trans * stars(normalize(v));
    }
    // Photon-ring glow: impact parameters near b_crit pile up light.
    let bimp = sqrt(max(h2, 0.0));
    color += vec3<f32>(1.0, 0.82, 0.55) * exp(-abs(bimp - 2.598) * 9.0) * 0.16;
    return vec4<f32>(color, 1.0);
}
"#;

const PRESET_NAMES: [&str; 3] = ["Precessing rosette", "Near-circular", "Plunge"];

/// Where a timelike orbit counts as captured.
const CAPTURE_R: f64 = 2.15;

/// Height of Flamm's paraboloid for M = 1. The horizon at r = 2 is the throat,
/// at the bottom of the funnel, and space flattens out above it.
fn flamm_height(r: f64) -> f64 {
    2.0 * (2.0f64 * (r - 2.0).max(0.0)).sqrt()
}

pub struct SchwarzschildCourt {
    params: Params,
    /// Orbit state: (r, pr, phi, tau).
    state: [f64; 4],
    trail: Vec<Vec3>,
    energy: f64,
    /// The orbit reached the horizon and has stopped.
    captured: bool,
    last_key: (usize, i64),
    funnel: Option<GpuMesh>,
}

impl SchwarzschildCourt {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("preset", "Orbit", "Symbolic", PRESET_NAMES.to_vec(), 0);
        p.slider("ll", "Angular momentum L", "Physics", 3.5, 4.6, 3.9, "M");
        p.slider(
            "r0",
            "Start radius r\u{2080}",
            "Physics",
            7.0,
            26.0,
            13.0,
            "M",
        );
        p.slider(
            "rate",
            "Proper-time rate",
            "Observer",
            2.0,
            80.0,
            28.0,
            "\u{00D7}",
        );
        Self {
            params: p,
            state: [13.0, 0.0, 0.0, 0.0],
            trail: Vec::new(),
            energy: 0.0,
            captured: false,
            last_key: (usize::MAX, 0),
            funnel: None,
        }
    }

    fn reset_orbit(&mut self) {
        let preset = self.params.c("preset");
        let (l, r0) = match preset {
            0 => (self.params.f("ll"), self.params.f("r0")),
            1 => (4.2, 12.6),
            _ => (3.20, 16.0),
        };
        self.state = [r0, 0.0, 0.0, 0.0];
        self.energy = ((1.0 - 2.0 / r0) * (1.0 + l * l / (r0 * r0))).sqrt();
        self.trail.clear();
        self.captured = false;
    }

    fn l_now(&self) -> f64 {
        match self.params.c("preset") {
            0 => self.params.f("ll"),
            1 => 4.2,
            _ => 3.20,
        }
    }

    fn veff(&self, r: f64) -> f64 {
        let l = self.l_now();
        ((1.0 - 2.0 / r) * (1.0 + l * l / (r * r))).sqrt()
    }

    /// E from the first integral of the current state, to compare with the E the
    /// orbit started with.
    fn energy_now(&self) -> f64 {
        let (r, pr, l) = (self.state[0], self.state[1], self.l_now());
        ((1.0 - 2.0 / r) * (1.0 + l * l / (r * r)) + pr * pr)
            .max(0.0)
            .sqrt()
    }

    /// One frame of the orbit, h in proper time. Capture stops the orbit where it
    /// is, values untouched, so the energy check still reports the integrator's
    /// real error and not an artefact of stopping.
    fn advance(&mut self, h: f64) {
        if self.captured {
            return;
        }
        let l = self.l_now();
        // Geodesic ODE in proper time: r'' from the effective potential.
        let f = move |_tau: f64, y: &[f64; 4]| -> [f64; 4] {
            let r = y[0].max(2.05);
            [
                y[1],
                -1.0 / (r * r) + l * l / (r * r * r) - 3.0 * l * l / (r * r * r * r),
                l / (r * r),
                1.0,
            ]
        };
        let steps = 6;
        for _ in 0..steps {
            self.state = rk4(&f, 0.0, &self.state, h / steps as f64);
        }
        // This coordinate stops meaning anything at the horizon, so the orbit stops
        // here until it is restarted.
        self.captured = self.state[0] <= CAPTURE_R;
        let (r, phi) = (self.state[0], self.state[2]);
        let p = Vec3::new((r * phi.cos()) as f32, (r * phi.sin()) as f32, 0.0);
        if self.trail.last().is_none_or(|q| (*q - p).length() > 0.03) {
            self.trail.push(p);
            if self.trail.len() > 5000 {
                self.trail.remove(0);
            }
        }
    }
}

impl ConceptModule for SchwarzschildCourt {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "gr.schwarzschild",
            name: "The Schwarzschild Court",
            tagline: "Geodesics and lensing at the horizon",
            branch: Branch::Gravitation,
            description: "Around a mass, straight lines bend because spacetime does. Light rays integrate the exact null geodesic equation per pixel: the photon ring at b = 3\u{221A}3 M, the doubled disk images, the black shadow. Massive orbits precess \u{2014} the failure of the ellipse that announced general relativity at Mercury.",
            epigraph: Some((
                "Matter tells spacetime how to curve; spacetime tells matter how to move.",
                "John Archibald Wheeler",
            )),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit("lensing", "Gravitational Lensing", "Per-pixel null geodesics, thin disk, photon ring", 16.0, 0.10, std::f32::consts::FRAC_PI_2),
            RenderMode::plane("orbit", "Precessing Geodesic", "The rosette Mercury drew first", 17.0),
            RenderMode::plane("veff", "Effective Potential", "V(r) = \u{221A}((1\u{2212}2M/r)(1+L\u{00B2}/r\u{00B2})); turning points bound the orbit", 1.55)
                .look_at(2.85, 0.55, 0.0),
            RenderMode::orbit("funnel", "Flamm's Paraboloid", "The spatial slice embedded: the funnel is real geometry", 58.0, 0.42, 0.9).look_at(0.0, 5.0, 0.0),
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
        rend.register_field(gpu, "gr.lensing", LENSING_SHADE, false);
        // Flamm funnel: revolve z = flamm_height(r).
        let d = MeshData::parametric(80, 96, |u, v| {
            let r = 2.0 + (u as f64).powi(2) * 26.0;
            let ang = v as f64 * std::f64::consts::TAU;
            let z = flamm_height(r);
            let shade = 0.25 + 0.6 * u;
            (
                Vec3::new((r * ang.cos()) as f32, z as f32, (r * ang.sin()) as f32),
                [0.25 * shade, 0.35 * shade, 0.62 * shade, 1.0],
            )
        });
        let mut m = d.upload(gpu, &rend.mesh_bgl);
        m.set_model(gpu, glam::Mat4::IDENTITY, [1.0; 4], [0.0, 0.5, 0.55, 0.0]);
        self.funnel = Some(m);
        self.reset_orbit();
    }

    fn rewind(&mut self) {
        self.reset_orbit();
    }

    fn update(&mut self, _gpu: &GpuContext, _t: f64, dt: f64) {
        let key = (
            self.params.c("preset"),
            ((self.params.f("ll") * 100.0) as i64) ^ ((self.params.f("r0") * 100.0) as i64) << 16,
        );
        // The rosette accumulates across timeline loops; only parameter changes
        // and the viewer rewinding restart it.
        if key != self.last_key {
            self.last_key = key;
            self.reset_orbit();
        }
        let h = (dt * self.params.f("rate")).clamp(0.0, 1.0);
        self.advance(h);
    }

    fn render(
        &mut self,
        _gpu: &GpuContext,
        mode: usize,
        _t: f64,
        _cam: &Camera,
        scene: &mut Scene,
    ) {
        let (r_now, phi_now) = (self.state[0], self.state[2]);
        let pos = Vec3::new(
            (r_now * phi_now.cos()) as f32,
            (r_now * phi_now.sin()) as f32,
            0.0,
        );

        match mode {
            0 => {
                scene.field = Some("gr.lensing");
            }
            1 => {
                // Horizon and ISCO.
                let circle = |r: f32, z: f32| -> Vec<Vec3> {
                    (0..=128)
                        .map(|i| {
                            let a = i as f32 / 128.0 * std::f32::consts::TAU;
                            Vec3::new(a.cos() * r, a.sin() * r, z)
                        })
                        .collect()
                };
                scene.polyline(
                    LineLayer::World,
                    &circle(2.0, 0.0),
                    2.2,
                    with_alpha(EMBER, 0.9),
                );
                scene.polyline(
                    LineLayer::World,
                    &circle(6.0, 0.0),
                    1.1,
                    with_alpha(ICE, 0.30),
                );
                scene.polyline(
                    LineLayer::World,
                    &circle(3.0, 0.0),
                    1.0,
                    with_alpha([1.0, 0.85, 0.5, 1.0], 0.22),
                );
                // Trail with age gradient.
                let n = self.trail.len();
                for i in 1..n {
                    let age = i as f32 / n as f32;
                    let c = [
                        0.3 + 0.65 * age,
                        0.45 + 0.30 * age,
                        0.95 - 0.35 * age,
                        0.10 + 0.85 * age * age,
                    ];
                    scene.line(
                        LineLayer::WorldAdd,
                        self.trail[i - 1],
                        self.trail[i],
                        1.7,
                        c,
                    );
                }
                scene.point(pos, 5.0, [0.9, 0.95, 1.0, 1.0], 2.2);
            }
            2 => {
                let l = self.l_now();
                // Plot transform: x = r/5, y = (V - 0.90) * 14.
                let px_of =
                    |r: f64, v: f64| Vec3::new((r / 5.0) as f32, ((v - 0.90) * 14.0) as f32, 0.0);
                let pts: Vec<Vec3> = (0..=400)
                    .map(|i| {
                        let r = 2.05 + 26.0 * i as f64 / 400.0;
                        px_of(r, self.veff(r))
                    })
                    .collect();
                scene.glow_polyline(&pts, 2.2, ICE, 2.8);
                // Energy line and the live marker.
                scene.line(
                    LineLayer::World,
                    px_of(2.05, self.energy),
                    px_of(28.0, self.energy),
                    1.3,
                    with_alpha(EMBER, 0.65),
                );
                scene.point(
                    px_of(r_now, self.veff(r_now)),
                    5.0,
                    [0.9, 0.95, 1.0, 1.0],
                    2.2,
                );
                // Horizon wall and ISCO mast.
                scene.line(
                    LineLayer::World,
                    px_of(2.0, 0.845),
                    px_of(2.0, 1.06),
                    2.0,
                    with_alpha(EMBER, 0.8),
                );
                scene.line(
                    LineLayer::World,
                    px_of(6.0, 0.85),
                    px_of(6.0, 1.05),
                    1.1,
                    with_alpha([1.0, 0.85, 0.5, 1.0], 0.45),
                );
                // Baseline V = 1 (rest energy at infinity).
                scene.line(
                    LineLayer::World,
                    px_of(2.05, 1.0),
                    px_of(28.0, 1.0),
                    0.9,
                    [0.55, 0.6, 0.7, 0.30],
                );
                let _ = l;
            }
            _ => {
                if let Some(m) = &self.funnel {
                    scene.mesh(m);
                }
                // Meridian grid engraved on the funnel.
                for k in 0..12 {
                    let ang = k as f64 / 12.0 * std::f64::consts::TAU;
                    let mer: Vec<Vec3> = (0..=40)
                        .map(|i| {
                            let u = i as f64 / 40.0;
                            let r = 2.0 + u * u * 26.0;
                            let z = flamm_height(r);
                            Vec3::new((r * ang.cos()) as f32, z as f32, (r * ang.sin()) as f32)
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &mer, 0.9, with_alpha(ICE, 0.20));
                }
                for kr in [3.0f64, 6.0, 10.0, 16.0, 24.0] {
                    let z = flamm_height(kr);
                    let ring: Vec<Vec3> = (0..=96)
                        .map(|i| {
                            let a = i as f64 / 96.0 * std::f64::consts::TAU;
                            Vec3::new((kr * a.cos()) as f32, z as f32, (kr * a.sin()) as f32)
                        })
                        .collect();
                    scene.polyline(LineLayer::World, &ring, 0.9, with_alpha(ICE, 0.22));
                }
                // Orbit projected onto the funnel.
                let n = self.trail.len();
                for i in 1..n {
                    let a = self.trail[i - 1];
                    let b = self.trail[i];
                    let za = flamm_height(a.length() as f64);
                    let zb = flamm_height(b.length() as f64);
                    let age = i as f32 / n as f32;
                    scene.line(
                        LineLayer::WorldAdd,
                        Vec3::new(a.x, za as f32, a.y),
                        Vec3::new(b.x, zb as f32, b.y),
                        1.8,
                        [0.95, 0.6 + 0.3 * age, 0.25, 0.10 + 0.8 * age * age],
                    );
                }
                let zp = flamm_height(r_now);
                scene.point(
                    Vec3::new(pos.x, zp as f32, pos.y),
                    5.0,
                    [0.95, 0.9, 0.8, 1.0],
                    2.2,
                );
                // The throat ring.
                let throat: Vec<Vec3> = (0..=96)
                    .map(|i| {
                        let a = i as f32 / 96.0 * std::f32::consts::TAU;
                        Vec3::new(a.cos() * 2.0, 0.0, a.sin() * 2.0)
                    })
                    .collect();
                scene.glow_polyline(&throat, 2.2, EMBER, 2.8);
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let l = self.l_now();
        let (r, phi, tau) = (self.state[0], self.state[2], self.state[3]);
        // Conserved-energy drift as an honesty check.
        let e_now = self.energy_now();
        let mut lines = vec![
            title("Schwarzschild, M = 1"),
            ftex("gr.metric", "ds\u{00B2} = \u{2212}(1\u{2212}2M/r)dt\u{00B2} + dr\u{00B2}/(1\u{2212}2M/r) + r\u{00B2}d\u{03A9}\u{00B2}"),
            ftex("gr.veff", "(dr/d\u{03C4})\u{00B2} = E\u{00B2} \u{2212} (1\u{2212}2M/r)(1+L\u{00B2}/r\u{00B2})"),
            def("horizon r\u{209B} = 2M \u{00B7} photon sphere 3M \u{00B7} ISCO 6M \u{00B7} shadow b = 3\u{221A}3 M"),
            value(format!("L = {l:.2}   E = {:.5}   r = {r:.2}   \u{03C6} = {:.1}\u{00B0}", self.energy, phi.to_degrees() % 360.0)),
            value(format!("proper time \u{03C4} = {tau:.1}   energy drift |\u{0394}E|/E = {:.1e}", ((e_now - self.energy) / self.energy).abs())),
            note("Perihelion advance per orbit \u{2248} 6\u{03C0}M/(a(1\u{2212}e\u{00B2})) \u{2014} the rosette never closes."),
        ];
        if self.captured {
            lines.push(note("Captured: the orbit stopped on crossing r = 2.15, just outside the horizon, and restarts with a new orbit or a rewind."));
        }
        lines
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            0 => Some("Null geodesics: up to 320 first-order kick-drift steps per pixel, so the capture edge falls near b = 2.53, against the exact 2.598; disk emission stylized."),
            1 | 3 => Some("Timelike geodesic by RK4 in proper time, stopped on crossing r = 2.15; energy drift shown in the side panel."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plunge_stops_at_capture_without_faking_an_energy_error() {
        use crate::engine::params::PValue;
        let mut c = SchwarzschildCourt::new();
        c.params.set("preset", PValue::C(2));
        c.reset_orbit();
        for _ in 0..200_000 {
            c.advance(0.02);
            if c.captured {
                break;
            }
        }
        assert!(
            c.captured,
            "the plunge never reached the horizon: r = {}",
            c.state[0]
        );
        let drift = ((c.energy_now() - c.energy) / c.energy).abs();
        assert!(drift < 1e-4, "energy drift {drift}");
        let frozen = c.state;
        c.advance(0.02);
        assert_eq!(c.state, frozen, "a captured orbit stays where it stopped");
        assert!(
            c.state[0] > 2.0,
            "stopped outside the horizon: r = {}",
            c.state[0]
        );
    }

    #[test]
    fn the_funnel_rises_from_its_throat_at_the_horizon() {
        assert_eq!(flamm_height(2.0), 0.0);
        assert!(flamm_height(3.0) > 0.0 && flamm_height(10.0) > flamm_height(3.0));
        // Flamm's paraboloid: z^2 = 8 M (r - 2 M), with M = 1.
        for r in [2.5f64, 6.0, 20.0] {
            assert!((flamm_height(r).powi(2) - 8.0 * (r - 2.0)).abs() < 1e-12);
        }
    }

    #[test]
    fn effective_potential_has_its_circular_orbit_where_theory_says() {
        // For M = 1, circular orbits sit at r = (L^2 + L sqrt(L^2 - 12)) / 2.
        let l = 4.0f64;
        let r_c = (l * l + l * (l * l - 12.0).sqrt()) / 2.0;
        let v2 = |r: f64| (1.0 - 2.0 / r) * (1.0 + l * l / (r * r));
        let h = 1e-4;
        let slope = (v2(r_c + h) - v2(r_c - h)) / (2.0 * h);
        assert!(slope.abs() < 1e-6, "{slope}");
    }
}
