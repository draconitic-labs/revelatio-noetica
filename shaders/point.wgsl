// Instanced luminous point sprites: bright core disc + Gaussian halo.

struct PtIn {
    @location(0) pos: vec3<f32>,
    @location(1) size: f32,        // core radius in px
    @location(2) color: vec4<f32>,
    @location(3) glow: f32,        // halo radius multiplier (0 = crisp dot)
};

struct PtOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,    // -1..1 across sprite
    @location(2) core_frac: f32,   // core radius / sprite radius
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, pt: PtIn) -> PtOut {
    var out: PtOut;
    let corner = vec2<f32>(f32(vi & 1u) * 2.0 - 1.0, f32(vi >> 1u) * 2.0 - 1.0);
    let c = U.view_proj * vec4<f32>(pt.pos, 1.0);
    if (c.w < 0.01) {
        out.pos = vec4<f32>(0.0, 0.0, 2.0, 1.0);
        out.color = vec4<f32>(0.0);
        return out;
    }
    let n = c.xyz / c.w;
    let r_px = pt.size * (1.0 + pt.glow) + 1.5;
    let s = (n.xy * 0.5 + 0.5) * U.res.xy + corner * r_px;
    out.pos = vec4<f32>(s / U.res.xy * 2.0 - 1.0, n.z, 1.0);
    out.color = pt.color;
    out.uv = corner;
    out.core_frac = pt.size / r_px;
    return out;
}

@fragment
fn fs_main(in: PtOut) -> @location(0) vec4<f32> {
    let r = length(in.uv);
    let core = 1.0 - smoothstep(in.core_frac * 0.72, in.core_frac, r);
    let halo = exp(-5.0 * r * r) * 0.55;
    let a = clamp(core + halo, 0.0, 1.0) * in.color.a;
    if (a < 0.003) { discard; }
    return vec4<f32>(in.color.rgb * a, a);
}
