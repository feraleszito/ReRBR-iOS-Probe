#![cfg(feature = "physics-stage")]
#[test]
#[ignore = "owner's private RBR_ROOT; original assets never uploaded"]
fn full_private_stage_and_audio_bank_parse() {
    let root = std::path::PathBuf::from(std::env::var_os("RBR_ROOT").expect("RBR_ROOT"));
    let stage = rerbr_probe::harwood::stage::load(&root.join("Maps")).unwrap();
    assert_eq!(stage.report.blocks, 282);
    assert_eq!(stage.report.objects, 442);
    assert_eq!(stage.report.interactive_instances, 604);
    assert_eq!(stage.report.withheld_instances, 0);
    assert_eq!(stage.report.route_points, stage.report.supported_points);
    assert_eq!(
        stage.report.route_points,
        stage.report.collision_matched_points
    );
    assert!(stage.report.maximum_ground_gap < 0.001);
    assert!(stage.report.resident_bytes < 192 * 1024 * 1024);
    for d in &stage.render.mesh.draws {
        assert!(!d.indices.is_empty());
        let b = rerbr_probe::harwood::stage::draw_bounds(&stage.render.mesh, d);
        assert!(b.min.is_finite() && b.max.is_finite());
    }
    let audio = rerbr_sim::audio_audit::inspect(&root).unwrap();
    assert_eq!((audio.banks, audio.parts), (6, 12));
    assert_eq!(audio.engine_path, "Audio/Cars/subaru.eng");
    assert!(audio.samples > 0 && audio.cycle_markers > 0);
}
