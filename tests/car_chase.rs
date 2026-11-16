// This benchmark asserts the original 20-block workload. Full-stage has its own private test.
#![cfg(all(feature = "car-chase-benchmark", not(feature = "physics-stage")))]
use rerbr_probe::car;

#[test]
#[ignore = "requires private RBR_CAR_DATA installation; no assets in CI"]
fn real_corolla_eight_files_and_harwood_chase() {
    let root = std::env::var_os("RBR_CAR_DATA").expect("set RBR_CAR_DATA");
    let root = std::path::Path::new(&root);
    let p = car::prepare(root, &root.join("Maps")).unwrap();
    let g = &p.car.geometry;
    assert_eq!(p.car.files.len(), 8);
    assert_eq!(g.drawn.len(), 75);
    assert_eq!(g.stats(None).vertices, 11225);
    assert_eq!(g.stats(None).triangles, 11166);
    assert_eq!(g.stats(None).draws, 99);
    assert_eq!(g.stats(Some(false)).triangles, 10756);
    assert_eq!(g.stats(Some(false)).draws, 83);
    assert_eq!(g.stats(Some(true)).triangles, 410);
    assert_eq!(g.stats(Some(true)).draws, 16);
    assert_eq!(g.stats(None).textures, 3);
    assert_eq!(p.terrain.mesh.indices.len() / 3, 9539);
    assert_eq!(p.terrain.mesh.draws.len(), 124);
    assert!((p.path.length() - 588.53).abs() < 0.1);
    for i in 0..=40 {
        let f = p.path.frame(p.path.length() * i as f32 / 40.0).unwrap();
        assert!(f.min_wheel_clearance > 0.0048);
        assert!(f.max_wheel_travel < 0.07);
        assert_eq!(f.boom, 6.0);
        for aspect in [0.65, 1.55, 2.0] {
            let vp = car::placement::view_projection(f.camera, aspect);
            for draw in &g.draws {
                for &index in &g.indices[draw.indices.clone()] {
                    let p = rerbr_render_core::Vec3::from(g.vertices[index as usize].position)
                        .extend(1.0);
                    let clip = vp * f.models[draw.slot as usize] * p;
                    let q = clip.truncate() / clip.w;
                    assert!(
                        clip.w > 0.0
                            && q.x.abs() < 1.0
                            && q.y.abs() < 1.0
                            && q.z > 0.0
                            && q.z < 1.0,
                        "car clipped at {i}, aspect {aspect}: {q:?}"
                    );
                }
            }
        }
    }
    assert!(g.skipped.iter().any(|(_, why)| why == "ghost"));
}
