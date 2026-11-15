struct Camera { vp: mat4x4<f32>, models: array<mat4x4<f32>,5>, options: vec4<u32> }
@group(0) @binding(0) var<uniform> camera: Camera;
@group(1) @binding(0) var diffuse: texture_2d<f32>;
@group(1) @binding(1) var filtering: sampler;
struct Out { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32> }
@vertex fn vs_main(@location(0) p:vec3<f32>,@location(1) normal:vec3<f32>,@location(2) uv:vec2<f32>,@builtin(instance_index) slot:u32)->Out {
    var out:Out;
    out.position=camera.vp*camera.models[min(slot,4u)]*vec4<f32>(p,1.0);
    out.uv=uv;
    return out;
}
fn sampled(uv:vec2<f32>)->vec4<f32> {
    var c=textureSample(diffuse,filtering,uv);
    // Same legacy unorm convention as the terrain; no damage-alpha multiplication.
    if camera.options.x!=0u {
        c=vec4<f32>(select(pow((c.rgb+0.055)/1.055,vec3<f32>(2.4)),c.rgb/12.92,c.rgb<=vec3<f32>(0.04045)),c.a);
    }
    return c;
}
@fragment fn fs_opaque(in:Out)->@location(0) vec4<f32> { return vec4<f32>(sampled(in.uv).rgb,1.0); }
@fragment fn fs_blend(in:Out)->@location(0) vec4<f32> {
    let c=sampled(in.uv);if c.a<=0.0 {discard;}return c;
}
