//! Named settings per chamber: real parameter values worth visiting.

use crate::engine::params::{PValue, Params};

#[derive(Clone, Copy, Debug)]
pub enum V {
    F(f64),
    I(i64),
    B(bool),
    C(usize),
}

impl V {
    pub fn value(self) -> PValue {
        match self {
            V::F(v) => PValue::F(v),
            V::I(v) => PValue::I(v),
            V::B(v) => PValue::B(v),
            V::C(v) => PValue::C(v),
        }
    }
}

pub struct Preset {
    pub module: &'static str,
    pub name: &'static str,
    /// View to open with this preset, or None to stay on the current one.
    pub view: Option<&'static str>,
    pub note: &'static str,
    pub values: &'static [(&'static str, V)],
}

impl Preset {
    pub fn apply(&self, params: &mut Params) {
        for &(key, v) in self.values {
            params.set(key, v.value());
        }
    }

    /// True while every value it sets is still in place.
    pub fn is_active(&self, params: &Params) -> bool {
        self.values.iter().all(|&(key, v)| {
            params
                .param(key)
                .is_some_and(|p| match (p.value, v.value()) {
                    (PValue::F(a), PValue::F(b)) => (a - b).abs() < 1e-9,
                    (a, b) => a == b,
                })
        })
    }
}

pub const PRESETS: &[Preset] = &[
// newton.fluxions
Preset {
    module: "newton.fluxions",
    name: "Constant acceleration",
    view: Some("fluxion"),
    note: "Parabola: ẍ = 0 and ÿ is constant, so the burnt-orange arrow never changes.",
    values: &[("curve", V::C(1)), ("accel", V::B(true))],
},
Preset {
    module: "newton.fluxions",
    name: "Uniform circular motion",
    view: Some("fluxion"),
    note: "Semicircle at constant angular speed: acceleration points at the centre with constant length.",
    values: &[("curve", V::C(4)), ("accel", V::B(true))],
},
Preset {
    module: "newton.fluxions",
    name: "Kepler at high eccentricity",
    view: Some("principia"),
    note: "e = 0.8: speed and force vary strongly, while wedges keep equal area.",
    values: &[("ecc", V::F(0.8))],
},
Preset {
    module: "newton.fluxions",
    name: "Circular orbit",
    view: Some("principia"),
    note: "e = 0: foci coincide, speed and force are constant, wedges are congruent.",
    values: &[("ecc", V::F(0.0))],
},

// complex.domain
Preset {
    module: "complex.domain",
    name: "Confocal parabolas",
    view: Some("conformal"),
    note: "z² sends grid lines to confocal parabolas with focus at 0.",
    values: &[("fn", V::C(0)), ("animate", V::B(false)), ("morph", V::F(1.0))],
},
Preset {
    module: "complex.domain",
    name: "Essential singularity",
    view: Some("dc"),
    note: "Near 0, exp(1/z) takes every nonzero value infinitely often (great Picard).",
    values: &[("fn", V::C(8))],
},
Preset {
    module: "complex.domain",
    name: "Blaschke factor, fixed a",
    view: Some("landscape"),
    note: "|B| = 1 on the unit circle, so height is constant along it.",
    values: &[("fn", V::C(7)), ("animate", V::B(false))],
},
Preset {
    module: "complex.domain",
    name: "Inversion on the sphere",
    view: Some("sphere"),
    note: "1/z: pole at the bottom (z = 0), zero at the top (z = ∞).",
    values: &[("fn", V::C(2))],
},

// analysis.fourier
Preset {
    module: "analysis.fourier",
    name: "Single harmonic",
    view: Some("epicycles"),
    note: "One circle: a sine of amplitude 4/π ≈ 1.27.",
    values: &[("wave", V::C(0)), ("n", V::I(1))],
},
Preset {
    module: "analysis.fourier",
    name: "Square to 63rd harmonic",
    view: Some("stack"),
    note: "32 odd terms in the sum; only the first nine are drawn singly.",
    values: &[("wave", V::C(0)), ("n", V::I(64))],
},
Preset {
    module: "analysis.fourier",
    name: "Triangle has no overshoot",
    view: Some("gibbs"),
    note: "Continuous waveform: partial sums converge uniformly, with no Gibbs overshoot at any N.",
    values: &[("wave", V::C(2))],
},

// analysis.taylor
Preset {
    module: "analysis.taylor",
    name: "Poles at ±i set the radius",
    view: Some("taylor"),
    note: "1/(1+x²) is smooth on the real line, yet the series diverges past |x| = 1.",
    values: &[("fn", V::C(3)), ("grow", V::B(false)), ("deg", V::I(24))],
},
Preset {
    module: "analysis.taylor",
    name: "Logarithm at degree 24",
    view: Some("taylor"),
    note: "Series converges on (−1, 1]; past x = 1 it diverges though ln(1+x) is finite.",
    values: &[("fn", V::C(2)), ("grow", V::B(false)), ("deg", V::I(24))],
},
Preset {
    module: "analysis.taylor",
    name: "Sine at degree 24",
    view: Some("taylor"),
    note: "Entire function: terms through x²³ match sin x across the plotted window.",
    values: &[("fn", V::C(0)), ("grow", V::B(false)), ("deg", V::I(24))],
},
Preset {
    module: "analysis.taylor",
    name: "Barnsley fern",
    view: Some("chaosgame"),
    note: "The standard four-map IFS, with probabilities 0.01, 0.85, 0.07 and 0.07.",
    values: &[("fractal", V::C(1))],
},

// vector.fields
Preset {
    module: "vector.fields",
    name: "Rigid rotation",
    view: Some("heat"),
    note: "F = (−y, x): curl is 2 everywhere and divergence is 0.",
    values: &[("field", V::C(0)), ("curlheat", V::B(true))],
},
Preset {
    module: "vector.fields",
    name: "Straight flow with curl",
    view: Some("heat"),
    note: "F = (y, 0): straight streamlines, yet curl is −1 everywhere.",
    values: &[("field", V::C(3)), ("curlheat", V::B(true))],
},
Preset {
    module: "vector.fields",
    name: "Saddle: no div, no curl",
    view: Some("heat"),
    note: "F = (x, −y): divergence and curl both vanish, so the tint stays neutral.",
    values: &[("field", V::C(2))],
},
Preset {
    module: "vector.fields",
    name: "Flux of one source",
    view: Some("probe"),
    note: "The circle encloses only the source, so the outward flux is 2π.",
    values: &[("field", V::C(1)), ("probe_x", V::F(1.0)), ("probe_y", V::F(0.0)), ("probe_r", V::F(0.9))],
},

// forms.stokes
Preset {
    module: "forms.stokes",
    name: "Four cells, no cancellation",
    view: Some("cancel"),
    note: "Every interior edge shows two opposed arrows from neighbouring cells.",
    values: &[("cells", V::I(2)), ("autocancel", V::B(false)), ("cancel", V::F(0.0))],
},
Preset {
    module: "forms.stokes",
    name: "Interior fully cancelled",
    view: Some("cancel"),
    note: "144 cells: interior arrows drop to 3% opacity and the copper rim remains.",
    values: &[("cells", V::I(12)), ("autocancel", V::B(false)), ("cancel", V::F(1.0))],
},
Preset {
    module: "linalg.transform",
    name: "Rotation by 30°",
    view: Some("eigen"),
    note: "Eigenvalues e^(±iπ/6), det 1: iterates circle the origin, shrunk by the 0.999 factor.",
    values: &[("a", V::F(0.8660254)), ("c", V::F(0.5)), ("b", V::F(-0.5)), ("d", V::F(0.8660254)), ("animate", V::B(false)), ("blend", V::F(1.0))],
},
Preset {
    module: "linalg.transform",
    name: "Shear",
    view: Some("eigen"),
    note: "Repeated eigenvalue 1 with one eigendirection, the x axis; area is preserved.",
    values: &[("a", V::F(1.0)), ("c", V::F(0.0)), ("b", V::F(1.0)), ("d", V::F(1.0)), ("animate", V::B(false)), ("blend", V::F(1.0))],
},
Preset {
    module: "linalg.transform",
    name: "Reflection in the x axis",
    view: Some("grid"),
    note: "Determinant −1: the outlined square turns red; eigenvalues are +1 and −1.",
    values: &[("a", V::F(1.0)), ("c", V::F(0.0)), ("b", V::F(0.0)), ("d", V::F(-1.0)), ("animate", V::B(false)), ("blend", V::F(1.0))],
},
Preset {
    module: "linalg.transform",
    name: "Rank-one matrix",
    view: Some("svd"),
    note: "Determinant 0 and σ₂ = 0: the circle flattens to a segment of half-length 2.5.",
    values: &[("a", V::F(1.0)), ("c", V::F(0.5)), ("b", V::F(2.0)), ("d", V::F(1.0)), ("animate", V::B(false)), ("blend", V::F(1.0))],
},
Preset {
    module: "geometry.manifolds",
    name: "Half-turn holonomy",
    view: Some("transport"),
    note: "θ₀ = π/3 gives holonomy π: the vector returns reversed.",
    values: &[("surf", V::C(0)), ("lat", V::F(std::f64::consts::FRAC_PI_3))],
},
Preset {
    module: "geometry.manifolds",
    name: "Small polar loop",
    view: Some("transport"),
    note: "θ₀ = 0.15 encloses little curvature: holonomy is about 4.0°.",
    values: &[("surf", V::C(0)), ("lat", V::F(0.15))],
},
Preset {
    module: "geometry.manifolds",
    name: "Torus curvature",
    view: Some("curvature"),
    note: "Copper outer side, blue inner side; total curvature is zero.",
    values: &[("surf", V::C(1)), ("kheat", V::B(true))],
},
Preset {
    module: "geometry.manifolds",
    name: "Saddle curvature",
    view: Some("curvature"),
    note: "K < 0 everywhere, so the surface is blue, strongest at the centre.",
    values: &[("surf", V::C(2)), ("kheat", V::B(true))],
},
Preset {
    module: "topology.knots",
    name: "Trefoil at rest",
    view: Some("gallery"),
    note: "The (2,3) torus knot with breathing and tumble off; only orbiting moves it.",
    values: &[("knot", V::C(0)), ("breathe", V::B(false)), ("spin", V::B(false))],
},
Preset {
    module: "topology.knots",
    name: "Cinquefoil",
    view: Some("gallery"),
    note: "The (2,5) torus knot: crossing number 5, genus 2.",
    values: &[("knot", V::C(1))],
},
Preset {
    module: "topology.knots",
    name: "Thin (3,4) torus knot",
    view: Some("gallery"),
    note: "Crossing number 8, genus 3; a thinner tube leaves wider gaps between strands.",
    values: &[("knot", V::C(2)), ("thick", V::F(0.05))],
},
Preset {
    module: "topology.knots",
    name: "Hopf link",
    view: Some("gallery"),
    note: "Two circles in tilted planes, each through the other: linking number ±1.",
    values: &[("knot", V::C(4))],
},
Preset {
    module: "number.rotunda",
    name: "Cardioid, k = 2",
    view: Some("cardioid"),
    note: "One cusp: with N = 400 chords the envelope is the cardioid.",
    values: &[("sweepk", V::B(false)), ("mulk", V::F(2.0)), ("modn", V::I(400))],
},
Preset {
    module: "number.rotunda",
    name: "Nephroid, k = 3",
    view: Some("cardioid"),
    note: "Two cusps: the same chord rule at k = 3 envelopes a nephroid.",
    values: &[("sweepk", V::B(false)), ("mulk", V::F(3.0)), ("modn", V::I(400))],
},
Preset {
    module: "number.rotunda",
    name: "Four cusps, k = 5",
    view: Some("cardioid"),
    note: "The envelope for multiplier k has k − 1 cusps.",
    values: &[("sweepk", V::B(false)), ("mulk", V::F(5.0)), ("modn", V::I(400))],
},
Preset {
    module: "number.rotunda",
    name: "First zeta zero",
    view: Some("zeta"),
    note: "t = 14.1347 at time 0; the 700-term sum still ends 1.6 units from the origin.",
    values: &[("zeta_t", V::F(14.1347))],
},
Preset {
    module: "waves.harmonics",
    name: "Zonal ℓ = 4, m = 0",
    view: Some("nodal"),
    note: "Zonal: four parallels, no meridians; the pattern depends only on θ.",
    values: &[("l", V::I(4)), ("m", V::I(0))],
},
Preset {
    module: "waves.harmonics",
    name: "Sectoral ℓ = m = 4",
    view: Some("nodal"),
    note: "Sectoral: no parallels, eight half-meridians; the sign changes only with φ.",
    values: &[("l", V::I(4)), ("m", V::I(4))],
},
Preset {
    module: "waves.harmonics",
    name: "Tesseral ℓ = 4, m = 2",
    view: Some("nodal"),
    note: "Tesseral: two parallels and four half-meridians divide the sphere into 12 patches.",
    values: &[("l", V::I(4)), ("m", V::I(2))],
},
Preset {
    module: "waves.harmonics",
    name: "Dipole pattern ℓ = 1",
    view: Some("breathe"),
    note: "Y ∝ cos θ: one hemisphere swells as the other hollows.",
    values: &[("l", V::I(1)), ("m", V::I(0))],
},
Preset {
    module: "statmech.montecarlo",
    name: "Slow dart rain",
    view: Some("pi"),
    note: "A few darts per frame, so individual landings stay visible.",
    values: &[("rate", V::F(150.0))],
},
Preset {
    module: "statmech.montecarlo",
    name: "Full-rate needles",
    view: Some("buffon"),
    note: "Needle drawing stops at 9000 after about 4.5 s; crossing counts keep running.",
    values: &[("rate", V::F(2000.0))],
},
Preset {
    module: "statmech.montecarlo",
    name: "Full-rate rain",
    view: Some("integrate"),
    note: "About 24,000 points by the 12 s wrap outline the area under the curve.",
    values: &[("rate", V::F(2000.0))],
},
// waves.membrane
Preset {
    module: "waves.membrane",
    name: "Double slit, short wave",
    view: Some("amplitude"),
    note: "λ ≈ c/f = 1/3; fringes near y = 0 and ±0.92 at x = 3.",
    values: &[("preset", V::C(2)), ("c", V::F(1.0)), ("freq", V::F(3.0)), ("damp", V::F(0.001))],
},
Preset {
    module: "waves.membrane",
    name: "Two in-phase sources",
    view: Some("fronts"),
    note: "λ ≈ 1, sources 2 apart: dark hyperbolas where paths differ by odd multiples of λ/2.",
    values: &[("preset", V::C(3)), ("c", V::F(1.5)), ("freq", V::F(1.5)), ("damp", V::F(0.001))],
},
Preset {
    module: "waves.membrane",
    name: "Chladni mode (3, 4)",
    view: Some("chladni"),
    note: "Odd n + m puts a nodal line on y = −x.",
    values: &[("chl_n", V::I(3)), ("chl_m", V::I(4))],
},
Preset {
    module: "waves.membrane",
    name: "Mach 1.5 cone",
    view: Some("doppler"),
    note: "Cone half-angle asin(1/1.5) ≈ 41.8°.",
    values: &[("mach", V::F(1.5))],
},

// quantum.schrodinger
Preset {
    module: "quantum.schrodinger",
    name: "Dipole swing",
    view: Some("prob"),
    note: "⟨x⟩ = cos(t)/√2; current keeps one sign everywhere, reversing each π.",
    values: &[("state", V::C(2)), ("current", V::B(true))],
},
Preset {
    module: "quantum.schrodinger",
    name: "Breather",
    view: Some("psi"),
    note: "⟨x⟩ stays 0; ⟨x²⟩ = 3/2 + cos(2t)/√2, period π.",
    values: &[("state", V::C(3))],
},
Preset {
    module: "quantum.schrodinger",
    name: "φ₀ + φ₃ superposition",
    view: Some("prob"),
    note: "⟨x⟩ and ⟨x²⟩ stay fixed while the density skews with period 2π/3.",
    values: &[("state", V::C(1)), ("current", V::B(true))],
},
Preset {
    module: "quantum.schrodinger",
    name: "Dense Born rain",
    view: Some("measure"),
    note: "60 samples per time unit; bars approach the swinging packet's time-averaged density.",
    values: &[("state", V::C(0)), ("rate", V::F(60.0))],
},

// quantum.hydrogen
Preset {
    module: "quantum.hydrogen",
    name: "1s ground state",
    view: Some("cloud"),
    note: "E = −½ Ha, no nodes, ⟨r⟩ = 1.5 a₀.",
    values: &[("n", V::I(1)), ("l", V::I(0)), ("m", V::I(0))],
},
Preset {
    module: "quantum.hydrogen",
    name: "3d, m = 0",
    view: Some("nodal"),
    note: "Gold lobes on the axis, indigo collar, nodal cones at θ ≈ 54.7° and 125.3°.",
    values: &[("n", V::I(3)), ("l", V::I(2)), ("m", V::I(0))],
},
Preset {
    module: "quantum.hydrogen",
    name: "4f, m = 3",
    view: Some("cloud"),
    note: "|ψ|² forms one ring about the axis; hue winds three times.",
    values: &[("n", V::I(4)), ("l", V::I(3)), ("m", V::I(3))],
},
Preset {
    module: "quantum.hydrogen",
    name: "4s radial shells",
    view: Some("radial"),
    note: "Three radial nodes, four peaks, ⟨r⟩ = 24 a₀.",
    values: &[("n", V::I(4)), ("l", V::I(0)), ("m", V::I(0))],
},

// relativity.special
Preset {
    module: "relativity.special",
    name: "Rest frame, β = 0",
    view: Some("minkowski"),
    note: "Primed axes and grid coincide with the rest axes.",
    values: &[("animate", V::B(false)), ("beta", V::F(0.0))],
},
Preset {
    module: "relativity.special",
    name: "γ = 2 light clock",
    view: Some("clock"),
    note: "β = √3/2: moving tick rate and rod length both halve.",
    values: &[("animate", V::B(false)), ("beta", V::F(0.8660254))],
},
Preset {
    module: "relativity.special",
    name: "Pancake field at β = 0.9",
    view: Some("charge"),
    note: "γ ≈ 2.29: |E| rises by γ across the motion, falls by γ² along it.",
    values: &[("animate", V::B(false)), ("beta", V::F(0.9))],
},
Preset {
    module: "relativity.special",
    name: "Twins at β = 0.6",
    view: Some("twins"),
    note: "γ = 1.25: traveller ages 4.16 while the home twin ages 5.2.",
    values: &[("animate", V::B(false)), ("beta", V::F(0.6))],
},

// gr.schwarzschild
Preset {
    module: "gr.schwarzschild",
    name: "Zoom-whirl",
    view: Some("orbit"),
    note: "Energy just under the barrier top: about four turns per radial cycle.",
    values: &[("preset", V::C(0)), ("ll", V::F(3.5)), ("r0", V::F(8.38))],
},
Preset {
    module: "gr.schwarzschild",
    name: "Wide rosette",
    view: Some("orbit"),
    note: "Radius swings 13 to 26, advancing about 85° per radial cycle.",
    values: &[("preset", V::C(0)), ("ll", V::F(4.6)), ("r0", V::F(26.0))],
},
Preset {
    module: "gr.schwarzschild",
    name: "Unbound, E > 1",
    view: Some("veff"),
    note: "Energy line sits above V = 1: the mass leaves and never returns.",
    values: &[("preset", V::C(0)), ("ll", V::F(4.6)), ("r0", V::F(7.0))],
},
Preset {
    module: "gr.schwarzschild",
    name: "Plunge, L below √12 M",
    view: Some("veff"),
    note: "L = 3.20 < √12 ≈ 3.46: V has no barrier, so the orbit falls in.",
    values: &[("preset", V::C(2))],
},

// em.fields
Preset {
    module: "em.fields",
    name: "Potential only",
    view: Some("lines"),
    note: "Field lines hidden: potential shading and equipotential bands remain.",
    values: &[("lines", V::B(false))],
},
Preset {
    module: "em.fields",
    name: "Long-wave dipole",
    view: Some("dipole"),
    note: "λ = π; the transition kr = 1 falls at r = 0.5.",
    values: &[("k", V::F(2.0))],
},
Preset {
    module: "em.fields",
    name: "Short-wave dipole",
    view: Some("dipole"),
    note: "λ = 2π/9 ≈ 0.70; with ω = 2.2, phase speed ω/k ≈ 0.24.",
    values: &[("k", V::F(9.0))],
},
Preset {
    module: "dynamics.phase",
    name: "Damped pendulum",
    view: Some("portrait"),
    note: "Damping 0.15: orbits lose energy and spiral into the wells.",
    values: &[("damp", V::F(0.15))],
},
Preset {
    module: "dynamics.phase",
    name: "Deep well, g/ℓ = 3",
    view: Some("portrait"),
    note: "Separatrix speed 2√3 ≈ 3.46 exceeds every seed speed, so restarted orbits all librate.",
    values: &[("k", V::F(3.0)), ("damp", V::F(0.0))],
},
Preset {
    module: "dynamics.phase",
    name: "Fan at δ₀ = 10⁻⁷",
    view: Some("fan"),
    note: "Neighbours 10⁻⁷ apart in θ₁: the fan holds one line far longer.",
    values: &[("spread", V::F(-7.0))],
},
Preset {
    module: "dynamics.phase",
    name: "Fan at δ₀ = 10⁻²",
    view: Some("fan"),
    note: "Neighbours 10⁻² apart: θ₁ starts across 1.49 rad, so the fan opens at once.",
    values: &[("spread", V::F(-2.0))],
},
Preset {
    module: "dynamics.attractor",
    name: "Lorenz 1963, ρ = 28",
    view: Some("swarm"),
    note: "σ = 10, β = 8/3, ρ = 28: the parameters of Lorenz's 1963 paper.",
    values: &[("system", V::C(0)), ("rho", V::F(28.0))],
},
Preset {
    module: "dynamics.attractor",
    name: "Stable fixed points, ρ = 20",
    view: Some("swarm"),
    note: "Below the strange-attractor range: the swarm collapses onto the two fixed points C±.",
    values: &[("system", V::C(0)), ("rho", V::F(20.0))],
},
Preset {
    module: "dynamics.attractor",
    name: "Rössler, c = 5.7",
    view: Some("swarm"),
    note: "Classic a = b = 0.2, c = 5.7, with time scaled by 3.",
    values: &[("system", V::C(1))],
},
Preset {
    module: "diffeq.odes",
    name: "Euler at h = 0.6",
    view: Some("euler"),
    note: "Euler's radius grows by √(1 + h²) ≈ 1.166 per step; RK4 stays near the circle.",
    values: &[("h", V::F(0.6))],
},
Preset {
    module: "diffeq.odes",
    name: "Van der Pol at μ = 4",
    view: Some("vdp"),
    note: "μ = 4: slow drift, fast jumps; cycle reaches |ẋ| ≈ 6.3, beyond the default frame.",
    values: &[("mu", V::F(4.0))],
},
Preset {
    module: "diffeq.odes",
    name: "Butterworth response",
    view: Some("resonance"),
    note: "ζ = 1/√2 gives |H| = 1/√(1 + ω⁴): flat at low ω, no peak.",
    values: &[("zeta", V::F(std::f64::consts::FRAC_1_SQRT_2))],
},
Preset {
    module: "diffeq.odes",
    name: "Subcritical epidemic",
    view: Some("sir"),
    note: "R₀ = 0.8 is below 1: infections decline from the start.",
    values: &[("r0", V::F(0.8))],
},
Preset {
    module: "diffeq.pdes",
    name: "Mitosis spots",
    view: Some("grayscott"),
    note: "F = 0.0367, k = 0.0649: from the seeds, spots grow and split across the grid.",
    values: &[("gs", V::C(0))],
},
Preset {
    module: "diffeq.pdes",
    name: "Coral stripes",
    view: Some("grayscott"),
    note: "F = 0.0545, k = 0.0620: from the seeds, rings break into a labyrinth of stripes.",
    values: &[("gs", V::C(1))],
},
Preset {
    module: "diffeq.pdes",
    name: "U-skate parameters",
    view: Some("grayscott"),
    note: "F = 0.0620, k = 0.0609: from seeds, v fills the grid around nearly static holes.",
    values: &[("gs", V::C(2))],
},
Preset {
    module: "diffeq.pdes",
    name: "Thin Burgers front",
    view: Some("burgers"),
    note: "ν = 0.001, the slider minimum: the front narrows to a few grid cells.",
    values: &[("nu", V::F(0.001))],
},
Preset {
    module: "statmech.ensemble",
    name: "Critical Ising, T = T_c",
    view: Some("ising"),
    note: "T_c = 2/ln(1 + √2) ≈ 2.2692: clusters of many sizes and no settled magnetization.",
    values: &[("temp", V::F(2.2692))],
},
Preset {
    module: "statmech.ensemble",
    name: "Ordered Ising, T = 1.5",
    view: Some("ising"),
    note: "Below T_c: domains coarsen toward one colour, though a striped state can persist.",
    values: &[("temp", V::F(1.5))],
},
Preset {
    module: "statmech.ensemble",
    name: "Disordered Ising, T = 4",
    view: Some("ising"),
    note: "Above T_c: only small clusters, with magnetization near zero.",
    values: &[("temp", V::F(4.0))],
},
Preset {
    module: "statmech.ensemble",
    name: "Percolation threshold",
    view: Some("perc"),
    note: "p_c ≈ 0.5927: the top-connected cluster reaches the bottom in about half of lattices.",
    values: &[("pfill", V::F(0.5927))],
},
];

/// Sliders whose every new value restarts or re-rolls a simulation, so sweeping
/// them shows only restarts. Their sweep button is not offered.
pub const NO_SWEEP: &[(&str, &str)] = &[
    ("gr.schwarzschild", "ll"),
    ("gr.schwarzschild", "r0"),
    ("quantum.hydrogen", "n"),
    ("quantum.hydrogen", "l"),
    ("quantum.hydrogen", "m"),
    ("dynamics.phase", "spread"),
    ("statmech.ensemble", "pfill"),
];

pub fn can_sweep(module: &str, key: &str) -> bool {
    !NO_SWEEP.contains(&(module, key))
}

pub fn for_chamber(id: &str) -> impl Iterator<Item = (usize, &'static Preset)> + '_ {
    PRESETS
        .iter()
        .enumerate()
        .filter(move |(_, p)| p.module == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules;
    use std::collections::HashSet;

    #[test]
    fn every_preset_sets_real_values_in_range() {
        let all = modules::all();
        let mut names = HashSet::new();
        for p in PRESETS {
            let m = all
                .iter()
                .find(|m| m.info().id == p.module)
                .unwrap_or_else(|| panic!("{}: no chamber", p.name));
            assert!(
                names.insert((p.module, p.name)),
                "duplicate preset {}",
                p.name
            );
            if let Some(view) = p.view {
                assert!(
                    m.modes().iter().any(|r| r.id == view),
                    "{}: no view {view}",
                    p.name
                );
            }
            assert!(!p.values.is_empty(), "{} changes nothing", p.name);
            let mut params = m.params_ref().clone();
            for &(key, v) in p.values {
                assert!(
                    params.set(key, v.value()),
                    "{}: bad key or type for {key}",
                    p.name
                );
                let stored = params.param(key).unwrap().value;
                let same = match (stored, v.value()) {
                    (PValue::F(a), PValue::F(b)) => (a - b).abs() < 1e-12,
                    (a, b) => a == b,
                };
                assert!(same, "{}: {key} is outside its range", p.name);
            }
        }
    }

    #[test]
    fn sweep_exclusions_name_real_sliders() {
        let all = modules::all();
        for &(module, key) in NO_SWEEP {
            let m = all.iter().find(|m| m.info().id == module).unwrap();
            let p = m.params_ref().param(key);
            assert!(p.is_some(), "{module} has no parameter {key}");
        }
    }

    #[test]
    fn apply_then_is_active_agree() {
        let all = modules::all();
        for p in PRESETS {
            let m = all.iter().find(|m| m.info().id == p.module).unwrap();
            let mut params = m.params_ref().clone();
            p.apply(&mut params);
            assert!(
                p.is_active(&params),
                "{} does not read back as active",
                p.name
            );
        }
    }
}
