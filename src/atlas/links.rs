//! How the chambers connect. A `BuildsOn` link means the mathematics of `to`
//! uses the mathematics of `from`; a `Related` link is a parallel worth seeing,
//! with neither one required first. Each `why` must read correctly from either
//! end, and states an established fact rather than a claim about this program.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinkKind {
    BuildsOn,
    Related,
}

pub struct Link {
    pub from: &'static str,
    /// View to open when arriving at `from`, if one shows the connection best.
    pub from_view: Option<&'static str>,
    pub to: &'static str,
    /// View to open when arriving at `to`, if one shows the connection best.
    pub to_view: Option<&'static str>,
    pub kind: LinkKind,
    pub why: &'static str,
}

const fn builds(
    from: &'static str,
    to: &'static str,
    to_view: Option<&'static str>,
    why: &'static str,
) -> Link {
    Link {
        from,
        from_view: None,
        to,
        to_view,
        kind: LinkKind::BuildsOn,
        why,
    }
}

const fn related(
    from: (&'static str, Option<&'static str>),
    to: (&'static str, Option<&'static str>),
    why: &'static str,
) -> Link {
    Link {
        from: from.0,
        from_view: from.1,
        to: to.0,
        to_view: to.1,
        kind: LinkKind::Related,
        why,
    }
}

pub const LINKS: &[Link] = &[
    builds("newton.fluxions", "analysis.taylor", Some("taylor"),
        "Taylor polynomials are assembled from repeated derivatives at one point."),
    builds("newton.fluxions", "diffeq.odes", Some("slope"),
        "A differential equation prescribes a rate of change and asks for the quantity that has it."),
    builds("newton.fluxions", "vector.fields", Some("heat"),
        "Divergence and curl are built from partial derivatives of a field."),
    builds("analysis.taylor", "complex.domain", Some("dc"),
        "A power series converges on a disc that reaches only to the nearest complex singularity."),
    builds("analysis.fourier", "diffeq.pdes", Some("string"),
        "Fourier series solve the heat equation and the vibrating string, one mode at a time."),
    builds("analysis.fourier", "waves.harmonics", Some("breathe"),
        "Spherical harmonics play the role on a sphere that sines and cosines play on a circle."),
    builds("vector.fields", "forms.stokes", Some("stokes"),
        "Green's theorem in the plane is the two-dimensional case of Stokes' theorem."),
    builds("vector.fields", "em.fields", Some("lines"),
        "Electric and magnetic fields are vector fields, and Maxwell's equations are written in flux and circulation."),
    builds("linalg.transform", "relativity.special", Some("minkowski"),
        "A Lorentz boost is a linear map of spacetime that preserves the interval."),
    builds("linalg.transform", "quantum.schrodinger", Some("phasor"),
        "Energy eigenstates are eigenvectors of the Hamiltonian operator."),
    builds("linalg.transform", "geometry.manifolds", Some("tangent"),
        "The metric assigns a symmetric matrix, an inner product, to each tangent plane."),
    builds("geometry.manifolds", "gr.schwarzschild", Some("funnel"),
        "General relativity describes gravity as the curvature of spacetime."),
    builds("relativity.special", "gr.schwarzschild", Some("lensing"),
        "In a small enough region, general relativity reduces to special relativity."),
    builds("diffeq.odes", "dynamics.phase", Some("portrait"),
        "A phase portrait draws every solution of a differential equation at once."),
    builds("dynamics.phase", "dynamics.attractor", Some("swarm"),
        "An autonomous flow needs at least three dimensions for chaos; in the plane it cannot occur."),
    builds("diffeq.odes", "diffeq.pdes", Some("heat"),
        "Discretizing space turns a partial differential equation into a large system of coupled ordinary ones."),
    builds("diffeq.pdes", "waves.membrane", Some("amplitude"),
        "The membrane is the two-dimensional wave equation, stepped on a lattice."),
    builds("diffeq.pdes", "quantum.schrodinger", Some("psi"),
        "The Schr\u{00F6}dinger equation is a linear partial differential equation for the amplitude \u{03C8}."),
    builds("waves.membrane", "em.fields", Some("wave"),
        "In vacuum, each component of E and B satisfies the wave equation."),
    builds("quantum.schrodinger", "quantum.hydrogen", Some("cloud"),
        "Hydrogen's orbitals are stationary states of the Schr\u{00F6}dinger equation in a Coulomb potential."),
    builds("waves.harmonics", "quantum.hydrogen", Some("nodal"),
        "The angular part of every hydrogen orbital is a spherical harmonic."),
    builds("statmech.montecarlo", "statmech.ensemble", Some("ising"),
        "The Metropolis algorithm that samples Ising spins is a Monte Carlo method."),
    builds("em.fields", "relativity.special", Some("charge"),
        "Special relativity began as the electrodynamics of moving bodies, and a moving charge's field shows why."),
    related(("number.rotunda", Some("zeta")), ("complex.domain", Some("dc")),
        "The Riemann zeta function is a complex function, and the walk follows it along the critical line."),
    related(("analysis.taylor", Some("newton")), ("complex.domain", Some("julia")),
        "Newton's method on the complex plane and the map z\u{00B2}+c both draw Julia sets as their boundaries."),
    related(("analysis.taylor", Some("taylor")), ("analysis.fourier", Some("gibbs")),
        "Both truncate an infinite series, and both show where a finite sum stops being a good stand-in."),
    related(("statmech.ensemble", Some("walk")), ("diffeq.pdes", Some("heat")),
        "The density of many independent random walkers obeys the diffusion equation."),
    related(("statmech.montecarlo", None), ("quantum.hydrogen", Some("walkers")),
        "Metropolis walkers can sample any density they can evaluate, including |\u{03C8}|\u{00B2}."),
    related(("newton.fluxions", Some("principia")), ("gr.schwarzschild", Some("orbit")),
        "Newtonian gravity closes a bound orbit into an ellipse; the Schwarzschild correction makes it precess."),
    related(("dynamics.attractor", Some("logistic")), ("complex.domain", Some("mandel")),
        "The real line of the Mandelbrot set holds the same period-doubling cascade as the logistic map."),
    related(("waves.membrane", Some("chladni")), ("waves.harmonics", Some("nodal")),
        "A vibrating square and a vibrating sphere each have nodal lines, where the mode never moves."),
    related(("forms.stokes", None), ("geometry.manifolds", None),
        "Differential forms are the objects that integrate naturally over manifolds."),
    related(("topology.knots", None), ("geometry.manifolds", None),
        "Geometry measures shape with a metric; knot invariants keep what survives when distances are ignored."),
    related(("newton.fluxions", Some("fluent")), ("dynamics.phase", Some("energy")),
        "Hamilton's equations give the fluxions of position and momentum."),
    related(("linalg.transform", Some("grid")), ("complex.domain", Some("conformal")),
        "A holomorphic map acts locally as a rotation and a scaling, a special kind of 2\u{00D7}2 matrix."),
    related(("quantum.schrodinger", Some("psi")), ("waves.membrane", Some("amplitude")),
        "Both are linear wave equations whose solutions superpose, though \u{03C8} is first order in time and complex."),
    related(("dynamics.attractor", Some("threebody")), ("newton.fluxions", Some("principia")),
        "Newtonian gravity with two bodies gives conic sections; with three it has no general closed form."),
];

/// One end of a link as seen from a chamber.
pub struct Neighbour {
    pub chamber: &'static str,
    pub view: Option<&'static str>,
    pub why: &'static str,
}

/// Chambers whose mathematics `id` uses.
pub fn builds_on(id: &str) -> Vec<Neighbour> {
    LINKS
        .iter()
        .filter(|l| l.kind == LinkKind::BuildsOn && l.to == id)
        .map(|l| Neighbour {
            chamber: l.from,
            view: l.from_view,
            why: l.why,
        })
        .collect()
}

/// Chambers that use the mathematics of `id`.
pub fn leads_to(id: &str) -> Vec<Neighbour> {
    LINKS
        .iter()
        .filter(|l| l.kind == LinkKind::BuildsOn && l.from == id)
        .map(|l| Neighbour {
            chamber: l.to,
            view: l.to_view,
            why: l.why,
        })
        .collect()
}

/// Parallels with `id`, in either direction.
pub fn related_to(id: &str) -> Vec<Neighbour> {
    LINKS
        .iter()
        .filter(|l| l.kind == LinkKind::Related && (l.from == id || l.to == id))
        .map(|l| {
            if l.from == id {
                Neighbour {
                    chamber: l.to,
                    view: l.to_view,
                    why: l.why,
                }
            } else {
                Neighbour {
                    chamber: l.from,
                    view: l.from_view,
                    why: l.why,
                }
            }
        })
        .collect()
}

/// Where each chamber sits on the map, as fractions of the chart. Left to
/// right follows `BuildsOn`: foundations on the left, consequences on the right.
pub const CHART: &[(&str, f32, f32)] = &[
    ("number.rotunda", 0.05, 0.08),
    ("newton.fluxions", 0.05, 0.40),
    ("analysis.fourier", 0.05, 0.72),
    ("statmech.montecarlo", 0.05, 0.96),
    ("complex.domain", 0.235, 0.03),
    ("analysis.taylor", 0.225, 0.20),
    ("vector.fields", 0.225, 0.38),
    ("diffeq.odes", 0.225, 0.56),
    ("linalg.transform", 0.42, 0.10),
    ("forms.stokes", 0.42, 0.30),
    ("dynamics.phase", 0.42, 0.50),
    ("diffeq.pdes", 0.42, 0.72),
    ("statmech.ensemble", 0.42, 0.96),
    ("topology.knots", 0.60, 0.02),
    ("geometry.manifolds", 0.60, 0.20),
    ("dynamics.attractor", 0.60, 0.42),
    ("waves.membrane", 0.60, 0.64),
    ("waves.harmonics", 0.60, 0.88),
    ("relativity.special", 0.85, 0.12),
    ("em.fields", 0.76, 0.44),
    ("quantum.schrodinger", 0.76, 0.72),
    ("gr.schwarzschild", 0.97, 0.30),
    ("quantum.hydrogen", 0.97, 0.80),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules;
    use std::collections::{HashMap, HashSet};

    fn views() -> HashMap<&'static str, Vec<&'static str>> {
        modules::all()
            .iter()
            .map(|m| (m.info().id, m.modes().iter().map(|r| r.id).collect()))
            .collect()
    }

    #[test]
    fn every_link_names_real_chambers_and_views() {
        let views = views();
        for l in LINKS {
            for (chamber, view) in [(l.from, l.from_view), (l.to, l.to_view)] {
                let known = views
                    .get(chamber)
                    .unwrap_or_else(|| panic!("no chamber {chamber}"));
                if let Some(v) = view {
                    assert!(known.contains(&v), "{chamber} has no view {v}");
                }
            }
        }
    }

    #[test]
    fn no_chamber_is_an_island() {
        for (id, _) in views() {
            let n = builds_on(id).len() + leads_to(id).len() + related_to(id).len();
            assert!(n > 0, "{id} has no connections");
        }
    }

    #[test]
    fn prerequisites_never_loop() {
        let ids: Vec<&str> = views().keys().copied().collect();
        fn reaches(from: &str, target: &str, seen: &mut HashSet<String>) -> bool {
            leads_to(from).iter().any(|n| {
                n.chamber == target
                    || (seen.insert(n.chamber.to_string()) && reaches(n.chamber, target, seen))
            })
        }
        for id in ids {
            assert!(
                !reaches(id, id, &mut HashSet::new()),
                "{id} is its own prerequisite"
            );
        }
    }

    #[test]
    fn links_are_not_duplicated() {
        let mut seen = HashSet::new();
        for l in LINKS {
            let (a, b) = if l.from < l.to {
                (l.from, l.to)
            } else {
                (l.to, l.from)
            };
            let views = (l.from_view, l.to_view);
            assert!(seen.insert((a, b, views)), "{a} and {b} are linked twice");
        }
    }

    #[test]
    fn the_chart_places_every_chamber_once_inside_the_frame() {
        let views = views();
        assert_eq!(CHART.len(), views.len());
        let mut seen = HashSet::new();
        for &(id, x, y) in CHART {
            assert!(views.contains_key(id), "chart names unknown chamber {id}");
            assert!(seen.insert(id), "{id} is on the chart twice");
            assert!(
                (0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y),
                "{id} off chart"
            );
        }
    }

    #[test]
    fn prerequisites_run_left_to_right_on_the_chart() {
        let x: HashMap<&str, f32> = CHART.iter().map(|&(id, x, _)| (id, x)).collect();
        for l in LINKS.iter().filter(|l| l.kind == LinkKind::BuildsOn) {
            assert!(x[l.from] < x[l.to], "{} should be left of {}", l.from, l.to);
        }
    }
}
