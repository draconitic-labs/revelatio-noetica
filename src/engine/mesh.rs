//! GPU meshes and geometry builders: parametric surfaces, tubes, ribbons.

use glam::{Mat4, Vec3};

use super::gpu::GpuContext;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModelUniform {
    pub model: [[f32; 4]; 4],
    pub tint: [f32; 4],
    /// x: unlit, y: rim strength, z: alpha, w: unused
    pub material: [f32; 4],
}

impl Default for ModelUniform {
    fn default() -> Self {
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            tint: [1.0; 4],
            material: [0.0, 0.35, 1.0, 0.0],
        }
    }
}

#[derive(Clone)]
pub struct GpuMesh {
    pub vbuf: wgpu::Buffer,
    pub ibuf: wgpu::Buffer,
    pub n_idx: u32,
    pub ubuf: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
    alpha: f32,
}

impl GpuMesh {
    pub fn alpha(&self) -> f32 {
        self.alpha
    }
}

pub struct MeshData {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self { vertices: Vec::new(), indices: Vec::new() }
    }

    /// Build a parametric surface grid: `f(u,v) -> (pos, color)`, u,v in [0,1].
    pub fn parametric<F>(nu: usize, nv: usize, mut f: F) -> Self
    where
        F: FnMut(f32, f32) -> (Vec3, [f32; 4]),
    {
        let mut m = Self::new();
        let du = 1.0 / (nu - 1) as f32;
        let dv = 1.0 / (nv - 1) as f32;
        for i in 0..nu {
            for j in 0..nv {
                let u = i as f32 * du;
                let v = j as f32 * dv;
                let (p, c) = f(u, v);
                // Central-difference normal.
                let (pu1, _) = f((u + du * 0.5).min(1.0), v);
                let (pu0, _) = f((u - du * 0.5).max(0.0), v);
                let (pv1, _) = f(u, (v + dv * 0.5).min(1.0));
                let (pv0, _) = f(u, (v - dv * 0.5).max(0.0));
                let n = (pu1 - pu0).cross(pv1 - pv0).normalize_or_zero();
                m.vertices.push(MeshVertex { pos: p.to_array(), normal: n.to_array(), color: c });
            }
        }
        for i in 0..nu - 1 {
            for j in 0..nv - 1 {
                let a = (i * nv + j) as u32;
                let b = a + 1;
                let c = a + nv as u32;
                let d = c + 1;
                m.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        m
    }

    /// Sweep a circular tube along a polyline (parallel-transport frame).
    pub fn tube(path: &[Vec3], radius: f32, sides: usize, color_at: impl Fn(usize) -> [f32; 4]) -> Self {
        let mut m = Self::new();
        if path.len() < 2 {
            return m;
        }
        // Parallel transport frames along the path.
        let mut tangents: Vec<Vec3> = Vec::with_capacity(path.len());
        for i in 0..path.len() {
            let t = if i == 0 {
                path[1] - path[0]
            } else if i == path.len() - 1 {
                path[i] - path[i - 1]
            } else {
                path[i + 1] - path[i - 1]
            };
            tangents.push(t.normalize_or_zero());
        }
        let mut normal = tangents[0].cross(Vec3::Y);
        if normal.length_squared() < 1e-6 {
            normal = tangents[0].cross(Vec3::X);
        }
        normal = normal.normalize();
        for (i, &p) in path.iter().enumerate() {
            if i > 0 {
                // Transport the normal: remove the tangent component.
                normal = (normal - tangents[i] * normal.dot(tangents[i])).normalize_or_zero();
                if normal.length_squared() < 1e-6 {
                    normal = tangents[i].cross(Vec3::Y).normalize_or_zero();
                }
            }
            let binormal = tangents[i].cross(normal).normalize_or_zero();
            let col = color_at(i);
            for s in 0..sides {
                let a = s as f32 / sides as f32 * std::f32::consts::TAU;
                let dir = normal * a.cos() + binormal * a.sin();
                m.vertices.push(MeshVertex {
                    pos: (p + dir * radius).to_array(),
                    normal: dir.to_array(),
                    color: col,
                });
            }
        }
        let s32 = sides as u32;
        for i in 0..(path.len() - 1) as u32 {
            for s in 0..s32 {
                let sn = (s + 1) % s32;
                let a = i * s32 + s;
                let b = i * s32 + sn;
                let c = (i + 1) * s32 + s;
                let d = (i + 1) * s32 + sn;
                m.indices.extend_from_slice(&[a, c, b, b, c, d]);
            }
        }
        m
    }

    pub fn upload(&self, gpu: &GpuContext, layout: &wgpu::BindGroupLayout) -> GpuMesh {
        use wgpu::util::DeviceExt;
        let vbuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-vb"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });
        let ibuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-ib"),
            contents: bytemuck::cast_slice(&self.indices),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        });
        let u = ModelUniform::default();
        let ubuf = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh-ub"),
            contents: bytemuck::bytes_of(&u),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mesh-bg"),
            layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: ubuf.as_entire_binding() }],
        });
        GpuMesh { vbuf, ibuf, n_idx: self.indices.len() as u32, ubuf, bind_group, alpha: 1.0 }
    }
}

impl GpuMesh {
    /// Update per-mesh transform/material. Call from `update`, once per frame.
    pub fn set_model(&mut self, gpu: &GpuContext, model: Mat4, tint: [f32; 4], material: [f32; 4]) {
        self.alpha = material[2] * tint[3];
        let u = ModelUniform { model: model.to_cols_array_2d(), tint, material };
        gpu.queue.write_buffer(&self.ubuf, 0, bytemuck::bytes_of(&u));
    }

    /// Replace vertex data in-place (same layout/size or smaller).
    pub fn update_vertices(&self, gpu: &GpuContext, vertices: &[MeshVertex]) {
        gpu.queue.write_buffer(&self.vbuf, 0, bytemuck::cast_slice(vertices));
    }
}
