//! Per-view reading guides. Written from each module's render code: `read` says
//! what the picture encodes, `try_this` names one thing to do, and `params` lists
//! the parameter keys that actually change this view.

pub struct ViewGuide {
    pub module: &'static str,
    pub view: &'static str,
    pub read: &'static str,
    pub try_this: &'static str,
    pub params: &'static [&'static str],
}

pub const GUIDES: &[ViewGuide] = &[
// newton.fluxions
ViewGuide {
    module: "newton.fluxions",
    view: "fluent",
    read: "A point moves along the curve as s = t/12. Bright gold is the curve generated so far, faint gold the part to come. The gold fill is the area beneath it, and the pale vertical line is the ordinate y at the current x.",
    try_this: "With \"Fluent curve\" on Cycloid, watch the ordinate stall at the cusp at t = 6 s, where ẋ = 0 while time keeps running.",
    params: &["curve"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "fluxion",
    read: "The point rides the gold curve; the bright pale arrow is its velocity (ẋ, ẏ) per second, scaled by 1.44, and the dimmer pale gold and blue arrows are its components. Faint lines complete the parallelogram. Optional: burnt-orange acceleration arrow, faint gold tangent line.",
    try_this: "Turn on \"Second fluxion (acceleration)\" and set \"Fluent curve\" to Parabola: the burnt-orange arrow stays vertical and constant while the blue ẏ arrow lengthens.",
    params: &["curve", "tangent", "accel"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "moment",
    read: "Pale rings spread from where the point was at each pulse. At the point, short pale gold and blue segments are the moments ẋo and ẏo, with o = 0.42 s. The lens circle shows that triangle enlarged: pale gold ẋo, blue ẏo, gold hypotenuse.",
    try_this: "Set \"Fluent curve\" to Cycloid: near each cusp the enlarged triangle turns almost vertical, because ẋ falls to zero faster than ẏ.",
    params: &["curve", "pulse"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "inverse",
    read: "Lower axis, y = −2.6: the blue curve is the slope dy/dx, height scaled by 0.55, filled gold where positive and blue where negative, up to the current x. Upper axis: the gold fluent y(x), drawn to the same x. A faint line joins them.",
    try_this: "Set \"Fluent curve\" to Parabola: the blue slope is the straight line x/8, its fill a growing triangle, and the gold height follows x²/16.",
    params: &["curve"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "split",
    read: "Left, Newton: the gold curve grows behind a moving point with a pale velocity arrow. Right, Leibniz: the blue curve carries a fixed staircase of blue dx and pale gold dy steps, and the step nearest the current s is brightened, with a white chord.",
    try_this: "Raise \"Leibniz differences\" to 48: the staircase closes onto the blue curve and the highlighted triangle shrinks, while the Newton half stays the same.",
    params: &["curve", "steps"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "principia",
    read: "Sol sits at a focus and the pale blue-white point is the planet; gold wedges are equal-time slices. The pale arrow is velocity, the burnt-orange arrow inward force, ∝ 1/r² up to a cap. The grey-brown dot is the empty focus.",
    try_this: "Set \"Orbit eccentricity\" to 0.85: wedges near Sol turn short and wide, far ones long and thin, and all twelve keep equal area.",
    params: &["ecc"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "solids",
    read: "The pale gold curve is the profile y(x) at angle 0, and the grey-brown line is the axis of revolution. The gold surface is the sweep so far, brighter toward its gold leading-edge curve. Where y < 0 the surface radius is held at 0.",
    try_this: "Set \"Revolution sweep\" to 1.0 turns: the leading edge returns to the pale profile as the 12 s span ends, closing the surface.",
    params: &["curve", "revs"],
},
ViewGuide {
    module: "newton.fluxions",
    view: "cathedral",
    read: "The central gold cycloid, height × 2.2, is traced by the pale point on a rolling wheel, a pale ellipse with a spoke from its hub. Faint copies repeat it in depth, and columns mark the arch feet. Floor lattice, reflection and rays are decorative.",
    try_this: "Drag the time scrubber to 6 s: the tracing point touches the floor at the cusp x = 4, in line with the middle columns.",
    params: &[],
},
ViewGuide {
    module: "newton.fluxions",
    view: "brach",
    read: "Three tracks run from A to B, the white points: blue straight chute, burnt-orange chord with a sine-shaped sag, pale gold cycloid. Each bead's speed is √(2g × drop) with g = 1.4, and the race restarts every 6 s.",
    try_this: "Drag the time scrubber to 2.34 s: only the cycloid bead has reached B; the sagging track takes 2.38 s, the straight chute 2.73 s.",
    params: &[],
},
ViewGuide {
    module: "newton.fluxions",
    view: "tauto",
    read: "The gold curve is an inverted cycloid, a = 0.62. Six hued beads start at different arclengths and each follows s = s₀ cos ωt, with ω = √(g/4a) and g = 1.4, so all share one period. The pale gold point marks the bottom.",
    try_this: "Drag the time scrubber to about 2.09 s: all six beads sit at the bottom together, and again near 6.27 s.",
    params: &[],
},

// complex.domain
ViewGuide {
    module: "complex.domain",
    view: "dc",
    read: "Each point z shows f(z): hue is arg f, dark means |f| near 0, white means |f| large. Brightness sawteeth repeat at each doubling of |f| and each sixth of a phase turn. Blue-grey lines are the axes; a faint lattice marks integer z.",
    try_this: "Set \"Function f(z)\" to z²: going once around the dark origin, each hue appears twice, because arg z² = 2 arg z.",
    params: &["fn", "contours", "plines", "animate"],
},
ViewGuide {
    module: "complex.domain",
    view: "landscape",
    read: "The base plane is z, with Re along x and Im along depth; blue-grey lines are the axes. Height is ln(1+|f|), rescaled and capped below 3.1 by a tanh. Hue is arg f, and brightness sawteeth repeat at each doubling of |f|.",
    try_this: "Raise \"Landscape height\" to 2.5: slopes steepen, but peaks flatten toward the 3.1 cap instead of rising further.",
    params: &["fn", "animate", "hscale"],
},
ViewGuide {
    module: "complex.domain",
    view: "conformal",
    read: "Grid lines of z move to (1−m)z + m f(z), with morph m, and points beyond |w| = 6.5 are pulled back to that circle. A line's hue is the polar angle of its current first point. The pale gold circle is |w| = 1.",
    try_this: "Turn off \"Animate parameter / flow\", drag \"Grid morph z → f(z)\" to 1: crossings below |w| = 6.5 stay right angles, critical points aside.",
    params: &["fn", "animate", "morph", "grid_n"],
},
ViewGuide {
    module: "complex.domain",
    view: "sphere",
    read: "Stereographic projection wraps the plane onto a unit sphere: z = ∞ is the top point and z = 0 the bottom. Colours follow the domain colouring, with added lighting. The pale band is the equator, |z| = 1, and background stars carry no data.",
    try_this: "Set \"Function f(z)\" to 1/z and orbit: white pole at the bottom (z = 0), dark zero at the top (z = ∞).",
    params: &["fn", "contours", "plines", "animate"],
},
ViewGuide {
    module: "complex.domain",
    view: "julia",
    read: "Each point is a starting z under z → z² + c, with c = 0.7885 e^{0.35it}. Black means no escape in 160 steps, and very fast escapes fade toward black. Gold and hue bands show escape time, brightest near the frontier.",
    try_this: "Drag the time scrubber to 9 s: c ≈ −0.7885 lies in the period-2 bulb, and a black interior appears that t = 0 lacks.",
    params: &[],
},
ViewGuide {
    module: "complex.domain",
    view: "mandel",
    read: "Each point is a parameter c; z → z² + c runs from z = 0 for up to 220 steps. Black means no escape, which is treated as inside the set. Outside, a gold ramp mixed with hue bands shows the smoothed escape count.",
    try_this: "Drag c ≈ −0.75 + 0.1i to the centre, then scroll to zoom: finer spirals appear because the shader recomputes every pixel.",
    params: &[],
},
ViewGuide {
    module: "complex.domain",
    view: "moebius",
    read: "Faint grey-blue circles are the net: circles through ±1 and the Apollonian circles around them. Gold +1 and blue −1 are the fixed points of one loxodromic map. Hued trails are 26 orbits under it, each ending in a dot, spiralling from +1 toward −1.",
    try_this: "Drag the time scrubber from 0 to 12 s: each trail grows in steps from 8 to 22 iterates, closing in on the blue point.",
    params: &[],
},

// analysis.fourier
ViewGuide {
    module: "analysis.fourier",
    view: "epicycles",
    read: "Each hued arm is one nonzero harmonic, length |bₙ|, turning at 1.1n rad/s, with its teal circle. The chain tip's height is the partial sum. Orange plots that height over the last 10 s, newest at the grey line; a pale thread joins them.",
    try_this: "Raise \"Harmonics N\" from 3 to 4 on Square: nothing changes, since bₙ = 0 for even n. At 5 a new circle appears.",
    params: &["wave", "n", "circles"],
},
ViewGuide {
    module: "analysis.fourier",
    view: "stack",
    read: "Lower hued curves are single terms of the series, at most the first nine nonzero ones, with n increasing downward, over two periods. The orange curve on top is the sum of all N terms; the grey curve is the true waveform. All drift with time.",
    try_this: "Set \"Waveform\" to Triangle: at N = 7 the orange sum nearly covers the grey curve, because bₙ falls as 1/n² rather than 1/n.",
    params: &["wave", "n", "ghost"],
},
ViewGuide {
    module: "analysis.fourier",
    view: "spectrum",
    read: "Bars give |bₙ| for n = 1 to 32, scaled by 1.6; bars within N pulse at their own frequency and carry a dot, and bars beyond N stay dim. The orange line is the square wave's 4/(πn), whatever the waveform. Grey is the baseline.",
    try_this: "Set \"Waveform\" to Triangle: the odd bars drop far below the orange line, since triangle coefficients fall as 1/n² rather than 1/n.",
    params: &["wave", "n"],
},
ViewGuide {
    module: "analysis.fourier",
    view: "gibbs",
    read: "Orange is the partial sum on x from −0.8 to 1.8, with N rising from 1 to 61 over 12 s, independent of the harmonics control. Grey is the true waveform. The red line, square wave only, is the limit of the first peak, y ≈ 1.18.",
    try_this: "On Square, drag the time scrubber to 12 s: ripples narrow toward the jump at x = 0, and the first peak settles onto the red line.",
    params: &["wave"],
},
ViewGuide {
    module: "analysis.fourier",
    view: "lissajous",
    read: "The curve is x = 1.25 sin(3s + φ), y = 1.25 sin(4s), with hue running along s and phase φ = 0.35t. Teal and orange margin points show the x and y oscillators; the white point is the pair on the curve.",
    try_this: "Drag the time scrubber to about 1.12 s: φ = π/8, and the closed figure folds onto an open curve traced twice.",
    params: &[],
},
ViewGuide {
    module: "analysis.fourier",
    view: "drawing",
    read: "The grey heart is the target. Orange is the pen's path so far, one pass per 12 s. Hued arms and teal rings are rotating terms of the heart's discrete Fourier transform, chained tip to tip and ending at the orange pen point.",
    try_this: "Turn off \"Show epicycles\": the teal rings vanish, the hued arms stay, and the orange trace still follows the grey heart.",
    params: &["circles"],
},

// analysis.taylor
ViewGuide {
    module: "analysis.taylor",
    view: "taylor",
    read: "Grey is f; orange is its Taylor polynomial of degree N about 0, the orange dot; sky-blue ghosts are degrees N−1 and N−2. Red verticals at ±1 mark the radius of convergence, for ln(1+x) and 1/(1+x²) only. Values past |y| = 6 are dropped.",
    try_this: "Turn off \"Grow N with time\", set \"Function\" to 1/(1+x²), raise \"Degree N\" to 24: orange hugs grey inside the red lines, diverging outside.",
    params: &["fn", "deg", "grow"],
},
ViewGuide {
    module: "analysis.taylor",
    view: "newton",
    read: "Each point starts Newton's method for z³ = 1, and hue is the angle of the result after at most 42 steps, so it names the root reached. Brightness falls with step count, so slow boundary points go dark. White points are the three roots.",
    try_this: "Drag a basin border to the centre, then scroll to zoom: lobes of every hue keep appearing, since each boundary point touches all three basins.",
    params: &[],
},
ViewGuide {
    module: "analysis.taylor",
    view: "linear",
    read: "Both axes are rescaled about (x₀, f(x₀)) so ±span fills ±3 units, and span breathes from 3 to 0.04 and back over 12 s. Sky blue is f, orange is its tangent at x₀, and the orange point marks x₀.",
    try_this: "Set \"Microscope center x\" to 1.2, drag the time scrubber to 6 s: at span 0.04 the blue curve nearly lies on the orange tangent.",
    params: &["fn", "zoomx"],
},
ViewGuide {
    module: "analysis.taylor",
    view: "fresnel",
    read: "The curve (∫cos u² du, ∫sin u² du), from 0 to s, is shifted by its limit and scaled by 1.6; s reaches 9.5 at 12 s. Hue and opacity vary along it. The orange point is the limit, the white point the current end.",
    try_this: "Drag the time scrubber to 12 s: the white end winds ever tighter around the orange limit point, √(π/8) in each coordinate.",
    params: &[],
},
ViewGuide {
    module: "analysis.taylor",
    view: "chaosgame",
    read: "Each dot is one iterate of randomly chosen contractions: gold for Sierpiński, green for the Barnsley fern. The cloud stops at 46000 dots and clears when time jumps back. The white point is the current iterate.",
    try_this: "Set \"Chaos-game target\" to Barnsley fern: the gold cloud clears and green dots rebuild a fern at 600 iterates per frame.",
    params: &["fractal"],
},
ViewGuide {
    module: "analysis.taylor",
    view: "hilbert",
    read: "One line visits the centre of every cell of a 2^k × 2^k grid in Hilbert order. The order k rises from 1 to 6, one step every 2 s. Hue shifts along the line, about 0.8 of the wheel from start to end.",
    try_this: "Drag the time scrubber from 0 to 12 s: each step replaces every cell by four, while the line's ends stay at the bottom corners.",
    params: &[],
},

// vector.fields
ViewGuide {
    module: "vector.fields",
    view: "flow",
    read: "Arrows on a 16 × 16 lattice point along F; length saturates with |F|, and colour runs from green toward blue as |F| grows. White streaks are tracer particles, each trail showing its last six RK2 steps, fading behind.",
    try_this: "Raise \"Flow speed\" to 2.5: the white trails lengthen because each of their six steps covers more distance. At 0 they freeze.",
    params: &["field", "flow"],
},
ViewGuide {
    module: "vector.fields",
    view: "heat",
    read: "Background tint is curl, or divergence with the toggle off: orange where positive, blue where negative, deeper with magnitude. Fine stripes follow the local flow direction and carry no magnitude. Arrows are F: direction, saturating length, and green to blue as |F| grows.",
    try_this: "Set \"Field F(x,y)\" to Spiral sink and flip \"Heat: curl (else divergence)\": uniform orange for curl = 2, uniform blue for divergence = −0.8.",
    params: &["field", "curlheat"],
},
ViewGuide {
    module: "vector.fields",
    view: "probe",
    read: "The white ring is the probe circle. At 28 boundary points, orange arrows show outward normal flow, blue arrows inward normal flow, and green lines the tangential component. Dim arrows are the field, and the white dot circles at a fixed rate, carrying no data.",
    try_this: "Set \"Field F(x,y)\" to Source & sink, then drag the probe over (1, 0): every normal arrow turns orange. Over (−1, 0) they turn blue.",
    params: &["field", "probe_x", "probe_y", "probe_r"],
},
ViewGuide {
    module: "vector.fields",
    view: "weave",
    read: "Each faint curve is an integral curve traced both ways from one of 225 lattice seeds, in fixed steps of 0.045 that ignore |F|. Hue depends only on the seed position. Bright areas are where curves overlap, because their light adds.",
    try_this: "Set \"Field F(x,y)\" to Dipole: the curves bend into circles tangent to the x-axis at the origin.",
    params: &["field"],
},

// forms.stokes
ViewGuide {
    module: "forms.stokes",
    view: "stokes",
    read: "Cap tint and spines show the component of curl F along the upward normal: yellow tint and copper spines where positive, blue where negative. The teal curve is the rim. Pale runners circle it, and teal arrows give F along the rim at each runner.",
    try_this: "Orbit half a turn: on the rim section nearest you, teal arrows point against the runners' travel, because F runs backward along the rim there.",
    params: &[],
},
ViewGuide {
    module: "forms.stokes",
    view: "cancel",
    read: "Each square cell carries four counterclockwise arrows on its edges. Copper arrows lie on the outer boundary and stay solid. Teal arrows lie on interior edges, where two neighbouring cells draw opposite arrows on the same line; they fade as cancellation rises.",
    try_this: "Turn off \"Animate cancellation\" and drag \"Interior cancellation\" to 0: each interior edge shows two opposed arrowheads. At 1 only the copper rim stands out.",
    params: &["cells", "cancel", "autocancel"],
},
ViewGuide {
    module: "forms.stokes",
    view: "oneform",
    read: "Teal lines are level sets of H = y + 0.35 sin 2x, spaced 0.22 apart. Copper is the path γ, bright where travelled and faint ahead; the pale point is its current end. ∫γ dH is about 0.22 times the net contour crossings.",
    try_this: "Drag the time scrubber to 12 s: the path has crossed a net 11 contours, and the inspector's ∫γ dH reads about 11 × 0.22.",
    params: &[],
},
ViewGuide {
    module: "linalg.transform",
    view: "grid",
    read: "The blue-grey lattice is the square grid carried by M = (1−s)I + sA. Red and blue arrows are M's columns; thin lines in those colours are the original basis. The outlined unit square is gold when det M ≥ 0, red when negative.",
    try_this: "Drag the red arrow tip left across the origin: the outline turns from gold to red as det goes negative.",
    params: &["a", "c", "b", "d", "animate", "blend"],
},
ViewGuide {
    module: "linalg.transform",
    view: "eigen",
    read: "Blue lines join 48 points on the pale unit circle to gold images under M = (1−s)I + sA. Real eigenvalues: red and gold lines are eigendirections, with arrows λv. Complex eigenvalues: a red polyline follows iterates of 0.999M from (1.3, 0).",
    try_this: "From defaults, lower \"c  (col 1, y)\" to −0.5: eigenvalues turn complex, eigen rays vanish, and a red spiral of iterates appears.",
    params: &["a", "c", "b", "d", "animate", "blend"],
},
ViewGuide {
    module: "linalg.transform",
    view: "svd",
    read: "Over 12 s the pale unit circle is rotated, stretched, then rotated into the gold curve. Coloured spokes track 24 directions, red and blue arrows the x and y unit vectors, and dim gold arrows the final semi-axes.",
    try_this: "Scrub time: the first third turns the spokes, the middle stretches the circle, the last swings the ellipse onto the dim gold arrows.",
    params: &["a", "c", "b", "d"],
},
ViewGuide {
    module: "geometry.manifolds",
    view: "tangent",
    read: "The surface carries a faint blue parameter grid. At the moving silver point, the silver outline spans the tangent plane; orange and blue arrows are the ∂u and ∂v directions and the silver arrow the normal. The orange trail is the path so far.",
    try_this: "Set \"Manifold\" to Saddle and scrub time: the outline skews slightly as ∂u and ∂v stop being perpendicular away from the axes.",
    params: &["surf", "kheat"],
},
ViewGuide {
    module: "geometry.manifolds",
    view: "transport",
    read: "On the sphere, the glowing orange arc is the loop at angle θ₀ from the pole. Silver arrows are the transported vector, brighter toward the current position; the orange arrow is its starting value. On torus and saddle no transport is drawn.",
    try_this: "Set \"Transport colatitude θ₀\" to 1.05 and scrub time to 12: the silver arrow ends opposite the orange one, a holonomy near π.",
    params: &["surf", "lat", "kheat"],
},
ViewGuide {
    module: "geometry.manifolds",
    view: "curvature",
    read: "With curvature heat on, colour encodes Gaussian curvature K: copper where K > 0, blue where K < 0, stronger with |K| and saturating at |K| ≈ 0.71. Off, the surface is uniform slate blue. The faint grid marks parameter lines.",
    try_this: "Turn on \"Curvature heat\", set \"Manifold\" to Torus: the outer side turns copper, the inner side blue, meeting where K = 0.",
    params: &["surf", "kheat"],
},
ViewGuide {
    module: "geometry.manifolds",
    view: "hyperbolic",
    read: "The silver circle is the boundary of the Poincaré disk. Each of the 30 coloured arcs is a hyperbolic geodesic, an arc of a circle orthogonal to the rim. Their endpoints drift slowly around the rim with time.",
    try_this: "Zoom in on the rim and scrub time: wherever a coloured arc reaches the silver circle, it meets it at a right angle.",
    params: &[],
},
ViewGuide {
    module: "geometry.manifolds",
    view: "voronoi",
    read: "Twelve seeds, the pale white spots, move on separate sinusoidal paths. Each faintly tinted cell is the set of points nearest one seed. Gold lines are frontiers, where the nearest and second-nearest seeds are equally distant.",
    try_this: "Scrub time and follow two seeds sharing a gold frontier: the frontier stays on their perpendicular bisector as both move.",
    params: &[],
},
ViewGuide {
    module: "topology.knots",
    view: "gallery",
    read: "A solid tube follows the chosen knot or link, its hue set by position along the curve. Breathing varies its winding amplitude by up to 10% over 12 s without changing knot type; tumble spins it about the vertical axis.",
    try_this: "Set \"Knot / link\" to Hopf link and orbit: each ring passes once through the other, and breathing no longer changes the shape.",
    params: &["knot", "thick", "breathe", "spin"],
},
ViewGuide {
    module: "topology.knots",
    view: "diagram",
    read: "Each curve is flattened to its (x, y) shadow, with z as depth and hue set by position along it. A strand is cut where a higher-z strand crosses it in the shadow, so every gap marks a real undercrossing.",
    try_this: "With Trefoil selected, turn off \"Isotopy breathing\": this side-on shadow has more crossings than 3, the trefoil's crossing number.",
    params: &["knot", "breathe"],
},
ViewGuide {
    module: "topology.knots",
    view: "winding",
    read: "Sixteen faint violet rings outline a torus. The coloured line winds 14 turns the long way round and slope turns around the tube per turn; time raises slope from 1 to 1.618. Hue and brightness change along the line.",
    try_this: "Scrub time to about 9.7 s: slope passes 3/2, so the line retraces one closed (2,3) curve; nearby times smear over the torus.",
    params: &[],
},
ViewGuide {
    module: "number.rotunda",
    view: "ulam",
    read: "Integers 1 to 40,000 coil outward on a square grid from the gold dot at 1, revealed with time. Green dots are primes, very dim dots are odd non-primes, and even non-primes are not drawn.",
    try_this: "Scrub time from 0 to 12: rings of integers appear outward until n = 40,000, and the green dots line up along diagonals.",
    params: &[],
},
ViewGuide {
    module: "number.rotunda",
    view: "cardioid",
    read: "The grey-green ring carries N unmarked points, numbered clockwise from the top. Each coloured chord joins point i to point ki mod N, with hue set by i. Additive light brightens the envelope where chords bunch; k need not be an integer while sweeping.",
    try_this: "Turn off \"Sweep k with time\" and set \"Multiplier k\" to 3: the one-cusped cardioid at k = 2 becomes the two-cusped nephroid.",
    params: &["modn", "mulk", "sweepk"],
},
ViewGuide {
    module: "number.rotunda",
    view: "zeta",
    read: "The coloured polyline joins successive partial sums of Σ n^(−½−it), n up to 700, recentred on its centroid, with t = start + 2 × time. The gold dot is the complex origin, the white dot the 700th sum, and a faint line joins them.",
    try_this: "Set \"Critical-line start t\" to 14.13 and scrub time to 0: this is ζ's first zero, yet the white dot stays well away from gold, because the series does not converge here.",
    params: &["zeta_t"],
},
ViewGuide {
    module: "number.rotunda",
    view: "staircase",
    read: "The green staircase is π(x), stepping up at each prime; the gold curve is x/ln x. Time grows the range from x = 40 to 2000, and both axes rescale so x/ln x always reaches the same height at the right end.",
    try_this: "Scrub time to 12 so x reaches 2000: the green staircase ends above gold, π(2000) = 303 against 2000/ln 2000 ≈ 263.",
    params: &[],
},
ViewGuide {
    module: "number.rotunda",
    view: "phyllo",
    read: "Seed n sits at radius 0.055√n and angle nα, where α oscillates 0.25° around the golden angle 137.508° over 12 s. Dots grow with n. Hue cycles with n mod 34, so near the golden angle each hue forms one of 34 spiral arms.",
    try_this: "Scrub time to 9 s: the angle sits 0.25° below golden and the even packing opens into curved arms with wide gaps.",
    params: &[],
},
ViewGuide {
    module: "number.rotunda",
    view: "collatz",
    read: "Each of 46 coloured paths is the Collatz sequence of one start value, 27 + 173k, drawn as a walk from a shared base point. Every step has length 0.035, turning right 0.16 rad on even values and left 0.27 rad on odd ones.",
    try_this: "Scrub time from 0 upward: paths lengthen until about 9.2 s, when the longest, 168 steps, reaches 1; after that nothing changes.",
    params: &[],
},
ViewGuide {
    module: "number.rotunda",
    view: "primepolar",
    read: "Each green dot is a prime p at polar radius 0.0122√p and angle p radians, revealed up to 40,000 with time. The gold dot is the origin. Curved arms near the centre arise because 44 radians is nearly 7 full turns.",
    try_this: "Scrub time from 0 to 12: curved arms form near the centre, then the outer primes land in scattered patches rather than clean arms.",
    params: &[],
},
ViewGuide {
    module: "waves.harmonics",
    view: "breathe",
    read: "The radius is 1.35(1 + A Y cos ωt), where Y = P(cos θ) cos(mφ) is scaled for display. Gold marks outward displacement, blue inward, and near-black the nodal lines; shape and colour reverse every half period.",
    try_this: "Set \"Deformation amplitude\" to 0: the sphere stays round while colour keeps reversing, because colour follows Y cos ωt without A.",
    params: &["l", "m", "amp", "omega"],
},
ViewGuide {
    module: "waves.harmonics",
    view: "nodal",
    read: "The same breathing surface, with nodal sets in pale gold at fixed radius 1.42: rings where P(cos θ) = 0 and half-meridians where cos(mφ) = 0. There are ℓ−m rings and 2m half-meridians, with m limited to ℓ.",
    try_this: "Set \"Degree ℓ\" to 4 and \"Order m\" to 2: 2 gold rings and 4 half-meridians sit above the dark nodal lines.",
    params: &["l", "m", "amp", "omega"],
},
ViewGuide {
    module: "statmech.montecarlo",
    view: "pi",
    read: "Darts land uniformly in the grey square [−1, 1]². Gold dots fall inside the gold unit circle and blue dots outside, so 4 × gold ÷ all estimates π. Rewinding time clears them.",
    try_this: "Raise \"Samples per second\" to 2000: the square fills toward the 26,000 darts drawn, while the estimate in the side panel keeps sharpening.",
    params: &["rate"],
},
ViewGuide {
    module: "statmech.montecarlo",
    view: "buffon",
    read: "Grey lines are floorboard edges one unit apart. Each needle has length 0.5 with random centre and angle; red needles cross an edge and pale ones do not. With ℓ = d/2 the crossing probability is 1/π, so all ÷ red estimates π.",
    try_this: "Raise \"Samples per second\" to 2000: the floor fills, and drawing stops at 9000 needles about 4.5 s in.",
    params: &["rate"],
},
ViewGuide {
    module: "statmech.montecarlo",
    view: "integrate",
    read: "The teal curve is f(x) = 1.4(sin²(3x) e^(−x²) + 0.15) over the grey axis. Random points fill the box [0, 3] × [0, 1.7]: gold below the curve, blue above. Box area × gold fraction estimates the integral.",
    try_this: "Raise \"Samples per second\" to 2000: gold fills the area under the teal curve until the cloud itself outlines its shape.",
    params: &["rate"],
},
ViewGuide {
    module: "statmech.montecarlo",
    view: "galton",
    read: "Grey dots are 13 rows of pegs; gold dots are balls stepping left or right at random per row. Gold bars count landings in 14 bins. After 40 landings a blue Gaussian with the binomial's σ appears, peak matched to the tallest bar.",
    try_this: "Jump the time scrubber back by more than 0.5 s: the bins empty, then refill, and the blue curve returns after 40 landings.",
    params: &[],
},
// waves.membrane (wave.rs)
ViewGuide {
    module: "waves.membrane",
    view: "amplitude",
    read: "Colour is the displacement u(x, y, t) on a 200×200 lattice: gold where u > 0, blue where u < 0, brightening with |u| until |u| ≈ 0.56. In Double slit, a grey strip at x = 0 marks the barrier.",
    try_this: "With \"Scenario\" on Double slit, raise \"Source frequency\" from 1.8 to 3.5: wavelength shortens and the fringes right of the barrier move closer together.",
    params: &["preset", "c", "freq", "damp"],
},
ViewGuide {
    module: "waves.membrane",
    view: "relief",
    read: "Height and colour both follow u: the sheet rises 1.6u above the rest plane, gold where u > 0, blue where u < 0, brighter with |u|. The faint square is the rim; in Double slit, grey strokes mark the barrier walls.",
    try_this: "Set \"Scenario\" to Gaussian pluck and orbit low: the hump near x = −1.6 collapses into a ring spreading outward at about speed c.",
    params: &["preset", "c", "freq", "damp"],
},
ViewGuide {
    module: "waves.membrane",
    view: "energy",
    read: "Inferno colour map, black at zero rising through purple and orange to pale yellow, of log(1 + 1.5E), with E = ½(uₜ² + c²|∇u|²), the energy density of the wave. In Double slit, a grey strip marks the barrier.",
    try_this: "With \"Scenario\" on Gaussian pluck, click the canvas to strike: a second bright ring spreads from the click and passes through the first.",
    params: &["preset", "c", "freq", "damp"],
},
ViewGuide {
    module: "waves.membrane",
    view: "fronts",
    read: "Only |u| is drawn, so sign is lost: teal bands where |u| is about 0.05 to 0.2, pale gold growing from 0.2 to full at 0.55, dark where |u| is small. In Double slit, a grey strip marks the barrier.",
    try_this: "Set \"Scenario\" to Two sources: the dark gaps fanning out between the bright ridges are where the two waves cancel.",
    params: &["preset", "c", "freq", "damp"],
},
ViewGuide {
    module: "waves.membrane",
    view: "chladni",
    read: "Closed-form amplitude sin(nx)sin(my) + sin(mx)sin(ny) on a square. Sand-coloured grain marks where it nears zero, the nodal lines, fading at the rim. Dim indigo shimmer grows with |amplitude| and pulses as cos(2.4t).",
    try_this: "Set \"Chladni mode m\" to 4 with n at 3: odd n + m puts a nodal line on the diagonal y = −x.",
    params: &["chl_n", "chl_m"],
},
ViewGuide {
    module: "waves.membrane",
    view: "doppler",
    read: "Circles are wavefronts emitted every 0.14 time units, growing at speed 1.1 and fading with radius: pale blue below Mach 1, orange above. The bright dot is the source with a pale gold streak behind it. Above Mach 1, two orange lines mark the cone.",
    try_this: "Raise \"Source speed (Mach)\" past 1: rings turn orange and two lines appear behind the source at half-angle asin(1/M), touching the rings.",
    params: &["mach"],
},

// quantum.schrodinger (schrodinger.rs)
ViewGuide {
    module: "quantum.schrodinger",
    view: "psi",
    read: "Curve height is Re ψ(x, t); its hue is arg ψ. Violet lines are the envelope ±|ψ|. The grey horizontal line is ψ = 0, and the grey parabola below is a scaled sketch of the well V = x²/2.",
    try_this: "Set \"Prepared state\" to Breather: the envelope widens and narrows with period π while staying centred at x = 0.",
    params: &["state"],
},
ViewGuide {
    module: "quantum.schrodinger",
    view: "prob",
    read: "Filled height is |ψ|², coloured by arg ψ and fading toward its base. Amber arrows below the axis show the probability current j = Im(ψ* ∂ₓψ), pointing with its sign, length proportional to its size. The grey parabola is a scaled sketch of the well.",
    try_this: "Set \"Prepared state\" to Dipole: all amber arrows point the same way and reverse every π time units as the density swings.",
    params: &["state", "current"],
},
ViewGuide {
    module: "quantum.schrodinger",
    view: "phasor",
    read: "One clock per eigenstate with |cₙ| > 0.02, ordered by n from the left. Circle radius follows cₙ; the needle turns clockwise at rate Eₙ = n + ½, coloured by its angle. The amber dash below brightens with cₙ².",
    try_this: "Set \"Prepared state\" to Dipole and scrub time: the right needle (E = 3/2) turns three times as fast as the left (E = 1/2).",
    params: &["state"],
},
ViewGuide {
    module: "quantum.schrodinger",
    view: "measure",
    read: "The violet curve is the current |ψ|²; the grey parabola is a sketch of the well. Amber dots are sampled positions falling to the baseline. Amber bars count every sample since the last time reset, so they follow the time-averaged density, not the instantaneous curve.",
    try_this: "Pause, jump the time scrubber back to clear the bars, then click repeatedly: with t frozen, the bars converge on the violet curve's shape.",
    params: &["state", "rate"],
},

// quantum.hydrogen (hydrogen.rs)
ViewGuide {
    module: "quantum.hydrogen",
    view: "cloud",
    read: "Fog density is |ψ|² ray-marched through a ball of radius 2n² + 8, with brightness scaled by n⁶. Hue is arg ψ, cycling m times around the vertical axis (faint grey line). Dark gaps are nodes.",
    try_this: "Flip \"Magnetic m\" from 1 to −1 and orbit overhead: the fog stays identical while hue runs the opposite way around the axis.",
    params: &["n", "l", "m", "dens", "cut"],
},
ViewGuide {
    module: "quantum.hydrogen",
    view: "walkers",
    read: "Each dot is one of 24,000 Metropolis walkers whose stationary density is |ψ|²; every frame each proposes a random step. Hue is mφ, where φ is the angle around the vertical grey axis. The bright central dot marks the nucleus.",
    try_this: "Set \"Magnetic m\" to 0: every dot takes one hue, because walker colour is mφ and the phase no longer winds.",
    params: &["n", "l", "m"],
},
ViewGuide {
    module: "quantum.hydrogen",
    view: "nodal",
    read: "A cutaway removes the half-space z < 0 to show the interior. Gold fog marks R(r)P(cos θ) ≥ 0 and indigo marks it negative, with fog density still |ψ|². Dark walls between them are nodes; the faint grey line is the axis.",
    try_this: "At \"Azimuthal ℓ\" 0, step \"Principal n\" from 1 to 4: gold and indigo shells alternate, adding one radial node per step.",
    params: &["n", "l", "m", "dens"],
},
ViewGuide {
    module: "quantum.hydrogen",
    view: "radial",
    read: "Horizontal is r, scaled so the plot spans 0 to 2.4n² + 6 Bohr radii. The cyan curve is P(r) = r²R². The red vertical line marks ⟨r⟩ = ½[3n² − ℓ(ℓ+1)], and red dots under the axis mark Bohr radii 1, 4, ... n².",
    try_this: "Zoom out, set \"Principal n\" to 3, and step \"Azimuthal ℓ\" 2, 1, 0: each step adds a peak and moves the red line out.",
    params: &["n", "l"],
},
ViewGuide {
    module: "quantum.hydrogen",
    view: "sample",
    read: "Each dot is one accepted rejection-sampling draw weighted by |ψ|²r², accumulating to 6000 at up to 40 per frame. Hue is mφ, where φ is the angle around the vertical grey axis. The bright central dot marks the nucleus.",
    try_this: "Change \"Azimuthal ℓ\" and orbit: the old dots vanish and the new orbital builds from empty within a few seconds.",
    params: &["n", "l", "m"],
},

// relativity.special (relativity.rs)
ViewGuide {
    module: "relativity.special",
    view: "minkowski",
    read: "Axes are x and ct: grey for the rest frame, bright blue with faint grid for the boosted frame. Gold diagonals are light; faint gold curves are s² = ±1. Gold and blue dots are events; the soft blue line through gold is boosted simultaneity.",
    try_this: "Turn off \"Sweep β with time\" and move \"Boost velocity β\" across 0.10: the soft blue slice through the gold event passes the blue event.",
    params: &["beta", "animate", "grid"],
},
ViewGuide {
    module: "relativity.special",
    view: "clock",
    read: "Left: grey mirrors and a gold photon, a clock at rest, above a grey rod of length 1.6. Right: blue mirrors gliding and wrapping, a photon ticking at 1/γ with a faint gold zigzag, above a blue rod of length 1.6/γ. Labels give tick rates.",
    try_this: "With \"Sweep β with time\" off, set \"Boost velocity β\" to 0.866: γ = 2, halving the moving tick rate and the blue rod.",
    params: &["beta", "animate"],
},
ViewGuide {
    module: "relativity.special",
    view: "charge",
    read: "Gold heat is log |E| for a charge in uniform motion along x, drawn at the origin with a blue velocity arrow. Gold rays are straight field lines, crowding toward the y axis as |β| grows. Magenta and teal mark the sign of B.",
    try_this: "With \"Sweep β with time\" off, raise \"Boost velocity β\" to 0.9: gold rays crowd toward the y axis as the heat flattens along x.",
    params: &["beta", "animate"],
},
ViewGuide {
    module: "relativity.special",
    view: "twins",
    read: "The grey vertical line is the twin at home in (x, ct); the blue V is the traveller at speed |β|, floored at 0.25. Dots mark proper-time steps of 0.4, grey at home, gold travelling. The white dot is the reunion.",
    try_this: "With \"Sweep β with time\" off, set \"Boost velocity β\" to 0.6: gold dots sit 1.25 times wider in ct, so the traveller counts fewer.",
    params: &["beta", "animate"],
},

// gr.schwarzschild (schwarzschild.rs)
ViewGuide {
    module: "gr.schwarzschild",
    view: "lensing",
    read: "Each pixel traces a null geodesic; captured rays stay black. The gold band is a thin disk bent around the hole, brighter and bluer on the side moving toward you. A pale glow follows impact parameter 3√3 M; specks and the faint band are stars.",
    try_this: "Drag to orbit from the low default angle to nearly overhead: the arc bent over the shadow opens into a flat ring around it.",
    params: &[],
},
ViewGuide {
    module: "gr.schwarzschild",
    view: "orbit",
    read: "The white dot is the test mass; its trail runs from faint blue (old) to warm (new). Circles mark the horizon r = 2M (orange), photon sphere 3M (pale gold) and ISCO 6M (pale blue).",
    try_this: "Lower \"Angular momentum L\" from 3.9 to 3.6: the orbit restarts, dips to r ≈ 5, and each radial cycle winds nearly three times.",
    params: &["preset", "ll", "r0", "rate"],
},
ViewGuide {
    module: "gr.schwarzschild",
    view: "veff",
    read: "The blue curve is V(r) = √((1 − 2M/r)(1 + L²/r²)); the orange line is the energy, meeting it at turning points. The white marker sits at the current r. Masts mark horizon (orange) and ISCO (pale gold); the grey line is V = 1.",
    try_this: "Lower \"Angular momentum L\" to 3.5: the well and barrier of the blue curve nearly merge, close to the ISCO value √12 M ≈ 3.46.",
    params: &["preset", "ll", "r0", "rate"],
},
ViewGuide {
    module: "gr.schwarzschild",
    view: "funnel",
    read: "The blue surface is the equatorial slice embedded with height 2√(2(r − 2)), so the horizon throat sits at the bottom as the glowing orange ring. Faint lines are meridians and circles of constant r. The amber line and dot are the orbit.",
    try_this: "Set \"Orbit\" to Plunge and orbit the camera low: the amber trail winds along the surface to the orange throat.",
    params: &["preset", "ll", "r0", "rate"],
},

// em.fields (em.rs)
ViewGuide {
    module: "em.fields",
    view: "lines",
    read: "Background colour is the potential Σq/r: rose where positive, blue where negative, deeper with |φ|, with darker equipotential bands. Gold lines follow E out of positive charges; cyan lines trace backward from negative ones. Magenta dots are +, cyan dots −, engraved with their sign.",
    try_this: "Drag a magenta charge onto a cyan one: their potentials nearly cancel, so the shading and lines follow the other pair alone.",
    params: &["lines"],
},
ViewGuide {
    module: "em.fields",
    view: "dipole",
    read: "Bright curves are level sets of the dipole field-line function ψ(r, θ, t), with the axis vertical: orange where ψ > 0, blue where ψ < 0, grey near zero, fading with r. The gold stub and glow pulse at ω = 2.2.",
    try_this: "Raise \"Dipole wavenumber k\" from 2 to 9: wavelength 2π/k shortens and, with ω fixed at 2.2, the outgoing loops drift more slowly.",
    params: &["k"],
},
ViewGuide {
    module: "em.fields",
    view: "wave",
    read: "The grey line is the propagation axis. Gold arrows and curve are E along the vertical; cyan arrows and curve are B, perpendicular to E and the axis. Both follow cos(kx − ωt) with k = 1.6, ω = 2.2, in phase, travelling toward +x.",
    try_this: "Drag to orbit until you look straight down the grey axis: gold and cyan arrows hold a right angle while pulsing in step.",
    params: &[],
},
ViewGuide {
    module: "em.fields",
    view: "cyclotron",
    read: "Faint blue dots stand for a uniform B out of the page; a uniform E along +x acts but is not drawn. Magenta charges are positive and cyan negative, each with a fading trail and gold velocity arrow. Circling orbits drift under E × B, and a charge that leaves the bottom re-enters at the top.",
    try_this: "Jump the time scrubber back to reseed: magenta and cyan circle in opposite senses, yet both drift downward at E/B ≈ 0.16.",
    params: &[],
},
ViewGuide {
    module: "em.fields",
    view: "caustic",
    read: "The grey circle is the cup rim. Faint lines are 110 parallel incoming rays; brighter gold lines are their first reflections, ended at the next rim crossing. Where reflections overlap they add to the bright nephroid, its cusp at half the radius along the light.",
    try_this: "Scrub time: the light direction turns at 0.15 radians per time unit, and the bright cusp at half the rim radius turns with it.",
    params: &[],
},
ViewGuide {
    module: "dynamics.phase",
    view: "portrait",
    read: "Horizontal is angle θ, vertical is angular speed ω. Gold lines are energy levels and the orange glow is the separatrix H = g/ℓ. The teal dot is the stable point, coral dots are saddles at θ = ±π, and coloured curves are live orbits.",
    try_this: "Click just above and just below the orange glow at θ = 0: one orbit closes around the origin, the other runs along θ.",
    params: &["k", "damp", "rate"],
},
ViewGuide {
    module: "dynamics.phase",
    view: "swing",
    read: "The rod and coral bob show the first orbit's angle θ, with θ = 0 hanging straight down. The fading coral dots are its recent positions, the teal arrow is velocity scaled by ω, and the faint ring is the circle the bob can reach.",
    try_this: "Raise \"Damping\" to 0.30: the rod tumbles a few more turns, then swings in a shrinking arc as the teal arrow shrinks toward zero.",
    params: &["k", "damp", "rate"],
},
ViewGuide {
    module: "dynamics.phase",
    view: "energy",
    read: "The surface height and colour are the energy H(θ, ω) over the θ and ω axes, dark at low energy and gold-orange at high, with engraved contour bands. Coloured curves with head dots are the live orbits, lifted onto the surface.",
    try_this: "Raise \"Damping\" to 0.15: the coloured curves stop following level contours and spiral down the surface into its wells.",
    params: &["k", "damp", "rate"],
},
ViewGuide {
    module: "dynamics.phase",
    view: "fan",
    read: "Each of 150 two-segment pendulums is one initial condition, with neighbours differing in the first angle by δ₀. Hue runs with index and overlaps add to brighter light. Every third pendulum also draws a short trail at its tip.",
    try_this: "Set \"Fan spread δ₀\" to −7 (10⁻⁷): the restarted fan stays one line far longer than at −4, which splits after about 6 s.",
    params: &["k", "rate", "spread"],
},
ViewGuide {
    module: "dynamics.phase",
    view: "wave",
    read: "Thirty bobs hang from one bar with frequencies (24 + n)/12 cycles per second for n = 0 to 29, hue marking n. Slower pendulums have longer rods. Positions are the exact cosine, and the small dim dots echo each bob's last 0.2 s.",
    try_this: "Drag the time scrubber to 6: alternate bobs oppose each other in two rows. At 4 every third bob matches, at 3 every fourth.",
    params: &[],
},
ViewGuide {
    module: "dynamics.attractor",
    view: "swarm",
    read: "Each of 26,000 dots is one state carried by the selected flow, coloured from blue at low speed toward orange at high speed. Bright regions are where states concentrate. Height is z, and the horizontal axes are x and y.",
    try_this: "Lower \"Lorenz ρ\" to 20: over about 30 s the swarm drains out of the butterfly onto two fixed points.",
    params: &["system", "rho", "speed"],
},
ViewGuide {
    module: "dynamics.attractor",
    view: "ribbon",
    read: "Gold and blue are two trajectories started 2×10⁻⁵ apart in z, brighter toward the present and ending in head dots. The white segment joins their current states. Height is z, and the horizontal axes are x and y.",
    try_this: "Switch \"System\" to Rössler and back to Lorenz to restart the twins: gold and blue overlap for over 10 s, then the white gap opens.",
    params: &["system", "rho", "speed"],
},
ViewGuide {
    module: "dynamics.attractor",
    view: "logistic",
    read: "Horizontal is r from 2.5 to 4 and vertical is x. Each amber column plots the last 130 of 350 iterates of x → rx(1−x). Blue lines mark r = 3, 3.44949 and 3.56995: two period doublings and their accumulation point.",
    try_this: "Drag to pan right, then wheel-zoom on the gap near r ≈ 3.83: three branches appear, the period-3 window inside the chaos.",
    params: &[],
},
ViewGuide {
    module: "dynamics.attractor",
    view: "threebody",
    read: "Three equal masses attract under Newtonian gravity from the Chenciner–Montgomery figure-eight initial conditions, integrated by leapfrog. Gold, blue and pink mark the bodies and their fading trails, which all lie on one figure-eight curve.",
    try_this: "Zoom in on the central crossing: the three bodies pass through it in turn, one after another, on the same curve.",
    params: &[],
},
ViewGuide {
    module: "diffeq.odes",
    view: "slope",
    read: "Teal ticks show the slope f(x, y) on a 24 by 24 grid, with x horizontal and y vertical. Coloured curves are RK4 solutions through seven fixed starts near the left edge and through your clicks.",
    try_this: "Set \"Right-hand side\" to logistic: curves starting above y = 0 converge on y = 2, and those below fall away.",
    params: &["f"],
},
ViewGuide {
    module: "diffeq.odes",
    view: "euler",
    read: "The pale circle is the exact solution of (x, y)′ = (y, −x). Red is forward Euler and teal is RK4, both at step h from the same start with dots at their latest points. Three steps are added per second of t.",
    try_this: "Raise \"Integrator step h\" to 0.6: red Euler spirals off the circle within a few steps while teal RK4 stays close.",
    params: &["h"],
},
ViewGuide {
    module: "diffeq.odes",
    view: "vdp",
    read: "Horizontal is x and vertical is ẋ. Each coloured trail is one solution of the Van der Pol equation from a different start, with a dot at its current state. The trails converge on one closed curve, the limit cycle.",
    try_this: "Set \"Van der Pol μ\" to 0.2: the orbits relax onto a near-circle of radius 2. At 4 the cycle stretches past the frame.",
    params: &["mu"],
},
ViewGuide {
    module: "diffeq.odes",
    view: "lv",
    read: "Horizontal is prey x and vertical is predator y. Four coloured orbits run counter-clockwise with dots at their current states. The pale dot is the equilibrium at (1.5, 1.22), and the grey lines are the axes.",
    try_this: "Zoom in and follow one dot counter-clockwise: prey peaks at the rightmost point, predators about a quarter turn later at the top.",
    params: &[],
},
ViewGuide {
    module: "diffeq.odes",
    view: "resonance",
    read: "The teal curve is amplitude response |H(ω)| against drive frequency ω. The apricot bar marks the chosen ω and its response. At right, the apricot bob on a pale string oscillates at 3ω, sped up for display, with amplitude proportional to it.",
    try_this: "Slide \"Drive frequency ω\" across 1: the apricot bar tops the teal peak near there, and the bob's swing is largest.",
    params: &["zeta", "omega"],
},
ViewGuide {
    module: "diffeq.odes",
    view: "sir",
    read: "Horizontal is time and vertical is population fraction. Teal is susceptible, glowing red is infected and pale is recovered. The apricot line marks S = 1/R₀, and the curves extend with the timeline to model time 22 at t = 12.",
    try_this: "Drag the time scrubber through the outbreak: the red I curve peaks where teal S crosses the apricot line.",
    params: &["r0"],
},
ViewGuide {
    module: "diffeq.pdes",
    view: "heat",
    read: "Horizontal is position on a rod whose ends stay at zero, vertical is temperature u. The glowing apricot curve is the current profile, and the faint grey curves are up to nine earlier snapshots, later ones brighter. The grey line is u = 0.",
    try_this: "Lower \"Diffusivity α\" to 0.2: the wide block keeps its flat top for seconds while the narrow block's peak is already falling.",
    params: &["alpha"],
},
ViewGuide {
    module: "diffeq.pdes",
    view: "laplace",
    read: "Inferno colour shows u on a 128² grid, dark at 0 and bright at 1, with gold level rings every 1/14. The left edge is held at 1, the right at 0, and top and bottom at opposite sinusoids.",
    try_this: "Scrub time back to restart from the blended first guess: the gold rings shift for about a second as the sweeps converge.",
    params: &[],
},
ViewGuide {
    module: "diffeq.pdes",
    view: "burgers",
    read: "Horizontal is x on a periodic domain and vertical is u; the grey line is u = 0. Hue marks fixed position x, not a fluid parcel. Positive u moves right and negative u moves left, steepening the slope into a front at the centre.",
    try_this: "Set \"Burgers viscosity ν\" to 0.02, then 0.001: the central front changes from a soft ramp to a near-vertical step.",
    params: &["nu"],
},
ViewGuide {
    module: "diffeq.pdes",
    view: "string",
    read: "The teal glow at top is the sum of the first N modes of a centre-plucked string with fixed ends, over a grey rest line. Coloured curves below are the individual non-zero modes, each oscillating at its own frequency.",
    try_this: "Scrub time to 0, then step \"String modes N\" from 1 to 24: a sine sharpens into a triangle, and even N change nothing.",
    params: &["modes"],
},
ViewGuide {
    module: "diffeq.pdes",
    view: "grayscott",
    read: "Colour shows chemical v on a 128² periodic grid, black at v = 0 and teal near v ≈ 0.4. Chemical u is not drawn. Fourteen seeded squares grow under the F and k of the chosen regime.",
    try_this: "Switch \"Gray–Scott regime\" from Spots (mitosis) to Stripes (coral) mid-run: nothing resets, and the spots swell and join into stripes.",
    params: &["gs"],
},
ViewGuide {
    module: "diffeq.pdes",
    view: "fhn",
    read: "Inferno colour shows the fast variable u, mapped by (u + 2)/4, on a 128² periodic grid. The sheet starts at rest, with one band excited and a refractory patch beside it, so the wavefront breaks and curls into rotating spirals.",
    try_this: "Scrub time back to restart: the band sets off, its free end curls, and spirals fill the sheet.",
    params: &[],
},
ViewGuide {
    module: "statmech.ensemble",
    view: "ising",
    read: "Each cell is one spin on a 128 by 128 periodic lattice: gold is up, deep blue is down. Spins flip by Metropolis at temperature T. Same-coloured regions are domains of aligned spins.",
    try_this: "At \"Ising temperature T\" 1.5, drag a large gold patch onto blue: it shrinks slowly from its rim. At 4.0 it dissolves at once.",
    params: &["temp", "sweeps"],
},
ViewGuide {
    module: "statmech.ensemble",
    view: "gas",
    read: "In the box, 900 discs are coloured by speed, blue slow to orange fast. Below it, speed rises rightward: orange bars are the histogram and the blue glow is the 2D Maxwell–Boltzmann curve for the current ⟨v²⟩, both scaled to one peak height.",
    try_this: "Scrub time back to restart: every disc has speed 1.35, one histogram spike, which spreads onto the blue curve within about a second.",
    params: &[],
},
ViewGuide {
    module: "statmech.ensemble",
    view: "walk",
    read: "Each orange dot is a walker that starts at the origin and takes one fixed-length step in a random direction per frame. The inner blue ring is the root-mean-square radius of those steps, and the outer ring is twice that.",
    try_this: "Set \"Walker step\" to 0.03 and scrub back to restart: cloud and rings spread 2.5 times wider than at 0.012 at equal t.",
    params: &["dstep"],
},
ViewGuide {
    module: "statmech.ensemble",
    view: "perc",
    read: "Each cell is a site occupied with probability p. Teal sites connect to the top row through occupied neighbours, dim bronze sites are occupied but unconnected, and dark sites are empty. A spanning path exists when teal reaches the bottom edge.",
    try_this: "Step \"Percolation p\" from 0.55 to 0.62: teal goes from clusters hanging off the top edge to most occupied sites, reaching the bottom.",
    params: &["pfill"],
},
ViewGuide {
    module: "statmech.ensemble",
    view: "life",
    read: "Teal cells are alive and dark cells are dead on a 128 by 128 torus, updated by Conway's rules about every 0.09 s of timeline time. The start is random with 14% alive.",
    try_this: "Zoom in with the wheel: among the active debris, look for blocks that hold still and blinkers that flip each generation.",
    params: &[],
},
];

pub fn guide(module: &str, view: &str) -> Option<&'static ViewGuide> {
    GUIDES.iter().find(|g| g.module == module && g.view == view)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules;
    use std::collections::HashSet;

    #[test]
    fn every_view_has_exactly_one_guide() {
        let mut expected = HashSet::new();
        for m in modules::all() {
            for r in m.modes() {
                expected.insert((m.info().id, r.id));
            }
        }
        let mut seen = HashSet::new();
        for g in GUIDES {
            assert!(
                expected.contains(&(g.module, g.view)),
                "no such view: {}/{}",
                g.module,
                g.view
            );
            assert!(
                seen.insert((g.module, g.view)),
                "two guides for {}/{}",
                g.module,
                g.view
            );
        }
        assert_eq!(seen.len(), expected.len(), "some views have no guide");
    }

    #[test]
    fn guide_params_are_real_keys() {
        let all = modules::all();
        for g in GUIDES {
            let m = all.iter().find(|m| m.info().id == g.module).unwrap();
            for key in g.params {
                assert!(
                    m.params_ref().param(key).is_some(),
                    "{}/{}: no param {key}",
                    g.module,
                    g.view
                );
            }
        }
    }

    #[test]
    fn guide_text_stays_short_enough_to_read() {
        for g in GUIDES {
            let id = format!("{}/{}", g.module, g.view);
            assert!(
                g.read.split_whitespace().count() <= 60,
                "{id} read is too long"
            );
            assert!(
                g.try_this.split_whitespace().count() <= 34,
                "{id} try_this is too long"
            );
        }
    }

    #[test]
    fn suggestions_quote_real_control_names() {
        let all = modules::all();
        for g in GUIDES {
            let m = all.iter().find(|m| m.info().id == g.module).unwrap();
            for quoted in g.try_this.split('"').skip(1).step_by(2) {
                let known = m.params_ref().items.iter().any(|p| p.label == quoted);
                assert!(
                    known,
                    "{}/{}: no control named {quoted:?}",
                    g.module, g.view
                );
            }
        }
    }
}
