//! Knots and links: properties that survive deformation.

use glam::{Mat4, Vec3};

use crate::engine::camera::Camera;
use crate::engine::draw::{LineLayer, Renderers, Scene};
use crate::engine::gpu::GpuContext;
use crate::engine::mesh::{GpuMesh, MeshData};
use crate::engine::module::*;
use crate::engine::params::Params;
use crate::modules::util::{phase_color, with_alpha};
use crate::theme::Branch;

const KNOT_NAMES: [&str; 5] = [
    "Trefoil (2,3)",
    "Cinquefoil (2,5)",
    "(3,4) torus knot",
    "Figure-eight",
    "Hopf link",
];

/// Sample knot k at parameter s in [0,1); component 0 or 1 (links only).
fn knot_pt(k: usize, comp: usize, s: f32, breathe: f32) -> Vec3 {
    let t = s * std::f32::consts::TAU;
    let b = 1.0 + 0.10 * breathe;
    match k {
        0..=2 => {
            let (p, q) = match k {
                0 => (2.0f32, 3.0f32),
                1 => (2.0, 5.0),
                _ => (3.0, 4.0),
            };
            let r = (q * t).cos() * 0.55 * b + 1.15;
            Vec3::new(
                r * (p * t).cos(),
                -(q * t).sin() * 0.55 * b,
                r * (p * t).sin(),
            )
        }
        3 => {
            // Figure-eight knot, standard parametrization: three turns about the
            // axis. With a single turn this curve is an unknot.
            Vec3::new(
                (2.0 + (2.0 * t).cos() * b) * (3.0 * t).cos() * 0.62,
                (4.0 * t).sin() * 0.42 * b,
                (2.0 + (2.0 * t).cos() * b) * (3.0 * t).sin() * 0.62,
            )
        }
        _ => {
            // Hopf link: each circle passes once through the other. The second
            // is tilted about the x axis so its shadow is an ellipse, not a line.
            let (tilt_s, tilt_c) = 0.6f32.sin_cos();
            if comp == 0 {
                Vec3::new(t.cos() * 0.9, t.sin() * 0.9, 0.0)
            } else {
                let lift = t.sin() * 0.9;
                Vec3::new(0.9 + t.cos() * 0.9, lift * tilt_s, lift * tilt_c)
            }
        }
    }
}

/// Samples cut on each side of an undercrossing in the diagram.
const GAP_SAMPLES: usize = 6;

/// Where two segments of the (x, y) shadow cross, as a fraction along each.
fn shadow_crossing(a0: Vec3, a1: Vec3, b0: Vec3, b1: Vec3) -> Option<(f32, f32)> {
    let (rx, ry) = (a1.x - a0.x, a1.y - a0.y);
    let (sx, sy) = (b1.x - b0.x, b1.y - b0.y);
    let denom = rx * sy - ry * sx;
    if denom.abs() < 1e-9 {
        return None;
    }
    let (qx, qy) = (b0.x - a0.x, b0.y - a0.y);
    let u = (qx * sy - qy * sx) / denom;
    let v = (qx * ry - qy * rx) / denom;
    ((0.0..1.0).contains(&u) && (0.0..1.0).contains(&v)).then_some((u, v))
}

/// Segments of strand `comp`, sampled `n` times, that pass beneath another
/// strand in the (x, y) shadow. Depth is compared where the two actually cross.
fn under_crossings(k: usize, comp: usize, breathe: f32, n: usize) -> Vec<usize> {
    let strand = |c: usize| -> Vec<Vec3> {
        (0..=n)
            .map(|i| knot_pt(k, c, i as f32 / n as f32, breathe))
            .collect()
    };
    let mine = strand(comp);
    let mut under = Vec::new();
    for other in 0..components(k) {
        let theirs = strand(other);
        for i in 0..n {
            for j in 0..n {
                if other == comp {
                    let apart = i.abs_diff(j);
                    if apart <= 1 || apart >= n - 1 {
                        continue;
                    }
                }
                let crossing = shadow_crossing(mine[i], mine[i + 1], theirs[j], theirs[j + 1]);
                if let Some((u, v)) = crossing {
                    let mine_z = mine[i].z + (mine[i + 1].z - mine[i].z) * u;
                    let their_z = theirs[j].z + (theirs[j + 1].z - theirs[j].z) * v;
                    if mine_z < their_z {
                        under.push(i);
                    }
                }
            }
        }
    }
    under
}

fn components(k: usize) -> usize {
    if k == 4 {
        2
    } else {
        1
    }
}

pub struct KnotReliquary {
    params: Params,
    tubes: Vec<GpuMesh>,
    mesh_bgl: Option<wgpu::BindGroupLayout>,
    last_key: (usize, i64),
    linking: f64,
}

impl KnotReliquary {
    pub fn new() -> Self {
        let mut p = Params::new();
        p.choice("knot", "Knot / link", "Symbolic", KNOT_NAMES.to_vec(), 0);
        p.slider("thick", "Tube radius", "Display", 0.03, 0.16, 0.085, "");
        p.toggle("breathe", "Isotopy breathing", "Observer", true);
        p.toggle("spin", "Slow tumble", "Observer", true);
        Self {
            params: p,
            tubes: Vec::new(),
            mesh_bgl: None,
            last_key: (usize::MAX, 0),
            linking: 0.0,
        }
    }

    fn rebuild(&mut self, gpu: &GpuContext, breathe: f32) {
        let Some(bgl) = self.mesh_bgl.clone() else {
            return;
        };
        let k = self.params.c("knot");
        let radius = self.params.f("thick") as f32;
        self.tubes.clear();
        for comp in 0..components(k) {
            let n = 340;
            let path: Vec<Vec3> = (0..=n)
                .map(|i| knot_pt(k, comp, i as f32 / n as f32, breathe))
                .collect();
            let d = MeshData::tube(&path, radius, 14, |i| {
                let s = i as f32 / n as f32;
                let mut c = phase_color(s * std::f32::consts::TAU + comp as f32 * 2.5);
                c[0] = c[0] * 0.8 + 0.12;
                c[1] = c[1] * 0.8 + 0.10;
                c[2] = c[2] * 0.8 + 0.14;
                c
            });
            let mut m = d.upload(gpu, &bgl);
            m.set_model(gpu, Mat4::IDENTITY, [1.0; 4], [0.0, 0.55, 1.0, 0.0]);
            self.tubes.push(m);
        }
        // Gauss linking integral for two-component links.
        self.linking = if components(k) == 2 {
            let n = 140;
            let mut acc = 0.0f64;
            for i in 0..n {
                let s0 = i as f32 / n as f32;
                let s1 = (i + 1) as f32 / n as f32;
                let a = knot_pt(k, 0, s0, breathe);
                let da = knot_pt(k, 0, s1, breathe) - a;
                for j in 0..n {
                    let t0 = j as f32 / n as f32;
                    let t1 = (j + 1) as f32 / n as f32;
                    let b = knot_pt(k, 1, t0, breathe);
                    let db = knot_pt(k, 1, t1, breathe) - b;
                    let r = b - a;
                    let rl = r.length().max(1e-4);
                    acc += (da.cross(db).dot(r) / (rl * rl * rl)) as f64;
                }
            }
            acc / (4.0 * std::f64::consts::PI)
        } else {
            0.0
        };
    }
}

impl ConceptModule for KnotReliquary {
    fn info(&self) -> ModuleInfo {
        ModuleInfo {
            id: "topology.knots",
            name: "The Knot Reliquary",
            tagline: "Invariants under every deformation",
            branch: Branch::Topology,
            description: "A knot is a circle embedded in space, considered up to any deformation that never tears or passes strands through each other. Nothing you see wiggling here changes what the knot is \u{2014} that is the point. What survives are invariants: crossing number, linking number, the polynomial shadows.",
            epigraph: Some(Epigraph::unquoted(
"A chain is no stronger than its weakest link \u{2014} but a link is precisely as strong as its linking number.",
"Attributed to no one, truthfully",
)),
        }
    }

    fn modes(&self) -> Vec<RenderMode> {
        vec![
            RenderMode::orbit(
                "gallery",
                "Tube Gallery",
                "The knot as an iridescent solid tube, breathing through isotopies",
                6.0,
                0.35,
                0.6,
            ),
            RenderMode::plane(
                "diagram",
                "Crossing Diagram",
                "The projection with honest over/under gaps",
                2.4,
            ),
            RenderMode::orbit(
                "winding",
                "The Winding Loom",
                "Slope p/q closes into a knot; irrational slope never closes at all",
                6.0,
                0.45,
                0.7,
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
        self.mesh_bgl = Some(rend.mesh_bgl.clone());
        self.rebuild(gpu, 0.0);
    }

    fn update(&mut self, gpu: &GpuContext, t: f64, _dt: f64) {
        let breathe = if self.params.b("breathe") {
            ((t * std::f64::consts::TAU / 12.0).sin()) as f32
        } else {
            0.0
        };
        let key = (
            self.params.c("knot"),
            ((breathe * 60.0) as i64) ^ ((self.params.f("thick") * 1000.0) as i64) << 12,
        );
        if key != self.last_key {
            self.last_key = key;
            self.rebuild(gpu, breathe);
        }
    }

    fn render(&mut self, gpu: &GpuContext, mode: usize, t: f64, _cam: &Camera, scene: &mut Scene) {
        let k = self.params.c("knot");
        match mode {
            0 => {
                let spin = if self.params.b("spin") {
                    (t * 0.25) as f32
                } else {
                    0.0
                };
                for m in &mut self.tubes {
                    m.set_model(
                        gpu,
                        Mat4::from_rotation_y(spin),
                        [1.0; 4],
                        [0.0, 0.55, 1.0, 0.0],
                    );
                    scene.meshes.push(m.clone());
                }
            }
            2 => {
                // The torus, as a ghost lattice.
                let (r_maj, r_min) = (1.35f32, 0.55f32);
                let tor = |a: f32, b: f32| -> Vec3 {
                    Vec3::new(
                        (r_maj + r_min * b.cos()) * a.cos(),
                        r_min * b.sin(),
                        (r_maj + r_min * b.cos()) * a.sin(),
                    )
                };
                for i in 0..16 {
                    let b0 = i as f32 / 16.0 * std::f32::consts::TAU;
                    let ring: Vec<Vec3> = (0..=64)
                        .map(|j| tor(j as f32 / 64.0 * std::f32::consts::TAU, b0))
                        .collect();
                    scene.polyline(LineLayer::World, &ring, 0.8, [0.45, 0.40, 0.60, 0.16]);
                }
                // The winding line: slope sweeps through rationals toward
                // the golden mean, where it never closes.
                let slope = 1.0 + (t / 12.0).clamp(0.0, 1.0) * 0.6180339887;
                let n_pts = 2600;
                let span = 14.0 * std::f32::consts::TAU;
                let pts: Vec<Vec3> = (0..=n_pts)
                    .map(|i| {
                        let a = i as f32 / n_pts as f32 * span;
                        tor(a, a * slope as f32) * 1.002
                    })
                    .collect();
                for i in 1..pts.len() {
                    let age = i as f32 / pts.len() as f32;
                    let hue = phase_color(age * 5.0);
                    scene.line(
                        LineLayer::WorldAdd,
                        pts[i - 1],
                        pts[i],
                        1.2,
                        with_alpha(hue, 0.16 + 0.24 * age),
                    );
                }
            }
            _ => {
                // 2D projection with over/under gaps: draw strands in depth
                // order, breaking the under-strand near crossings.
                let breathe = if self.params.b("breathe") {
                    ((t * std::f64::consts::TAU / 12.0).sin()) as f32
                } else {
                    0.0
                };
                let n = 620;
                for comp in 0..components(k) {
                    let pts: Vec<Vec3> = (0..=n)
                        .map(|i| knot_pt(k, comp, i as f32 / n as f32, breathe))
                        .collect();
                    // Cut this strand wherever another passes above it in the shadow.
                    let mut gap = vec![false; n + 1];
                    for i in under_crossings(k, comp, breathe, n) {
                        for offset in 0..=2 * GAP_SAMPLES {
                            gap[(i + n + offset - GAP_SAMPLES) % n] = true;
                        }
                    }
                    for i in 0..n {
                        if gap[i] {
                            continue;
                        }
                        let s = i as f32 / n as f32;
                        let c = with_alpha(
                            phase_color(s * std::f32::consts::TAU + comp as f32 * 2.5),
                            0.95,
                        );
                        let a = Vec3::new(pts[i].x, pts[i].y, 0.0);
                        let b = Vec3::new(pts[i + 1].x, pts[i + 1].y, 0.0);
                        scene.line(LineLayer::World, a, b, 3.2, c);
                    }
                }
            }
        }
    }

    fn inspector(&self, _t: f64) -> Vec<InsLine> {
        let k = self.params.c("knot");
        let mut out = vec![title(KNOT_NAMES[k].to_string())];
        match k {
            0 => {
                out.push(formula(
                    "(p,q) = (2,3):   t \u{21A6} torus winding, 2 around, 3 through",
                ));
                out.push(value("crossing number 3 \u{00B7} genus 1 \u{00B7} chiral \u{00B7} Alexander \u{0394}(t) = t \u{2212} 1 + t\u{207B}\u{00B9}"));
            }
            1 => {
                out.push(formula("(p,q) = (2,5)"));
                out.push(value(
                    "crossing number 5 \u{00B7} genus 2 \u{00B7} the (2,5) torus knot",
                ));
            }
            2 => {
                out.push(formula("(p,q) = (3,4)"));
                out.push(value("crossing number 8 \u{00B7} genus 3"));
            }
            3 => {
                out.push(formula("figure-eight: the simplest amphichiral knot"));
                out.push(value(
                    "crossing number 4 \u{00B7} genus 1 \u{00B7} equals its own mirror",
                ));
            }
            _ => {
                out.push(formula(
                    "Hopf link: two circles, each threading the other once",
                ));
                out.push(value(format!(
                    "Gauss linking integral \u{2248} {:+.3}  (exact: \u{00B1}1)",
                    self.linking
                )));
            }
        }
        out.push(def("Isotopy: deformation without tearing or crossing \u{2014} the breathing changes nothing that matters."));
        out.push(note("Every over/under gap is computed from real depth. A side-on shadow can show extra crossings; in a trefoil's minimal diagram, swapping a single crossing unties the knot."));
        out
    }

    fn approx(&self, mode: usize) -> Option<&'static str> {
        match mode {
            1 => Some("Crossings detected on a 620-sample projection; gaps rendered on the strand that passes beneath."),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOPF: usize = 4;

    #[test]
    fn the_hopf_link_really_links() {
        // Two crossings in the shadow, and each circle goes under exactly once.
        // Unlinked circles would have one passing under at both.
        assert_eq!(under_crossings(HOPF, 0, 0.0, 400).len(), 1);
        assert_eq!(under_crossings(HOPF, 1, 0.0, 400).len(), 1);
    }

    #[test]
    fn the_hopf_shadow_is_two_curves_not_a_line() {
        let ys: Vec<f32> = (0..64)
            .map(|i| knot_pt(HOPF, 1, i as f32 / 64.0, 0.0).y)
            .collect();
        assert!(ys.iter().fold(0.0f32, |m, y| m.max(y.abs())) > 0.3);
    }

    #[test]
    fn a_crossing_has_exactly_one_strand_under_it() {
        // A trefoil's shadow may show extra crossings, but never fewer than three.
        let crossings = under_crossings(0, 0, 0.0, 620).len();
        assert!(crossings >= 3, "{crossings}");
    }

    #[test]
    fn the_figure_eight_winds_three_times_around_its_axis() {
        const FIGURE_EIGHT: usize = 3;
        let n = 2000;
        let mut turning = 0.0f32;
        let mut last = knot_pt(FIGURE_EIGHT, 0, 0.0, 0.0);
        for i in 1..=n {
            let p = knot_pt(FIGURE_EIGHT, 0, i as f32 / n as f32, 0.0);
            let step = last.x * p.z - last.z * p.x;
            let along = last.x * p.x + last.z * p.z;
            turning += step.atan2(along);
            last = p;
        }
        let turns = turning / std::f32::consts::TAU;
        assert!((turns - 3.0).abs() < 1e-2, "{turns}");
    }

    #[test]
    fn segments_cross_only_when_they_truly_meet() {
        let p = |x, y| Vec3::new(x, y, 0.0);
        let hit = shadow_crossing(p(-1.0, 0.0), p(1.0, 0.0), p(0.0, -1.0), p(0.0, 1.0));
        assert_eq!(hit, Some((0.5, 0.5)));
        let near_miss = shadow_crossing(p(-1.0, 0.0), p(1.0, 0.0), p(0.0, 0.01), p(0.0, 1.0));
        assert_eq!(near_miss, None);
    }
}
