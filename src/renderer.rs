use eframe::egui_wgpu;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
use crate::entities::GlyphShape;
use crate::mesh_gen::{
    make_axes, icosphere, make_cube, make_cylinder, make_torus, make_cone,
    make_infinite_grid, plane_model,
};
use crate::viewport::MeshRenderData;

// ─────────────────────────────────────────────────────────────────────────────
// Vertex types
// ─────────────────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vtx {
    pub pos:   [f32; 3],
    pub color: [f32; 4],
}

pub fn vtx(x: f32, y: f32, z: f32, r: f32, g: f32, b: f32, a: f32) -> Vtx {
    Vtx { pos: [x, y, z], color: [r, g, b, a] }
}

pub fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vtx>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
        ],
    }
}

/// Lit vertex: position + color + surface normal.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LitVtx {
    pub pos:    [f32; 3],
    pub color:  [f32; 4],
    pub normal: [f32; 3],
}

pub fn lit_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<LitVtx>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 28, shader_location: 2, format: wgpu::VertexFormat::Float32x3 },
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Uniform
// ─────────────────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    pub mvp:        [[f32; 4]; 4],
    pub color:      [f32; 4],       // multiplied in shader on top of vertex color
    pub light_dir:  [f32; 4],       // eye-space light direction (xyz), w=0 padding
    // normal_mat: 3×3 stored as 3 columns of vec4 for std140 alignment
    pub normal_mat_col0: [f32; 4],
    pub normal_mat_col1: [f32; 4],
    pub normal_mat_col2: [f32; 4],
}

// ─────────────────────────────────────────────────────────────────────────────
// Renderer — holds wgpu pipelines + static geometry
// ─────────────────────────────────────────────────────────────────────────────

pub struct Renderer {
    pub line_pipeline:      wgpu::RenderPipeline,
    pub tri_pipeline:       wgpu::RenderPipeline,
    pub lit_tri_pipeline:   wgpu::RenderPipeline,
    pub bgl:                wgpu::BindGroupLayout,

    pub axes_vbuf:      wgpu::Buffer,
    pub axes_count:     u32,

    pub sphere_vbuf:    wgpu::Buffer,
    pub sphere_ibuf:    wgpu::Buffer,
    pub sphere_icount:  u32,
}

impl Renderer {
    pub fn new(device: &wgpu::Device, fmt: wgpu::TextureFormat) -> Self {
        let src = include_str!("shaders/viewport.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("vp_shader"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });

        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("vp_bgl"),
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

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });

        let color_target = wgpu::ColorTargetState {
            format: fmt,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        };

        let vs = wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[vertex_layout()],
            compilation_options: Default::default(),
        };

        let line_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("line_pip"),
            layout: Some(&layout),
            vertex: vs.clone(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(color_target.clone())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::LineList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let tri_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("tri_pip"),
            layout: Some(&layout),
            vertex: vs,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(color_target)],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let lit_vs = wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_lit"),
            buffers: &[lit_vertex_layout()],
            compilation_options: Default::default(),
        };

        let lit_tri_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lit_tri_pip"),
            layout: Some(&layout),
            vertex: lit_vs,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_lit"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fmt,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // Geometry
        let (av, ac) = make_axes(1.0);
        let axes_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("axes"), contents: bytemuck::cast_slice(&av),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let (sv, si) = icosphere(2);
        let sphere_icount = si.len() as u32;
        let sphere_vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sph_v"), contents: bytemuck::cast_slice(&sv),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let sphere_ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sph_i"), contents: bytemuck::cast_slice(&si),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            line_pipeline, tri_pipeline, lit_tri_pipeline, bgl,
            axes_vbuf, axes_count: ac,
            sphere_vbuf, sphere_ibuf, sphere_icount,
        }
    }

    pub fn make_bg(&self, device: &wgpu::Device, u: &Uniforms) -> wgpu::BindGroup {
        let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(u),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buf.as_entire_binding(),
            }],
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Per-frame draw list built in prepare(), consumed in paint()
// ─────────────────────────────────────────────────────────────────────────────

#[allow(dead_code)]
pub enum DrawCmd {
    Lines    { bg: wgpu::BindGroup, count: u32 },
    Sphere   { bg: wgpu::BindGroup },
    /// Dynamic line list — stores its own vertex buffer built each frame.
    DynLines { bg: wgpu::BindGroup, vbuf: wgpu::Buffer, count: u32 },
    /// Dynamic triangle list — stores vertex+index buffers built each frame.
    DynTris  { bg: wgpu::BindGroup, vbuf: wgpu::Buffer, ibuf: wgpu::Buffer, icount: u32 },
    /// Lit dynamic triangle list — uses the lit pipeline with normals.
    LitTris  { bg: wgpu::BindGroup, vbuf: wgpu::Buffer, ibuf: wgpu::Buffer, icount: u32 },
}

pub struct FrameList(pub Vec<DrawCmd>);

// ─────────────────────────────────────────────────────────────────────────────
// egui_wgpu callback
// ─────────────────────────────────────────────────────────────────────────────

pub struct VpCallback {
    pub vp:            Mat4,   // proj * view
    pub view:          Mat4,   // view matrix (for computing normal matrix per model)
    pub show_coord_sys: bool,
    pub show_xy:       bool,
    pub show_yz:       bool,
    pub show_zx:       bool,
    pub nodes:         Vec<[f32; 3]>,
    pub node_colors:   Vec<[f32; 4]>,
    pub node_scales:   Vec<f32>,
    /// Local coord sys for each node (3x3 column-major). Identity = no local axes drawn.
    pub node_coord_sys:     Vec<[[f32; 3]; 3]>,
    /// Per-node visibility flag for their local coord sys axes.
    pub node_coord_sys_visible: Vec<bool>,
    /// Global switch: if false no local coord sys axes are drawn for any node.
    pub show_local_coord_sys: bool,
    /// Resolved (from, to) world-space pairs for valid edges.
    pub edge_segments: Vec<([f32; 3], [f32; 3])>,
    /// Per-edge colors: (from_color, to_color) for each edge segment.
    pub edge_colors:   Vec<([f32; 4], [f32; 4])>,
    /// Camera distance — used to keep coord sys/planes constant screen size.
    pub distance:      f32,
    /// Base size of node spheres.
    pub node_size:     f32,
    /// Per-node selection flags for highlight rings.
    pub selected_nodes: Vec<bool>,
    /// Arm length of local coord sys as a fraction of view_scale * 0.25 (same formula as global coord sys).
    pub local_coord_sys_scale: f32,
    /// Global radius of edge cylinders (fallback).
    pub edge_thickness: f32,
    /// Per-edge radii (overrides edge_thickness when present).
    pub edge_thicknesses: Vec<f32>,
    /// Glyph data: (world_pos, shape, color_rgba, size, stretch, tube_ratio)
    pub glyphs: Vec<([f32; 3], GlyphShape, [f32; 4], f32, [f32; 3], f32)>,
    /// Per-glyph selection flags for highlight rings.
    pub glyph_selected: Vec<bool>,
    /// Pre-triangulated mesh surfaces.
    pub mesh_surfaces: Vec<MeshRenderData>,
    /// Wireframe edges: undeformed (base) geometry, drawn as dull gray lines.
    pub wireframe_edges: Vec<([f32; 3], [f32; 3])>,
    /// Lighting brightness multiplier.
    pub light_brightness: f32,
    /// Vector arrows: (world_pos, normalised_dir, color_rgba, world_length).
    pub arrows: Vec<([f32; 3], [f32; 3], [f32; 4], f32)>,
}

impl egui_wgpu::CallbackTrait for VpCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let renderer: &Renderer = resources.get().unwrap();
        let mut cmds: Vec<DrawCmd> = Vec::new();

        // Eye-space light direction — fixed upper-left-front.
        let light_eye = Vec3::new(0.3, 0.8, 0.5).normalize();
        let light_dir_arr: [f32; 4] = [light_eye.x, light_eye.y, light_eye.z, self.light_brightness];

        // Helper: build unlit Uniforms (for lines / overlay geometry).
        let unlit = |mvp: Mat4, color: [f32; 4]| -> Uniforms {
            Uniforms {
                mvp: mvp.to_cols_array_2d(),
                color,
                light_dir: [0.0; 4],
                normal_mat_col0: [1.0, 0.0, 0.0, 0.0],
                normal_mat_col1: [0.0, 1.0, 0.0, 0.0],
                normal_mat_col2: [0.0, 0.0, 1.0, 0.0],
            }
        };

        // Helper: build lit Uniforms (for triangle geometry with normals).
        let view = self.view;
        let lit = |mvp: Mat4, model: Mat4, color: [f32; 4]| -> Uniforms {
            // Normal matrix = transpose(inverse(view * model)).  For uniform
            // scaling the inverse-transpose equals the original normalised.
            // We compute the full 3×3 inverse-transpose for correctness with
            // non-uniform scales (glyph stretch).  We store it as 3 columns
            // of vec4 for std140 alignment.
            let mv = view * model;
            let mv3 = glam::Mat3::from_cols(
                mv.x_axis.truncate(),
                mv.y_axis.truncate(),
                mv.z_axis.truncate(),
            );
            let nm = mv3.inverse().transpose();
            Uniforms {
                mvp: mvp.to_cols_array_2d(),
                color,
                light_dir: light_dir_arr,
                normal_mat_col0: [nm.x_axis.x, nm.x_axis.y, nm.x_axis.z, 0.0],
                normal_mat_col1: [nm.y_axis.x, nm.y_axis.y, nm.y_axis.z, 0.0],
                normal_mat_col2: [nm.z_axis.x, nm.z_axis.y, nm.z_axis.z, 0.0],
            }
        };

        // Scale that keeps CSYS/planes a constant fraction of the viewport in
        // both perspective and orthographic modes.
        // view_scale = distance × tan(fov/2); objects with this world size
        // fill roughly half the viewport height on screen.
        let fov_half = 45_f32.to_radians() / 2.0;
        let view_scale = self.distance * fov_half.tan();

        if self.show_coord_sys {
            // Arms = 25% of view_scale → about 12% of viewport height each.
            let model = Mat4::from_scale(Vec3::splat(view_scale * 0.25));
            let u = unlit(self.vp * model, [1.0; 4]);
            cmds.push(DrawCmd::Lines { bg: renderer.make_bg(device, &u), count: renderer.axes_count });
        }

        let plane_cfgs: &[(bool, u8, [f32;4])] = &[
            (self.show_xy, 0, [0.35, 0.55, 1.00, 0.85]),
            (self.show_yz, 1, [1.00, 0.45, 0.35, 0.85]),
            (self.show_zx, 2, [0.35, 1.00, 0.45, 0.85]),
        ];
        for &(enabled, rot, color) in plane_cfgs {
            if enabled {
                // Generate an adaptive infinite grid in world space.
                let grid = make_infinite_grid(view_scale, color);
                if grid.is_empty() { continue; }

                // Rotate into the correct plane orientation, but do NOT
                // scale — the grid is already in world coordinates.
                let model = plane_model(rot);
                let mvp   = self.vp * model;

                let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("plane_wire"),
                    contents: bytemuck::cast_slice(&grid),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let u = unlit(mvp, [1.0; 4]);
                cmds.push(DrawCmd::DynLines {
                    bg: renderer.make_bg(device, &u),
                    vbuf,
                    count: grid.len() as u32,
                });
            }
        }

        // ── Wireframe overlay (undeformed geometry) ───────────────────────
        if !self.wireframe_edges.is_empty() {
            let wire_color: [f32; 4] = [0.45, 0.45, 0.5, 0.5];
            let mut wire_verts: Vec<Vtx> = Vec::with_capacity(self.wireframe_edges.len() * 2);
            for &([ax, ay, az], [bx, by, bz]) in &self.wireframe_edges {
                wire_verts.push(vtx(ax, ay, az, wire_color[0], wire_color[1], wire_color[2], wire_color[3]));
                wire_verts.push(vtx(bx, by, bz, wire_color[0], wire_color[1], wire_color[2], wire_color[3]));
            }
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("wireframe"),
                contents: bytemuck::cast_slice(&wire_verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let u = unlit(self.vp, [1.0; 4]);
            cmds.push(DrawCmd::DynLines {
                bg: renderer.make_bg(device, &u),
                vbuf,
                count: wire_verts.len() as u32,
            });
        }

        // ── Edges (cylinders) — rendered first so nodes draw on top ───────
        if !self.edge_segments.is_empty() {
            const SEGS: u32 = 8;

            let mut verts: Vec<LitVtx> = Vec::new();
            let mut indices: Vec<u32> = Vec::new();

            for (i, &([ax, ay, az], [bx, by, bz])) in self.edge_segments.iter().enumerate() {
                let (ca, cb) = self.edge_colors.get(i)
                    .copied()
                    .unwrap_or(([0.3, 0.85, 1.0, 1.0], [0.3, 0.85, 1.0, 1.0]));

                let radius = self.edge_thicknesses.get(i).copied().unwrap_or(self.edge_thickness);

                let a = Vec3::new(ax, ay, az);
                let b = Vec3::new(bx, by, bz);
                let dir = b - a;
                let len = dir.length();
                if len < 1e-8 { continue; }

                let fwd = dir / len;
                let up_vec = if fwd.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
                let u_ax = fwd.cross(up_vec).normalize();
                let v_ax = u_ax.cross(fwd);

                let base_idx = verts.len() as u32;

                for seg in 0..SEGS {
                    let angle = std::f32::consts::TAU * seg as f32 / SEGS as f32;
                    let (sin_a, cos_a) = angle.sin_cos();
                    let radial = u_ax * cos_a + v_ax * sin_a;
                    let offset = radial * radius;
                    let n = [radial.x, radial.y, radial.z];

                    let pa = a + offset;
                    verts.push(LitVtx { pos: [pa.x, pa.y, pa.z], color: ca, normal: n });
                    let pb = b + offset;
                    verts.push(LitVtx { pos: [pb.x, pb.y, pb.z], color: cb, normal: n });
                }

                for seg in 0..SEGS {
                    let next = (seg + 1) % SEGS;
                    let bl = base_idx + seg * 2;
                    let tl = base_idx + seg * 2 + 1;
                    let br = base_idx + next * 2;
                    let tr = base_idx + next * 2 + 1;
                    indices.extend_from_slice(&[bl, br, tl, tl, br, tr]);
                }
            }

            if !indices.is_empty() {
                let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("edge_cyl_v"),
                    contents: bytemuck::cast_slice(&verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("edge_cyl_i"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                let model = Mat4::IDENTITY;
                let u = lit(self.vp * model, model, [1.0; 4]);
                cmds.push(DrawCmd::LitTris {
                    bg: renderer.make_bg(device, &u),
                    vbuf,
                    ibuf,
                    icount: indices.len() as u32,
                });
            }
        }

        // ── Nodes (spheres) — rendered after edges so they appear on top ──
        for (ni, &[x, y, z]) in self.nodes.iter().enumerate() {
            let size_scale = self.node_scales.get(ni).copied().unwrap_or(1.0);
            let model = Mat4::from_translation(Vec3::new(x, y, z))
                * Mat4::from_scale(Vec3::splat(self.node_size * size_scale));
            let color = self.node_colors.get(ni).copied().unwrap_or([1.0, 0.85, 0.1, 1.0]);
            let u = lit(self.vp * model, model, color);
            cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &u) });

            // Highlight ring for all selected nodes
            if self.selected_nodes.get(ni).copied().unwrap_or(false) {
                let highlight_model = Mat4::from_translation(Vec3::new(x, y, z))
                    * Mat4::from_scale(Vec3::splat(self.node_size * size_scale * 1.5));
                let hu = lit(self.vp * highlight_model, highlight_model, [1.0, 0.9, 0.3, 1.0]);
                cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &hu) });
            }

            // ── Local CSYS axes ──────────────────────────────────────────
            if self.show_local_coord_sys
                && self.node_coord_sys_visible.get(ni).copied().unwrap_or(true)
            {
                if let Some(&csys) = self.node_coord_sys.get(ni) {
                    let id = [[1.0f32,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]];
                    if csys != id {
                        let origin   = Vec3::new(x, y, z);
                        // Scale exactly like the global coord sys: view_scale * 0.25 is the
                        // global arm length; multiply by local_coord_sys_scale for relative size.
                        let axis_len = view_scale * 0.25 * self.local_coord_sys_scale;
                        let axis_colors: [[f32;4]; 3] = [
                            [0.9, 0.25, 0.25, 1.0], // X: red
                            [0.25, 0.85, 0.25, 1.0], // Y: green
                            [0.25, 0.45, 0.95, 1.0], // Z: blue
                        ];
                        for (&ax_col, &ax_vec) in axis_colors.iter().zip(csys.iter()) {
                            let tip = origin + Vec3::new(ax_vec[0], ax_vec[1], ax_vec[2]) * axis_len;
                            let local_axis_verts = vec![
                                vtx(origin.x, origin.y, origin.z, ax_col[0], ax_col[1], ax_col[2], ax_col[3]),
                                vtx(tip.x, tip.y, tip.z, ax_col[0], ax_col[1], ax_col[2], ax_col[3]),
                            ];
                            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("local_ax"),
                                contents: bytemuck::cast_slice(&local_axis_verts),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                            let u = unlit(self.vp, [1.0; 4]);
                            cmds.push(DrawCmd::DynLines {
                                bg: renderer.make_bg(device, &u),
                                vbuf,
                                count: 2,
                            });
                        }
                    }
                }
            }
        }

        // ── Glyphs ─────────────────────────────────────────────────────────
        for (gi, &(pos, ref shape, color, size, stretch, tube_ratio)) in self.glyphs.iter().enumerate() {
            let model = Mat4::from_translation(Vec3::from(pos))
                * Mat4::from_scale(Vec3::new(size * stretch[0], size * stretch[1], size * stretch[2]));
            let u = lit(self.vp * model, model, color);

            match shape {
                GlyphShape::Sphere => {
                    cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &u) });
                }
                GlyphShape::Cube => {
                    let (verts, indices) = make_cube([1.0; 4]);
                    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_cube_v"),
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_cube_i"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    cmds.push(DrawCmd::LitTris {
                        bg: renderer.make_bg(device, &u),
                        vbuf, ibuf, icount: indices.len() as u32,
                    });
                }
                GlyphShape::Cylinder => {
                    let (verts, indices) = make_cylinder(16, [1.0; 4]);
                    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_cyl_v"),
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_cyl_i"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    cmds.push(DrawCmd::LitTris {
                        bg: renderer.make_bg(device, &u),
                        vbuf, ibuf, icount: indices.len() as u32,
                    });
                }
                GlyphShape::Torus => {
                    let (verts, indices) = make_torus(16, 8, tube_ratio, [1.0; 4]);
                    let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_tor_v"),
                        contents: bytemuck::cast_slice(&verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("glyph_tor_i"),
                        contents: bytemuck::cast_slice(&indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    cmds.push(DrawCmd::LitTris {
                        bg: renderer.make_bg(device, &u),
                        vbuf, ibuf, icount: indices.len() as u32,
                    });
                }
            }

            // Selection highlight ring
            if self.glyph_selected.get(gi).copied().unwrap_or(false) {
                let hm = Mat4::from_translation(Vec3::from(pos))
                    * Mat4::from_scale(Vec3::splat(size * 1.3));
                let hu = lit(self.vp * hm, hm, [1.0, 0.6, 0.2, 1.0]);
                cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &hu) });
            }
        }

        // ── Mesh surfaces (lit with per-face normals) ──────────────────────
        for mesh in &self.mesh_surfaces {
            if mesh.indices.is_empty() || mesh.verts.is_empty() { continue; }
            // Build LitVtx with per-face normals.
            let positions: Vec<[f32; 3]> = mesh.verts.iter().map(|v| [v[0], v[1], v[2]]).collect();
            let colors: Vec<[f32; 4]> = mesh.verts.iter().map(|v| [v[3], v[4], v[5], v[6]]).collect();
            // Accumulate face normals per vertex.
            let mut normals = vec![[0.0f32; 3]; positions.len()];
            for tri in mesh.indices.chunks(3) {
                if tri.len() < 3 { continue; }
                let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                let a = Vec3::from(positions[i0]);
                let b = Vec3::from(positions[i1]);
                let c = Vec3::from(positions[i2]);
                let n = (b - a).cross(c - a);
                for &idx in &[i0, i1, i2] {
                    normals[idx][0] += n.x;
                    normals[idx][1] += n.y;
                    normals[idx][2] += n.z;
                }
            }
            let lit_verts: Vec<LitVtx> = positions.iter().zip(colors.iter()).zip(normals.iter()).map(|((&p, &c), n)| {
                let len = (n[0]*n[0] + n[1]*n[1] + n[2]*n[2]).sqrt().max(1e-8);
                LitVtx { pos: p, color: c, normal: [n[0]/len, n[1]/len, n[2]/len] }
            }).collect();
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_v"),
                contents: bytemuck::cast_slice(&lit_verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_i"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let model = Mat4::IDENTITY;
            let u = lit(self.vp * model, model, [1.0; 4]);
            cmds.push(DrawCmd::LitTris {
                bg: renderer.make_bg(device, &u),
                vbuf, ibuf, icount: mesh.indices.len() as u32,
            });
        }

        // ── Vector arrows ──────────────────────────────────────────────────
        let (cone_verts, cone_indices) = make_cone(12);
        let (cyl_verts,  cyl_indices)  = make_cylinder(8, [1.0; 4]);
        for &(pos, dir, color, length) in &self.arrows {
            if length < 1e-9 { continue; }
            let dir_vec = Vec3::from(dir);
            // Rotation from +Y to dir.
            let base_rot = if dir_vec.dot(Vec3::Y) > 0.9999 {
                Mat4::IDENTITY
            } else if dir_vec.dot(Vec3::Y) < -0.9999 {
                Mat4::from_axis_angle(Vec3::X, std::f32::consts::PI)
            } else {
                Mat4::from_quat(glam::Quat::from_rotation_arc(Vec3::Y, dir_vec))
            };
            let base = Mat4::from_translation(Vec3::from(pos)) * base_rot * Mat4::from_scale(Vec3::splat(length));

            // Shaft: cylinder occupies y ∈ [-1, 1] centred at origin; scale and
            // translate so it spans y ∈ [0, 0.75] in arrow space.
            const SHAFT_R: f32 = 0.04;
            const SHAFT_HALF: f32 = 0.375; // half-height of shaft in arrow space
            let shaft_local = Mat4::from_translation(Vec3::new(0.0, SHAFT_HALF, 0.0))
                * Mat4::from_scale(Vec3::new(SHAFT_R, SHAFT_HALF, SHAFT_R));
            let shaft_model = base * shaft_local;
            // Rebuild the cylinder with the arrow's colour.
            let shaft_colored: Vec<LitVtx> = cyl_verts.iter()
                .map(|v| LitVtx { pos: v.pos, color, normal: v.normal })
                .collect();
            let sv = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arrow_shaft_v"),
                contents: bytemuck::cast_slice(&shaft_colored),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let si = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arrow_shaft_i"),
                contents: bytemuck::cast_slice(&cyl_indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            cmds.push(DrawCmd::LitTris {
                bg: renderer.make_bg(device, &lit(self.vp * shaft_model, shaft_model, color)),
                vbuf: sv, ibuf: si, icount: cyl_indices.len() as u32,
            });

            // Cone tip: spans y ∈ [0, 1] in cone space; translate to y=0.75
            // and scale so it spans y ∈ [0.75, 1.0] in arrow space.
            const CONE_R: f32 = 0.12;
            const CONE_H: f32 = 0.25; // cone height in arrow space
            let cone_local = Mat4::from_translation(Vec3::new(0.0, 0.75, 0.0))
                * Mat4::from_scale(Vec3::new(CONE_R, CONE_H, CONE_R));
            let cone_model = base * cone_local;
            let cone_colored: Vec<LitVtx> = cone_verts.iter()
                .map(|v| LitVtx { pos: v.pos, color, normal: v.normal })
                .collect();
            let cv = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arrow_cone_v"),
                contents: bytemuck::cast_slice(&cone_colored),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ci = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("arrow_cone_i"),
                contents: bytemuck::cast_slice(&cone_indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            cmds.push(DrawCmd::LitTris {
                bg: renderer.make_bg(device, &lit(self.vp * cone_model, cone_model, color)),
                vbuf: cv, ibuf: ci, icount: cone_indices.len() as u32,
            });
        }

        resources.insert(FrameList(cmds));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let renderer: &Renderer = resources.get().unwrap();
        let frame: &FrameList = resources.get().unwrap();

        for cmd in &frame.0 {
            match cmd {
                DrawCmd::Lines { bg, count } => {
                    pass.set_pipeline(&renderer.line_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, renderer.axes_vbuf.slice(..));
                    pass.draw(0..*count, 0..1);
                }
                DrawCmd::Sphere { bg } => {
                    pass.set_pipeline(&renderer.lit_tri_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, renderer.sphere_vbuf.slice(..));
                    pass.set_index_buffer(renderer.sphere_ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..renderer.sphere_icount, 0, 0..1);
                }
                DrawCmd::DynLines { bg, vbuf, count } => {
                    pass.set_pipeline(&renderer.line_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..));
                    pass.draw(0..*count, 0..1);
                }
                DrawCmd::DynTris { bg, vbuf, ibuf, icount } => {
                    pass.set_pipeline(&renderer.tri_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..));
                    pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*icount, 0, 0..1);
                }
                DrawCmd::LitTris { bg, vbuf, ibuf, icount } => {
                    pass.set_pipeline(&renderer.lit_tri_pipeline);
                    pass.set_bind_group(0, bg, &[]);
                    pass.set_vertex_buffer(0, vbuf.slice(..));
                    pass.set_index_buffer(ibuf.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..*icount, 0, 0..1);
                }
            }
        }
    }
}
