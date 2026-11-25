// One-block diagnostic: original baked colour/two-diffuse blend, no full lighting.
struct Camera {
    view_projection: mat4x4<f32>,
    options: vec4<u32>, // visual mode, sRGB output surface, reserved, reserved
};
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var first_texture: texture_2d<f32>;
@group(1) @binding(1) var second_texture: texture_2d<f32>;
@group(1) @binding(2) var texture_sampler: sampler;
@group(1) @binding(3) var<uniform> linear_samples: u32;

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
    var linear = linear_samples != 0u;
    var base = t0.rgb;
    var detail = t1.rgb;
    var baked = v.color.rgb;
    if linear {
        if (linear_samples & 1u) == 0u { base = decode_srgb(base); }
        if (linear_samples & 2u) == 0u { detail = decode_srgb(detail); }
        baked = decode_srgb(baked);
    }
    var color = mix(baked * base, baked * detail, t1.a * v.color.a);
    if camera.options.x == 1u { color = v.color.rgb; linear = false; }
    if camera.options.x == 2u {
        let checker = f32((u32(abs(floor(v.uv0.x * 8.0))) + u32(abs(floor(v.uv0.y * 8.0)))) % 2u);
        color = mix(vec3<f32>(0.08, 0.14, 0.2), vec3<f32>(0.8, 0.8, 0.8), checker);
        linear = false;
    }
    return vec4<f32>(output_rgb(color, linear), 1.0);
}

fn decode_srgb(rgb: vec3<f32>) -> vec3<f32> {
    return select(pow((rgb + 0.055) / 1.055, vec3<f32>(2.4)), rgb / 12.92, rgb <= vec3<f32>(0.04045));
}

fn output_rgb(rgb: vec3<f32>, linear: bool) -> vec3<f32> {
    let srgb_target = camera.options.y != 0u;
    if !linear && srgb_target { return decode_srgb(rgb); }
    if linear && !srgb_target {
        return select(1.055 * pow(rgb, vec3<f32>(1.0 / 2.4)) - 0.055, rgb * 12.92, rgb <= vec3<f32>(0.0031308));
    }
    return rgb;
}

// Original reRBR stage object state: two-sided, alpha > 60/255, depth writes.
// These vertices already contain their world placement and baked lighting.
@fragment
fn fs_object(v: Output) -> @location(0) vec4<f32> {
    let texel = textureSample(first_texture, texture_sampler, v.uv0);
    if texel.a <= 60.0 / 255.0 { discard; }
    var linear = (linear_samples & 1u) != 0u;
    var baked = v.color.rgb;
    if linear { baked = decode_srgb(baked); }
    var color = texel.rgb * baked;
    if camera.options.x == 1u { color = v.color.rgb; linear = false; }
    if camera.options.x == 2u {
        let checker = f32((u32(abs(floor(v.uv0.x * 8.0))) + u32(abs(floor(v.uv0.y * 8.0)))) % 2u);
        color = mix(vec3<f32>(0.08, 0.14, 0.2), vec3<f32>(0.8), checker);
        linear = false;
    }
    return vec4<f32>(output_rgb(color, linear), 1.0);
}
