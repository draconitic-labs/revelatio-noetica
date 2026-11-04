//! Guided paths: ordered walks through views, across chambers, each step saying
//! why it follows the last. Every step's `say` is written against what that view
//! draws, checked by tests for existence and by reading for accuracy.

pub struct Step {
    pub module: &'static str,
    pub view: &'static str,
    pub say: &'static str,
}

pub struct Trail {
    pub name: &'static str,
    pub summary: &'static str,
    pub steps: &'static [Step],
}

const fn s(module: &'static str, view: &'static str, say: &'static str) -> Step {
    Step { module, view, say }
}

pub const TRAILS: &[Trail] = &[
    Trail {
        name: "From motion to orbits",
        summary: "Newton's fluxions, from a moving point to a precessing planet",
        steps: &[
            s("newton.fluxions", "fluent", "A moving point generates the curve, and the ordinate sweeps out the area beneath it."),
            s("newton.fluxions", "fluxion", "The fluxions are the velocities of that motion, composed into the tangent."),
            s("newton.fluxions", "principia", "Under an inverse-square force the same language gives Kepler's law of equal areas."),
            s("dynamics.attractor", "threebody", "Add a third body and no general closed form remains. This figure-eight is a special periodic solution."),
            s("gr.schwarzschild", "orbit", "General relativity corrects the two-body problem too: the bound orbit no longer closes, and precesses."),
        ],
    },
    Trail {
        name: "Circles into waves",
        summary: "Fourier's idea, followed from a chain of circles to the hydrogen atom",
        steps: &[
            s("analysis.fourier", "epicycles", "A chain of rotating circles traces the partial sum of a Fourier series."),
            s("analysis.fourier", "stack", "The same sum, unstacked: each harmonic alone, then all of them together."),
            s("analysis.fourier", "gibbs", "Truncation has a price. At a jump the overshoot narrows but keeps its height."),
            s("diffeq.pdes", "string", "On a vibrating string those sines are the eigenmodes, each with its own frequency."),
            s("waves.membrane", "chladni", "In two dimensions a square membrane's modes have nodal lines, drawn here as Chladni's sand."),
            s("waves.harmonics", "breathe", "On a sphere the modes become spherical harmonics."),
            s("quantum.hydrogen", "cloud", "Those same harmonics are the angular part of every hydrogen orbital."),
        ],
    },
    Trail {
        name: "Order out of chance",
        summary: "Random samples that compute, converge, and organize",
        steps: &[
            s("statmech.montecarlo", "pi", "Darts in a square estimate \u{03C0}: a number computed with nothing but chance."),
            s("statmech.montecarlo", "galton", "Sum many coin flips and a bell curve appears: the central limit theorem."),
            s("statmech.ensemble", "walk", "Thousands of random walks spread as \u{221A}t, as the diffusion equation says."),
            s("statmech.ensemble", "ising", "With spins that feel their neighbours, chance produces collective order below the critical temperature."),
            s("quantum.hydrogen", "walkers", "The same Metropolis sampling draws a quantum density, |\u{03C8}|\u{00B2}, point by point."),
        ],
    },
    Trail {
        name: "From rules to chaos",
        summary: "One differential equation at a time, until prediction fails",
        steps: &[
            s("diffeq.odes", "slope", "A differential equation is a rule at every point. A solution threads through it."),
            s("diffeq.odes", "euler", "Numerical solutions differ. With the same step, Euler drifts where RK4 holds."),
            s("dynamics.phase", "portrait", "A phase portrait draws every solution of the pendulum at once."),
            s("dynamics.phase", "fan", "Join a second pendulum to the first and nearby starts come apart."),
            s("dynamics.attractor", "swarm", "In the Lorenz flow, trajectories separate yet stay on one shape, the attractor."),
            s("dynamics.attractor", "logistic", "Even a one-line map reaches chaos, through a cascade of period doublings."),
        ],
    },
    Trail {
        name: "The shape of space",
        summary: "From flux through a loop to light bent by curved spacetime",
        steps: &[
            s("vector.fields", "probe", "A probe circle measures a field's flux and circulation, and Green's theorem relates them to its interior."),
            s("forms.stokes", "stokes", "Stokes' theorem lifts the same statement onto a curved surface and its rim."),
            s("geometry.manifolds", "transport", "Carry a vector around a loop on a curved surface and it returns turned."),
            s("geometry.manifolds", "curvature", "That turning measures Gaussian curvature, positive on some of a torus and negative on the rest."),
            s("gr.schwarzschild", "funnel", "Curvature can belong to space itself: a slice of Schwarzschild geometry, embedded as a funnel."),
            s("gr.schwarzschild", "lensing", "Light follows geodesics of that geometry, so the image of the disk bends around the hole."),
        ],
    },
    Trail {
        name: "Light and moving observers",
        summary: "From static charges to the geometry of spacetime",
        steps: &[
            s("em.fields", "lines", "Static charges fill space with field lines that begin and end on them."),
            s("em.fields", "dipole", "Shake a charge pair and the field lines detach and leave: radiation."),
            s("em.fields", "wave", "Far away, E and B are perpendicular, in phase, and carry each other along."),
            s("relativity.special", "charge", "Move a charge at high speed and its field flattens across the direction of motion."),
            s("relativity.special", "minkowski", "The Lorentz transformation tilts time and space axes toward the light cone, and the cone stays put."),
            s("relativity.special", "twins", "The interval along a path is proper time, so two paths between the same events can age differently."),
        ],
    },
];

/// Trails that pass through a chamber.
pub fn through(id: &str) -> impl Iterator<Item = (usize, &'static Trail)> + '_ {
    TRAILS
        .iter()
        .enumerate()
        .filter(move |(_, t)| t.steps.iter().any(|s| s.module == id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules;

    #[test]
    fn every_step_names_a_real_view() {
        let all = modules::all();
        for t in TRAILS {
            assert!(t.steps.len() >= 3, "{} is too short to be a path", t.name);
            for s in t.steps {
                let m = all
                    .iter()
                    .find(|m| m.info().id == s.module)
                    .unwrap_or_else(|| panic!("{}: no chamber {}", t.name, s.module));
                assert!(
                    m.modes().iter().any(|r| r.id == s.view),
                    "{}: {} has no view {}",
                    t.name,
                    s.module,
                    s.view
                );
            }
        }
    }
}
