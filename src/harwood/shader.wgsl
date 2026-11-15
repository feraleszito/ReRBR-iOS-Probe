// One-block diagnostic: original baked colour/two-diffuse blend, no full lighting.
struct Camera {
    view_projection: mat4x4<f32>,
    options: vec4<u32>, // visual mode, sRGB output surface, reserved, reserved
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var first_texture: texture_2d<f32>;
@group(1) @binding(1) var second_texture: texture_2d<f32>;
@group(1) @binding(2) var texture_sampler: sampler;

struct Input {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv0: vec2<f32>,
    @location(3) uv1: vec2<f32>,
};
struct Output {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv0: vec2<f32>,
    @location(2) uv1: vec2<f32>,
};
@vertex
fn vs_main(v: Input) -> Output {
    var out: Output;
    out.position = camera.view_projection * vec4<f32>(v.position, 1.0);
    out.color = v.color;
    out.uv0 = v.uv0;
    out.uv1 = v.uv1;
    return out;
}

@fragment
fn fs_main(v: Output) -> @location(0) vec4<f32> {
    let t0 = textureSample(first_texture, texture_sampler, v.uv0);
    let t1 = textureSample(second_texture, texture_sampler, v.uv1);
    var color = mix(v.color.rgb * t0.rgb, v.color.rgb * t1.rgb, t1.a * v.color.a);
    if camera.options.x == 1u { color = v.color.rgb; }
    if camera.options.x == 2u {
        let checker = f32((u32(abs(floor(v.uv0.x * 8.0))) + u32(abs(floor(v.uv0.y * 8.0)))) % 2u);
        color = mix(vec3<f32>(0.08, 0.14, 0.2), vec3<f32>(0.8, 0.8, 0.8), checker);
    }
    // ReRBR's legacy inputs are unorm, not sRGB decoded. Cancel an sRGB surface's
    // automatic encoding so it displays the same baked RGB values as an unorm one.
    if camera.options.y != 0u {
        color = select(pow((color + 0.055) / 1.055, vec3<f32>(2.4)), color / 12.92, color <= vec3<f32>(0.04045));
    }
    return vec4<f32>(color, 1.0);
}
