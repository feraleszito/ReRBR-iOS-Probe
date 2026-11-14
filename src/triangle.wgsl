// One uniform, no textures, assets, vertex buffers or optional GPU features.
struct Animation {
    rotation: vec2<f32>,
    aspect: f32,
    pulse: f32,
};

@group(0) @binding(0) var<uniform> animation: Animation;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) colour: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let positions = array<vec2<f32>, 3>(
        vec2<f32>(0.0, 0.55),
        vec2<f32>(-0.48, -0.28),
        vec2<f32>(0.48, -0.28),
    );
    let colours = array<vec3<f32>, 3>(
        vec3<f32>(1.0, 0.35, 0.20),
        vec3<f32>(0.20, 0.90, 0.65),
        vec3<f32>(0.25, 0.50, 1.0),
    );
    let point = positions[index];
    let c = animation.rotation.x;
    let s = animation.rotation.y;
    let rotated = vec2<f32>(c * point.x - s * point.y, s * point.x + c * point.y);
    // Fit in both portrait and landscape without stretching the triangle.
    let fit = min(animation.aspect, 1.0);
    var out: VertexOutput;
    out.position = vec4<f32>(rotated.x * fit / animation.aspect, rotated.y * fit, 0.0, 1.0);
    out.colour = colours[index] * animation.pulse;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.colour, 1.0);
}
