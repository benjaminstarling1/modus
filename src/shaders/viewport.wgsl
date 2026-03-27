// Viewport WGSL shader
// Inputs: position (location 0), vertex color (location 1)
// Uniform: MVP matrix + color tint

struct Uniforms {
    mvp:   mat4x4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

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
