// Common WGSL prelude: per-view uniforms + mathematical / chromatic helpers.
// Prepended to every pipeline in the engine.

struct VU {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    res: vec4<f32>,               // xy = resolution px, z = time, w = aspect
    p: array<vec4<f32>, 12>,      // module parameters
};
@group(0) @binding(0) var<uniform> U: VU;

const PI: f32 = 3.14159265358979;
const TAU: f32 = 6.28318530717959;

// ---------- complex arithmetic (vec2 = a + bi) ----------
fn cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}
fn cconj(a: vec2<f32>) -> vec2<f32> { return vec2<f32>(a.x, -a.y); }
fn cinv(a: vec2<f32>) -> vec2<f32> {
    let d = max(dot(a, a), 1e-20);
    return vec2<f32>(a.x, -a.y) / d;
}
fn cdiv(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> { return cmul(a, cinv(b)); }
fn cexp(a: vec2<f32>) -> vec2<f32> {
    return exp(a.x) * vec2<f32>(cos(a.y), sin(a.y));
}
fn clog(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.5 * log(max(dot(a, a), 1e-20)), atan2(a.y, a.x));
}
fn cpow(a: vec2<f32>, w: vec2<f32>) -> vec2<f32> { return cexp(cmul(w, clog(a))); }
fn csqrt(a: vec2<f32>) -> vec2<f32> {
    let r = length(a);
    let s = sqrt(max(0.5 * (r + a.x), 0.0));
    let t = sqrt(max(0.5 * (r - a.x), 0.0));
    return vec2<f32>(s, select(-t, t, a.y >= 0.0));
}
fn csin(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(a.x) * cosh(a.y), cos(a.x) * sinh(a.y));
}
fn ccos(a: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(cos(a.x) * cosh(a.y), -sin(a.x) * sinh(a.y));
}
fn ctan(a: vec2<f32>) -> vec2<f32> { return cdiv(csin(a), ccos(a)); }

// ---------- color ----------
fn srgb_to_linear3(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

fn oklab_to_linear(L: f32, a: f32, b: f32) -> vec3<f32> {
    let l_ = L + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = L - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = L - 0.0894841775 * a - 1.2914855480 * b;
    let l = l_ * l_ * l_;
    let m = m_ * m_ * m_;
    let s = s_ * s_ * s_;
    return vec3<f32>(
        4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
        -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
        -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s,
    );
}

/// Perceptually-even hue wheel for complex phase (radians).
fn phase_color(theta: f32) -> vec3<f32> {
    return clamp(
        oklab_to_linear(0.70, 0.148 * cos(theta), 0.148 * sin(theta)),
        vec3<f32>(0.0), vec3<f32>(1.0),
    );
}

/// Polynomial viridis fit (returns linear RGB).
fn viridis(x: f32) -> vec3<f32> {
    let t = clamp(x, 0.0, 1.0);
    let c0 = vec3<f32>(0.2777273272, 0.0054073455, 0.3340998053);
    let c1 = vec3<f32>(0.1050930431, 1.4046135299, 1.3845901626);
    let c2 = vec3<f32>(-0.3308618287, 0.2148475595, 0.0950951630);
    let c3 = vec3<f32>(-4.6342304990, -5.7991009734, -19.3324409563);
    let c4 = vec3<f32>(6.2282699363, 14.1799333668, 56.6905526007);
    let c5 = vec3<f32>(-4.7763849977, -13.7451453777, -65.3530326334);
    let c6 = vec3<f32>(2.8098594277, 4.8458270789, 26.3129002132);
    let s = c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
    return srgb_to_linear3(clamp(s, vec3<f32>(0.0), vec3<f32>(1.0)));
}

/// Polynomial inferno fit (returns linear RGB).
fn inferno(x: f32) -> vec3<f32> {
    let t = clamp(x, 0.0, 1.0);
    let c0 = vec3<f32>(0.0002189404, 0.0016510046, -0.0194808984);
    let c1 = vec3<f32>(0.1065134195, 0.5639564368, 3.9327123889);
    let c2 = vec3<f32>(11.6024930825, -3.9728539657, -15.9423941063);
    let c3 = vec3<f32>(-41.7039961314, 17.4363988821, 44.3541451987);
    let c4 = vec3<f32>(77.1629356994, -33.4023589421, -81.8073092574);
    let c5 = vec3<f32>(-71.3194282450, 32.6260642640, 73.2095198580);
    let c6 = vec3<f32>(25.1311262248, -12.2426689524, -23.0703250029);
    let s = c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
    return srgb_to_linear3(clamp(s, vec3<f32>(0.0), vec3<f32>(1.0)));
}

/// Black -> amber -> gold -> pale light. The Newtonian ramp.
fn aurum(x: f32) -> vec3<f32> {
    let t = clamp(x, 0.0, 1.0);
    let a = vec3<f32>(0.0);
    let b = vec3<f32>(0.135, 0.055, 0.012);
    let c = vec3<f32>(0.780, 0.430, 0.095);
    let d = vec3<f32>(1.000, 0.905, 0.640);
    let t1 = smoothstep(0.0, 0.45, t);
    let t2 = smoothstep(0.35, 0.80, t);
    let t3 = smoothstep(0.72, 1.0, t);
    return mix(mix(mix(a, b, t1), c, t2), d, t3);
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp(x * (a * x + b) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn hash21(p: vec2<f32>) -> f32 {
    var q = fract(p * vec2<f32>(123.34, 456.21));
    q += dot(q, q + 45.32);
    return fract(q.x * q.y);
}

// World-plane (z=0) position under an NDC point, for 2D orthographic views.
fn plane_pos(ndc: vec2<f32>) -> vec2<f32> {
    let p = U.inv_view_proj * vec4<f32>(ndc, 0.5, 1.0);
    return p.xy / p.w;
}

// Camera ray through NDC point (uses inv_view_proj; wgpu depth 0..1).
fn ray_dir(ndc: vec2<f32>) -> vec3<f32> {
    let pn = U.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
    let pf = U.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
    return normalize(pf.xyz / pf.w - pn.xyz / pn.w);
}

// Soft luminous grid lines: distance-based glow around integer lattice of `w`.
fn grid_glow(w: vec2<f32>, thickness: f32) -> f32 {
    let g = abs(fract(w + 0.5) - 0.5);
    let fw = fwidth(w) + vec2<f32>(1e-5);
    let d = min(g.x / fw.x, g.y / fw.y);
    return exp(-d * d / (thickness * thickness));
}
