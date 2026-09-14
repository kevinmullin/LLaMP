struct Uniforms {
    onset: f32,
    rms: f32,
    time: f32,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var corners = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    var out: VsOut;
    out.pos = vec4<f32>(corners[i], 0.0, 1.0);
    out.uv = corners[i] * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let d = length(in.uv - vec2<f32>(0.5, 0.5));
    let ring = smoothstep(0.35 + u.onset * 0.2, 0.15, d);
    let rest = 0.08 + u.rms * 0.15;
    let live = rest + ring * (0.4 + u.onset * 0.6);
    return vec4<f32>(live, rest, 0.12 + live * 0.4, 1.0);
}
