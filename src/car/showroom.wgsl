// Lightweight procedural rally showroom studio backdrop + reflective turntable floor.
struct ShowroomUniform {
    vp: mat4x4<f32>,
    // xy = car footprint half-extents (half_x, half_z), z = pedestal radius, w = sRGB flag (1.0 or 0.0)
    params: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: ShowroomUniform;

fn to_output(color_srgb: vec3<f32>) -> vec3<f32> {
    if u.params.w > 0.5 {
        return select(
            pow((color_srgb + 0.055) / 1.055, vec3<f32>(2.4)),
            color_srgb / 12.92,
            color_srgb <= vec3<f32>(0.04045)
        );
    }
    return color_srgb;
}

// --- Fullscreen studio gradient backdrop (no depth write, drawn first) ---
struct BgOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex fn vs_backdrop(@builtin(vertex_index) vid: u32) -> BgOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = positions[vid];
    var out: BgOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = p * 0.5 + 0.5;
    return out;
}

@fragment fn fs_backdrop(in: BgOut) -> @location(0) vec4<f32> {
    // Deep rally paddock studio backdrop with overhead key spotlight & warm-slate horizon glow.
    let uv = in.uv;
    let y = clamp(uv.y, 0.0, 1.0);
    let base_top = vec3<f32>(0.045, 0.058, 0.082);
    let base_mid = vec3<f32>(0.105, 0.125, 0.160);
    let base_low = vec3<f32>(0.035, 0.042, 0.055);
    var col = mix(base_low, base_mid, smoothstep(0.05, 0.48, y));
    col = mix(col, base_top, smoothstep(0.50, 0.98, y));

    // Soft studio overhead spotlight behind the car.
    let d_spot = length((uv - vec2<f32>(0.50, 0.56)) * vec2<f32>(1.35, 1.85));
    let spot = exp(-d_spot * d_spot * 2.4) * 0.16;
    col = col + vec3<f32>(0.90, 0.94, 1.00) * spot;

    // Subtle rally red horizon accent line.
    let band = exp(-pow((y - 0.37) * 32.0, 2.0)) * 0.045;
    col = col + vec3<f32>(0.85, 0.18, 0.16) * band;

    // Gentle edge vignette so HUD pills stay effortlessly readable.
    let d_vig = length((uv - 0.5) * vec2<f32>(1.25, 1.15));
    col = col * (1.0 - smoothstep(0.45, 1.05, d_vig) * 0.35);

    return vec4<f32>(to_output(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
}

// --- Studio turntable floor quad on y = 0.0 ---
struct FloorOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world_xz: vec2<f32>,
};

@vertex fn vs_floor(@builtin(vertex_index) vid: u32) -> FloorOut {
    let r = 26.0;
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-r, -r),
        vec2<f32>( r, -r),
        vec2<f32>( r,  r),
        vec2<f32>(-r, -r),
        vec2<f32>( r,  r),
        vec2<f32>(-r,  r),
    );
    let xz = corners[vid];
    var out: FloorOut;
    out.pos = u.vp * vec4<f32>(xz.x, 0.0, xz.y, 1.0);
    out.world_xz = xz;
    return out;
}

@fragment fn fs_floor(in: FloorOut) -> @location(0) vec4<f32> {
    let xz = in.world_xz;
    let dist = length(xz);

    // Studio floor dark concrete/epoxy base + radial overhead light pool.
    let pool = exp(-dist * dist / 24.0);
    var col = mix(vec3<f32>(0.038, 0.045, 0.060), vec3<f32>(0.155, 0.172, 0.200), pool);

    // Subtle architectural paddock grid (1.0 m tiles) fading smoothly into the studio fog.
    let g = abs(fract(xz + 0.5) - 0.5);
    let line_x = 1.0 - smoothstep(0.008, 0.024, g.x);
    let line_z = 1.0 - smoothstep(0.008, 0.024, g.y);
    let grid = max(line_x, line_z) * smoothstep(14.0, 2.0, dist) * 0.085;
    col = col + vec3<f32>(0.55, 0.64, 0.78) * grid;

    // Sleek turntable ring around the car.
    let ped_r = max(u.params.z, 2.6);
    let ring = exp(-pow((dist - ped_r) * 14.0, 2.0));
    col = col + vec3<f32>(0.86, 0.19, 0.19) * ring * 0.42;
    let inner_glow = smoothstep(ped_r + 0.15, ped_r - 0.35, dist) * 0.045;
    col = col + vec3<f32>(0.65, 0.75, 0.90) * inner_glow;

    // Soft grounded ambient-occlusion shadow directly beneath the car's footprint.
    let hx = max(u.params.x, 0.85);
    let hz = max(u.params.y, 1.95);
    let ell = length(vec2<f32>(xz.x / (hx * 1.18), xz.y / (hz * 1.12)));
    let shadow = smoothstep(0.55, 1.25, ell);
    col = col * mix(0.24, 1.0, shadow);

    // Fade smoothly into backdrop alpha near outer radius so there is no hard square edge.
    let alpha = smoothstep(24.0, 10.0, dist);
    return vec4<f32>(to_output(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0))), alpha);
}
