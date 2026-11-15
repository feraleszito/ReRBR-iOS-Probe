#![cfg(feature = "harwood-benchmark")]
use rerbr_probe::harwood::benchmark;

#[test]
#[ignore = "requires private Maps; explicitly set HARWOOD_MAPS"]
fn real_camera_route_has_ground_and_reuses_the_neighbor_workload() {
    let maps = std::env::var_os("HARWOOD_MAPS").expect("set HARWOOD_MAPS");
    let prepared = benchmark::load(std::path::Path::new(&maps)).unwrap();
    assert_eq!(prepared.render.mesh.vertices.len(), 18983);
    assert_eq!(prepared.render.mesh.indices.len() / 3, 9539);
    assert_eq!(prepared.render.mesh.draws.len(), 124);
    assert_eq!(prepared.render.texture_info().count(), 8);
    assert!((580.0..600.0).contains(&prepared.route.length()));
    assert!(prepared.route.minimum_clearance() >= 1.3999);
    assert!(prepared.route.maximum_clearance() < 2.0);
    let mut previous = prepared.route.camera(0.0).unwrap();
    for i in 1..=2000 {
        let camera = prepared
            .route
            .camera(prepared.route.length() * i as f32 / 2000.0)
            .unwrap();
        assert!(camera.eye.is_finite() && camera.target.is_finite());
        assert!(camera.eye.distance(previous.eye) < 0.4);
        assert!(
            (camera.target - camera.eye)
                .normalize()
                .dot((previous.target - previous.eye).normalize())
                > 0.99
        );
        previous = camera;
    }
}
