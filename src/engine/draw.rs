//! Shared render infrastructure: luminous lines, point sprites, lit meshes,
//! and registered fullscreen field shaders. All colors premultiplied-alpha.

use std::collections::HashMap;

use super::gpu::{GpuContext, DEPTH_FORMAT, FORMAT, SAMPLES};
use super::mesh::GpuMesh;

pub const COMMON_WGSL: &str = include_str!("../../shaders/common.wgsl");

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineInst {
    pub p0: [f32; 3],
    pub w0: f32,
    pub p1: [f32; 3],
    pub w1: f32,
    pub color: [f32; 4],
    pub style: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PointInst {
    pub pos: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
    pub glow: f32,
    pub _pad: [f32; 3],
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineLayer {
    /// Depth-tested, blended over surfaces.
    World,
    /// Depth-tested, additive light.
    WorldAdd,
    /// Always on top, blended.
    Overlay,
    /// Always on top, additive light.
    OverlayAdd,
}
pub const LINE_LAYERS: [LineLayer; 4] = [
    LineLayer::World,
    LineLayer::WorldAdd,
    LineLayer::Overlay,
    LineLayer::OverlayAdd,
];

/// What a module wants drawn this frame in one view.
pub struct Scene {
    pub clear: [f64; 4],
    /// Key of a registered fullscreen shader drawn behind everything.
    pub field: Option<&'static str>,
    /// Extra texture bound at group(1) for the field shader.
    pub field_tex: Option<wgpu::TextureView>,
    /// Scalar params forwarded to shaders as `U.p` (12 vec4 = 48 floats).
    pub params: [[f32; 4]; 12],
    pub lines: HashMap<LineLayer, Vec<LineInst>>,
    pub points_world: Vec<PointInst>,
    pub meshes: Vec<GpuMesh>,
}

impl Scene {
    pub fn new() -> Self {
        Self {
            clear: [0.012, 0.014, 0.022, 1.0],
            field: None,
            field_tex: None,
            params: [[0.0; 4]; 12],
            lines: HashMap::new(),
            points_world: Vec::new(),
            meshes: Vec::new(),
        }
    }

    /// Set shader scalar param `i` (flat index into 48 floats).
    pub fn p(&mut self, i: usize, v: f32) {
        self.params[i / 4][i % 4] = v;
    }

    pub fn line(
        &mut self,
        layer: LineLayer,
        p0: glam::Vec3,
        p1: glam::Vec3,
        w: f32,
        color: [f32; 4],
    ) {
        self.lines.entry(layer).or_default().push(LineInst {
            p0: p0.to_array(),
            w0: w,
            p1: p1.to_array(),
            w1: w,
            color,
            style: [0.0; 4],
        });
    }

    pub fn line_soft(
        &mut self,
        layer: LineLayer,
        p0: glam::Vec3,
        p1: glam::Vec3,
        w: f32,
        color: [f32; 4],
    ) {
        self.lines.entry(layer).or_default().push(LineInst {
            p0: p0.to_array(),
            w0: w,
            p1: p1.to_array(),
            w1: w,
            color,
            style: [1.0, 0.0, 0.0, 0.0],
        });
    }

    pub fn polyline(&mut self, layer: LineLayer, pts: &[glam::Vec3], w: f32, color: [f32; 4]) {
        for seg in pts.windows(2) {
            self.line(layer, seg[0], seg[1], w, color);
        }
    }

    /// Core stroke plus wide additive halo — the signature luminous curve.
    pub fn glow_polyline(&mut self, pts: &[glam::Vec3], w: f32, color: [f32; 4], halo: f32) {
        let hc = [color[0], color[1], color[2], color[3] * 0.25];
        for seg in pts.windows(2) {
            self.line(LineLayer::World, seg[0], seg[1], w, color);
            self.lines
                .entry(LineLayer::WorldAdd)
                .or_default()
                .push(LineInst {
                    p0: seg[0].to_array(),
                    w0: w * halo,
                    p1: seg[1].to_array(),
                    w1: w * halo,
                    color: hc,
                    style: [1.0, 0.0, 0.0, 0.0],
                });
        }
    }

    /// A directed vector drawn as a tapered comet stroke with a bright tip.
    pub fn vector(&mut self, p: glam::Vec3, v: glam::Vec3, w: f32, color: [f32; 4]) {
        let tip = p + v;
        self.lines
            .entry(LineLayer::WorldAdd)
            .or_default()
            .push(LineInst {
                p0: p.to_array(),
                w0: w * 0.35,
                p1: tip.to_array(),
                w1: w,
                color,
                style: [0.35, 0.0, 0.0, 0.0],
            });
        self.points_world.push(PointInst {
            pos: tip.to_array(),
            size: w * 0.9,
            color,
            glow: 1.4,
            _pad: [0.0; 3],
        });
    }

    pub fn point(&mut self, pos: glam::Vec3, size: f32, color: [f32; 4], glow: f32) {
        self.points_world.push(PointInst {
            pos: pos.to_array(),
            size,
            color,
            glow,
            _pad: [0.0; 3],
        });
    }

    pub fn mesh(&mut self, m: &GpuMesh) {
        self.meshes.push(m.clone());
    }
}

fn premul_over() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn additive() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

fn depth_state(test: bool, write: bool) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format: DEPTH_FORMAT,
        depth_write_enabled: write,
        depth_compare: if test {
            wgpu::CompareFunction::LessEqual
        } else {
            wgpu::CompareFunction::Always
        },
        stencil: Default::default(),
        bias: Default::default(),
    }
}

#[allow(clippy::too_many_arguments)]
fn make_pipeline(
    gpu: &GpuContext,
    label: &str,
    layout: &wgpu::PipelineLayout,
    module: &wgpu::ShaderModule,
    buffers: &[wgpu::VertexBufferLayout],
    topology: wgpu::PrimitiveTopology,
    blend: wgpu::BlendState,
    depth: wgpu::DepthStencilState,
) -> wgpu::RenderPipeline {
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers,
            },
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: FORMAT,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(depth),
            multisample: wgpu::MultisampleState {
                count: SAMPLES,
                ..Default::default()
            },
            multiview: None,
            cache: None,
        })
}

/// A dynamically grown instance buffer.
struct InstBuf {
    buf: wgpu::Buffer,
    cap: u64,
}

impl InstBuf {
    fn new(gpu: &GpuContext, cap: u64) -> Self {
        Self {
            buf: Self::alloc(gpu, cap),
            cap,
        }
    }
    fn alloc(gpu: &GpuContext, cap: u64) -> wgpu::Buffer {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("inst-buf"),
            size: cap,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }
    fn write(&mut self, gpu: &GpuContext, bytes: &[u8]) {
        if bytes.len() as u64 > self.cap {
            self.cap = (bytes.len() as u64).next_power_of_two();
            self.buf = Self::alloc(gpu, self.cap);
        }
        if !bytes.is_empty() {
            gpu.queue.write_buffer(&self.buf, 0, bytes);
        }
    }
}

struct FsShader {
    pipeline: wgpu::RenderPipeline,
    tex_bgl: Option<wgpu::BindGroupLayout>,
}

/// All shared pipelines + the fullscreen shader registry.
pub struct Renderers {
    pub view_bgl: wgpu::BindGroupLayout,
    pub mesh_bgl: wgpu::BindGroupLayout,
    line_pipes: HashMap<LineLayer, wgpu::RenderPipeline>,
    point_world: wgpu::RenderPipeline,
    mesh_pipe: wgpu::RenderPipeline,
    mesh_pipe_transparent: wgpu::RenderPipeline,
    line_buf: InstBuf,
    point_buf: InstBuf,
    fs: HashMap<&'static str, FsShader>,
    sampler: wgpu::Sampler,
}

impl Renderers {
    pub fn new(gpu: &GpuContext) -> Self {
        let view_bgl = super::gpu::view_bind_group_layout(&gpu.device);

        let mesh_bgl = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("mesh-model-bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let line_src = format!(
            "{}\n{}",
            COMMON_WGSL,
            include_str!("../../shaders/line.wgsl")
        );
        let point_src = format!(
            "{}\n{}",
            COMMON_WGSL,
            include_str!("../../shaders/point.wgsl")
        );
        let mesh_src = format!(
            "{}\n{}",
            COMMON_WGSL,
            include_str!("../../shaders/mesh.wgsl")
        );
        let line_mod = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("line"),
                source: wgpu::ShaderSource::Wgsl(line_src.into()),
            });
        let point_mod = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("point"),
                source: wgpu::ShaderSource::Wgsl(point_src.into()),
            });
        let mesh_mod = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("mesh"),
                source: wgpu::ShaderSource::Wgsl(mesh_src.into()),
            });

        let view_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("view-only"),
                bind_group_layouts: &[&view_bgl],
                push_constant_ranges: &[],
            });
        let mesh_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("view+model"),
                bind_group_layouts: &[&view_bgl, &mesh_bgl],
                push_constant_ranges: &[],
            });

        let line_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x3, 3 => Float32, 4 => Float32x4, 5 => Float32x4];
        let line_vbl = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineInst>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &line_attrs,
        };
        let point_attrs =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32, 2 => Float32x4, 3 => Float32];
        let point_vbl = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PointInst>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &point_attrs,
        };
        let mesh_attrs = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x4];
        let mesh_vbl = wgpu::VertexBufferLayout {
            array_stride: (10 * 4) as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &mesh_attrs,
        };

        let strip = wgpu::PrimitiveTopology::TriangleStrip;
        let mut line_pipes = HashMap::new();
        line_pipes.insert(
            LineLayer::World,
            make_pipeline(
                gpu,
                "line-world",
                &view_layout,
                &line_mod,
                std::slice::from_ref(&line_vbl),
                strip,
                premul_over(),
                depth_state(true, false),
            ),
        );
        line_pipes.insert(
            LineLayer::WorldAdd,
            make_pipeline(
                gpu,
                "line-world-add",
                &view_layout,
                &line_mod,
                std::slice::from_ref(&line_vbl),
                strip,
                additive(),
                depth_state(true, false),
            ),
        );
        line_pipes.insert(
            LineLayer::Overlay,
            make_pipeline(
                gpu,
                "line-ov",
                &view_layout,
                &line_mod,
                std::slice::from_ref(&line_vbl),
                strip,
                premul_over(),
                depth_state(false, false),
            ),
        );
        line_pipes.insert(
            LineLayer::OverlayAdd,
            make_pipeline(
                gpu,
                "line-ov-add",
                &view_layout,
                &line_mod,
                std::slice::from_ref(&line_vbl),
                strip,
                additive(),
                depth_state(false, false),
            ),
        );

        let point_world = make_pipeline(
            gpu,
            "pt-world",
            &view_layout,
            &point_mod,
            std::slice::from_ref(&point_vbl),
            strip,
            additive(),
            depth_state(true, false),
        );

        let tri = wgpu::PrimitiveTopology::TriangleList;
        let mesh_pipe = make_pipeline(
            gpu,
            "mesh",
            &mesh_layout,
            &mesh_mod,
            std::slice::from_ref(&mesh_vbl),
            tri,
            premul_over(),
            depth_state(true, true),
        );
        let mesh_pipe_transparent = make_pipeline(
            gpu,
            "mesh-t",
            &mesh_layout,
            &mesh_mod,
            &[mesh_vbl],
            tri,
            premul_over(),
            depth_state(true, false),
        );

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("field-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Self {
            view_bgl,
            mesh_bgl,
            line_pipes,
            point_world,
            mesh_pipe,
            mesh_pipe_transparent,
            line_buf: InstBuf::new(gpu, 1 << 20),
            point_buf: InstBuf::new(gpu, 1 << 18),
            fs: HashMap::new(),
            sampler,
        }
    }

    /// Register a fullscreen field shader. `frag_src` must define
    /// `fn shade(ndc: vec2<f32>, uv: vec2<f32>) -> vec4<f32>`.
    /// If `with_texture`, group(1) provides `t_field` + `s_field`.
    pub fn register_field(
        &mut self,
        gpu: &GpuContext,
        key: &'static str,
        frag_src: &str,
        with_texture: bool,
    ) {
        if self.fs.contains_key(key) {
            return;
        }
        let tex_decl = if with_texture {
            "@group(1) @binding(0) var t_field: texture_2d<f32>;\n@group(1) @binding(1) var s_field: sampler;\n"
        } else {
            ""
        };
        let src = format!(
            r#"{common}
{tex}
struct FsOut {{ @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32>, @location(1) ndc: vec2<f32> }};
@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> FsOut {{
    var out: FsOut;
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi >> 1u) * 4 - 1);
    out.pos = vec4<f32>(x, y, 0.999, 1.0);
    out.ndc = vec2<f32>(x, y);
    out.uv = vec2<f32>(x, -y) * 0.5 + 0.5;
    return out;
}}
{frag}
@fragment
fn fs_main(in: FsOut) -> @location(0) vec4<f32> {{
    return shade(in.ndc, in.uv);
}}
"#,
            common = COMMON_WGSL,
            tex = tex_decl,
            frag = frag_src
        );
        let module = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(key),
                source: wgpu::ShaderSource::Wgsl(src.into()),
            });
        let tex_bgl = with_texture.then(|| {
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("field-tex-bgl"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                })
        });
        let layout = {
            let mut bgls: Vec<&wgpu::BindGroupLayout> = vec![&self.view_bgl];
            if let Some(ref b) = tex_bgl {
                bgls.push(b);
            }
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some(key),
                    bind_group_layouts: &bgls,
                    push_constant_ranges: &[],
                })
        };
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(key),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FORMAT,
                        blend: Some(premul_over()),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(depth_state(false, false)),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    ..Default::default()
                },
                multiview: None,
                cache: None,
            });
        self.fs.insert(key, FsShader { pipeline, tex_bgl });
    }

    /// Render a scene into a view target. One encoder+submit per view so
    /// dynamic instance buffers can be safely reused across views.
    pub fn render_scene(
        &mut self,
        gpu: &GpuContext,
        target: &super::gpu::ViewTarget,
        scene: &Scene,
        pre: Option<&mut dyn FnMut(&mut wgpu::CommandEncoder)>,
    ) {
        // Pack line + point instances into contiguous buffers.
        let mut line_ranges: HashMap<LineLayer, (u32, u32)> = HashMap::new();
        let mut line_data: Vec<LineInst> = Vec::new();
        for layer in LINE_LAYERS {
            if let Some(v) = scene.lines.get(&layer) {
                line_ranges.insert(layer, (line_data.len() as u32, v.len() as u32));
                line_data.extend_from_slice(v);
            }
        }
        let pw = scene.points_world.len() as u32;
        let point_data = &scene.points_world;

        self.line_buf.write(gpu, bytemuck::cast_slice(&line_data));
        self.point_buf.write(gpu, bytemuck::cast_slice(point_data));

        let field_bg = scene.field.and_then(|key| {
            let sh = self.fs.get(key)?;
            match (&sh.tex_bgl, &scene.field_tex) {
                (Some(bgl), Some(view)) => Some((
                    key,
                    Some(gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("field-tex-bg"),
                        layout: bgl,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.sampler),
                            },
                        ],
                    })),
                )),
                (None, _) => Some((key, None)),
                _ => None,
            }
        });

        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        if let Some(pre) = pre {
            pre(&mut encoder);
        }
        {
            let clear = wgpu::Color {
                r: scene.clear[0],
                g: scene.clear[1],
                b: scene.clear[2],
                a: scene.clear[3],
            };
            let mut pass = target.begin(&mut encoder, clear);
            pass.set_bind_group(0, &target.bind_group, &[]);

            if let Some((key, tex_bg)) = &field_bg {
                let sh = &self.fs[key];
                pass.set_pipeline(&sh.pipeline);
                if let Some(bg) = tex_bg {
                    pass.set_bind_group(1, bg, &[]);
                }
                pass.draw(0..3, 0..1);
            }

            for m in &scene.meshes {
                let transparent = m.alpha() < 0.999;
                pass.set_pipeline(if transparent {
                    &self.mesh_pipe_transparent
                } else {
                    &self.mesh_pipe
                });
                pass.set_bind_group(1, &m.bind_group, &[]);
                pass.set_vertex_buffer(0, m.vbuf.slice(..));
                pass.set_index_buffer(m.ibuf.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..m.n_idx, 0, 0..1);
            }

            pass.set_vertex_buffer(0, self.line_buf.buf.slice(..));
            for layer in [LineLayer::World, LineLayer::WorldAdd] {
                if let Some(&(start, n)) = line_ranges.get(&layer) {
                    pass.set_pipeline(&self.line_pipes[&layer]);
                    pass.draw(0..4, start..start + n);
                }
            }

            if pw > 0 {
                pass.set_pipeline(&self.point_world);
                pass.set_vertex_buffer(0, self.point_buf.buf.slice(..));
                pass.draw(0..4, 0..pw);
            }

            pass.set_vertex_buffer(0, self.line_buf.buf.slice(..));
            for layer in [LineLayer::Overlay, LineLayer::OverlayAdd] {
                if let Some(&(start, n)) = line_ranges.get(&layer) {
                    pass.set_pipeline(&self.line_pipes[&layer]);
                    pass.draw(0..4, start..start + n);
                }
            }
        }
        gpu.queue.submit([encoder.finish()]);
    }
}
