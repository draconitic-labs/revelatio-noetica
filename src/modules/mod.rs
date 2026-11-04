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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::gpu::GpuContext;
    use crate::engine::module::{Engine, ViewRef};

    /// Views that are data edge to edge, so no pixel is empty canvas. Everything
    /// else must leave its empty canvas exactly #000000.
    const FILLS_THE_CANVAS: &[(&str, &str)] = &[
        ("complex.domain", "dc"),
        ("vector.fields", "heat"),
        ("analysis.taylor", "newton"),
    ];

    /// Renders every view through the real pipelines. wgpu reports shader or
    /// binding mistakes as panics, so this also proves each view is valid.
    /// Needs a Vulkan or other GPU adapter; lavapipe is enough. Without one it
    /// says so and passes, because a missing driver is not a code failure.
    #[test]
    fn every_view_renders_and_leaves_its_empty_canvas_true_black() {
        let gpu = match GpuContext::headless() {
            Ok(gpu) => gpu,
            Err(e) => {
                eprintln!("skipped: no usable GPU adapter here: {e:#}");
                return;
            }
        };
        let mut e = Engine::new(gpu, all());
        let mut failures = Vec::new();
        for module in 0..e.modules.len() {
            let id = e.modules[module].info().id;
            e.set_active(module);
            e.tick(0.0);
            for _ in 0..12 {
                e.tick(0.1);
            }
            let views: Vec<&str> = e.modules[module].modes().iter().map(|m| m.id).collect();
            for (mode, view) in views.into_iter().enumerate() {
                let px = e
                    .render_still(ViewRef { module, mode }, 160, 90)
                    .unwrap_or_else(|err| panic!("{id}/{view}: {err:#}"));
                assert!(
                    px.chunks_exact(4).all(|p| p[3] == 255),
                    "{id}/{view} is not opaque"
                );
                let has_black = px.chunks_exact(4).any(|p| p[..3] == [0, 0, 0]);
                if !has_black && !FILLS_THE_CANVAS.contains(&(id, view)) {
                    failures.push(format!("{id}/{view}"));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "no #000000 pixel in these views: {failures:?}"
        );
    }
}
