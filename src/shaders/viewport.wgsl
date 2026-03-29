// Viewport WGSL shader
// Inputs: position (location 0), vertex color (location 1)
// Uniform: MVP matrix + color tint

struct Uniforms {
    mvp:        mat4x4<f32>,
    color:      vec4<f32>,
    light_dir:  vec4<f32>,      // eye-space light direction (xyz), w = brightness multiplier
    normal_mat: mat3x3<f32>,    // inverse-transpose of model-view (for normals)
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

// ─── Unlit pipeline (lines, axes, grids) ────────────────────────────────────

struct VsIn {
    @location(0) pos:   vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0)       color:    vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = u.mvp * vec4<f32>(in.pos, 1.0);
    // Vertex color is modulated by the uniform tint color.
    out.color = in.color * u.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}

// ─── Lit pipeline (triangles: nodes, edges, glyphs, meshes) ─────────────────

struct LitVsIn {
    @location(0) pos:    vec3<f32>,
    @location(1) color:  vec4<f32>,
    @location(2) normal: vec3<f32>,
};

struct LitVsOut {
    @builtin(position) clip_pos:  vec4<f32>,
    @location(0)       color:     vec4<f32>,
    @location(1)       normal_eye: vec3<f32>,
};

@vertex
fn vs_lit(in: LitVsIn) -> LitVsOut {
    var out: LitVsOut;
    out.clip_pos   = u.mvp * vec4<f32>(in.pos, 1.0);
    out.color      = in.color * u.color;
    out.normal_eye = normalize(u.normal_mat * in.normal);
    return out;
}

@fragment
fn fs_lit(in: LitVsOut) -> @location(0) vec4<f32> {
    let brightness = u.light_dir.w;
    if (brightness <= 0.0) {
        return in.color;
    }

    let N = normalize(in.normal_eye);
    let L = normalize(u.light_dir.xyz);

    // Half-vector for specular (view direction is -Z in eye space)
    let V = vec3<f32>(0.0, 0.0, 1.0);
    let H = normalize(L + V);

    let ambient  = 0.35;
    let diffuse  = 0.55 * max(dot(N, L), 0.0) * brightness;
    let spec_raw = pow(max(dot(N, H), 0.0), 32.0);
    let specular = 0.15 * spec_raw * brightness;

    let lit = vec3<f32>(
        in.color.r * (ambient + diffuse) + specular,
        in.color.g * (ambient + diffuse) + specular,
        in.color.b * (ambient + diffuse) + specular,
    );
    return vec4<f32>(clamp(lit, vec3<f32>(0.0), vec3<f32>(1.0)), in.color.a);
}
