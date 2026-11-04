// Instanced luminous line segments, expanded to screen-space quads with
// per-endpoint width, feathered edges, and an optional Gaussian glow profile.

struct LineIn {
    @location(0) p0: vec3<f32>,
    @location(1) w0: f32,
    @location(2) p1: vec3<f32>,
    @location(3) w1: f32,
    @location(4) color: vec4<f32>,
    @location(5) style: vec4<f32>,   // x: softness 0..1
};

struct LineOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) v: f32,          // signed cross coordinate, +-1 at quad edge
    @location(2) half_px: f32,    // expanded half width in px
    @location(3) core_px: f32,    // core half width in px
    @location(4) soft: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, li: LineIn) -> LineOut {
    var out: LineOut;
    let t = f32(vi & 1u);
    let side = f32(vi >> 1u) * 2.0 - 1.0;

    var c0 = U.view_proj * vec4<f32>(li.p0, 1.0);
    var c1 = U.view_proj * vec4<f32>(li.p1, 1.0);

    // Cull segments crossing behind the eye (cheap, robust for our scenes).
    if (c0.w < 0.01 || c1.w < 0.01) {
        out.pos = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        out.color = vec4<f32>(0.0);
        return out;
    }

    let n0 = c0.xyz / c0.w;
    let n1 = c1.xyz / c1.w;
    let s0 = (n0.xy * 0.5 + 0.5) * U.res.xy;
    let s1 = (n1.xy * 0.5 + 0.5) * U.res.xy;
    var d = s1 - s0;
    let len = length(d);
    if (len < 1e-4) { d = vec2<f32>(1.0, 0.0); } else { d = d / len; }
    let nrm = vec2<f32>(-d.y, d.x);

    let w = mix(li.w0, li.w1, t);
    let core = max(w * 0.5, 0.10);
    let half_w = core + 1.6;

    // Extend endpoints just enough to close the joint on a bend. A long overlap
    // would double the light at every joint of an additive polyline and bead it.
    let cap = core * 0.5 * (1.0 - clamp(li.style.x, 0.0, 1.0));
    let s = mix(s0, s1, t) + d * (t * 2.0 - 1.0) * cap + nrm * side * half_w;
    let ndc_xy = s / U.res.xy * 2.0 - 1.0;
    let z = mix(n0.z, n1.z, t);

    out.pos = vec4<f32>(ndc_xy, z, 1.0);
    out.color = li.color;
    out.v = side;
    out.half_px = half_w;
    out.core_px = core;
    out.soft = li.style.x;
    return out;
}

@fragment
fn fs_main(in: LineOut) -> @location(0) vec4<f32> {
    let d = abs(in.v) * in.half_px;
    let crisp = 1.0 - smoothstep(in.core_px - 0.8, in.core_px + 0.8, d);
    let g = d / max(in.half_px, 1e-3);
    let soft = exp(-4.5 * g * g) * (1.0 - smoothstep(0.85, 1.0, g));
    let a = mix(crisp, soft, in.soft) * in.color.a;
    if (a < 0.002) { discard; }
    return vec4<f32>(in.color.rgb * a, a);
}
