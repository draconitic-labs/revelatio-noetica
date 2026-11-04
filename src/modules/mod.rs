//! The atlas: every concept family registered with the engine.

pub mod util;

pub mod attractor;
pub mod domain_coloring;
pub mod dynamics;
pub mod em;
pub mod fluxion;
pub mod forms;
pub mod fourier;
pub mod harmonics;
pub mod hydrogen;
pub mod knots;
pub mod linalg;
pub mod manifold;
pub mod montecarlo;
pub mod number_theory;
pub mod odes;
pub mod pdes;
pub mod relativity;
pub mod schrodinger;
pub mod schwarzschild;
pub mod statmech;
pub mod taylor;
pub mod vector_field;
pub mod wave;

use crate::engine::module::ConceptModule;

pub fn all() -> Vec<Box<dyn ConceptModule>> {
    vec![
        Box::new(fluxion::FluxionChamber::new()),
        Box::new(domain_coloring::DomainColoring::new()),
        Box::new(fourier::FourierAtelier::new()),
        Box::new(vector_field::VectorField::new()),
        Box::new(linalg::LinearAtelier::new()),
        Box::new(wave::WaveTable::new()),
        Box::new(schrodinger::SchrodingerHall::new()),
        Box::new(hydrogen::HydrogenSanctum::new()),
        Box::new(relativity::RelativityTheatre::new()),
        Box::new(schwarzschild::SchwarzschildCourt::new()),
        Box::new(forms::FormsOratory::new()),
        Box::new(manifold::ManifoldAtlas::new()),
        Box::new(knots::KnotReliquary::new()),
        Box::new(dynamics::DynamicsHall::new()),
        Box::new(attractor::ChaosObservatory::new()),
        Box::new(em::FieldForge::new()),
        Box::new(statmech::EnsembleGallery::new()),
        Box::new(number_theory::ArithmeticRotunda::new()),
        Box::new(taylor::ApproximationAtelier::new()),
        Box::new(harmonics::SphericalChoir::new()),
        Box::new(odes::OdeWorkshop::new()),
        Box::new(pdes::PdeHall::new()),
        Box::new(montecarlo::MonteCarloCasino::new()),
    ]
}
