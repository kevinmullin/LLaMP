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
    let wave = sin(in.uv.x * 12.0 + u.time + u.onset * 8.0);
    let glow = 0.15 + u.onset * 0.7 + u.rms * 0.2;
    return vec4<f32>(0.2 + glow * 0.4, 0.05 + wave * 0.05, 0.4 + glow * 0.3, 1.0);
}
