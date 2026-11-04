//! The atlas as data: how chambers group, connect, and read. Pure tables and
//! logic with no GPU or UI, checked by tests against the real modules.

pub mod guides;
pub mod links;
pub mod presets;
pub mod search;
pub mod trails;

/// A shelf of the atlas: related chambers shown together.
pub struct Domain {
    pub name: &'static str,
    pub chambers: &'static [&'static str],
}

pub const DOMAINS: &[Domain] = &[
    Domain {
        name: "Calculus and analysis",
        chambers: &[
            "newton.fluxions",
            "analysis.taylor",
            "analysis.fourier",
            "complex.domain",
        ],
    },
    Domain {
        name: "Fields and forms",
        chambers: &["vector.fields", "forms.stokes"],
    },
    Domain {
        name: "Space and shape",
        chambers: &["linalg.transform", "geometry.manifolds", "topology.knots"],
    },
    Domain {
        name: "Equations of change",
        chambers: &[
            "diffeq.odes",
            "diffeq.pdes",
            "dynamics.phase",
            "dynamics.attractor",
        ],
    },
    Domain {
        name: "Waves and light",
        chambers: &["waves.membrane", "waves.harmonics", "em.fields"],
    },
    Domain {
        name: "Quantum",
        chambers: &["quantum.schrodinger", "quantum.hydrogen"],
    },
    Domain {
        name: "Spacetime",
        chambers: &["relativity.special", "gr.schwarzschild"],
    },
    Domain {
        name: "Chance and number",
        chambers: &["statmech.montecarlo", "statmech.ensemble", "number.rotunda"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules;

    #[test]
    fn every_chamber_sits_on_exactly_one_shelf() {
        let all = modules::all();
        for m in &all {
            let id = m.info().id;
            let shelves = DOMAINS.iter().filter(|d| d.chambers.contains(&id)).count();
            assert_eq!(shelves, 1, "{id} is on {shelves} shelves");
        }
        let listed: usize = DOMAINS.iter().map(|d| d.chambers.len()).sum();
        assert_eq!(listed, all.len());
    }
}
