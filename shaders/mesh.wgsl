// Lit mesh pipeline: two key lights, hemisphere fill, fresnel rim.
// Tuned for surfaces floating in the void — the instrument-glass look.

struct Model {
    model: mat4x4<f32>,
    tint: vec4<f32>,
    material: vec4<f32>,   // x: unlit, y: rim strength, z: alpha, w: flat shade mix
};
@group(1) @binding(0) var<uniform> M: Model;

struct MeshIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct MeshOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) wpos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

@vertex
fn vs_main(v: MeshIn) -> MeshOut {
    var out: MeshOut;
    let wp = M.model * vec4<f32>(v.pos, 1.0);
    out.pos = U.view_proj * wp;
    out.wpos = wp.xyz;
    // Assumes uniform-ish scaling (true for all engine meshes).
    out.normal = (M.model * vec4<f32>(v.normal, 0.0)).xyz;
    out.color = v.color * M.tint;
    return out;
}

@fragment
fn fs_main(in: MeshOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(in.normal);
    if (!front) { n = -n; }
    let base = in.color.rgb;
    if (M.material.x > 0.5) {
        let a = in.color.a * M.material.z;
        return vec4<f32>(base * a, a);
    }
    let vdir = normalize(U.eye.xyz - in.wpos);
    let l1 = normalize(vec3<f32>(0.45, 0.80, 0.40));
    let l2 = normalize(vec3<f32>(-0.60, -0.15, -0.55));
    let hemi = 0.24 + 0.16 * (n.y * 0.5 + 0.5);
    var d = 0.85 * max(dot(n, l1), 0.0) + 0.30 * max(dot(n, l2), 0.0);
    let h = normalize(l1 + vdir);
    let spec = pow(max(dot(n, h), 0.0), 48.0) * 0.5;
    let fres = pow(1.0 - max(dot(n, vdir), 0.0), 3.0);
    let rim = fres * M.material.y;
    var c = base * (hemi + d) + vec3<f32>(spec) + base * rim * 2.2 + vec3<f32>(0.9, 0.95, 1.0) * rim * 0.35;
    let a = in.color.a * M.material.z;
    return vec4<f32>(c * a, a);
}
