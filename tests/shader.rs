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
