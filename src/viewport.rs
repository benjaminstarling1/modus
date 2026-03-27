use eframe::egui_wgpu;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;
use crate::table::GlyphShape;
use crate::persist::DistanceUnit;

/// Pre-computed mesh surface data: vertices (pos + rgba) and triangle indices.
pub struct MeshRenderData {
    /// Each vertex: [x, y, z, r, g, b, a]
    pub verts:   Vec<[f32; 7]>,
    pub indices: Vec<u32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public state
// ─────────────────────────────────────────────────────────────────────────────

pub struct Viewport3D {
    pub azimuth:         f32,
    pub elevation:       f32,
    pub distance:        f32,
    pub target:          Vec3,
    pub show_csys:       bool,
    pub show_local_csys: bool,   // global toggle for all local node CSYS
    pub show_xy:         bool,
    pub show_yz:         bool,
    pub show_zx:         bool,
    pub orthographic:    bool,
}

impl Default for Viewport3D {
    fn default() -> Self {
        Self {
            azimuth:         45_f32.to_radians(),
            elevation:       30_f32.to_radians(),
            distance:        5.0,
            target:          Vec3::ZERO,
            show_csys:       true,
            show_local_csys: true,
            show_xy:         false,
            show_yz:         false,
            show_zx:         false,
            orthographic:    true,
        }
    }
}

impl Viewport3D {
    fn eye(&self) -> Vec3 {
        self.target
            + Vec3::new(
                self.distance * self.elevation.cos() * self.azimuth.sin(),
                self.distance * self.elevation.sin(),
                self.distance * self.elevation.cos() * self.azimuth.cos(),
            )
    }
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }
    pub fn proj_matrix(&self, aspect: f32) -> Mat4 {
        // Scale near/far with camera distance for consistent depth precision.
        let near = self.distance * 0.001;
        let far  = self.distance * 200.0;
        if self.orthographic {
            // Half-height matches what you'd see in perspective at this distance.
            let half_h = self.distance * (45_f32.to_radians() / 2.0).tan();
            let half_w = half_h * aspect;
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, -far, far)
        } else {
            Mat4::perspective_rh(45_f32.to_radians(), aspect, near.max(1e-6), far)
        }
    }
    pub fn init_renderer(wrs: &egui_wgpu::RenderState) {
        let r = Renderer::new(&wrs.device, wrs.target_format);
        wrs.renderer.write().callback_resources.insert(r);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Vertex type
// ─────────────────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vtx {
    pos:   [f32; 3],
    color: [f32; 4],
}

fn vtx(x: f32, y: f32, z: f32, r: f32, g: f32, b: f32, a: f32) -> Vtx {
    Vtx { pos: [x, y, z], color: [r, g, b, a] }
}

fn vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vtx>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { offset: 0,  shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
        ],
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Uniform
// ─────────────────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    mvp:   [[f32; 4]; 4],
    color: [f32; 4],   // multiplied in shader on top of vertex color
}

// ─────────────────────────────────────────────────────────────────────────────
// Renderer — holds wgpu pipelines + static geometry
// ─────────────────────────────────────────────────────────────────────────────

pub struct Renderer {
    line_pipeline:  wgpu::RenderPipeline,
    tri_pipeline:   wgpu::RenderPipeline,
    bgl:            wgpu::BindGroupLayout,

    axes_vbuf:      wgpu::Buffer,
    axes_count:     u32,

    sphere_vbuf:    wgpu::Buffer,
    sphere_ibuf:    wgpu::Buffer,
    sphere_icount:  u32,
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
            line_pipeline, tri_pipeline, bgl,
            axes_vbuf, axes_count: ac,
            sphere_vbuf, sphere_ibuf, sphere_icount,
        }
    }

    fn make_bg(&self, device: &wgpu::Device, u: &Uniforms) -> wgpu::BindGroup {
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

enum DrawCmd {
    Lines    { bg: wgpu::BindGroup, count: u32 },
    Sphere   { bg: wgpu::BindGroup },
    /// Dynamic line list — stores its own vertex buffer built each frame.
    DynLines { bg: wgpu::BindGroup, vbuf: wgpu::Buffer, count: u32 },
    /// Dynamic triangle list — stores vertex+index buffers built each frame.
    DynTris  { bg: wgpu::BindGroup, vbuf: wgpu::Buffer, ibuf: wgpu::Buffer, icount: u32 },
}

struct FrameList(Vec<DrawCmd>);

// ─────────────────────────────────────────────────────────────────────────────
// egui_wgpu callback
// ─────────────────────────────────────────────────────────────────────────────

pub struct VpCallback {
    vp:            Mat4,   // proj * view
    show_csys:     bool,
    show_xy:       bool,
    show_yz:       bool,
    show_zx:       bool,
    nodes:         Vec<[f32; 3]>,
    node_colors:   Vec<[f32; 4]>,
    node_scales:   Vec<f32>,
    /// Local CSYS for each node (3x3 column-major). Identity = no local axes drawn.
    node_csys:     Vec<[[f32; 3]; 3]>,
    /// Per-node visibility flag for their local CSYS axes.
    node_csys_visible: Vec<bool>,
    /// Global switch: if false no local CSYS axes are drawn for any node.
    show_local_csys: bool,
    /// Resolved (from, to) world-space pairs for valid edges.
    edge_segments: Vec<([f32; 3], [f32; 3])>,
    /// Per-edge colors: (from_color, to_color) for each edge segment.
    edge_colors:   Vec<([f32; 4], [f32; 4])>,
    /// Camera distance — used to keep CSYS/planes constant screen size.
    distance:      f32,
    /// Base size of node spheres.
    node_size:     f32,
    /// Per-node selection flags for highlight rings.
    selected_nodes: Vec<bool>,
    /// Arm length of local CSYS as a fraction of view_scale * 0.25 (same formula as global CSYS).
    local_csys_scale: f32,
    /// Radius of edge cylinders.
    edge_thickness: f32,
    /// Glyph data: (world_pos, shape, color_rgba, size, stretch, tube_ratio)
    glyphs: Vec<([f32; 3], GlyphShape, [f32; 4], f32, [f32; 3], f32)>,
    /// Per-glyph selection flags for highlight rings.
    glyph_selected: Vec<bool>,
    /// Pre-triangulated mesh surfaces.
    mesh_surfaces: Vec<MeshRenderData>,
    /// Wireframe edges: undeformed (base) geometry, drawn as dull gray lines.
    wireframe_edges: Vec<([f32; 3], [f32; 3])>,
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

        // Scale that keeps CSYS/planes a constant fraction of the viewport in
        // both perspective and orthographic modes.
        // view_scale = distance × tan(fov/2); objects with this world size
        // fill roughly half the viewport height on screen.
        let fov_half = 45_f32.to_radians() / 2.0;
        let view_scale = self.distance * fov_half.tan();

        if self.show_csys {
            // Arms = 25% of view_scale → about 12% of viewport height each.
            let model = Mat4::from_scale(Vec3::splat(view_scale * 0.25));
            let u = Uniforms { mvp: (self.vp * model).to_cols_array_2d(), color: [1.0; 4] };
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
                let u = Uniforms { mvp: mvp.to_cols_array_2d(), color: [1.0; 4] };
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
            let u = Uniforms { mvp: self.vp.to_cols_array_2d(), color: [1.0; 4] };
            cmds.push(DrawCmd::DynLines {
                bg: renderer.make_bg(device, &u),
                vbuf,
                count: wire_verts.len() as u32,
            });
        }

        // ── Edges (cylinders) — rendered first so nodes draw on top ───────
        if !self.edge_segments.is_empty() {
            const SEGS: u32 = 8;
            let radius = self.edge_thickness;

            let mut verts: Vec<Vtx> = Vec::new();
            let mut indices: Vec<u32> = Vec::new();

            for (i, &([ax, ay, az], [bx, by, bz])) in self.edge_segments.iter().enumerate() {
                let (ca, cb) = self.edge_colors.get(i)
                    .copied()
                    .unwrap_or(([0.3, 0.85, 1.0, 1.0], [0.3, 0.85, 1.0, 1.0]));

                let a = Vec3::new(ax, ay, az);
                let b = Vec3::new(bx, by, bz);
                let dir = b - a;
                let len = dir.length();
                if len < 1e-8 { continue; }

                let fwd = dir / len;
                let up = if fwd.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
                let u = fwd.cross(up).normalize();
                let v = u.cross(fwd);

                let base_idx = verts.len() as u32;

                for seg in 0..SEGS {
                    let angle = std::f32::consts::TAU * seg as f32 / SEGS as f32;
                    let (sin_a, cos_a) = angle.sin_cos();
                    let offset = (u * cos_a + v * sin_a) * radius;

                    let pa = a + offset;
                    verts.push(Vtx { pos: [pa.x, pa.y, pa.z], color: ca });
                    let pb = b + offset;
                    verts.push(Vtx { pos: [pb.x, pb.y, pb.z], color: cb });
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
                let u = Uniforms { mvp: self.vp.to_cols_array_2d(), color: [1.0; 4] };
                cmds.push(DrawCmd::DynTris {
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
            let u = Uniforms {
                mvp: (self.vp * model).to_cols_array_2d(),
                color,
            };
            cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &u) });

            // Highlight ring for all selected nodes
            if self.selected_nodes.get(ni).copied().unwrap_or(false) {
                let highlight_model = Mat4::from_translation(Vec3::new(x, y, z))
                    * Mat4::from_scale(Vec3::splat(self.node_size * size_scale * 1.5));
                let hu = Uniforms {
                    mvp: (self.vp * highlight_model).to_cols_array_2d(),
                    color: [1.0, 0.9, 0.3, 1.0],  // yellow highlight for selection
                };
                cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &hu) });
            }

            // ── Local CSYS axes ──────────────────────────────────────────
            if self.show_local_csys
                && self.node_csys_visible.get(ni).copied().unwrap_or(true)
            {
                if let Some(&csys) = self.node_csys.get(ni) {
                    let id = [[1.0f32,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]];
                    if csys != id {
                        let origin   = Vec3::new(x, y, z);
                        // Scale exactly like the global CSYS: view_scale * 0.25 is the
                        // global arm length; multiply by local_csys_scale for relative size.
                        let axis_len = view_scale * 0.25 * self.local_csys_scale;
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
                            let u = Uniforms { mvp: self.vp.to_cols_array_2d(), color: [1.0; 4] };
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
            let u = Uniforms {
                mvp: (self.vp * model).to_cols_array_2d(),
                color,
            };

            match shape {
                GlyphShape::Sphere => {
                    cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &u) });
                }
                GlyphShape::Cube => {
                    let (verts, indices) = make_cube(color);
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
                    cmds.push(DrawCmd::DynTris {
                        bg: renderer.make_bg(device, &u),
                        vbuf, ibuf, icount: indices.len() as u32,
                    });
                }
                GlyphShape::Cylinder => {
                    let (verts, indices) = make_cylinder(16, color);
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
                    cmds.push(DrawCmd::DynTris {
                        bg: renderer.make_bg(device, &u),
                        vbuf, ibuf, icount: indices.len() as u32,
                    });
                }
                GlyphShape::Torus => {
                    let (verts, indices) = make_torus(16, 8, tube_ratio, color);
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
                    cmds.push(DrawCmd::DynTris {
                        bg: renderer.make_bg(device, &u),
                        vbuf, ibuf, icount: indices.len() as u32,
                    });
                }
            }

            // Selection highlight ring
            if self.glyph_selected.get(gi).copied().unwrap_or(false) {
                let hm = Mat4::from_translation(Vec3::from(pos))
                    * Mat4::from_scale(Vec3::splat(size * 1.3));
                let hu = Uniforms {
                    mvp: (self.vp * hm).to_cols_array_2d(),
                    color: [1.0, 0.6, 0.2, 1.0],
                };
                cmds.push(DrawCmd::Sphere { bg: renderer.make_bg(device, &hu) });
            }
        }

        // ── Mesh surfaces ──────────────────────────────────────────────────
        for mesh in &self.mesh_surfaces {
            if mesh.indices.is_empty() || mesh.verts.is_empty() { continue; }
            let verts: Vec<Vtx> = mesh.verts.iter().map(|v| {
                Vtx { pos: [v[0], v[1], v[2]], color: [v[3], v[4], v[5], v[6]] }
            }).collect();
            let vbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_v"),
                contents: bytemuck::cast_slice(&verts),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ibuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("mesh_i"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
            let u = Uniforms { mvp: self.vp.to_cols_array_2d(), color: [1.0; 4] };
            cmds.push(DrawCmd::DynTris {
                bg: renderer.make_bg(device, &u),
                vbuf, ibuf, icount: mesh.indices.len() as u32,
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
                    pass.set_pipeline(&renderer.tri_pipeline);
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
            }
        }
    }
}

fn plane_model(rot: u8) -> Mat4 {
    match rot {
        0 => Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2), // XY
        1 => Mat4::from_rotation_z( std::f32::consts::FRAC_PI_2), // YZ
        _ => Mat4::IDENTITY,                                        // ZX (already in XZ)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public show function
// ─────────────────────────────────────────────────────────────────────────────

/// Data returned from `show_viewport` so the caller can do picking and selection.
pub struct ViewportResponse {
    /// Click position in select mode (non-drag LMB), or single-node pick in orbit mode.
    pub clicked_pos: Option<egui::Pos2>,
    /// Ctrl+click position in select mode (additive toggle).
    pub ctrl_clicked_pos: Option<egui::Pos2>,
    /// Completed rubber-band selection rect (drag released in select mode).
    pub rect_selection: Option<egui::Rect>,
    /// The view-projection matrix used for rendering.
    pub vp: Mat4,
    /// The viewport rect in screen coordinates.
    pub rect: egui::Rect,
    /// If the user chose a different distance unit via the context menu.
    pub unit_change: Option<DistanceUnit>,
}

pub fn show_viewport(
    ui:               &mut egui::Ui,
    state:            &mut Viewport3D,
    nodes:            &[[f32; 3]],
    edge_segments:    &[([f32; 3], [f32; 3])],
    edge_colors:      &[([f32; 4], [f32; 4])],
    node_colors:      &[[f32; 4]],
    node_scales:      &[f32],
    node_csys:        &[[[f32; 3]; 3]],
    node_csys_visible: &[bool],
    selected_nodes:   &[bool],
    node_size:        f32,
    local_csys_scale: f32,
    select_mode:      bool,
    edge_thickness:   f32,
    viewport_bg:      [f32; 3],
    mmb_orbit:        bool,
    glyph_positions:  &[([f32; 3], GlyphShape, [f32; 4], f32, [f32; 3], f32)],
    glyph_selected:   &[bool],
    mesh_surfaces:    Vec<MeshRenderData>,
    wireframe_edges:  &[([f32; 3], [f32; 3])],
    unit_label:       &str,
    current_unit:     &DistanceUnit,
) -> ViewportResponse {
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

    // Paint viewport background
    ui.painter().rect_filled(
        rect,
        0.0,
        egui::Color32::from_rgb(
            (viewport_bg[0] * 255.0) as u8,
            (viewport_bg[1] * 255.0) as u8,
            (viewport_bg[2] * 255.0) as u8,
        ),
    );

    let mut unit_change: Option<DistanceUnit> = None;

    // Right-click context menu
    response.context_menu(|ui| {
        ui.label(egui::RichText::new("Viewport Settings").strong());
        ui.separator();
        ui.checkbox(&mut state.show_csys, "Global CSYS");
        ui.checkbox(&mut state.show_local_csys, "Local CSYS axes");
        ui.separator();
        ui.label("Reference Planes");
        ui.checkbox(&mut state.show_xy, "XY Plane");
        ui.checkbox(&mut state.show_yz, "YZ Plane");
        ui.checkbox(&mut state.show_zx, "ZX Plane");
        ui.separator();
        ui.label("Projection");
        ui.radio_value(&mut state.orthographic, false, "Perspective");
        ui.radio_value(&mut state.orthographic, true,  "Orthographic");
        ui.separator();
        ui.label("Model Unit");
        for u in DistanceUnit::ALL {
            let selected = current_unit == u;
            if ui.selectable_label(selected, u.label()).clicked() && !selected {
                unit_change = Some(u.clone());
                ui.close_menu();
            }
        }
    });

    let mut clicked_pos:      Option<egui::Pos2>  = None;
    let mut ctrl_clicked_pos: Option<egui::Pos2>  = None;
    let mut rect_selection:   Option<egui::Rect>  = None;

    if select_mode {
        // ── SELECT MODE: rubber-band rect + Ctrl+click ────────────────────
        let ctrl = ui.input(|i| i.modifiers.ctrl);
        let drag_start_id = egui::Id::new("vp_rect_sel_start");

        // On the very first frame of a drag, record the anchor position
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                ui.ctx().data_mut(|d| d.insert_temp(drag_start_id, pos));
            }
        }

        // While dragging: draw the rect from the stored anchor to current pos
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(current) = response.interact_pointer_pos() {
                let anchor: Option<egui::Pos2> =
                    ui.ctx().data(|d| d.get_temp(drag_start_id));
                if let Some(start) = anchor {
                    let painter = ui.painter_at(rect);
                    let drag_rect = egui::Rect::from_two_pos(start, current);
                    painter.rect(
                        drag_rect,
                        egui::CornerRadius::ZERO,
                        egui::Color32::from_rgba_unmultiplied(80, 180, 255, 30),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 180, 255)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }

        // On drag end: emit rect or treat as click depending on distance
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            let anchor: Option<egui::Pos2> =
                ui.ctx().data(|d| d.get_temp(drag_start_id));
            ui.ctx().data_mut(|d| d.remove::<egui::Pos2>(drag_start_id));

            if let (Some(start), Some(end)) = (anchor, response.interact_pointer_pos()) {
                if (end - start).length() > 5.0 {
                    rect_selection = Some(egui::Rect::from_two_pos(start, end));
                } else {
                    // Tiny movement = treat as click
                    if ctrl { ctrl_clicked_pos = Some(end); }
                    else     { clicked_pos = Some(end); }
                }
            }
        } else if response.clicked() {
            // True zero-movement tap
            if ctrl { ctrl_clicked_pos = response.interact_pointer_pos(); }
            else     { clicked_pos = response.interact_pointer_pos(); }
        }
    } else if !mmb_orbit {
        // ── ORBIT MODE with LMB ──────────────────────────────────────────
        // Track whether the drag covered meaningful distance — persisted in
        // egui temp data so it survives across frames.
        let orbit_dragged_id = egui::Id::new("vp_orbit_was_dragged");

        if response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().data_mut(|d| d.insert_temp(orbit_dragged_id, false));
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            let d = response.drag_delta();
            if d.length() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(orbit_dragged_id, true));
                state.azimuth   -= d.x * 0.005;
                state.elevation  = (state.elevation + d.y * 0.005)
                                      .clamp(-89f32.to_radians(), 89f32.to_radians());
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            let was_dragged: bool =
                ui.ctx().data(|d| d.get_temp(orbit_dragged_id).unwrap_or(false));
            ui.ctx().data_mut(|d| d.remove::<bool>(orbit_dragged_id));
            if !was_dragged {
                clicked_pos = response.interact_pointer_pos();
            }
        } else if response.clicked() {
            clicked_pos = response.interact_pointer_pos();
        }
    } else {
        // ── MMB orbit mode: LMB click picks nodes ────────────────────────
        if response.clicked() {
            clicked_pos = response.interact_pointer_pos();
        }
    }

    // ── MMB orbit (always active, regardless of select/orbit mode) ────────
    if mmb_orbit {
        if response.dragged_by(egui::PointerButton::Middle) {
            let d = response.drag_delta();
            if d.length() > 0.5 {
                state.azimuth   -= d.x * 0.005;
                state.elevation  = (state.elevation + d.y * 0.005)
                                      .clamp(-89f32.to_radians(), 89f32.to_radians());
            }
        }
    }

    // Pan
    let pan_btn = if mmb_orbit { egui::PointerButton::Secondary } else { egui::PointerButton::Middle };
    if response.dragged_by(pan_btn) {
        let d = response.drag_delta();
        let right = Vec3::new(state.azimuth.cos(), 0.0, -state.azimuth.sin());
        state.target -= right   * d.x * state.distance * 0.001;
        state.target += Vec3::Y * d.y * state.distance * 0.001;
    }

    // Zoom (scroll)
    let scroll = ui.input(|i| i.raw_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        state.distance = (state.distance * (1.0 - scroll * 0.001)).clamp(1e-6, 1e6);
    }

    // Issue wgpu callback
    let aspect = rect.width() / rect.height().max(1.0);
    let vp = state.proj_matrix(aspect) * state.view_matrix();
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        VpCallback {
            vp,
            show_csys:         state.show_csys,
            show_local_csys:   state.show_local_csys,
            show_xy:           state.show_xy,
            show_yz:           state.show_yz,
            show_zx:           state.show_zx,
            nodes:             nodes.to_vec(),
            node_colors:       node_colors.to_vec(),
            node_scales:       node_scales.to_vec(),
            node_csys:         node_csys.to_vec(),
            node_csys_visible: node_csys_visible.to_vec(),
            selected_nodes:    selected_nodes.to_vec(),
            edge_segments:     edge_segments.to_vec(),
            edge_colors:       edge_colors.to_vec(),
            distance:          state.distance,
            node_size,
            local_csys_scale,
            edge_thickness,
            glyphs:            glyph_positions.to_vec(),
            glyph_selected:    glyph_selected.to_vec(),
            mesh_surfaces,
            wireframe_edges:   wireframe_edges.to_vec(),
        },
    ));

    // 2D ruler overlay
    draw_ruler(ui.painter(), rect, state.distance, aspect, ui.visuals().dark_mode, unit_label);

    // ── Triad Overlay ────────────────────────────────────────────────────────
    let triad_center = rect.left_bottom() + egui::vec2(60.0, -60.0);
    let triad_radius = 40.0;
    
    let view_mat = state.view_matrix();
    let mut triad_axes = [
        (Vec3::X, egui::Color32::from_rgb(230, 60, 60), "X"),
        (Vec3::Y, egui::Color32::from_rgb(60, 210, 60), "Y"),
        (Vec3::Z, egui::Color32::from_rgb(60, 110, 240), "Z"),
    ];

    // Sort axes by depth (Z in view space) so axes pointing towards user are drawn last
    triad_axes.sort_by(|a, b| {
        let za = view_mat.transform_vector3(a.0).z;
        let zb = view_mat.transform_vector3(b.0).z;
        za.partial_cmp(&zb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut triad_hit = false;
    let actual_click = clicked_pos.or(ctrl_clicked_pos);
    let hover_pos = response.hover_pos();

    for &(axis, color, label) in &triad_axes {
        let eye_dir = view_mat.transform_vector3(axis);
        let screen_dir = egui::vec2(eye_dir.x, -eye_dir.y);
        
        let tip_pos = triad_center + screen_dir * triad_radius;
        let neg_tip_pos = triad_center - screen_dir * 0.6 * triad_radius;
        
        let mut fwd_stroke = 3.0;
        let mut bwd_radius = 4.0;
        let mut font_size = 13.0;
        
        if let Some(hpos) = hover_pos {
            if hpos.distance(tip_pos) < 15.0 {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                fwd_stroke = 5.0;
                font_size = 15.0;
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(format!("tt_{}", label)), |ui| {
                    ui.label(format!("+{} View", label));
                });
            } else if hpos.distance(neg_tip_pos) < 12.0 {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                bwd_radius = 6.0;
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(format!("tt_neg_{}", label)), |ui| {
                    ui.label(format!("-{} View", label));
                });
            }
        }

        // Draw line
        ui.painter().line_segment(
            [triad_center, tip_pos],
            egui::Stroke::new(fwd_stroke, color),
        );
        
        // Label (only if axis isn't pointing straight into camera)
        if screen_dir.length() > 0.1 {
            let label_pos = triad_center + screen_dir * (triad_radius + 12.0);
            ui.painter().text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(font_size),
                color,
            );
        }
        
        // Negative axis dot
        ui.painter().circle_filled(neg_tip_pos, bwd_radius, color.linear_multiply(0.4));
        
        if let Some(cpos) = actual_click {
            if cpos.distance(tip_pos) < 15.0 {
                triad_hit = true;
                match label {
                    "X" => { state.azimuth = 90f32.to_radians(); state.elevation = 0.0; }
                    "Y" => { state.azimuth = 0.0; state.elevation = 89.9f32.to_radians(); }
                    "Z" => { state.azimuth = 0.0; state.elevation = 0.0; }
                    _ => {}
                }
            } else if cpos.distance(neg_tip_pos) < 12.0 {
                triad_hit = true;
                match label {
                    "X" => { state.azimuth = -90f32.to_radians(); state.elevation = 0.0; }
                    "Y" => { state.azimuth = 0.0; state.elevation = -89.9f32.to_radians(); }
                    "Z" => { state.azimuth = 180f32.to_radians(); state.elevation = 0.0; }
                    _ => {}
                }
            }
        }
    }
    
    // Draw central origin dot
    ui.painter().circle_filled(triad_center, 3.0, egui::Color32::WHITE);

    // Isometric dot
    let iso_world_dir = Vec3::new(1.0, 1.0, 1.0).normalize();
    let iso_eye_dir = view_mat.transform_vector3(iso_world_dir);
    let iso_screen_dir = egui::vec2(iso_eye_dir.x, -iso_eye_dir.y);
    
    // When exactly isometric, iso_screen_dir is ~zero and it sits in the middle.
    let iso_pos = triad_center + iso_screen_dir * (triad_radius * 0.82);
    let iso_color = egui::Color32::from_rgb(80, 180, 255);
    let mut iso_radius = 6.0;
    if let Some(hpos) = hover_pos {
        if hpos.distance(iso_pos) < 15.0 {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            iso_radius = 8.0;
            egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new("tt_iso"), |ui| {
                ui.label("Isometric View");
            });
        }
    }
    ui.painter().circle_filled(iso_pos, iso_radius, iso_color);

    if let Some(cpos) = actual_click {
        if cpos.distance(iso_pos) < 15.0 {
            triad_hit = true;
            state.azimuth = 45f32.to_radians();
            state.elevation = 35.264f32.to_radians();
        }
    }

    if triad_hit {
        clicked_pos = None;
        ctrl_clicked_pos = None;
    }

    ViewportResponse { clicked_pos, ctrl_clicked_pos, rect_selection, vp, rect, unit_change }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scale ruler (2D egui overlay)
// ─────────────────────────────────────────────────────────────────────────────

/// Draw a horizontal scale bar at the bottom-centre of the viewport.
///
/// The bar is a fixed ~175 screen-pixels wide; the label shows the
/// snapped real-world length it represents.
fn draw_ruler(
    painter: &egui::Painter,
    rect: egui::Rect,
    distance: f32,
    aspect: f32,
    dark_mode: bool,
    unit_label: &str,
) {
    // World units visible across the full viewport width.
    // This formula is identical for perspective and the matched orthographic mode.
    let fov_half = 45_f32.to_radians() / 2.0;
    let world_width = 2.0 * distance * fov_half.tan() * aspect;
    let world_per_px = world_width / rect.width();

    // Target ruler width in screen pixels, then snap world value to 1-2-5.
    let target_px    = 175.0_f32;
    let raw_world    = target_px * world_per_px;
    let world_nice   = nice_scale(raw_world);
    let ruler_px     = world_nice / world_per_px;

    // Layout: centred horizontally, 28 px from the bottom edge.
    let cx     = rect.center().x;
    let y_bar  = rect.bottom() - 28.0;
    let y_tick = y_bar - 7.0;                   // tick height above the bar

    let x0 = cx - ruler_px * 0.5;
    let x1 = cx + ruler_px * 0.5;

    // Colours — adapt for dark/light mode
    let (bar_color, bg_color, text_color) = if dark_mode {
        (
            egui::Color32::from_rgba_premultiplied(220, 220, 220, 200),
            egui::Color32::from_rgba_premultiplied(20,  20,  30,  160),
            egui::Color32::from_rgb(230, 230, 230),
        )
    } else {
        (
            egui::Color32::from_rgba_premultiplied(40, 40, 50, 220),
            egui::Color32::from_rgba_premultiplied(240, 240, 245, 180),
            egui::Color32::from_rgb(30, 30, 40),
        )
    };

    // Background pill so the ruler is readable over any scene.
    let bg_rect = egui::Rect::from_min_max(
        egui::pos2(x0 - 10.0, y_tick - 4.0),
        egui::pos2(x1 + 10.0, y_bar  + 6.0),
    );
    painter.rect_filled(bg_rect, egui::CornerRadius::same(4), bg_color);

    let stroke = egui::Stroke::new(1.5, bar_color);

    // Main horizontal bar
    painter.line_segment([egui::pos2(x0, y_bar), egui::pos2(x1, y_bar)], stroke);
    // Left tick
    painter.line_segment([egui::pos2(x0, y_bar), egui::pos2(x0, y_tick)], stroke);
    // Right tick
    painter.line_segment([egui::pos2(x1, y_bar), egui::pos2(x1, y_tick)], stroke);
    // Centre tick (half way)
    let xm = (x0 + x1) * 0.5;
    painter.line_segment(
        [egui::pos2(xm, y_bar), egui::pos2(xm, y_bar - 4.0)],
        egui::Stroke::new(1.0, bar_color),
    );

    // Label: choose unit automatically.
    let label = format!("{} {}", format_ruler_value(world_nice), unit_label);
    painter.text(
        egui::pos2(cx, y_tick - 6.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(11.0),
        text_color,
    );
}

/// Snap a raw world length to the nearest nice 1-2-5 value.
fn nice_scale(raw: f32) -> f32 {
    if raw <= 0.0 { return 1.0; }
    let exp   = raw.log10().floor();
    let base  = 10_f32.powi(exp as i32);
    let frac  = raw / base;
    let nice  = if frac < 1.5 { 1.0 }
                else if frac < 3.5 { 2.0 }
                else if frac < 7.5 { 5.0 }
                else { 10.0 };
    nice * base
}

/// Format a world-space ruler value with sensible precision.
fn format_ruler_value(v: f32) -> String {
    if v == 0.0 { return "0".to_string(); }
    let abs = v.abs();
    if abs >= 1_000_000.0 {
        format!("{:.3e}", v)
    } else if abs >= 100.0 {
        format!("{:.0}", v)
    } else if abs >= 10.0 {
        format!("{:.1}", v)
    } else if abs >= 1.0 {
        format!("{:.2}", v)
    } else if abs >= 0.01 {
        format!("{:.3}", v)
    } else {
        format!("{:.3e}", v)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Geometry helpers
// ─────────────────────────────────────────────────────────────────────────────
/// Build an adaptive infinite grid in the XZ plane (Y=0).
///
/// * `view_scale`  – `distance × tan(fov/2)`, the "world half-height" of the viewport.
/// * `color`       – base RGBA colour for the grid lines.
///
/// The grid snaps to 1-2-5 intervals based on zoom, draws major and minor
/// lines, and fades alpha with distance from the origin.
fn make_infinite_grid(view_scale: f32, color: [f32; 4]) -> Vec<Vtx> {
    if view_scale <= 0.0 { return Vec::new(); }

    // ── Spacing via 1-2-5 snapping ──────────────────────────────────────
    let raw_major = view_scale / 2.5;
    let major = grid_nice_scale(raw_major);
    let minor = major / 5.0;

    // ── Grid extent ─────────────────────────────────────────────────────
    let extent = view_scale * 3.0;
    let fade_start = extent * 0.35;
    let fade_end   = extent;

    // Number of sub-segments per line for smooth fading.
    // Each line from -extent..+extent is split into SEG_COUNT pieces so
    // the GPU gets intermediate vertices with correct alpha.
    const SEG_COUNT: i32 = 20;

    let mut verts: Vec<Vtx> = Vec::new();

    let emit_lines = |verts: &mut Vec<Vtx>, spacing: f32, alpha_mul: f32, bright_mul: f32, skip_every: i32| {
        if spacing < 1e-12 { return; }

        let n = (extent / spacing).ceil() as i32;

        for i in -n..=n {
            // Skip positions that coincide with a coarser grid level
            if skip_every > 0 && i % skip_every == 0 { continue; }

            let coord = i as f32 * spacing;
            // Skip lines whose perpendicular distance alone exceeds fade_end
            if coord.abs() > fade_end { continue; }

            // Each grid line runs from -extent to +extent along the
            // other axis.  We split it into SEG_COUNT pieces so that
            // the per-vertex alpha fading is smooth.
            for axis in 0..2_u8 {
                // axis 0: line parallel to X  (constant Z = coord)
                // axis 1: line parallel to Z  (constant X = coord)
                let mut prev_pos: Option<(f32, f32, f32)> = None;
                for si in 0..=SEG_COUNT {
                    let t = -extent + 2.0 * extent * (si as f32 / SEG_COUNT as f32);
                    let (px, pz) = if axis == 0 { (t, coord) } else { (coord, t) };
                    let dist = px.abs().max(pz.abs());
                    let alpha = alpha_mul * fade_alpha(dist, fade_start, fade_end);

                    if let Some((prev_x, prev_z, prev_a)) = prev_pos {
                        // Skip fully transparent segments
                        if !(alpha < 0.005 && prev_a < 0.005) {
                            verts.push(Vtx {
                                pos: [prev_x, 0.0, prev_z],
                                color: [color[0] * bright_mul, color[1] * bright_mul, color[2] * bright_mul, color[3] * prev_a],
                            });
                            verts.push(Vtx {
                                pos: [px, 0.0, pz],
                                color: [color[0] * bright_mul, color[1] * bright_mul, color[2] * bright_mul, color[3] * alpha],
                            });
                        }
                    }
                    prev_pos = Some((px, pz, alpha));
                }
            }
        }
    };

    // Minor lines first (skip positions where major lines go), then major on top.
    emit_lines(&mut verts, minor, 0.20, 1.0, 5);
    emit_lines(&mut verts, major, 0.90, 0.60, 0);

    verts
}

/// Compute a fade factor: 1.0 close to origin, 0.0 at `end`.
fn fade_alpha(dist: f32, start: f32, end: f32) -> f32 {
    if dist <= start { return 1.0; }
    if dist >= end   { return 0.0; }
    1.0 - (dist - start) / (end - start)
}

/// Snap a raw spacing to the nearest 1-2-5 value.
fn grid_nice_scale(raw: f32) -> f32 {
    if raw <= 0.0 { return 1.0; }
    let exp  = raw.log10().floor();
    let base = 10_f32.powi(exp as i32);
    let frac = raw / base;
    let nice = if frac < 1.5 { 1.0 }
               else if frac < 3.5 { 2.0 }
               else if frac < 7.5 { 5.0 }
               else { 10.0 };
    nice * base
}

fn make_axes(len: f32) -> (Vec<Vtx>, u32) {
    let d = 0.55; // dimmed magnitude for negative arms
    let verts = vec![
        // +X red,  -X dim red
        vtx(0.0, 0.0, 0.0,  1.0, 0.2, 0.2, 1.0), vtx( len, 0.0, 0.0,  1.0, 0.2, 0.2, 1.0),
        vtx(0.0, 0.0, 0.0,  d*0.6, 0.1, 0.1, 0.5), vtx(-len*0.5, 0.0, 0.0, d*0.6, 0.1, 0.1, 0.5),
        // +Y green, -Y dim green
        vtx(0.0, 0.0, 0.0,  0.2, 1.0, 0.2, 1.0), vtx(0.0,  len, 0.0,  0.2, 1.0, 0.2, 1.0),
        vtx(0.0, 0.0, 0.0,  0.1, d*0.6, 0.1, 0.5), vtx(0.0, -len*0.5, 0.0, 0.1, d*0.6, 0.1, 0.5),
        // +Z blue,  -Z dim blue
        vtx(0.0, 0.0, 0.0,  0.2, 0.5, 1.0, 1.0), vtx(0.0, 0.0,  len,  0.2, 0.5, 1.0, 1.0),
        vtx(0.0, 0.0, 0.0,  0.1, 0.2, d*0.6, 0.5), vtx(0.0, 0.0, -len*0.5, 0.1, 0.2, d*0.6, 0.5),
    ];
    let n = verts.len() as u32;
    (verts, n)
}



/// Subdivided icosahedron.
fn icosphere(subs: u32) -> (Vec<Vtx>, Vec<u32>) {
    let t = (1.0 + 5.0_f32.sqrt()) / 2.0;
    let mut pos: Vec<[f32; 3]> = vec![
        [-1.0, t, 0.0],[1.0, t, 0.0],[-1.0,-t, 0.0],[1.0,-t, 0.0],
        [0.0,-1.0, t],[0.0, 1.0, t],[0.0,-1.0,-t],[0.0, 1.0,-t],
        [t, 0.0,-1.0],[t, 0.0, 1.0],[-t, 0.0,-1.0],[-t, 0.0, 1.0],
    ];
    let mut idx: Vec<u32> = vec![
        0,11,5, 0,5,1, 0,1,7, 0,7,10, 0,10,11,
        1,5,9, 5,11,4, 11,10,2, 10,7,6, 7,1,8,
        3,9,4, 3,4,2, 3,2,6, 3,6,8, 3,8,9,
        4,9,5, 2,4,11, 6,2,10, 8,6,7, 9,8,1,
    ];

    let norm = |p: [f32;3]| { let l=(p[0]*p[0]+p[1]*p[1]+p[2]*p[2]).sqrt(); [p[0]/l,p[1]/l,p[2]/l] };

    for _ in 0..subs {
        let mut new_idx = Vec::new();
        let mut cache: std::collections::HashMap<(u32,u32),u32> = Default::default();
        let mut mid = |a: u32, b: u32, pos: &mut Vec<[f32;3]>| -> u32 {
            let key = if a<b{(a,b)}else{(b,a)};
            if let Some(&v) = cache.get(&key) { return v; }
            let pa=pos[a as usize]; let pb=pos[b as usize];
            let m = norm([(pa[0]+pb[0])/2.0,(pa[1]+pb[1])/2.0,(pa[2]+pb[2])/2.0]);
            let i = pos.len() as u32; pos.push(m); cache.insert(key,i); i
        };
        for tri in idx.chunks(3) {
            let (a,b,c)=(tri[0],tri[1],tri[2]);
            let ab=mid(a,b,&mut pos); let bc=mid(b,c,&mut pos); let ca=mid(c,a,&mut pos);
            new_idx.extend_from_slice(&[a,ab,ca, b,bc,ab, c,ca,bc, ab,bc,ca]);
        }
        idx = new_idx;
    }

    let verts: Vec<Vtx> = pos.iter().map(|&p| {
        let n=norm(p);
        Vtx { pos: n, color: [1.0, 1.0, 1.0, 1.0] }
    }).collect();
    (verts, idx)
}

/// Unit cube centred at origin (half-extent = 1).
fn make_cube(color: [f32; 4]) -> (Vec<Vtx>, Vec<u32>) {
    let v = |x: f32, y: f32, z: f32| Vtx { pos: [x, y, z], color };
    let verts = vec![
        // front (+Z)
        v(-1.0, -1.0,  1.0), v( 1.0, -1.0,  1.0), v( 1.0,  1.0,  1.0), v(-1.0,  1.0,  1.0),
        // back (-Z)
        v( 1.0, -1.0, -1.0), v(-1.0, -1.0, -1.0), v(-1.0,  1.0, -1.0), v( 1.0,  1.0, -1.0),
        // top (+Y)
        v(-1.0,  1.0,  1.0), v( 1.0,  1.0,  1.0), v( 1.0,  1.0, -1.0), v(-1.0,  1.0, -1.0),
        // bottom (-Y)
        v(-1.0, -1.0, -1.0), v( 1.0, -1.0, -1.0), v( 1.0, -1.0,  1.0), v(-1.0, -1.0,  1.0),
        // right (+X)
        v( 1.0, -1.0,  1.0), v( 1.0, -1.0, -1.0), v( 1.0,  1.0, -1.0), v( 1.0,  1.0,  1.0),
        // left (-X)
        v(-1.0, -1.0, -1.0), v(-1.0, -1.0,  1.0), v(-1.0,  1.0,  1.0), v(-1.0,  1.0, -1.0),
    ];
    let mut indices = Vec::new();
    for face in 0..6u32 {
        let b = face * 4;
        indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
    }
    (verts, indices)
}

/// Cylinder along Y axis, radius=1, height=2 (centred at origin).
fn make_cylinder(segs: u32, color: [f32; 4]) -> (Vec<Vtx>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    // Top and bottom cap centres
    let top_c = verts.len() as u32;
    verts.push(Vtx { pos: [0.0,  1.0, 0.0], color });
    let bot_c = verts.len() as u32;
    verts.push(Vtx { pos: [0.0, -1.0, 0.0], color });

    let ring_base = verts.len() as u32;
    for i in 0..segs {
        let angle = std::f32::consts::TAU * i as f32 / segs as f32;
        let (s, c_val) = angle.sin_cos();
        verts.push(Vtx { pos: [c_val,  1.0, s], color }); // top ring
        verts.push(Vtx { pos: [c_val, -1.0, s], color }); // bottom ring
    }
    for i in 0..segs {
        let next = (i + 1) % segs;
        let ti = ring_base + i * 2;
        let bi = ring_base + i * 2 + 1;
        let tn = ring_base + next * 2;
        let bn = ring_base + next * 2 + 1;
        // Side quad
        indices.extend_from_slice(&[ti, bi, bn,  ti, bn, tn]);
        // Top cap
        indices.extend_from_slice(&[top_c, ti, tn]);
        // Bottom cap
        indices.extend_from_slice(&[bot_c, bn, bi]);
    }
    (verts, indices)
}

/// Torus in the XZ plane, centred at origin.
/// `major_segs` = divisions around the ring, `minor_segs` = divisions of the tube cross-section.
/// `tube_ratio` = tube_radius / major_radius (the overall radius is 1.0).
fn make_torus(major_segs: u32, minor_segs: u32, tube_ratio: f32, color: [f32; 4]) -> (Vec<Vtx>, Vec<u32>) {
    let major_r = 1.0;
    let minor_r = major_r * tube_ratio;
    let mut verts = Vec::new();
    let mut indices = Vec::new();

    for i in 0..major_segs {
        let theta = std::f32::consts::TAU * i as f32 / major_segs as f32;
        let (st, ct) = theta.sin_cos();
        for j in 0..minor_segs {
            let phi = std::f32::consts::TAU * j as f32 / minor_segs as f32;
            let (sp, cp) = phi.sin_cos();
            let x = (major_r + minor_r * cp) * ct;
            let y = minor_r * sp;
            let z = (major_r + minor_r * cp) * st;
            verts.push(Vtx { pos: [x, y, z], color });
        }
    }

    for i in 0..major_segs {
        let next_i = (i + 1) % major_segs;
        for j in 0..minor_segs {
            let next_j = (j + 1) % minor_segs;
            let a = i * minor_segs + j;
            let b = next_i * minor_segs + j;
            let c = next_i * minor_segs + next_j;
            let d = i * minor_segs + next_j;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    (verts, indices)
}
