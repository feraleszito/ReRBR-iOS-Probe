struct Out {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}
@group(0) @binding(0) var font: texture_2d<f32>;
@group(0) @binding(1) var nearest: sampler;
@vertex fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> Out {
    var out: Out;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}
@fragment fn fs_main(in: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color.rgb, in.color.a * textureSample(font, nearest, in.uv).r);
}
@fragment fn fs_rgba(in: Out) -> @location(0) vec4<f32> {
    return textureSample(font, nearest, in.uv) * in.color;
}
