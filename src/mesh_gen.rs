// ─────────────────────────────────────────────────────────────────────────────
// Geometry generation helpers
// ─────────────────────────────────────────────────────────────────────────────
//
// All functions here produce vertex/index data for the wgpu renderer.
// Types `LitVtx` and `Vtx` are defined in renderer.rs and re-used here.

use crate::renderer::{Vtx, LitVtx, vtx};

pub fn plane_model(rot: u8) -> glam::Mat4 {
    match rot {
        0 => glam::Mat4::from_rotation_x(-std::f32::consts::FRAC_PI_2), // XY
        1 => glam::Mat4::from_rotation_z( std::f32::consts::FRAC_PI_2), // YZ
        _ => glam::Mat4::IDENTITY,                                        // ZX (already in XZ)
    }
}

/// Build an adaptive infinite grid in the XZ plane (Y=0).
///
/// * `view_scale`  – `distance × tan(fov/2)`, the "world half-height" of the viewport.
/// * `color`       – base RGBA colour for the grid lines.
///
/// The grid snaps to 1-2-5 intervals based on zoom, draws major and minor
/// lines, and fades alpha with distance from the origin.
pub fn make_infinite_grid(view_scale: f32, color: [f32; 4]) -> Vec<Vtx> {
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

pub fn make_axes(len: f32) -> (Vec<Vtx>, u32) {
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

/// Subdivided icosahedron with normals (normal = normalized position for a unit sphere).
pub fn icosphere(subs: u32) -> (Vec<LitVtx>, Vec<u32>) {
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

    let verts: Vec<LitVtx> = pos.iter().map(|&p| {
        let n=norm(p);
        LitVtx { pos: n, color: [1.0, 1.0, 1.0, 1.0], normal: n }
    }).collect();
    (verts, idx)
}

/// Unit cube centred at origin (half-extent = 1) with per-face normals.
pub fn make_cube(color: [f32; 4]) -> (Vec<LitVtx>, Vec<u32>) {
    let v = |x: f32, y: f32, z: f32, nx: f32, ny: f32, nz: f32| LitVtx { pos: [x, y, z], color, normal: [nx, ny, nz] };
    let verts = vec![
        // front (+Z)
        v(-1.0, -1.0,  1.0, 0.0, 0.0, 1.0), v( 1.0, -1.0,  1.0, 0.0, 0.0, 1.0), v( 1.0,  1.0,  1.0, 0.0, 0.0, 1.0), v(-1.0,  1.0,  1.0, 0.0, 0.0, 1.0),
        // back (-Z)
        v( 1.0, -1.0, -1.0, 0.0, 0.0,-1.0), v(-1.0, -1.0, -1.0, 0.0, 0.0,-1.0), v(-1.0,  1.0, -1.0, 0.0, 0.0,-1.0), v( 1.0,  1.0, -1.0, 0.0, 0.0,-1.0),
        // top (+Y)
        v(-1.0,  1.0,  1.0, 0.0, 1.0, 0.0), v( 1.0,  1.0,  1.0, 0.0, 1.0, 0.0), v( 1.0,  1.0, -1.0, 0.0, 1.0, 0.0), v(-1.0,  1.0, -1.0, 0.0, 1.0, 0.0),
        // bottom (-Y)
        v(-1.0, -1.0, -1.0, 0.0,-1.0, 0.0), v( 1.0, -1.0, -1.0, 0.0,-1.0, 0.0), v( 1.0, -1.0,  1.0, 0.0,-1.0, 0.0), v(-1.0, -1.0,  1.0, 0.0,-1.0, 0.0),
        // right (+X)
        v( 1.0, -1.0,  1.0, 1.0, 0.0, 0.0), v( 1.0, -1.0, -1.0, 1.0, 0.0, 0.0), v( 1.0,  1.0, -1.0, 1.0, 0.0, 0.0), v( 1.0,  1.0,  1.0, 1.0, 0.0, 0.0),
        // left (-X)
        v(-1.0, -1.0, -1.0,-1.0, 0.0, 0.0), v(-1.0, -1.0,  1.0,-1.0, 0.0, 0.0), v(-1.0,  1.0,  1.0,-1.0, 0.0, 0.0), v(-1.0,  1.0, -1.0,-1.0, 0.0, 0.0),
    ];
    let mut indices = Vec::new();
    for face in 0..6u32 {
        let b = face * 4;
        indices.extend_from_slice(&[b, b+1, b+2, b, b+2, b+3]);
    }
    (verts, indices)
}

/// Cylinder along Y axis, radius=1, height=2 (centred at origin), with normals.
pub fn make_cylinder(segs: u32, color: [f32; 4]) -> (Vec<LitVtx>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    // Top and bottom cap centres
    let top_c = verts.len() as u32;
    verts.push(LitVtx { pos: [0.0,  1.0, 0.0], color, normal: [0.0, 1.0, 0.0] });
    let bot_c = verts.len() as u32;
    verts.push(LitVtx { pos: [0.0, -1.0, 0.0], color, normal: [0.0,-1.0, 0.0] });

    // Side ring vertices (with radial normals) + cap ring vertices (with Y normals)
    let side_base = verts.len() as u32;
    for i in 0..segs {
        let angle = std::f32::consts::TAU * i as f32 / segs as f32;
        let (s, c_val) = angle.sin_cos();
        // Side top and bottom
        verts.push(LitVtx { pos: [c_val,  1.0, s], color, normal: [c_val, 0.0, s] });
        verts.push(LitVtx { pos: [c_val, -1.0, s], color, normal: [c_val, 0.0, s] });
    }
    // Cap vertices (separate so they have Y-facing normals)
    let cap_base = verts.len() as u32;
    for i in 0..segs {
        let angle = std::f32::consts::TAU * i as f32 / segs as f32;
        let (s, c_val) = angle.sin_cos();
        verts.push(LitVtx { pos: [c_val,  1.0, s], color, normal: [0.0, 1.0, 0.0] }); // top cap
        verts.push(LitVtx { pos: [c_val, -1.0, s], color, normal: [0.0,-1.0, 0.0] }); // bottom cap
    }
    for i in 0..segs {
        let next = (i + 1) % segs;
        let ti = side_base + i * 2;
        let bi = side_base + i * 2 + 1;
        let tn = side_base + next * 2;
        let bn = side_base + next * 2 + 1;
        // Side quad
        indices.extend_from_slice(&[ti, bi, bn,  ti, bn, tn]);
        // Top cap (using cap ring vertices)
        let cti = cap_base + i * 2;
        let ctn = cap_base + next * 2;
        indices.extend_from_slice(&[top_c, cti, ctn]);
        // Bottom cap
        let cbi = cap_base + i * 2 + 1;
        let cbn = cap_base + next * 2 + 1;
        indices.extend_from_slice(&[bot_c, cbn, cbi]);
    }
    (verts, indices)
}

/// Torus in the XZ plane, centred at origin, with normals.
/// `major_segs` = divisions around the ring, `minor_segs` = divisions of the tube cross-section.
/// `tube_ratio` = tube_radius / major_radius (the overall radius is 1.0).
pub fn make_torus(major_segs: u32, minor_segs: u32, tube_ratio: f32, color: [f32; 4]) -> (Vec<LitVtx>, Vec<u32>) {
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
            // Normal points outward from the tube centre at (major_r*ct, 0, major_r*st)
            let nx = cp * ct;
            let ny = sp;
            let nz = cp * st;
            verts.push(LitVtx { pos: [x, y, z], color, normal: [nx, ny, nz] });
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

/// Cone with tip at (0, 1, 0), base centre at origin, base radius = 1.
/// Colour is set per-vertex at call time; the caller overwrites it before uploading.
/// Normals are computed for a unit 45° half-angle cone (H = R = 1).
pub fn make_cone(segs: u32) -> (Vec<LitVtx>, Vec<u32>) {
    use std::f32::consts::TAU;
    // For H=R=1 the outward side normal at azimuth φ is (cos(φ)/√2, 1/√2, sin(φ)/√2).
    let slope: f32 = std::f32::consts::FRAC_1_SQRT_2;
    let placeholder = [1.0_f32; 4]; // colour filled in by caller
    let mut verts: Vec<LitVtx> = Vec::new();
    let mut indices: Vec<u32>  = Vec::new();

    let tip = verts.len() as u32;
    verts.push(LitVtx { pos: [0.0, 1.0, 0.0], color: placeholder, normal: [0.0, 1.0, 0.0] });
    let base_c = verts.len() as u32;
    verts.push(LitVtx { pos: [0.0, 0.0, 0.0], color: placeholder, normal: [0.0, -1.0, 0.0] });

    let side_base = verts.len() as u32;
    for i in 0..segs {
        let a = TAU * i as f32 / segs as f32;
        let (s, c) = a.sin_cos();
        verts.push(LitVtx { pos: [c, 0.0, s], color: placeholder, normal: [c * slope, slope, s * slope] });
    }
    let cap_base = verts.len() as u32;
    for i in 0..segs {
        let a = TAU * i as f32 / segs as f32;
        let (s, c) = a.sin_cos();
        verts.push(LitVtx { pos: [c, 0.0, s], color: placeholder, normal: [0.0, -1.0, 0.0] });
    }

    for i in 0..segs {
        let next = (i + 1) % segs;
        indices.extend_from_slice(&[tip, side_base + i, side_base + next]);
        indices.extend_from_slice(&[base_c, cap_base + next, cap_base + i]);
    }
    (verts, indices)
}
