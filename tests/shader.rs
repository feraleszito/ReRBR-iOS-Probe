#[test]
fn triangle_shader_validates_without_optional_gpu_capabilities() {
    let module = naga::front::wgsl::parse_str(include_str!("../src/triangle.wgsl"))
        .expect("triangle WGSL must parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("triangle WGSL must validate without optional GPU capabilities");
}

#[cfg(feature = "rerbr-3d")]
#[test]
fn scene_shader_validates_without_optional_gpu_capabilities() {
    let module = naga::front::wgsl::parse_str(include_str!("../src/scene3d.wgsl"))
        .expect("3D scene WGSL must parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("3D scene WGSL must validate without optional GPU capabilities");
}

#[cfg(feature = "harwood-block")]
#[test]
fn harwood_shader_validates_without_optional_shader_capabilities() {
    let module = naga::front::wgsl::parse_str(include_str!("../src/harwood/shader.wgsl"))
        .expect("Harwood WGSL must parse");
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .expect("Harwood WGSL must validate");
}
