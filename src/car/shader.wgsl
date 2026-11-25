struct Camera { vp: mat4x4<f32>, models: array<mat4x4<f32>,6>, options: vec4<u32> }
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var diffuse: texture_2d<f32>;
@group(1) @binding(1) var filtering: sampler;
@group(1) @binding(2) var<uniform> linear_sample: u32;
struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) world_pos: vec3<f32>,
}
@vertex fn vs_main(@location(0) p:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@builtin(instance_index) slot:u32)->Out {
    let m = camera.models[min(slot,5u)];
    let wp = m * vec4<f32>(p, 1.0);
    var out:Out;
    out.position = camera.vp * wp;
    out.uv = uv;
    out.world_normal = normalize((m * vec4<f32>(normal, 0.0)).xyz);
    out.world_pos = wp.xyz;
    return out;
}
fn apply_studio_lighting(rgb: vec3<f32>, n_in: vec3<f32>, wp: vec3<f32>) -> vec3<f32> {
    let n = normalize(n_in);
    let key_dir = normalize(vec3<f32>(0.55, 0.78, 0.35));
    let fill_dir = normalize(vec3<f32>(-0.65, 0.35, -0.55));
    let rim_dir = normalize(vec3<f32>(-0.20, 0.55, 0.80));
    let hemi = mix(vec3<f32>(0.24, 0.26, 0.30), vec3<f32>(0.62, 0.66, 0.72), n.y * 0.5 + 0.5);
    let key = max(dot(n, key_dir), 0.0) * vec3<f32>(0.85, 0.82, 0.78);
    let fill = max(dot(n, fill_dir), 0.0) * vec3<f32>(0.28, 0.33, 0.42);
    let view_dir = normalize(vec3<f32>(0.0, 1.5, 5.0) - wp);
    let half_vec = normalize(key_dir + view_dir);
    let spec = pow(max(dot(n, half_vec), 0.0), 36.0) * 0.28;
    let rim = pow(1.0 - max(dot(n, view_dir), 0.0), 3.0) * max(dot(n, rim_dir) * 0.5 + 0.5, 0.0) * vec3<f32>(0.22, 0.28, 0.38);
    return clamp(rgb * (hemi + key + fill) + vec3<f32>(spec) + rim, vec3<f32>(0.0), vec3<f32>(1.0));
}
fn sampled(uv:vec2<f32>, world_normal: vec3<f32>, world_pos: vec3<f32>)->vec4<f32> {
    var c = textureSample(diffuse, filtering, uv);
    if camera.options.y != 0u {
        c = vec4<f32>(apply_studio_lighting(c.rgb, world_normal, world_pos), c.a);
    }
    // Untagged DDS retain the retail renderer's encoded-color convention.
    if linear_sample == 0u && camera.options.x != 0u {
        c=vec4<f32>(select(pow((c.rgb+0.055)/1.055,vec3<f32>(2.4)),c.rgb/12.92,c.rgb<=vec3<f32>(0.04045)),c.a);
    } else if linear_sample != 0u && camera.options.x == 0u {
        c = vec4<f32>(select(1.055 * pow(c.rgb, vec3<f32>(1.0 / 2.4)) - 0.055, c.rgb * 12.92, c.rgb <= vec3<f32>(0.0031308)), c.a);
    }
    return c;
}
@fragment fn fs_opaque(in:Out)->@location(0) vec4<f32> { return vec4<f32>(sampled(in.uv, in.world_normal, in.world_pos).rgb,1.0); }
@fragment fn fs_blend(in:Out)->@location(0) vec4<f32> {
    let c=sampled(in.uv, in.world_normal, in.world_pos);if c.a<=0.0 {discard;}return c;
}
