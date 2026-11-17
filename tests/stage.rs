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

#[test]
#[ignore = "owner's private RBR_ROOT; original assets never uploaded"]
fn multi_stage_chirdonhead_and_cote_darbroz_parse() {
    let root = std::path::PathBuf::from(std::env::var_os("RBR_ROOT").expect("RBR_ROOT"));
    let maps = root.join("Maps");
    for mut desc in rerbr_probe::content::StageDescriptor::presets() {
        desc.documents_maps = "Maps".into();
        let stage = rerbr_probe::harwood::stage::load_selected(&maps, &desc).unwrap();
        assert_eq!(stage.report.blocks, desc.terrain_blocks);
        assert_eq!(stage.report.withheld_instances, 0);
        assert_eq!(stage.report.route_points, stage.report.supported_points);
        assert_eq!(
            stage.report.route_points,
            stage.report.collision_matched_points
        );
        assert!(stage.report.maximum_ground_gap < 0.001);
        assert!(stage.report.resident_bytes < 192 * 1024 * 1024);
    }
}

#[test]
#[ignore = "private RBR_ROOT and PHYSICS_RBR; CPU scene preparation, no GPU"]
fn selected_scenes_prepare_the_actual_car_path() {
    let assets = std::path::PathBuf::from(std::env::var_os("RBR_ROOT").unwrap());
    let vehicle = std::path::PathBuf::from(std::env::var_os("PHYSICS_RBR").unwrap());
    for stage in rerbr_probe::content::StageDescriptor::presets() {
        let selection = rerbr_probe::content::Selection {
            stage,
            ..Default::default()
        };
        let prepared =
            rerbr_probe::car::prepare_selected(&vehicle, &assets.join("Maps"), &selection)
                .unwrap_or_else(|e| panic!("{}: {e}", selection.stage.id));
        assert_eq!(prepared.selection, selection);
        if selection.stage.id == "harwood-forest" {
            assert!(prepared.path.length() > 50.0);
        } else {
            assert_eq!(prepared.path.length(), 0.0);
        }
        eprintln!("SELECTED SCENE PASS: {}", selection.stage.id);
    }
}
